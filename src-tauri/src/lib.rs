mod commands;
mod db;
mod error;
mod models;
mod services;

use tauri::Manager;

fn init_logging() {
    let filter = tracing_subscriber::EnvFilter::from_default_env()
        .add_directive(tracing::Level::INFO.into());
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            init_logging();

            let app_data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data_dir)?;
            let db_path = app_data_dir.join("liveemail.db");

            let db = db::init(&db_path)?;
            app.manage(db);

            tracing::info!("LiveEmail 后端初始化完成，数据库位于 {}", db_path.display());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::accounts::list_accounts,
            commands::accounts::add_account,
            commands::accounts::update_account,
            commands::accounts::delete_account,
            commands::accounts::provider_presets,
            commands::accounts::test_connection,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
