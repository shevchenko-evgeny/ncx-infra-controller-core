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

use std::str::FromStr;
use std::time::Duration;

use common::api_fixtures::{FIXTURE_DHCP_RELAY_ADDRESS, create_managed_host, create_test_env};
use mac_address::MacAddress;
use model::network_segment::NetworkSegmentType;

use crate::cfg::file::DhcpPeriodicCleanupConfig;
use crate::dhcp_periodic_cleanup::DhcpPeriodicCleanup;
use crate::tests::common;

/// Helper to build a cleanup config targeting admin segments (which is
/// what the test fixture creates for FIXTURE_DHCP_RELAY_ADDRESS).
fn test_config() -> DhcpPeriodicCleanupConfig {
    DhcpPeriodicCleanupConfig {
        enabled: true,
        run_interval: Duration::from_secs(3600),
        max_age: Duration::from_secs(7 * 24 * 3600),
        segment_types: vec![NetworkSegmentType::Admin],
        include_associated: false,
    }
}

#[crate::sqlx_test]
async fn test_stale_allocation_is_deleted(
    pool: sqlx::PgPool,
) -> Result<(), Box<dyn std::error::Error>> {
    let env = create_test_env(pool).await;
    let relay: std::net::IpAddr = FIXTURE_DHCP_RELAY_ADDRESS.parse().unwrap();

    // Create an interface (which also allocates an IP address)
    let mut txn = env.pool.begin().await?;
    let interface = db::machine_interface::validate_existing_mac_and_create(
        &mut txn,
        MacAddress::from_str("aa:bb:cc:dd:ee:01").unwrap(),
        relay,
        None,
    )
    .await?;
    assert!(
        !interface.addresses.is_empty(),
        "interface should have an IP"
    );

    // Set last_dhcp to 8 days ago (stale)
    let stale_time = chrono::Utc::now() - chrono::Duration::days(8);
    db::machine_interface::update_last_dhcp(&mut txn, interface.id, Some(stale_time)).await?;
    txn.commit().await?;

    // Run cleanup with 7-day max_age
    let cleanup = DhcpPeriodicCleanup::new(env.pool.clone(), test_config());
    cleanup.run_single_iteration().await?;

    // Verify the address was deleted
    let mut txn = env.pool.begin().await?;
    let result =
        db::machine_interface_address::find_ipv4_for_interface(&mut txn, interface.id).await;
    assert!(result.is_err(), "address should have been deleted");

    // Verify the interface itself still exists
    let iface = db::machine_interface::find_one(&mut *txn, interface.id).await?;
    assert_eq!(iface.id, interface.id, "interface should still exist");

    Ok(())
}

#[crate::sqlx_test]
async fn test_recent_allocation_is_preserved(
    pool: sqlx::PgPool,
) -> Result<(), Box<dyn std::error::Error>> {
    let env = create_test_env(pool).await;
    let relay: std::net::IpAddr = FIXTURE_DHCP_RELAY_ADDRESS.parse().unwrap();

    // Create an interface
    let mut txn = env.pool.begin().await?;
    let interface = db::machine_interface::validate_existing_mac_and_create(
        &mut txn,
        MacAddress::from_str("aa:bb:cc:dd:ee:02").unwrap(),
        relay,
        None,
    )
    .await?;
    let ip = interface.addresses[0];

    // Set last_dhcp to 1 day ago (recent)
    let recent_time = chrono::Utc::now() - chrono::Duration::days(1);
    db::machine_interface::update_last_dhcp(&mut txn, interface.id, Some(recent_time)).await?;
    txn.commit().await?;

    // Run cleanup with 7-day max_age
    let cleanup = DhcpPeriodicCleanup::new(env.pool.clone(), test_config());
    cleanup.run_single_iteration().await?;

    // Verify the address still exists
    let mut txn = env.pool.begin().await?;
    let addr =
        db::machine_interface_address::find_ipv4_for_interface(&mut txn, interface.id).await?;
    assert_eq!(addr.address, ip, "recent address should be preserved");

    Ok(())
}

#[crate::sqlx_test]
async fn test_cleanup_only_deletes_stale_not_recent(
    pool: sqlx::PgPool,
) -> Result<(), Box<dyn std::error::Error>> {
    let env = create_test_env(pool).await;
    let relay: std::net::IpAddr = FIXTURE_DHCP_RELAY_ADDRESS.parse().unwrap();

    let mut txn = env.pool.begin().await?;

    // Create two interfaces: one stale, one recent
    let stale_iface = db::machine_interface::validate_existing_mac_and_create(
        &mut txn,
        MacAddress::from_str("aa:bb:cc:dd:ee:03").unwrap(),
        relay,
        None,
    )
    .await?;

    let recent_iface = db::machine_interface::validate_existing_mac_and_create(
        &mut txn,
        MacAddress::from_str("aa:bb:cc:dd:ee:04").unwrap(),
        relay,
        None,
    )
    .await?;

    let stale_time = chrono::Utc::now() - chrono::Duration::days(10);
    let recent_time = chrono::Utc::now() - chrono::Duration::hours(1);
    db::machine_interface::update_last_dhcp(&mut txn, stale_iface.id, Some(stale_time)).await?;
    db::machine_interface::update_last_dhcp(&mut txn, recent_iface.id, Some(recent_time)).await?;
    txn.commit().await?;

    // Run cleanup
    let cleanup = DhcpPeriodicCleanup::new(env.pool.clone(), test_config());
    cleanup.run_single_iteration().await?;

    // Stale address should be gone
    let mut txn = env.pool.begin().await?;
    assert!(
        db::machine_interface_address::find_ipv4_for_interface(&mut txn, stale_iface.id)
            .await
            .is_err(),
        "stale address should be deleted"
    );

    // Recent address should remain
    assert!(
        db::machine_interface_address::find_ipv4_for_interface(&mut txn, recent_iface.id)
            .await
            .is_ok(),
        "recent address should be preserved"
    );

    Ok(())
}

#[crate::sqlx_test]
async fn test_interface_with_null_last_dhcp_is_not_deleted(
    pool: sqlx::PgPool,
) -> Result<(), Box<dyn std::error::Error>> {
    let env = create_test_env(pool).await;
    let relay: std::net::IpAddr = FIXTURE_DHCP_RELAY_ADDRESS.parse().unwrap();

    // Create an interface but DON'T set last_dhcp (it will be NULL)
    let mut txn = env.pool.begin().await?;
    let interface = db::machine_interface::validate_existing_mac_and_create(
        &mut txn,
        MacAddress::from_str("aa:bb:cc:dd:ee:05").unwrap(),
        relay,
        None,
    )
    .await?;
    let ip = interface.addresses[0];
    txn.commit().await?;

    // Run cleanup
    let cleanup = DhcpPeriodicCleanup::new(env.pool.clone(), test_config());
    cleanup.run_single_iteration().await?;

    // Address should still exist (NULL last_dhcp should not be treated as stale)
    let mut txn = env.pool.begin().await?;
    let addr =
        db::machine_interface_address::find_ipv4_for_interface(&mut txn, interface.id).await?;
    assert_eq!(
        addr.address, ip,
        "NULL last_dhcp address should not be deleted"
    );

    Ok(())
}

#[crate::sqlx_test]
async fn test_stale_allocation_on_non_matching_segment_type_is_preserved(
    pool: sqlx::PgPool,
) -> Result<(), Box<dyn std::error::Error>> {
    let env = create_test_env(pool).await;
    let relay: std::net::IpAddr = FIXTURE_DHCP_RELAY_ADDRESS.parse().unwrap();

    // Create a stale interface on the admin segment
    let mut txn = env.pool.begin().await?;
    let interface = db::machine_interface::validate_existing_mac_and_create(
        &mut txn,
        MacAddress::from_str("aa:bb:cc:dd:ee:06").unwrap(),
        relay,
        None,
    )
    .await?;
    let ip = interface.addresses[0];

    let stale_time = chrono::Utc::now() - chrono::Duration::days(10);
    db::machine_interface::update_last_dhcp(&mut txn, interface.id, Some(stale_time)).await?;
    txn.commit().await?;

    // Run cleanup targeting only Underlay segments (fixture is Admin)
    let cleanup = DhcpPeriodicCleanup::new(
        env.pool.clone(),
        DhcpPeriodicCleanupConfig {
            enabled: true,
            run_interval: Duration::from_secs(3600),
            max_age: Duration::from_secs(7 * 24 * 3600),
            segment_types: vec![NetworkSegmentType::Underlay],
            include_associated: false,
        },
    );
    cleanup.run_single_iteration().await?;

    // Address should still exist because the segment type doesn't match
    let mut txn = env.pool.begin().await?;
    let addr =
        db::machine_interface_address::find_ipv4_for_interface(&mut txn, interface.id).await?;
    assert_eq!(
        addr.address, ip,
        "admin segment address should not be deleted when only targeting underlay"
    );

    Ok(())
}

#[crate::sqlx_test]
async fn test_machine_associated_interface_is_preserved(
    pool: sqlx::PgPool,
) -> Result<(), Box<dyn std::error::Error>> {
    let env = create_test_env(pool).await;

    // Create a managed host — this creates a machine with associated interfaces
    let managed_host = create_managed_host(&env).await;

    // Find the host machine's interfaces
    let mut txn = env.pool.begin().await?;
    let interfaces_by_machine =
        db::machine_interface::find_by_machine_ids(&mut txn, &[managed_host.id]).await?;
    let interfaces: Vec<_> = interfaces_by_machine
        .values()
        .flat_map(|v| v.iter())
        .collect();
    assert!(
        !interfaces.is_empty(),
        "managed host should have interfaces"
    );

    // Set last_dhcp to 10 days ago on all interfaces (stale)
    let stale_time = chrono::Utc::now() - chrono::Duration::days(10);
    for iface in &interfaces {
        db::machine_interface::update_last_dhcp(&mut txn, iface.id, Some(stale_time)).await?;
    }
    txn.commit().await?;

    // Run cleanup targeting admin segments (which is what the fixture uses)
    let cleanup = DhcpPeriodicCleanup::new(
        env.pool.clone(),
        DhcpPeriodicCleanupConfig {
            enabled: true,
            run_interval: Duration::from_secs(3600),
            max_age: Duration::from_secs(7 * 24 * 3600),
            segment_types: vec![NetworkSegmentType::Admin],
            include_associated: false,
        },
    );
    cleanup.run_single_iteration().await?;

    // All addresses should still exist because these interfaces are
    // associated with a machine (machine_id IS NOT NULL)
    let mut txn = env.pool.begin().await?;
    for iface in &interfaces {
        if !iface.addresses.is_empty() {
            let addr =
                db::machine_interface_address::find_ipv4_for_interface(&mut txn, iface.id).await;
            assert!(
                addr.is_ok(),
                "machine-associated interface {} should not be cleaned up",
                iface.id
            );
        }
    }

    Ok(())
}

#[crate::sqlx_test]
async fn test_include_associated_deletes_machine_interfaces(
    pool: sqlx::PgPool,
) -> Result<(), Box<dyn std::error::Error>> {
    let env = create_test_env(pool).await;

    // Create a managed host with machine-associated interfaces
    let managed_host = create_managed_host(&env).await;

    let mut txn = env.pool.begin().await?;
    let interfaces_by_machine =
        db::machine_interface::find_by_machine_ids(&mut txn, &[managed_host.id]).await?;
    let interfaces: Vec<_> = interfaces_by_machine
        .values()
        .flat_map(|v| v.iter())
        .collect();
    let iface_with_addr = interfaces
        .iter()
        .find(|i| !i.addresses.is_empty())
        .expect("managed host should have an interface with an address");

    // Make it stale
    let stale_time = chrono::Utc::now() - chrono::Duration::days(10);
    db::machine_interface::update_last_dhcp(&mut txn, iface_with_addr.id, Some(stale_time)).await?;
    txn.commit().await?;

    // Run cleanup WITH include_associated = true
    let cleanup = DhcpPeriodicCleanup::new(
        env.pool.clone(),
        DhcpPeriodicCleanupConfig {
            enabled: true,
            run_interval: Duration::from_secs(3600),
            max_age: Duration::from_secs(7 * 24 * 3600),
            segment_types: vec![NetworkSegmentType::Admin],
            include_associated: true,
        },
    );
    cleanup.run_single_iteration().await?;

    // Address SHOULD be deleted because include_associated is true
    let mut txn = env.pool.begin().await?;
    let result =
        db::machine_interface_address::find_ipv4_for_interface(&mut txn, iface_with_addr.id).await;
    assert!(
        result.is_err(),
        "machine-associated interface should be cleaned up when include_associated is true"
    );

    Ok(())
}
