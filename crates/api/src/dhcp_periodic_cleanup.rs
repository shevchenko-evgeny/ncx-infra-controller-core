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

use std::io;

use sqlx::PgPool;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use crate::cfg::file::DhcpPeriodicCleanupConfig;
use crate::periodic_timer::PeriodicTimer;

/// DhcpPeriodicCleanup periodically releases IP address allocations
/// for machine interface addresses that have not renewed their DHCP
/// lease within the configured `max_age`.
pub struct DhcpPeriodicCleanup {
    database_connection: PgPool,
    config: DhcpPeriodicCleanupConfig,
}

impl DhcpPeriodicCleanup {
    pub fn new(database_connection: PgPool, config: DhcpPeriodicCleanupConfig) -> Self {
        Self {
            database_connection,
            config,
        }
    }

    pub fn start(
        self,
        join_set: &mut JoinSet<()>,
        cancel_token: CancellationToken,
    ) -> io::Result<()> {
        if self.config.enabled {
            tracing::info!(
                max_age_secs = self.config.max_age.as_secs(),
                run_interval_secs = self.config.run_interval.as_secs(),
                "Starting DHCP periodic cleanup"
            );
            join_set
                .build_task()
                .name("dhcp_periodic_cleanup")
                .spawn(async move { self.run(cancel_token).await })?;
        } else {
            tracing::info!("DHCP periodic cleanup is disabled");
        }
        Ok(())
    }

    async fn run(&self, cancel_token: CancellationToken) {
        let timer = PeriodicTimer::new(self.config.run_interval);
        loop {
            let tick = timer.tick();
            if let Err(e) = self.run_single_iteration().await {
                tracing::warn!("DHCP periodic cleanup error: {e}");
            }

            tokio::select! {
                _ = tick.sleep() => {},
                _ = cancel_token.cancelled() => {
                    tracing::info!("DHCP periodic cleanup stop was requested");
                    return;
                }
            }
        }
    }

    pub(crate) async fn run_single_iteration(&self) -> eyre::Result<()> {
        let mut txn = self.database_connection.begin().await?;
        let deleted = db::machine_interface_address::delete_stale_allocations(
            &mut txn,
            self.config.max_age,
            &self.config.segment_types,
            self.config.include_associated,
        )
        .await?;

        txn.commit().await?;

        if deleted > 0 {
            tracing::info!(
                deleted,
                max_age_secs = self.config.max_age.as_secs(),
                "Released stale DHCP IP allocations"
            );
        } else {
            tracing::trace!("No stale DHCP allocations to clean up");
        }

        Ok(())
    }
}
