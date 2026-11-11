use rusqlite::{params, Connection, OptionalExtension};

use crate::error::AppResult;
use crate::models::{Account, AccountInput};

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn row_to_account(row: &rusqlite::Row<'_>) -> rusqlite::Result<Account> {
    Ok(Account {
        id: row.get(0)?,
        email: row.get(1)?,
        display_name: row.get(2)?,
        provider: row.get(3)?,
        imap_host: row.get(4)?,
        imap_port: row.get(5)?,
        imap_encryption: row.get(6)?,
        smtp_host: row.get(7)?,
        smtp_port: row.get(8)?,
        smtp_encryption: row.get(9)?,
        auth_type: row.get(10)?,
        created_at: row.get(11)?,
    })
}

const SELECT_COLS: &str = "id, email, display_name, provider, imap_host, imap_port, imap_encryption, smtp_host, smtp_port, smtp_encryption, auth_type, created_at";

pub fn list(conn: &Connection) -> AppResult<Vec<Account>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLS} FROM accounts ORDER BY created_at ASC"
    ))?;
    let rows = stmt.query_map([], row_to_account)?;
    let mut accounts = Vec::new();
    for row in rows {
        accounts.push(row?);
    }
    Ok(accounts)
}

pub fn get(conn: &Connection, id: i64) -> AppResult<Option<Account>> {
    let account = conn
        .query_row(
            &format!("SELECT {SELECT_COLS} FROM accounts WHERE id = ?1"),
            params![id],
            row_to_account,
        )
        .optional()?;
    Ok(account)
}

pub fn insert(conn: &Connection, input: &AccountInput) -> AppResult<Account> {
    conn.execute(
        "INSERT INTO accounts
            (email, display_name, provider, imap_host, imap_port, imap_encryption,
             smtp_host, smtp_port, smtp_encryption, auth_type, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            input.email,
            input.display_name,
            input.provider,
            input.imap_host,
            input.imap_port,
            input.imap_encryption,
            input.smtp_host,
            input.smtp_port,
            input.smtp_encryption,
            input.auth_type,
            now_unix()
        ],
    )?;
    let id = conn.last_insert_rowid();
    Ok(get(conn, id)?.expect("just inserted account must exist"))
}

pub fn update(conn: &Connection, id: i64, input: &AccountInput) -> AppResult<Option<Account>> {
    let affected = conn.execute(
        "UPDATE accounts SET
            email = ?1, display_name = ?2, provider = ?3,
            imap_host = ?4, imap_port = ?5, imap_encryption = ?6,
            smtp_host = ?7, smtp_port = ?8, smtp_encryption = ?9,
            auth_type = ?10
         WHERE id = ?11",
        params![
            input.email,
            input.display_name,
            input.provider,
            input.imap_host,
            input.imap_port,
            input.imap_encryption,
            input.smtp_host,
            input.smtp_port,
            input.smtp_encryption,
            input.auth_type,
            id
        ],
    )?;
    if affected == 0 {
        return Ok(None);
    }
    get(conn, id)
}

pub fn delete(conn: &Connection, id: i64) -> AppResult<bool> {
    let affected = conn.execute("DELETE FROM accounts WHERE id = ?1", params![id])?;
    Ok(affected > 0)
}
