use rusqlite::{params, Connection, OptionalExtension};

use crate::error::AppResult;
use crate::models::OutboxItem;

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn enqueue(
    conn: &Connection,
    account_id: i64,
    subject: &str,
    to_addrs: &str,
    cc_addrs: &str,
    bcc_addrs: &str,
    body_html: &str,
    body_text: &str,
) -> AppResult<i64> {
    let now = now_unix();
    conn.execute(
        "INSERT INTO outbox (account_id, subject, to_addrs, cc_addrs, bcc_addrs, body_html,
            body_text, status, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'pending', ?8, ?8)",
        params![
            account_id,
            subject,
            to_addrs,
            cc_addrs,
            bcc_addrs,
            body_html,
            body_text,
            now
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn set_status(conn: &Connection, id: i64, status: &str, error: Option<&str>) -> AppResult<()> {
    conn.execute(
        "UPDATE outbox SET status = ?1, error = ?2, updated_at = ?3 WHERE id = ?4",
        params![status, error, now_unix(), id],
    )?;
    Ok(())
}

pub fn list(conn: &Connection) -> AppResult<Vec<OutboxItem>> {
    let mut stmt = conn.prepare(
        "SELECT id, account_id, subject, to_addrs, status, error, created_at, updated_at
         FROM outbox ORDER BY created_at DESC LIMIT 200",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(OutboxItem {
            id: row.get(0)?,
            account_id: row.get(1)?,
            subject: row.get(2)?,
            to_addrs: row.get(3)?,
            status: row.get(4)?,
            error: row.get(5)?,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn get(conn: &Connection, id: i64) -> AppResult<Option<OutboxItem>> {
    Ok(conn
        .query_row(
            "SELECT id, account_id, subject, to_addrs, status, error, created_at, updated_at
             FROM outbox WHERE id = ?1",
            params![id],
            |row| {
                Ok(OutboxItem {
                    id: row.get(0)?,
                    account_id: row.get(1)?,
                    subject: row.get(2)?,
                    to_addrs: row.get(3)?,
                    status: row.get(4)?,
                    error: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                })
            },
        )
        .optional()?)
}

pub fn body(
    conn: &Connection,
    id: i64,
) -> AppResult<Option<(i64, String, String, String, String, String)>> {
    Ok(conn
        .query_row(
            "SELECT account_id, to_addrs, cc_addrs, bcc_addrs, subject, body_html
             FROM outbox WHERE id = ?1",
            params![id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .optional()?)
}

pub fn remove(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute("DELETE FROM outbox WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn add_attachments(conn: &Connection, outbox_id: i64, paths: &[String]) -> AppResult<()> {
    for path in paths {
        let name = std::path::Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.clone());
        let size = std::fs::metadata(path).map(|m| m.len() as i64).unwrap_or(0);
        conn.execute(
            "INSERT INTO outbox_attachments (outbox_id, filename, path, size) VALUES (?1, ?2, ?3, ?4)",
            params![outbox_id, name, path, size],
        )?;
    }
    Ok(())
}

pub struct OutboxFull {
    pub account_id: i64,
    pub to_addrs: String,
    pub cc_addrs: String,
    pub bcc_addrs: String,
    pub subject: String,
    pub body_html: String,
    pub body_text: String,
    pub attachments: Vec<String>,
}

pub fn full(conn: &Connection, id: i64) -> AppResult<Option<OutboxFull>> {
    let base = conn
        .query_row(
            "SELECT account_id, to_addrs, cc_addrs, bcc_addrs, subject, body_html, body_text
             FROM outbox WHERE id = ?1",
            params![id],
            |row| {
                Ok(OutboxFull {
                    account_id: row.get(0)?,
                    to_addrs: row.get(1)?,
                    cc_addrs: row.get(2)?,
                    bcc_addrs: row.get(3)?,
                    subject: row.get(4)?,
                    body_html: row.get(5)?,
                    body_text: row.get(6)?,
                    attachments: Vec::new(),
                })
            },
        )
        .optional()?;
    let Some(mut full) = base else {
        return Ok(None);
    };
    let mut stmt = conn.prepare(
        "SELECT path FROM outbox_attachments WHERE outbox_id = ?1 ORDER BY id",
    )?;
    let rows = stmt.query_map(params![id], |row| row.get::<_, String>(0))?;
    for row in rows {
        full.attachments.push(row?);
    }
    Ok(Some(full))
}
