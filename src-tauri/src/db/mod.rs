use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use rusqlite::Connection;

use crate::error::{AppError, AppResult};

pub mod accounts;
pub mod drafts;
pub mod folders;
pub mod messages;
pub mod outbox;
pub mod settings;

/// 全局数据库连接（用 Mutex 保护，供 Tauri State 管理）
#[derive(Clone)]
pub struct Db {
    pub conn: Arc<Mutex<Connection>>,
    pub app_dir: PathBuf,
}

impl Db {
    pub fn lock(&self) -> AppResult<MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|e| AppError::Message(format!("数据库连接被占用: {e}")))
    }

    /// 附件缓存目录：$APPDATA/LiveEmail/attachments
    pub fn attachments_dir(&self) -> PathBuf {
        let dir = self.app_dir.join("attachments");
        let _ = std::fs::create_dir_all(&dir);
        dir
    }
}

/// 打开（必要时创建）数据库并执行迁移
pub fn init(app_dir: &Path) -> AppResult<Db> {
    std::fs::create_dir_all(app_dir)?;
    let path = app_dir.join("liveemail.db");
    let conn = Connection::open(&path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    migrate(&conn)?;
    Ok(Db {
        conn: Arc::new(Mutex::new(conn)),
        app_dir: app_dir.to_path_buf(),
    })
}

/// 以 PRAGMA user_version 管理 schema 版本
fn migrate(conn: &Connection) -> AppResult<()> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version < 1 {
        conn.execute_batch(SCHEMA_V1)?;
        conn.execute_batch("PRAGMA user_version = 1;")?;
    }
    if version < 2 {
        // 逐列检查后再添加，兼容迁移中途失败后重试（旧版曾因 references 保留字失败）
        for (table, column, definition) in V2_COLUMNS {
            add_column_if_missing(conn, table, column, definition)?;
        }
        conn.execute_batch(SCHEMA_V2_REST)?;
        conn.execute_batch("PRAGMA user_version = 2;")?;
    }
    if version < 3 {
        conn.execute_batch(SCHEMA_V3)?;
        conn.execute_batch("PRAGMA user_version = 3;")?;
    }
    if version < 4 {
        add_column_if_missing(conn, "accounts", "oauth_client_secret", "TEXT")?;
        conn.execute_batch("PRAGMA user_version = 4;")?;
    }
    Ok(())
}

fn column_exists(conn: &Connection, table: &str, column: &str) -> AppResult<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name.eq_ignore_ascii_case(column) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn add_column_if_missing(
    conn: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> AppResult<()> {
    if !column_exists(conn, table, column)? {
        conn.execute_batch(&format!(
            "ALTER TABLE {table} ADD COLUMN \"{column}\" {definition};"
        ))?;
    }
    Ok(())
}

/// v2 新增列（列名统一加引号，避免 references 等保留字冲突）
const V2_COLUMNS: &[(&str, &str, &str)] = &[
    ("accounts", "is_default", "INTEGER NOT NULL DEFAULT 0"),
    ("accounts", "last_sync_at", "INTEGER"),
    ("accounts", "last_error", "TEXT"),
    ("accounts", "oauth_client_id", "TEXT"),
    ("accounts", "oauth_tenant", "TEXT"),
    ("folders", "unread_count", "INTEGER NOT NULL DEFAULT 0"),
    ("folders", "total_count", "INTEGER NOT NULL DEFAULT 0"),
    ("messages", "preview", "TEXT NOT NULL DEFAULT ''"),
    ("messages", "in_reply_to", "TEXT"),
    ("messages", "references", "TEXT"),
    ("messages", "raw_size", "INTEGER NOT NULL DEFAULT 0"),
];

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
"#;

/// v2：补齐全文检索、设置表与消息摘要字段（新增列见 V2_COLUMNS）
const SCHEMA_V2_REST: &str = r#"
CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS outbox_attachments (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    outbox_id   INTEGER NOT NULL REFERENCES outbox(id) ON DELETE CASCADE,
    filename    TEXT NOT NULL,
    path        TEXT NOT NULL,
    size        INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS draft_attachments (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    draft_id    INTEGER NOT NULL REFERENCES drafts(id) ON DELETE CASCADE,
    filename    TEXT NOT NULL,
    path        TEXT NOT NULL,
    size        INTEGER NOT NULL DEFAULT 0
);

CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(subject, from_addr, body);

CREATE INDEX IF NOT EXISTS idx_messages_unread ON messages(account_id, is_read);
CREATE INDEX IF NOT EXISTS idx_messages_message_id ON messages(message_id);
"#;

const SCHEMA_V3: &str = r#"
CREATE TABLE IF NOT EXISTS outbox_attachments (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    outbox_id   INTEGER NOT NULL REFERENCES outbox(id) ON DELETE CASCADE,
    filename    TEXT NOT NULL,
    path        TEXT NOT NULL,
    size        INTEGER NOT NULL DEFAULT 0
);
"#;

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;

    /// 内存数据库 + 全量迁移，供各模块单元测试复用
    pub fn memory_db() -> Connection {
        let conn = Connection::open_in_memory().expect("open memory db");
        conn.pragma_update(None, "foreign_keys", "ON")
            .expect("enable foreign keys");
        migrate(&conn).expect("run migrations");
        conn
    }
}

#[cfg(test)]
mod tests {
    use super::messages::{self, NewAttachment, NewMessage};
    use super::test_support::memory_db;
    use super::{accounts, folders};
    use crate::models::{AccountInput, MessageQuery};

    fn account_input(email: &str) -> AccountInput {
        AccountInput {
            email: email.to_string(),
            display_name: "测试用户".into(),
            provider: "custom".into(),
            imap_host: "imap.example.com".into(),
            imap_port: 993,
            imap_encryption: "ssl".into(),
            smtp_host: "smtp.example.com".into(),
            smtp_port: 465,
            smtp_encryption: "ssl".into(),
            auth_type: "password".into(),
            secret: None,
            oauth_client_id: None,
            oauth_tenant: None,
            oauth_client_secret: None,
        }
    }

    fn seed(conn: &rusqlite::Connection) -> (i64, i64) {
        let account = accounts::insert(conn, &account_input("a@example.com")).unwrap();
        folders::upsert_remote(
            conn,
            account.id,
            &[folders::RemoteFolder {
                name: "收件箱".into(),
                path: "INBOX".into(),
                delimiter: "/".into(),
                kind: "inbox".into(),
                uid_validity: Some(1),
                uid_next: None,
            }],
        )
        .unwrap();
        let folder = folders::find_by_kind(conn, account.id, "inbox")
            .unwrap()
            .unwrap();
        (account.id, folder.id)
    }

    fn new_message(uid: i64, subject: &str, body: &str) -> NewMessage {
        NewMessage {
            uid,
            message_id: Some(format!("<{uid}@example.com>")),
            subject: subject.into(),
            from_name: "Alice".into(),
            from_addr: "alice@example.com".into(),
            to_addrs: "a@example.com".into(),
            cc_addrs: String::new(),
            date: Some(1_700_000_000 + uid),
            is_read: false,
            is_flagged: false,
            has_attachment: false,
            size: 1024,
            preview: body.chars().take(80).collect(),
            in_reply_to: None,
            references: None,
            text_body: Some(body.into()),
            html_body: None,
            attachments: vec![],
        }
    }

    #[test]
    fn search_hits_subject_and_body() {
        let conn = memory_db();
        let (account_id, folder_id) = seed(&conn);
        messages::upsert(
            &conn,
            account_id,
            folder_id,
            &new_message(1, "Project Update", "hello world"),
        )
        .unwrap();
        messages::upsert(
            &conn,
            account_id,
            folder_id,
            &new_message(2, "其他邮件", "无关内容"),
        )
        .unwrap();

        let q = MessageQuery {
            search: Some("hello".into()),
            ..Default::default()
        };
        let page = messages::list(&conn, &q).unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].subject, "Project Update");
    }

    #[test]
    fn chinese_search_falls_back_to_like() {
        let conn = memory_db();
        let (account_id, folder_id) = seed(&conn);
        messages::upsert(
            &conn,
            account_id,
            folder_id,
            &new_message(1, "增值税发票提醒", "请查收发票"),
        )
        .unwrap();
        messages::upsert(
            &conn,
            account_id,
            folder_id,
            &new_message(2, "Weekly report", "nothing here"),
        )
        .unwrap();

        // FTS 分词不一定能命中中文子串，应回退 LIKE
        let q = MessageQuery {
            search: Some("发票".into()),
            ..Default::default()
        };
        let page = messages::list(&conn, &q).unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].subject, "增值税发票提醒");
    }

    #[test]
    fn filters_and_pagination() {
        let conn = memory_db();
        let (account_id, folder_id) = seed(&conn);
        let mut a = new_message(1, "unread", "a");
        a.has_attachment = true;
        let mut b = new_message(2, "read-flagged", "b");
        b.is_read = true;
        b.is_flagged = true;
        messages::upsert(&conn, account_id, folder_id, &a).unwrap();
        messages::upsert(&conn, account_id, folder_id, &b).unwrap();

        let unread = messages::list(
            &conn,
            &MessageQuery {
                unread_only: Some(true),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(unread.total, 1);

        let flagged = messages::list(
            &conn,
            &MessageQuery {
                flagged_only: Some(true),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(flagged.total, 1);
        assert_eq!(flagged.items[0].subject, "read-flagged");

        let with_att = messages::list(
            &conn,
            &MessageQuery {
                has_attachment: Some(true),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(with_att.total, 1);

        let first = messages::list(
            &conn,
            &MessageQuery {
                limit: Some(1),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(first.items.len(), 1);
        assert_eq!(first.total, 2);
        // 日期倒序，uid 2 在前
        assert_eq!(first.items[0].uid, 2);

        let second = messages::list(
            &conn,
            &MessageQuery {
                limit: Some(1),
                offset: Some(1),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(second.items[0].uid, 1);
    }

    #[test]
    fn upsert_is_idempotent_and_keeps_attachments() {
        let conn = memory_db();
        let (account_id, folder_id) = seed(&conn);
        let mut msg = new_message(7, "with attachment", "body");
        msg.has_attachment = true;
        msg.attachments.push(NewAttachment {
            filename: "a.txt".into(),
            mime_type: "text/plain".into(),
            size: 3,
            content_id: None,
            local_path: None,
        });
        let (id1, is_new1) = messages::upsert(&conn, account_id, folder_id, &msg).unwrap();
        let (id2, is_new2) = messages::upsert(&conn, account_id, folder_id, &msg).unwrap();
        assert!(is_new1);
        assert!(!is_new2);
        assert_eq!(id1, id2);

        let detail = messages::get_detail(&conn, id1).unwrap().unwrap();
        assert_eq!(detail.attachments.len(), 1);
        assert_eq!(detail.summary.subject, "with attachment");
    }

    #[test]
    fn flags_and_delete_cleanup_fts() {
        let conn = memory_db();
        let (account_id, folder_id) = seed(&conn);
        let (id, _) = messages::upsert(
            &conn,
            account_id,
            folder_id,
            &new_message(3, "delete me", "x"),
        )
        .unwrap();

        messages::set_flags(&conn, &[id], Some(true), Some(true)).unwrap();
        let detail = messages::get_detail(&conn, id).unwrap().unwrap();
        assert!(detail.summary.is_read);
        assert!(detail.summary.is_flagged);

        assert_eq!(messages::unread_count(&conn, Some(account_id)).unwrap(), 0);

        messages::delete_ids(&conn, &[id]).unwrap();
        assert!(messages::get_detail(&conn, id).unwrap().is_none());
        let page = messages::list(
            &conn,
            &MessageQuery {
                search: Some("delete".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(page.total, 0);
    }

    #[test]
    fn sync_cursor_defaults_max_and_reset() {
        let conn = memory_db();
        let (account_id, _) = seed(&conn);

        assert_eq!(
            folders::sync_cursor(&conn, account_id, "INBOX").unwrap(),
            (None, 0)
        );

        folders::set_sync_cursor(&conn, account_id, "INBOX", Some(10), 42).unwrap();
        assert_eq!(
            folders::sync_cursor(&conn, account_id, "INBOX").unwrap(),
            (Some(10), 42)
        );

        // 更小的游标不应回退
        folders::set_sync_cursor(&conn, account_id, "INBOX", Some(10), 30).unwrap();
        assert_eq!(
            folders::sync_cursor(&conn, account_id, "INBOX").unwrap(),
            (Some(10), 42)
        );

        folders::reset_sync_cursor(&conn, account_id, "INBOX", Some(11)).unwrap();
        assert_eq!(
            folders::sync_cursor(&conn, account_id, "INBOX").unwrap(),
            (Some(11), 0)
        );
    }

    #[test]
    fn folder_counts_refresh() {
        let conn = memory_db();
        let (account_id, folder_id) = seed(&conn);
        messages::upsert(&conn, account_id, folder_id, &new_message(1, "one", "x")).unwrap();
        let mut read = new_message(2, "two", "y");
        read.is_read = true;
        messages::upsert(&conn, account_id, folder_id, &read).unwrap();

        folders::refresh_counts(&conn, Some(account_id)).unwrap();
        let folder = folders::get(&conn, folder_id).unwrap().unwrap();
        assert_eq!(folder.total_count, 2);
        assert_eq!(folder.unread_count, 1);
    }
}
