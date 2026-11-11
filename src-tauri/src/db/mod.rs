use std::path::Path;
use std::sync::Mutex;

use rusqlite::Connection;

use crate::error::AppResult;

/// 全局数据库连接（用 Mutex 保护，供 Tauri State 管理）
pub struct Db(pub Mutex<Connection>);

/// 打开（必要时创建）数据库并执行迁移
pub fn init(path: &Path) -> AppResult<Db> {
    let conn = Connection::open(path)?;
    migrate(&conn)?;
    Ok(Db(Mutex::new(conn)))
}

/// 以 PRAGMA user_version 管理 schema 版本
fn migrate(conn: &Connection) -> AppResult<()> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version < 1 {
        conn.execute_batch(SCHEMA_V1)?;
        conn.execute_batch("PRAGMA user_version = 1;")?;
    }
    Ok(())
}

const SCHEMA_V1: &str = r#"
CREATE TABLE IF NOT EXISTS accounts (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    email            TEXT NOT NULL UNIQUE,
    display_name     TEXT NOT NULL DEFAULT '',
    provider         TEXT NOT NULL DEFAULT 'custom',
    imap_host        TEXT NOT NULL,
    imap_port        INTEGER NOT NULL,
    imap_encryption  TEXT NOT NULL DEFAULT 'ssl',
    smtp_host        TEXT NOT NULL,
    smtp_port        INTEGER NOT NULL,
    smtp_encryption  TEXT NOT NULL DEFAULT 'ssl',
    auth_type        TEXT NOT NULL DEFAULT 'password',
    created_at       INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS folders (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id    INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    name          TEXT NOT NULL,
    path          TEXT NOT NULL,
    delimiter     TEXT NOT NULL DEFAULT '',
    kind          TEXT NOT NULL DEFAULT 'other',
    uid_validity  INTEGER,
    uid_next      INTEGER,
    UNIQUE (account_id, path)
);

CREATE TABLE IF NOT EXISTS messages (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id    INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    folder_id     INTEGER NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
    uid           INTEGER NOT NULL,
    message_id    TEXT,
    subject       TEXT NOT NULL DEFAULT '',
    from_name     TEXT NOT NULL DEFAULT '',
    from_addr     TEXT NOT NULL DEFAULT '',
    to_addrs      TEXT NOT NULL DEFAULT '',
    cc_addrs      TEXT NOT NULL DEFAULT '',
    date          INTEGER,
    is_read       INTEGER NOT NULL DEFAULT 0,
    is_flagged    INTEGER NOT NULL DEFAULT 0,
    has_attachment INTEGER NOT NULL DEFAULT 0,
    size          INTEGER NOT NULL DEFAULT 0,
    UNIQUE (folder_id, uid)
);
CREATE INDEX IF NOT EXISTS idx_messages_folder_date ON messages(folder_id, date DESC);
CREATE INDEX IF NOT EXISTS idx_messages_account_date ON messages(account_id, date DESC);

CREATE TABLE IF NOT EXISTS message_bodies (
    message_id  INTEGER PRIMARY KEY REFERENCES messages(id) ON DELETE CASCADE,
    text_body   TEXT,
    html_body   TEXT
);

CREATE TABLE IF NOT EXISTS attachments (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    message_id   INTEGER NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    filename     TEXT NOT NULL DEFAULT '',
    mime_type    TEXT NOT NULL DEFAULT 'application/octet-stream',
    size         INTEGER NOT NULL DEFAULT 0,
    content_id   TEXT,
    local_path   TEXT
);
CREATE INDEX IF NOT EXISTS idx_attachments_message ON attachments(message_id);

CREATE TABLE IF NOT EXISTS sync_state (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id    INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    folder_path   TEXT NOT NULL,
    uid_validity  INTEGER,
    last_uid      INTEGER,
    updated_at    INTEGER NOT NULL,
    UNIQUE (account_id, folder_path)
);

CREATE TABLE IF NOT EXISTS drafts (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id   INTEGER REFERENCES accounts(id) ON DELETE SET NULL,
    subject      TEXT NOT NULL DEFAULT '',
    to_addrs     TEXT NOT NULL DEFAULT '',
    cc_addrs     TEXT NOT NULL DEFAULT '',
    bcc_addrs    TEXT NOT NULL DEFAULT '',
    body_html    TEXT NOT NULL DEFAULT '',
    body_text    TEXT NOT NULL DEFAULT '',
    updated_at   INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS outbox (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id   INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    subject      TEXT NOT NULL DEFAULT '',
    to_addrs     TEXT NOT NULL DEFAULT '',
    cc_addrs     TEXT NOT NULL DEFAULT '',
    bcc_addrs    TEXT NOT NULL DEFAULT '',
    body_html    TEXT NOT NULL DEFAULT '',
    body_text    TEXT NOT NULL DEFAULT '',
    status       TEXT NOT NULL DEFAULT 'pending',
    error        TEXT,
    created_at   INTEGER NOT NULL,
    updated_at   INTEGER NOT NULL
);

CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
    subject, from_addr, body,
    content='messages',
    content_rowid='id'
);
"#;
