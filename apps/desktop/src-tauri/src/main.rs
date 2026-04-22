#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{env, net::SocketAddr, path::PathBuf};
use tauri::{
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, WindowEvent,
};

fn main() {
    tracing_subscriber::fmt::init();

    tauri::Builder::default()
        .setup(|app| {
            let window = app.get_webview_window("main").expect("main window not found");
            let window_for_events = window.clone();
            window.on_window_event(move |event| {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    // Keep the process alive and move the app to background.
                    api.prevent_close();
                    window_for_events.hide().ok();
                }
            });

            #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
            {
                if let Some(icon) = app.default_window_icon() {
                    TrayIconBuilder::with_id("main")
                        .icon(icon.clone())
                        .on_tray_icon_event(|tray, event| {
                            if let TrayIconEvent::Click {
                                button: MouseButton::Left,
                                button_state: MouseButtonState::Up,
                                ..
                            } = event
                            {
                                if let Some(window) = tray.app_handle().get_webview_window("main") {
                                    window.show().ok();
                                    window.unminimize().ok();
                                    window.set_focus().ok();
                                }
                            }
                        })
                        .build(app)?;
                } else {
                    tracing::warn!("default window icon unavailable, skipping tray icon");
                }
            }

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
