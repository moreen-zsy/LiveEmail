use std::collections::HashMap;

use rusqlite::{params, params_from_iter, Connection, OptionalExtension, ToSql};
use rusqlite::types::Value;

use crate::error::AppResult;
use crate::models::{
    AttachmentMeta, MessageDetail, MessagePage, MessageQuery, MessageSummary,
};

pub struct NewAttachment {
    pub filename: String,
    pub mime_type: String,
    pub size: i64,
    pub content_id: Option<String>,
    pub local_path: Option<String>,
}

pub struct NewMessage {
    pub uid: i64,
    pub message_id: Option<String>,
    pub subject: String,
    pub from_name: String,
    pub from_addr: String,
    pub to_addrs: String,
    pub cc_addrs: String,
    pub date: Option<i64>,
    pub is_read: bool,
    pub is_flagged: bool,
    pub has_attachment: bool,
    pub size: i64,
    pub preview: String,
    pub in_reply_to: Option<String>,
    pub references: Option<String>,
    pub text_body: Option<String>,
    pub html_body: Option<String>,
    pub attachments: Vec<NewAttachment>,
}

pub struct Loc {
    pub account_id: i64,
    pub folder_id: i64,
    pub folder_path: String,
    pub uid: i64,
}

/// 插入或更新邮件；返回 (message_id, 是否新邮件)
pub fn upsert(
    conn: &Connection,
    account_id: i64,
    folder_id: i64,
    msg: &NewMessage,
) -> AppResult<(i64, bool)> {
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM messages WHERE folder_id = ?1 AND uid = ?2",
            params![folder_id, msg.uid],
            |row| row.get(0),
        )
        .optional()?;

    let id = match existing {
        Some(id) => {
            conn.execute(
                "UPDATE messages SET
                    message_id = ?1, subject = ?2, from_name = ?3, from_addr = ?4,
                    to_addrs = ?5, cc_addrs = ?6, date = ?7, is_read = ?8, is_flagged = ?9,
                    has_attachment = ?10, size = ?11, preview = ?12, in_reply_to = ?13,
                    \"references\" = ?14, raw_size = ?11
                 WHERE id = ?15",
                params![
                    msg.message_id,
                    msg.subject,
                    msg.from_name,
                    msg.from_addr,
                    msg.to_addrs,
                    msg.cc_addrs,
                    msg.date,
                    msg.is_read as i64,
                    msg.is_flagged as i64,
                    msg.has_attachment as i64,
                    msg.size,
                    msg.preview,
                    msg.in_reply_to,
                    msg.references,
                    id
                ],
            )?;
            id
        }
        None => {
            conn.execute(
                "INSERT INTO messages
                    (account_id, folder_id, uid, message_id, subject, from_name, from_addr,
                     to_addrs, cc_addrs, date, is_read, is_flagged, has_attachment, size,
                     preview, in_reply_to, \"references\", raw_size)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?14)",
                params![
                    account_id,
                    folder_id,
                    msg.uid,
                    msg.message_id,
                    msg.subject,
                    msg.from_name,
                    msg.from_addr,
                    msg.to_addrs,
                    msg.cc_addrs,
                    msg.date,
                    msg.is_read as i64,
                    msg.is_flagged as i64,
                    msg.has_attachment as i64,
                    msg.size,
                    msg.preview,
                    msg.in_reply_to,
                    msg.references
                ],
            )?;
            conn.last_insert_rowid()
        }
    };

    // 正文
    conn.execute(
        "INSERT INTO message_bodies (message_id, text_body, html_body) VALUES (?1, ?2, ?3)
         ON CONFLICT(message_id) DO UPDATE SET
            text_body = COALESCE(excluded.text_body, message_bodies.text_body),
            html_body = COALESCE(excluded.html_body, message_bodies.html_body)",
        params![id, msg.text_body, msg.html_body],
    )?;

    // 附件：仅首次写入（同 UID 已存在则跳过，避免重复）
    let has_attachments: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM attachments WHERE message_id = ?1)",
        params![id],
        |row| row.get(0),
    )?;
    if !has_attachments {
        for a in &msg.attachments {
            conn.execute(
                "INSERT INTO attachments (message_id, filename, mime_type, size, content_id, local_path)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![id, a.filename, a.mime_type, a.size, a.content_id, a.local_path],
            )?;
        }
    }

    // 全文索引
    conn.execute("DELETE FROM messages_fts WHERE rowid = ?1", params![id])?;
    let body = msg
        .text_body
        .clone()
        .or_else(|| msg.html_body.clone())
        .unwrap_or_default();
    let body: String = body.chars().take(200_000).collect();
    conn.execute(
        "INSERT INTO messages_fts (rowid, subject, from_addr, body) VALUES (?1, ?2, ?3, ?4)",
        params![id, msg.subject, msg.from_addr, body],
    )?;

    Ok((id, existing.is_none()))
}

fn row_to_summary(row: &rusqlite::Row<'_>) -> rusqlite::Result<MessageSummary> {
    Ok(MessageSummary {
        id: row.get(0)?,
        account_id: row.get(1)?,
        folder_id: row.get(2)?,
        folder_name: row.get(3)?,
        account_email: row.get(4)?,
        uid: row.get(5)?,
        subject: row.get(6)?,
        from_name: row.get(7)?,
        from_addr: row.get(8)?,
        to_addrs: row.get(9)?,
        preview: row.get(10)?,
        date: row.get(11)?,
        is_read: row.get::<_, i64>(12)? != 0,
        is_flagged: row.get::<_, i64>(13)? != 0,
        has_attachment: row.get::<_, i64>(14)? != 0,
        size: row.get(15)?,
    })
}

const SUMMARY_COLS: &str = "m.id, m.account_id, m.folder_id, f.name, a.email, m.uid, m.subject, \
    m.from_name, m.from_addr, m.to_addrs, m.preview, m.date, m.is_read, m.is_flagged, \
    m.has_attachment, m.size";

fn build_where(query: &MessageQuery) -> (String, Vec<Value>) {
    let mut clauses: Vec<String> = Vec::new();
    let mut values: Vec<Value> = Vec::new();

    if let Some(account_id) = query.account_id {
        clauses.push("m.account_id = ?".into());
        values.push(account_id.into());
    }
    if let Some(folder_id) = query.folder_id {
        clauses.push("m.folder_id = ?".into());
        values.push(folder_id.into());
    } else if query.unified.unwrap_or(true) {
        clauses.push("f.kind = 'inbox'".into());
    }
    if query.unread_only.unwrap_or(false) {
        clauses.push("m.is_read = 0".into());
    }
    if query.flagged_only.unwrap_or(false) {
        clauses.push("m.is_flagged = 1".into());
    }
    if query.has_attachment.unwrap_or(false) {
        clauses.push("m.has_attachment = 1".into());
    }
    if let Some(from) = query.date_from {
        clauses.push("m.date >= ?".into());
        values.push(from.into());
    }
    if let Some(to) = query.date_to {
        clauses.push("m.date <= ?".into());
        values.push(to.into());
    }

    let search = query.search.as_deref().map(str::trim).filter(|s| !s.is_empty());
    if let Some(term) = search {
        let escaped = term.replace('"', "\"\"");
        clauses.push(
            "m.id IN (SELECT rowid FROM messages_fts WHERE messages_fts MATCH ?)".into(),
        );
        values.push(format!("\"{escaped}\"").into());
    }

    let where_sql = if clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", clauses.join(" AND "))
    };
    (where_sql, values)
}

pub fn list(conn: &Connection, query: &MessageQuery) -> AppResult<MessagePage> {
    let (where_sql, values) = build_where(query);
    let limit = query.limit.unwrap_or(50).clamp(1, 500);
    let offset = query.offset.unwrap_or(0).max(0);

    let count_sql = format!(
        "SELECT COUNT(*) FROM messages m
         JOIN folders f ON f.id = m.folder_id
         JOIN accounts a ON a.id = m.account_id
         {where_sql}"
    );
    let total: i64 = conn.query_row(&count_sql, params_from_iter(values.clone()), |row| row.get(0))?;

    // 统一收件箱：跨账户时用 LIKE 兜底，避免 FTS 语法差异导致空结果
    let sql = format!(
        "SELECT {SUMMARY_COLS} FROM messages m
         JOIN folders f ON f.id = m.folder_id
         JOIN accounts a ON a.id = m.account_id
         {where_sql}
         ORDER BY COALESCE(m.date, 0) DESC, m.id DESC
         LIMIT ? OFFSET ?"
    );
    let mut all_values = values;
    all_values.push(limit.into());
    all_values.push(offset.into());

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_from_iter(all_values), row_to_summary)?;
    let mut items = Vec::new();
    for row in rows {
        items.push(row?);
    }

    // FTS 查询可能因分词/语法返回 0，回退 LIKE
    if items.is_empty() && total == 0 {
        if let Some(term) = query.search.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            let like = format!("%{term}%");
            let (where2, mut values2) = {
                let mut q = query.clone();
                q.search = None;
                build_where(&q)
            };
            let extra = if where2.is_empty() {
                "WHERE".to_string()
            } else {
                format!("{where2} AND")
            };
            let sql2 = format!(
                "SELECT {SUMMARY_COLS} FROM messages m
                 JOIN folders f ON f.id = m.folder_id
                 JOIN accounts a ON a.id = m.account_id
                 {extra} (m.subject LIKE ? OR m.from_addr LIKE ? OR m.from_name LIKE ?
                     OR EXISTS (SELECT 1 FROM message_bodies b WHERE b.message_id = m.id
                         AND (b.text_body LIKE ? OR b.html_body LIKE ?)))
                 ORDER BY COALESCE(m.date, 0) DESC, m.id DESC LIMIT ? OFFSET ?"
            );
            values2.push(like.clone().into());
            values2.push(like.clone().into());
            values2.push(like.clone().into());
            values2.push(like.clone().into());
            values2.push(like.clone().into());
            values2.push(limit.into());
            values2.push(offset.into());
            let mut stmt2 = conn.prepare(&sql2)?;
            let rows2 = stmt2.query_map(params_from_iter(values2), row_to_summary)?;
            let mut fallback = Vec::new();
            for row in rows2 {
                fallback.push(row?);
            }
            let count2: i64 = conn.query_row(
                &format!(
                    "SELECT COUNT(*) FROM messages m
                     JOIN folders f ON f.id = m.folder_id
                     JOIN accounts a ON a.id = m.account_id
                     {extra} (m.subject LIKE ? OR m.from_addr LIKE ? OR m.from_name LIKE ?
                         OR EXISTS (SELECT 1 FROM message_bodies b WHERE b.message_id = m.id
                             AND (b.text_body LIKE ? OR b.html_body LIKE ?)))"
                ),
                params_from_iter(vec![
                    like.clone(),
                    like.clone(),
                    like.clone(),
                    like.clone(),
                    like,
                ]),
                |row| row.get(0),
            )?;
            return Ok(MessagePage {
                items: fallback,
                total: count2,
            });
        }
    }

    Ok(MessagePage { items, total })
}

pub fn get_summary(conn: &Connection, id: i64) -> AppResult<Option<MessageSummary>> {
    Ok(conn
        .query_row(
            &format!(
                "SELECT {SUMMARY_COLS} FROM messages m
                 JOIN folders f ON f.id = m.folder_id
                 JOIN accounts a ON a.id = m.account_id
                 WHERE m.id = ?1"
            ),
            params![id],
            row_to_summary,
        )
        .optional()?)
}

pub fn get_detail(conn: &Connection, id: i64) -> AppResult<Option<MessageDetail>> {
    let Some(summary) = get_summary(conn, id)? else {
        return Ok(None);
    };
    let extra = conn
        .query_row(
            "SELECT m.message_id, m.cc_addrs, '', m.in_reply_to, m.\"references\",
                    b.text_body, b.html_body
             FROM messages m LEFT JOIN message_bodies b ON b.message_id = m.id
             WHERE m.id = ?1",
            params![id],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                ))
            },
        )
        .optional()?;

    let (message_id, cc_addrs, bcc_addrs, in_reply_to, references, text_body, html_body) =
        extra.unwrap_or((None, String::new(), String::new(), None, None, None, None));

    let mut stmt = conn.prepare(
        "SELECT id, filename, mime_type, size, content_id, local_path
         FROM attachments WHERE message_id = ?1 ORDER BY id",
    )?;
    let attachments = stmt
        .query_map(params![id], |row| {
            Ok(AttachmentMeta {
                id: row.get(0)?,
                filename: row.get(1)?,
                mime_type: row.get(2)?,
                size: row.get(3)?,
                content_id: row.get(4)?,
                local_path: row.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(Some(MessageDetail {
        summary,
        message_id,
        cc_addrs,
        bcc_addrs,
        in_reply_to,
        references,
        text_body,
        html_body,
        attachments,
    }))
}

pub fn set_flags(
    conn: &Connection,
    ids: &[i64],
    read: Option<bool>,
    flagged: Option<bool>,
) -> AppResult<i64> {
    let mut updated = 0i64;
    for id in ids {
        if let Some(read) = read {
            updated += conn.execute(
                "UPDATE messages SET is_read = ?1 WHERE id = ?2",
                params![read as i64, id],
            )? as i64;
        }
        if let Some(flagged) = flagged {
            updated += conn.execute(
                "UPDATE messages SET is_flagged = ?1 WHERE id = ?2",
                params![flagged as i64, id],
            )? as i64;
        }
    }
    Ok(updated)
}

pub fn mark_read(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute(
        "UPDATE messages SET is_read = 1 WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}

pub fn locations(conn: &Connection, ids: &[i64]) -> AppResult<HashMap<i64, Loc>> {
    let mut out = HashMap::new();
    for id in ids {
        let loc = conn
            .query_row(
                "SELECT m.account_id, m.folder_id, f.path, m.uid
                 FROM messages m JOIN folders f ON f.id = m.folder_id WHERE m.id = ?1",
                params![id],
                |row| {
                    Ok(Loc {
                        account_id: row.get(0)?,
                        folder_id: row.get(1)?,
                        folder_path: row.get(2)?,
                        uid: row.get(3)?,
                    })
                },
            )
            .optional()?;
        if let Some(loc) = loc {
            out.insert(*id, loc);
        }
    }
    Ok(out)
}

pub fn loc_account_id(conn: &Connection, id: i64) -> AppResult<Option<i64>> {
    Ok(conn
        .query_row("SELECT account_id FROM messages WHERE id = ?1", params![id], |row| row.get(0))
        .optional()?)
}

pub fn loc_folder_path(conn: &Connection, id: i64) -> AppResult<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT f.path FROM messages m JOIN folders f ON f.id = m.folder_id WHERE m.id = ?1",
            params![id],
            |row| row.get(0),
        )
        .optional()?)
}

pub fn loc_uid(conn: &Connection, id: i64) -> AppResult<Option<i64>> {
    Ok(conn
        .query_row("SELECT uid FROM messages WHERE id = ?1", params![id], |row| row.get(0))
        .optional()?)
}

pub fn delete_ids(conn: &Connection, ids: &[i64]) -> AppResult<i64> {
    let mut deleted = 0;
    for id in ids {
        conn.execute("DELETE FROM messages_fts WHERE rowid = ?1", params![id])?;
        deleted += conn.execute("DELETE FROM messages WHERE id = ?1", params![id])? as i64;
    }
    Ok(deleted)
}

pub fn clear_folder(conn: &Connection, folder_id: i64) -> AppResult<()> {
    let ids: Vec<i64> = {
        let mut stmt = conn.prepare("SELECT id FROM messages WHERE folder_id = ?1")?;
        stmt.query_map(params![folder_id], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    delete_ids(conn, &ids)?;
    Ok(())
}

pub fn unread_count(conn: &Connection, account_id: Option<i64>) -> AppResult<i64> {
    let count = match account_id {
        Some(id) => conn.query_row(
            "SELECT COUNT(*) FROM messages m JOIN folders f ON f.id = m.folder_id
             WHERE m.account_id = ?1 AND m.is_read = 0 AND f.kind <> 'drafts'",
            params![id],
            |row| row.get(0),
        )?,
        None => conn.query_row(
            "SELECT COUNT(*) FROM messages m JOIN folders f ON f.id = m.folder_id
             WHERE m.is_read = 0 AND f.kind <> 'drafts'",
            [],
            |row| row.get(0),
        )?,
    };
    Ok(count)
}

pub fn unread_by_folder(conn: &Connection, account_id: i64) -> AppResult<HashMap<i64, i64>> {
    let mut stmt = conn.prepare(
        "SELECT folder_id, COUNT(*) FROM messages WHERE account_id = ?1 AND is_read = 0 GROUP BY folder_id",
    )?;
    let mut map = HashMap::new();
    for row in stmt.query_map(params![account_id], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
    })? {
        let (folder_id, count) = row?;
        map.insert(folder_id, count);
    }
    Ok(map)
}

pub fn count(conn: &Connection) -> AppResult<i64> {
    Ok(conn.query_row("SELECT COUNT(*) FROM messages", [], |row| row.get(0))?)
}

pub fn attachment_path(conn: &Connection, id: i64) -> AppResult<Option<(String, String, Option<String>)>> {
    Ok(conn
        .query_row(
            "SELECT filename, mime_type, local_path FROM attachments WHERE id = ?1",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?)
}

pub fn exists(conn: &Connection, id: i64) -> AppResult<bool> {
    Ok(conn.query_row("SELECT EXISTS(SELECT 1 FROM messages WHERE id = ?1)", params![id], |row| {
        row.get(0)
    })?)
}

/// 为兼容旧调用保留的 id 查询（用于命令层）
pub fn message_folder_id(conn: &Connection, id: i64) -> AppResult<Option<i64>> {
    Ok(conn
        .query_row("SELECT folder_id FROM messages WHERE id = ?1", params![id], |row| row.get(0))
        .optional()?)
}

pub fn clear_all(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(
        "DELETE FROM messages; DELETE FROM messages_fts; DELETE FROM attachments; DELETE FROM sync_state;",
    )?;
    Ok(())
}

/// 供测试/调试：按条件计数
pub fn count_where(conn: &Connection, sql_where: &str, values: Vec<Value>) -> AppResult<i64> {
    let sql = format!("SELECT COUNT(*) FROM messages m JOIN folders f ON f.id = m.folder_id {sql_where}");
    Ok(conn.query_row(&sql, params_from_iter(values), |row| row.get(0))?)
}

// 让动态参数编译期检查
fn _assert_to_sql<T: ToSql + ?Sized>() {}
