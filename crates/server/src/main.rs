use std::{env, net::SocketAddr};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    server::logging::init();

    let host = env::var("KAISHA_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = env::var("KAISHA_PORT")
        .ok()
        .and_then(|raw| raw.parse::<u16>().ok())
        .unwrap_or(8080);
    let config_file = env::var("KAISHA_WORKDIR_CONFIG")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from(".kaisha/workspace.json"));
    let workspace_init = server::resolve_workspace_from_env("KAISHA_WORKDIR", config_file)?;

    let addr: SocketAddr = format!("{}:{}", host, port).parse()?;
    server::run_http(addr, workspace_init).await
}
