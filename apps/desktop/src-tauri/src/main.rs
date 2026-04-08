#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{env, net::SocketAddr, path::PathBuf};
use tauri::Manager;

fn main() {
    tracing_subscriber::fmt::init();

    tauri::Builder::default()
        .setup(|app| {
            let window = app.get_webview_window("main").expect("main window not found");
            // 设置窗口背景色为深色，避免启动时显示白色
            window.set_background_color(Some(tauri::window::Color(30, 30, 30, 255))).ok();

            let config_file = workspace_config_file(app)?;
            let workspace_init = server::resolve_workspace_from_env("KAISHA_WORKDIR", config_file)?;

            tauri::async_runtime::spawn(async move {
                let addr: SocketAddr = "127.0.0.1:8080".parse().expect("valid addr");
                if let Err(err) = server::run_http(addr, workspace_init).await {
                    tracing::error!("server failed: {err}");
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run tauri app");
}

fn workspace_config_file(app: &tauri::App) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Ok(path) = env::var("KAISHA_WORKDIR_CONFIG") {
        return Ok(PathBuf::from(path));
    }

    let app_config_dir = app.path().app_config_dir()?;
    Ok(app_config_dir.join("workspace.json"))
}
