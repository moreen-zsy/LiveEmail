use rusqlite::{params, Connection, OptionalExtension};

use crate::error::AppResult;
use crate::models::Folder;

const SELECT_COLS: &str = "id, account_id, name, path, delimiter, kind, unread_count, total_count, \
    uid_validity, uid_next";

fn row_to_folder(row: &rusqlite::Row<'_>) -> rusqlite::Result<Folder> {
    Ok(Folder {
        id: row.get(0)?,
        account_id: row.get(1)?,
        name: row.get(2)?,
        path: row.get(3)?,
        delimiter: row.get(4)?,
        kind: row.get(5)?,
        unread_count: row.get(6)?,
        total_count: row.get(7)?,
        uid_validity: row.get(8)?,
        uid_next: row.get(9)?,
    })
}

pub struct RemoteFolder {
    pub name: String,
    pub path: String,
    pub delimiter: String,
    pub kind: String,
    pub uid_validity: Option<i64>,
    pub uid_next: Option<i64>,
}

pub fn list(conn: &Connection, account_id: Option<i64>) -> AppResult<Vec<Folder>> {
    let mut out = Vec::new();
    match account_id {
        Some(id) => {
            let mut stmt = conn.prepare(&format!(
                "SELECT {SELECT_COLS} FROM folders WHERE account_id = ?1 ORDER BY kind, name"
            ))?;
            for row in stmt.query_map(params![id], row_to_folder)? {
                out.push(row?);
            }
        }
        None => {
            let mut stmt = conn.prepare(&format!(
                "SELECT {SELECT_COLS} FROM folders ORDER BY account_id, kind, name"
            ))?;
            for row in stmt.query_map([], row_to_folder)? {
                out.push(row?);
            }
        }
    }
    Ok(out)
}

pub fn get(conn: &Connection, id: i64) -> AppResult<Option<Folder>> {
    Ok(conn
        .query_row(
            &format!("SELECT {SELECT_COLS} FROM folders WHERE id = ?1"),
            params![id],
            row_to_folder,
        )
        .optional()?)
}

pub fn find_by_kind(
    conn: &Connection,
    account_id: i64,
    kind: &str,
) -> AppResult<Option<Folder>> {
    Ok(conn
        .query_row(
            &format!(
                "SELECT {SELECT_COLS} FROM folders WHERE account_id = ?1 AND kind = ?2 LIMIT 1"
            ),
            params![account_id, kind],
            row_to_folder,
        )
        .optional()?)
}

pub fn find_by_path(
    conn: &Connection,
    account_id: i64,
    path: &str,
) -> AppResult<Option<Folder>> {
    Ok(conn
        .query_row(
            &format!(
                "SELECT {SELECT_COLS} FROM folders WHERE account_id = ?1 AND path = ?2"
            ),
            params![account_id, path],
            row_to_folder,
        )
        .optional()?)
}

/// 同步远端文件夹列表：新增/更新，删除远端已不存在的（消息随之级联删除）
pub fn upsert_remote(
    conn: &Connection,
    account_id: i64,
    folders: &[RemoteFolder],
) -> AppResult<()> {
    let tx = conn.unchecked_transaction()?;
    for f in folders {
        tx.execute(
            "INSERT INTO folders (account_id, name, path, delimiter, kind, uid_validity, uid_next)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(account_id, path) DO UPDATE SET
                name = excluded.name,
                delimiter = excluded.delimiter,
                kind = excluded.kind,
                uid_validity = COALESCE(excluded.uid_validity, folders.uid_validity),
                uid_next = COALESCE(excluded.uid_next, folders.uid_next)",
            params![
                account_id,
                f.name,
                f.path,
                f.delimiter,
                f.kind,
                f.uid_validity,
                f.uid_next
            ],
        )?;
    }

    // 清理远端已不存在的文件夹
    let paths: Vec<String> = folders.iter().map(|f| f.path.clone()).collect();
    let placeholders = if paths.is_empty() {
        "''".to_string()
    } else {
        std::iter::repeat("?")
            .take(paths.len())
            .collect::<Vec<_>>()
            .join(",")
    };
    let sql = format!(
        "DELETE FROM folders WHERE account_id = ? AND path NOT IN ({placeholders})"
    );
    let mut values: Vec<rusqlite::types::Value> = vec![account_id.into()];
    for p in paths {
        values.push(p.into());
    }
    tx.execute(&sql, rusqlite::params_from_iter(values))?;
    tx.commit()?;
    Ok(())
}

pub fn update_state(
    conn: &Connection,
    folder_id: i64,
    uid_validity: Option<i64>,
    uid_next: Option<i64>,
) -> AppResult<()> {
    conn.execute(
        "UPDATE folders SET uid_validity = ?1, uid_next = ?2 WHERE id = ?3",
        params![uid_validity, uid_next, folder_id],
    )?;
    Ok(())
}

pub fn refresh_counts(conn: &Connection, account_id: Option<i64>) -> AppResult<()> {
    let sql = match account_id {
        Some(_) => {
            "UPDATE folders SET
                unread_count = (SELECT COUNT(*) FROM messages m WHERE m.folder_id = folders.id AND m.is_read = 0),
                total_count = (SELECT COUNT(*) FROM messages m WHERE m.folder_id = folders.id)
             WHERE account_id = ?1"
        }
        None => {
            "UPDATE folders SET
                unread_count = (SELECT COUNT(*) FROM messages m WHERE m.folder_id = folders.id AND m.is_read = 0),
                total_count = (SELECT COUNT(*) FROM messages m WHERE m.folder_id = folders.id)"
        }
    };
    match account_id {
        Some(id) => conn.execute(sql, params![id])?,
        None => conn.execute(sql, [])?,
    };
    Ok(())
}

pub fn insert_local(
    conn: &Connection,
    account_id: i64,
    name: &str,
    path: &str,
    delimiter: &str,
) -> AppResult<Folder> {
    conn.execute(
        "INSERT INTO folders (account_id, name, path, delimiter, kind)
         VALUES (?1, ?2, ?3, ?4, 'other')
         ON CONFLICT(account_id, path) DO UPDATE SET name = excluded.name",
        params![account_id, name, path, delimiter],
    )?;
    Ok(find_by_path(conn, account_id, path)?.expect("folder just inserted"))
}

pub fn delete_by_path(conn: &Connection, account_id: i64, path: &str) -> AppResult<bool> {
    let affected = conn.execute(
        "DELETE FROM folders WHERE account_id = ?1 AND path = ?2",
        params![account_id, path],
    )?;
    Ok(affected > 0)
}

pub fn sync_cursor(conn: &Connection, account_id: i64, path: &str) -> AppResult<(Option<i64>, i64)> {
    let row = conn
        .query_row(
            "SELECT uid_validity, last_uid FROM sync_state WHERE account_id = ?1 AND folder_path = ?2",
            params![account_id, path],
            |row| Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, Option<i64>>(1)?)),
        )
        .optional()?;
    Ok(match row {
        Some((v, Some(last))) => (v, last),
        Some((v, None)) => (v, 0),
        None => (None, 0),
    })
}

pub fn reset_sync_cursor(
    conn: &Connection,
    account_id: i64,
    path: &str,
    uid_validity: Option<i64>,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO sync_state (account_id, folder_path, uid_validity, last_uid, updated_at)
         VALUES (?1, ?2, ?3, 0, ?4)
         ON CONFLICT(account_id, folder_path) DO UPDATE SET
            uid_validity = excluded.uid_validity,
            last_uid = 0,
            updated_at = excluded.updated_at",
        params![account_id, path, uid_validity, chrono_now()],
    )?;
    Ok(())
}

pub fn set_sync_cursor(
    conn: &Connection,
    account_id: i64,
    path: &str,
    uid_validity: Option<i64>,
    last_uid: i64,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO sync_state (account_id, folder_path, uid_validity, last_uid, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(account_id, folder_path) DO UPDATE SET
            uid_validity = excluded.uid_validity,
            last_uid = MAX(sync_state.last_uid, excluded.last_uid),
            updated_at = excluded.updated_at",
        params![
            account_id,
            path,
            uid_validity,
            last_uid,
            chrono_now()
        ],
    )?;
    Ok(())
}

fn chrono_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
