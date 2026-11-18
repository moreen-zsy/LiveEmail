use rusqlite::{params, Connection, OptionalExtension};

use crate::error::AppResult;
use crate::models::{Draft, DraftInput};

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn attachments(conn: &Connection, draft_id: i64) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT path FROM draft_attachments WHERE draft_id = ?1 ORDER BY id",
    )?;
    let rows = stmt.query_map(params![draft_id], |row| row.get::<_, String>(0))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

fn get(conn: &Connection, id: i64) -> AppResult<Option<Draft>> {
    let draft = conn
        .query_row(
            "SELECT id, account_id, subject, to_addrs, cc_addrs, bcc_addrs, body_html, body_text, updated_at
             FROM drafts WHERE id = ?1",
            params![id],
            |row| {
                Ok(Draft {
                    id: row.get(0)?,
                    account_id: row.get(1)?,
                    subject: row.get(2)?,
                    to_addrs: row.get(3)?,
                    cc_addrs: row.get(4)?,
                    bcc_addrs: row.get(5)?,
                    body_html: row.get(6)?,
                    body_text: row.get(7)?,
                    updated_at: row.get(8)?,
                    attachments: Vec::new(),
                })
            },
        )
        .optional()?;
    match draft {
        Some(mut d) => {
            d.attachments = attachments(conn, d.id)?;
            Ok(Some(d))
        }
        None => Ok(None),
    }
}

pub fn list(conn: &Connection) -> AppResult<Vec<Draft>> {
    let mut stmt = conn.prepare(
        "SELECT id, account_id, subject, to_addrs, cc_addrs, bcc_addrs, body_html, body_text, updated_at
         FROM drafts ORDER BY updated_at DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(Draft {
            id: row.get(0)?,
            account_id: row.get(1)?,
            subject: row.get(2)?,
            to_addrs: row.get(3)?,
            cc_addrs: row.get(4)?,
            bcc_addrs: row.get(5)?,
            body_html: row.get(6)?,
            body_text: row.get(7)?,
            updated_at: row.get(8)?,
            attachments: Vec::new(),
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        let mut d = row?;
        d.attachments = attachments(conn, d.id)?;
        out.push(d);
    }
    Ok(out)
}

pub fn save(conn: &Connection, input: &DraftInput) -> AppResult<Draft> {
    let now = now_unix();
    let id = match input.id {
        Some(id) if exists(conn, id)? => {
            conn.execute(
                "UPDATE drafts SET account_id = ?1, subject = ?2, to_addrs = ?3, cc_addrs = ?4,
                    bcc_addrs = ?5, body_html = ?6, body_text = ?7, updated_at = ?8
                 WHERE id = ?9",
                params![
                    input.account_id,
                    input.subject,
                    input.to_addrs,
                    input.cc_addrs,
                    input.bcc_addrs,
                    input.body_html,
                    input.body_text,
                    now,
                    id
                ],
            )?;
            id
        }
        _ => {
            conn.execute(
                "INSERT INTO drafts (account_id, subject, to_addrs, cc_addrs, bcc_addrs,
                    body_html, body_text, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    input.account_id,
                    input.subject,
                    input.to_addrs,
                    input.cc_addrs,
                    input.bcc_addrs,
                    input.body_html,
                    input.body_text,
                    now
                ],
            )?;
            conn.last_insert_rowid()
        }
    };

    conn.execute("DELETE FROM draft_attachments WHERE draft_id = ?1", params![id])?;
    for path in &input.attachments {
        let name = std::path::Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.clone());
        let size = std::fs::metadata(path).map(|m| m.len() as i64).unwrap_or(0);
        conn.execute(
            "INSERT INTO draft_attachments (draft_id, filename, path, size) VALUES (?1, ?2, ?3, ?4)",
            params![id, name, path, size],
        )?;
    }

    Ok(get(conn, id)?.expect("draft just saved"))
}

pub fn exists(conn: &Connection, id: i64) -> AppResult<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM drafts WHERE id = ?1)",
        params![id],
        |row| row.get(0),
    )?)
}

pub fn delete(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute("DELETE FROM drafts WHERE id = ?1", params![id])?;
    Ok(())
}
