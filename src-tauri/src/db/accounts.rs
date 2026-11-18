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
        is_default: row.get::<_, i64>(12)? != 0,
        last_sync_at: row.get(13)?,
        last_error: row.get(14)?,
        oauth_client_id: row.get(15)?,
        oauth_tenant: row.get(16)?,
        oauth_client_secret: row.get(17)?,
    })
}

const SELECT_COLS: &str = "id, email, display_name, provider, imap_host, imap_port, imap_encryption, \
    smtp_host, smtp_port, smtp_encryption, auth_type, created_at, is_default, last_sync_at, \
    last_error, oauth_client_id, oauth_tenant, oauth_client_secret";

pub fn list(conn: &Connection) -> AppResult<Vec<Account>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLS} FROM accounts ORDER BY is_default DESC, created_at ASC"
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

pub fn default_account(conn: &Connection) -> AppResult<Option<Account>> {
    let account = conn
        .query_row(
            &format!(
                "SELECT {SELECT_COLS} FROM accounts ORDER BY is_default DESC, created_at ASC LIMIT 1"
            ),
            [],
            row_to_account,
        )
        .optional()?;
    Ok(account)
}

pub fn insert(conn: &Connection, input: &AccountInput) -> AppResult<Account> {
    let is_first = count(conn)? == 0;
    conn.execute(
        "INSERT INTO accounts
            (email, display_name, provider, imap_host, imap_port, imap_encryption,
             smtp_host, smtp_port, smtp_encryption, auth_type, created_at, is_default,
             oauth_client_id, oauth_tenant, oauth_client_secret)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
        params![
            input.email.trim(),
            input.display_name,
            input.provider,
            input.imap_host.trim(),
            input.imap_port,
            input.imap_encryption,
            input.smtp_host.trim(),
            input.smtp_port,
            input.smtp_encryption,
            input.auth_type,
            now_unix(),
            if is_first { 1 } else { 0 },
            input.oauth_client_id,
            input.oauth_tenant,
            input.oauth_client_secret,
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
            auth_type = ?10, oauth_client_id = ?11, oauth_tenant = ?12,
            oauth_client_secret = ?13
         WHERE id = ?14",
        params![
            input.email.trim(),
            input.display_name,
            input.provider,
            input.imap_host.trim(),
            input.imap_port,
            input.imap_encryption,
            input.smtp_host.trim(),
            input.smtp_port,
            input.smtp_encryption,
            input.auth_type,
            input.oauth_client_id,
            input.oauth_tenant,
            input.oauth_client_secret,
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

pub fn count(conn: &Connection) -> AppResult<i64> {
    Ok(conn.query_row("SELECT COUNT(*) FROM accounts", [], |row| row.get(0))?)
}

pub fn set_default(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute("UPDATE accounts SET is_default = 0", [])?;
    conn.execute(
        "UPDATE accounts SET is_default = 1 WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}

pub fn mark_sync(conn: &Connection, id: i64, error: Option<&str>) -> AppResult<()> {
    conn.execute(
        "UPDATE accounts SET last_sync_at = ?1, last_error = ?2 WHERE id = ?3",
        params![now_unix(), error, id],
    )?;
    Ok(())
}
