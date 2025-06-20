// Copyright 2024-2025 Golem Cloud
//
// Licensed under the Golem Source License v1.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://license.golem.cloud/LICENSE
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use anyhow::anyhow;
use golem_common::config::DbConfig;
use golem_common::tracing::init_tracing_with_default_env_filter;
use golem_service_base::db;
use golem_service_base::migration::{Migrations, MigrationsDir};
use golem_worker_service::app::{app, dump_openapi_yaml};
use golem_worker_service::config::make_worker_service_config_loader;
use std::path::Path;
use tracing::{error, info};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if std::env::args().any(|arg| arg == "--dump-openapi-yaml") {
        println!("{}", dump_openapi_yaml().await.map_err(|err| anyhow!(err))?);
        Ok(())
    } else if let Some(config) = make_worker_service_config_loader().load_or_dump_config() {
        init_tracing_with_default_env_filter(&config.tracing);

        if config.is_local_env() {
            info!("Golem Worker Service starting up (local mode)...");
        } else {
            info!("Golem Worker Service starting up...");
        }

        let migrations = MigrationsDir::new(Path::new("./db/migration").to_path_buf());
        match config.db.clone() {
            DbConfig::Postgres(c) => {
                db::postgres::migrate(&c, migrations.postgres_migrations())
                    .await
                    .map_err(|e| {
                        error!("DB - init error: {}", &e);
                        std::io::Error::other(format!("Init error (pg): {e:?}"))
                    })?;
            }
            DbConfig::Sqlite(c) => {
                db::sqlite::migrate(&c, migrations.sqlite_migrations())
                    .await
                    .map_err(|e| {
                        error!("DB - init error: {}", e);
                        std::io::Error::other(format!("Init error (sqlite): {e:?}"))
                    })?;
            }
        };

        Ok(app(config).await?)
    } else {
        Ok(())
    }
}
