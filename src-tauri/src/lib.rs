mod commands;
mod db;
mod error;
mod models;
mod services;

use std::time::Duration;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, TrayIconBuilder, TrayIconEvent};
use tauri::{Manager, WindowEvent};

fn init_logging() {
    let filter = tracing_subscriber::EnvFilter::from_default_env()
        .add_directive(tracing::Level::INFO.into());
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}

fn build_tray(app: &tauri::App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
    let sync = MenuItem::with_id(app, "sync", "立即同步", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &sync, &quit])?;

    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("LiveEmail")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.unminimize();
                    let _ = window.set_focus();
                }
            }
            "sync" => {
                let app = app.clone();
                let db = app.state::<db::Db>().inner().clone();
                tauri::async_runtime::spawn(async move {
                    let accounts = match db.lock() {
                        Ok(conn) => crate::db::accounts::list(&conn).unwrap_or_default(),
                        Err(_) => Vec::new(),
                    };
                    for account in accounts {
                        let _ = services::sync::sync_account(&app, db.clone(), account.id).await;
                    }
                });
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                ..
            } = event
            {
                if let Some(window) = tray.app_handle().get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.unminimize();
                    let _ = window.set_focus();
                }
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

/// 后台同步：启动后先同步一次，之后按设置的间隔轮询
fn spawn_sync_loop(app: tauri::AppHandle, db: db::Db) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(3)).await;
        loop {
            let accounts = match db.lock() {
                Ok(conn) => crate::db::accounts::list(&conn).unwrap_or_default(),
                Err(_) => Vec::new(),
            };
            for account in accounts {
                if let Err(e) = services::sync::sync_account(&app, db.clone(), account.id).await {
                    tracing::warn!("账户 {} 后台同步失败: {e}", account.email);
                }
            }
            services::sync::ensure_idle_watchers(&app, &db);
            let interval = match db.lock() {
                Ok(conn) => crate::db::settings::all(&conn)
                    .map(|s| s.sync_interval_secs)
                    .unwrap_or(db::settings::DEFAULT_SYNC_INTERVAL),
                Err(_) => db::settings::DEFAULT_SYNC_INTERVAL,
            }
            .clamp(60, 3600);
            tokio::time::sleep(Duration::from_secs(interval as u64)).await;
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            init_logging();

            let app_data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data_dir)?;

            let db = db::init(&app_data_dir)?;
            app.manage(db.clone());

            build_tray(app)?;
            services::sync::ensure_idle_watchers(app.handle(), &db);
            spawn_sync_loop(app.handle().clone(), db);

            tracing::info!(
                "LiveEmail 后端初始化完成，数据目录 {}",
                app_data_dir.display()
            );
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                let close_to_tray = app
                    .try_state::<db::Db>()
                    .map(|db| {
                        db.lock()
                            .ok()
                            .and_then(|conn| db::settings::all(&conn).ok())
                            .map(|s| s.close_to_tray)
                            .unwrap_or(true)
                    })
                    .unwrap_or(false);
                if close_to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::accounts::list_accounts,
            commands::accounts::add_account,
            commands::accounts::update_account,
            commands::accounts::delete_account,
            commands::accounts::set_default_account,
            commands::accounts::provider_presets,
            commands::accounts::test_connection,
            commands::accounts::oauth_authorize,
            commands::mail::list_messages,
            commands::mail::get_message,
            commands::mail::unread_count,
            commands::mail::refresh_badge,
            commands::mail::sync_account,
            commands::mail::sync_all,
            commands::mail::list_folders,
            commands::mail::create_folder,
            commands::mail::delete_folder,
            commands::mail::set_message_flags,
            commands::mail::move_messages,
            commands::mail::attachment_path,
            commands::mail::save_attachment,
            commands::compose::send_mail,
            commands::compose::retry_outbox,
            commands::compose::list_outbox,
            commands::compose::delete_outbox,
            commands::compose::save_draft,
            commands::compose::list_drafts,
            commands::compose::delete_draft,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::settings::storage_stats,
            commands::settings::clear_cache,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
