use std::{env, net::SocketAddr};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let host = env::var("KAISHA_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = env::var("KAISHA_PORT")
        .ok()
        .and_then(|raw| raw.parse::<u16>().ok())
        .unwrap_or(8080);

    let addr: SocketAddr = format!("{}:{}", host, port).parse()?;
    server::run_http(addr).await
}
