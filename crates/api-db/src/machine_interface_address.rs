/*
 * SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
 * SPDX-License-Identifier: Apache-2.0
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
use std::net::IpAddr;

use carbide_uuid::machine::{MachineId, MachineInterfaceId};
use model::network_segment::NetworkSegmentType;
use sqlx::{FromRow, PgConnection};

use super::DatabaseError;
use crate::db_read::DbReader;

#[derive(Debug, FromRow, Clone)]
pub struct MachineInterfaceAddress {
    pub address: IpAddr,
}

pub async fn find_ipv4_for_interface(
    txn: &mut PgConnection,
    interface_id: MachineInterfaceId,
) -> Result<MachineInterfaceAddress, DatabaseError> {
    let query =
        "SELECT * FROM machine_interface_addresses WHERE interface_id = $1 AND family(address) = 4";
    sqlx::query_as(query)
        .bind(interface_id)
        .fetch_one(txn)
        .await
        .map_err(|e| DatabaseError::query(query, e))
}

pub async fn find_by_address(
    txn: impl DbReader<'_>,
    address: IpAddr,
) -> Result<Option<MachineInterfaceSearchResult>, DatabaseError> {
    let query = "SELECT mi.id, mi.machine_id, ns.name, ns.network_segment_type
            FROM machine_interface_addresses mia
            INNER JOIN machine_interfaces mi ON mi.id = mia.interface_id
            INNER JOIN network_segments ns ON ns.id = mi.segment_id
            WHERE mia.address = $1::inet
        ";
    sqlx::query_as(query)
        .bind(address)
        .fetch_optional(txn)
        .await
        .map_err(|e| DatabaseError::query(query, e))
}

pub async fn delete(
    txn: &mut PgConnection,
    interface_id: &MachineInterfaceId,
) -> Result<(), DatabaseError> {
    let query = "DELETE FROM machine_interface_addresses WHERE interface_id = $1";
    sqlx::query(query)
        .bind(interface_id)
        .execute(txn)
        .await
        .map(|_| ())
        .map_err(|e| DatabaseError::query(query, e))
}

/// delete_stale_allocations deletes IP address allocations for
/// interfaces whose `last_dhcp` timestamp is older than `max_age`
/// ago, scoped to the given network segment types. Used as part
/// of our periodic cleanup.
///
/// When `include_associated` is false (default), only interfaces
/// with no machine association (machine_id IS NULL) are targeted,
/// since those are the primary source of leaked allocations.
/// When true, all stale interfaces are eligible regardless of
/// machine association.
///
/// Returns the number of address rows deleted.
pub async fn delete_stale_allocations(
    txn: &mut PgConnection,
    max_age: std::time::Duration,
    segment_types: &[NetworkSegmentType],
    include_associated: bool,
) -> Result<u64, DatabaseError> {
    let unassociated_filter = if include_associated {
        ""
    } else {
        "AND mi.machine_id IS NULL"
    };
    let query = format!(
        "DELETE FROM machine_interface_addresses
        WHERE interface_id IN (
            SELECT mi.id FROM machine_interfaces mi
            INNER JOIN network_segments ns ON ns.id = mi.segment_id
            WHERE mi.last_dhcp IS NOT NULL
              AND mi.last_dhcp < NOW() - $1::interval
              AND ns.network_segment_type = ANY($2::network_segment_type_t[])
              {unassociated_filter}
        )"
    );
    let interval = pg_interval_from_duration(max_age);
    sqlx::query(&query)
        .bind(interval)
        .bind(segment_types)
        .execute(txn)
        .await
        .map(|r| r.rows_affected())
        .map_err(|e| DatabaseError::query(&query, e))
}

fn pg_interval_from_duration(d: std::time::Duration) -> sqlx::postgres::types::PgInterval {
    sqlx::postgres::types::PgInterval {
        months: 0,
        days: 0,
        microseconds: d.as_micros() as i64,
    }
}

#[derive(Debug, FromRow)]
pub struct MachineInterfaceSearchResult {
    pub id: MachineInterfaceId,
    pub machine_id: Option<MachineId>,
    pub name: String,
    pub network_segment_type: NetworkSegmentType,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pg_interval_from_7_days() {
        let interval = pg_interval_from_duration(std::time::Duration::from_secs(7 * 24 * 3600));
        assert_eq!(interval.months, 0);
        assert_eq!(interval.days, 0);
        assert_eq!(interval.microseconds, 7 * 24 * 3600 * 1_000_000);
    }

    #[test]
    fn pg_interval_from_zero() {
        let interval = pg_interval_from_duration(std::time::Duration::ZERO);
        assert_eq!(interval.microseconds, 0);
    }
}
