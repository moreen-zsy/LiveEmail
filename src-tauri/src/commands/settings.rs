use tauri::State;

use crate::db::{messages, settings, Db};
use crate::error::AppResult;
use crate::models::{AppSettings, StorageStats};

#[tauri::command]
pub fn get_settings(state: State<'_, Db>) -> AppResult<AppSettings> {
    let conn = state.lock()?;
    settings::all(&conn)
}

#[tauri::command]
pub fn update_settings(state: State<'_, Db>, value: AppSettings) -> AppResult<()> {
    let conn = state.lock()?;
    settings::update(&conn, &value)
}

fn dir_size(path: &std::path::Path) -> u64 {
    let mut total = 0u64;
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() {
            total += dir_size(&entry.path());
        } else {
            total += meta.len();
        }
    }
    total
}

fn collect_stats(db: &Db) -> Result<StorageStats, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let attachment_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM attachments WHERE local_path IS NOT NULL",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    let message_count = messages::count(&conn).map_err(|e| e.to_string())?;
    let account_count = crate::db::accounts::count(&conn).map_err(|e| e.to_string())?;
    drop(conn);

    let db_bytes = std::fs::metadata(db.app_dir.join("liveemail.db"))
        .map(|m| m.len())
        .unwrap_or(0);
    let attachments_bytes = dir_size(&db.attachments_dir());

    Ok(StorageStats {
        db_bytes,
        attachments_bytes,
        attachment_count,
        message_count,
        account_count,
    })
}

#[tauri::command]
pub fn storage_stats(state: State<'_, Db>) -> Result<StorageStats, String> {
    collect_stats(state.inner())
}

#[tauri::command]
pub fn clear_cache(state: State<'_, Db>, scope: String) -> Result<StorageStats, String> {
    let db = state.inner().clone();
    match scope.as_str() {
        "attachments" => {
            let conn = db.lock().map_err(|e| e.to_string())?;
            conn.execute("DELETE FROM attachments", [])
                .map_err(|e| e.to_string())?;
            conn.execute("UPDATE messages SET has_attachment = 0", [])
                .map_err(|e| e.to_string())?;
        }
        "messages" | "all" => {
            let conn = db.lock().map_err(|e| e.to_string())?;
            messages::clear_all(&conn).map_err(|e| e.to_string())?;
            conn.execute(
                "UPDATE folders SET uid_validity = NULL, uid_next = NULL, unread_count = 0, total_count = 0",
                [],
            )
            .map_err(|e| e.to_string())?;
        }
        other => return Err(format!("未知的清理范围：{other}")),
    }

    let attachments_dir = db.attachments_dir();
    if attachments_dir.exists() {
        for entry in std::fs::read_dir(&attachments_dir)
            .map_err(|e| e.to_string())?
            .flatten()
        {
            let path = entry.path();
            if path.is_dir() {
                let _ = std::fs::remove_dir_all(&path);
            } else {
                let _ = std::fs::remove_file(&path);
            }
        }
    }

    collect_stats(&db)
}
