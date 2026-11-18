use rusqlite::{params, Connection, OptionalExtension};

use crate::error::AppResult;
use crate::models::AppSettings;

pub const DEFAULT_THEME: &str = "system";
pub const DEFAULT_FONT_SIZE: &str = "md";
pub const DEFAULT_SYNC_INTERVAL: i64 = 300;

pub fn get(conn: &Connection, key: &str) -> AppResult<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .optional()?)
}

pub fn set(conn: &Connection, key: &str, value: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

fn get_bool(conn: &Connection, key: &str, default: bool) -> AppResult<bool> {
    Ok(get(conn, key)?
        .map(|v| v == "1" || v == "true")
        .unwrap_or(default))
}

pub fn all(conn: &Connection) -> AppResult<AppSettings> {
    let theme = get(conn, "theme")?.unwrap_or_else(|| DEFAULT_THEME.into());
    let font_size = get(conn, "font_size")?.unwrap_or_else(|| DEFAULT_FONT_SIZE.into());
    let sync_interval_secs = get(conn, "sync_interval_secs")?
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_SYNC_INTERVAL)
        .clamp(60, 3600);
    let signature = get(conn, "signature")?.unwrap_or_default();
    Ok(AppSettings {
        theme,
        font_size,
        sync_interval_secs,
        signature,
        signature_per_account: get_bool(conn, "signature_per_account", false)?,
        close_to_tray: get_bool(conn, "close_to_tray", true)?,
        notifications_enabled: get_bool(conn, "notifications_enabled", true)?,
        remote_images_trusted: get_bool(conn, "remote_images_trusted", false)?,
    })
}

pub fn update(conn: &Connection, s: &AppSettings) -> AppResult<()> {
    set(conn, "theme", &s.theme)?;
    set(conn, "font_size", &s.font_size)?;
    set(conn, "sync_interval_secs", &s.sync_interval_secs.to_string())?;
    set(conn, "signature", &s.signature)?;
    set(conn, "signature_per_account", if s.signature_per_account { "1" } else { "0" })?;
    set(conn, "close_to_tray", if s.close_to_tray { "1" } else { "0" })?;
    set(conn, "notifications_enabled", if s.notifications_enabled { "1" } else { "0" })?;
    set(conn, "remote_images_trusted", if s.remote_images_trusted { "1" } else { "0" })?;
    Ok(())
}
