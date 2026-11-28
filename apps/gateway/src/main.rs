use tracing_subscriber::EnvFilter;
use vocalis_gateway::{app, config::Config, AppState};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with_current_span(false)
        .init();

    let config = match Config::from_env() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(error = %e, "invalid configuration");
            std::process::exit(2);
        }
    };
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], config.port));
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!(%addr, error = %e, "could not bind the listen address");
            std::process::exit(1);
        }
    };
    tracing::info!(%addr, "gateway listening");
    if let Err(e) = axum::serve(listener, app(AppState::new(config))).await {
        tracing::error!(error = %e, "server stopped");
        std::process::exit(1);
    }
}
