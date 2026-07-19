mod auth;
mod jobs_setup;
mod routes;
mod routes_targets;
mod secrets;
mod sshfiles;
mod state;
mod worker;

use std::net::SocketAddr;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "projexity=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = state::Config::from_env()?;
    let master_key = secrets::MasterKey::from_env()?;
    std::fs::create_dir_all(&config.state_dir)?;
    let pool = projexity_db::connect(&config.database_url).await?;
    tracing::info!("database connected, migrations applied");

    let app_state = state::AppState::new(pool.clone(), config.clone(), master_key);

    // Background worker: claims jobs from the Postgres queue in-process.
    // One binary, one container — self-hosting stays `docker compose up`.
    tokio::spawn(worker::run(app_state.clone()));

    let app = routes::router(app_state);
    let addr: SocketAddr = config.listen_addr.parse()?;
    tracing::info!(%addr, "projexity listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
