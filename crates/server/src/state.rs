use std::path::PathBuf;

use sqlx::PgPool;

use crate::secrets::MasterKey;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub listen_addr: String,
    /// Directory holding the built dashboard (web/dist). The Docker image
    /// copies it next to the binary; dev runs point at ./web/dist.
    pub web_dist: String,
    /// Writable state directory (SSH runtime files, work dirs).
    pub state_dir: PathBuf,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            database_url: std::env::var("DATABASE_URL")
                .map_err(|_| anyhow::anyhow!("DATABASE_URL is required"))?,
            listen_addr: std::env::var("PJX_LISTEN_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".into()),
            web_dist: std::env::var("PJX_WEB_DIST").unwrap_or_else(|_| "web/dist".into()),
            state_dir: std::env::var("PJX_STATE_DIR")
                .unwrap_or_else(|_| "./data".into())
                .into(),
        })
    }
}

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Config,
    pub master_key: MasterKey,
}

impl AppState {
    pub fn new(pool: PgPool, config: Config, master_key: MasterKey) -> Self {
        Self {
            pool,
            config,
            master_key,
        }
    }
}
