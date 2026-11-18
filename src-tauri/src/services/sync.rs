use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use async_imap::extensions::idle::IdleResponse;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::db::folders::{self, RemoteFolder};
use crate::db::messages;
use crate::db::Db;
use crate::services::{imap, oauth, parse};

fn syncing_set() -> &'static Mutex<HashSet<i64>> {
    static SET: OnceLock<Mutex<HashSet<i64>>> = OnceLock::new();
    SET.get_or_init(|| Mutex::new(HashSet::new()))
}

fn idle_set() -> &'static Mutex<HashSet<i64>> {
    static SET: OnceLock<Mutex<HashSet<i64>>> = OnceLock::new();
    SET.get_or_init(|| Mutex::new(HashSet::new()))
}

async fn account_login(db: &Db, account: &crate::models::Account) -> Result<String, String> {
    oauth::account_secret(db, account).await
}

/// 为所有账户启动 IDLE 监听（每账户一个长连接）
pub fn ensure_idle_watchers(app: &AppHandle, db: &Db) {
    let ids: Vec<i64> = match db.lock() {
        Ok(conn) => crate::db::accounts::list(&conn)
            .unwrap_or_default()
            .into_iter()
            .map(|a| a.id)
            .collect(),
        Err(_) => return,
    };
    let mut started = match idle_set().lock() {
        Ok(set) => set,
        Err(_) => return,
    };
    for id in ids {
        if !started.insert(id) {
            continue;
        }
        let app = app.clone();
        let db = db.clone();
        tauri::async_runtime::spawn(async move {
            idle_watch(app, db, id).await;
            if let Ok(mut set) = idle_set().lock() {
                set.remove(&id);
            }
        });
    }
}

async fn idle_watch(app: AppHandle, db: Db, account_id: i64) {
    loop {
        let exists = match db.lock() {
            Ok(conn) => crate::db::accounts::get(&conn, account_id)
                .ok()
                .flatten()
                .is_some(),
            Err(_) => false,
        };
        if !exists {
            break;
        }

        match idle_once(&db, account_id).await {
            Ok(true) => {
                tracing::debug!("账户 {account_id} IDLE 收到新邮件，触发同步");
                if let Err(e) = sync_account(&app, db.clone(), account_id).await {
                    tracing::debug!("IDLE 触发的同步失败: {e}");
                }
            }
            Ok(false) => {}
            Err(e) => tracing::debug!("账户 {account_id} IDLE 连接结束: {e}"),
        }
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
}

/// 建立一次 IDLE 连接；返回是否因新数据而退出
async fn idle_once(db: &Db, account_id: i64) -> Result<bool, String> {
    let (account, inbox_path) = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        let account = crate::db::accounts::get(&conn, account_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "账户不存在".to_string())?;
        let inbox = crate::db::folders::find_by_kind(&conn, account_id, "inbox")
            .ok()
            .flatten()
            .map(|f| f.path)
            .unwrap_or_else(|| "INBOX".to_string());
        (account, inbox)
    };
    let secret = account_login(db, &account).await?;
    let session = imap::login(
        &imap::ImapConfig {
            host: account.imap_host.clone(),
            port: account.imap_port,
            encryption: account.imap_encryption.clone(),
            email: account.email.clone(),
        },
        &secret,
        account.auth_type == "oauth2",
    )
    .await?;

    let mut handle = imap::start_idle(session, &inbox_path).await?;
    let (future, _interrupt) = handle.wait();
    let outcome = future.await;
    drop(_interrupt);
    match outcome {
        Ok(IdleResponse::NewData(_)) => {
            let _ = handle.done().await;
            Ok(true)
        }
        Ok(_) => {
            let _ = handle.done().await;
            Ok(false)
        }
        Err(e) => Err(format!("IDLE 监听失败: {e}")),
    }
}

struct SyncGuard(i64);

impl SyncGuard {
    fn acquire(account_id: i64) -> Result<Self, String> {
        let mut set = syncing_set()
            .lock()
            .map_err(|e| format!("同步状态锁异常: {e}"))?;
        if !set.insert(account_id) {
            return Err("该账户正在同步中，请稍候".to_string());
        }
        Ok(SyncGuard(account_id))
    }
}

impl Drop for SyncGuard {
    fn drop(&mut self) {
        if let Ok(mut set) = syncing_set().lock() {
            set.remove(&self.0);
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncProgress {
    pub account_id: i64,
    pub folder: String,
    pub index: usize,
    pub total: usize,
    pub new_messages: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncDone {
    pub account_id: i64,
    pub new_messages: i64,
    pub folders: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncError {
    pub account_id: i64,
    pub error: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NewMailEvent {
    pub account_id: i64,
    pub subject: String,
    pub from: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncOutcome {
    pub new_messages: i64,
    pub folders: usize,
}

struct InboxNew {
    subject: String,
    from: String,
}

pub async fn sync_account(
    app: &AppHandle,
    db: Db,
    account_id: i64,
) -> Result<SyncOutcome, String> {
    let _guard = SyncGuard::acquire(account_id)?;

    let (account, notify_enabled) = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        let account = crate::db::accounts::get(&conn, account_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "账户不存在".to_string())?;
        let settings = crate::db::settings::all(&conn).map_err(|e| e.to_string())?;
        (account, settings.notifications_enabled)
    };
    let secret = oauth::account_secret(&db, &account).await?;

    let config = imap::ImapConfig {
        host: account.imap_host.clone(),
        port: account.imap_port,
        encryption: account.imap_encryption.clone(),
        email: account.email.clone(),
    };

    let mut session = imap::login(&config, &secret, account.auth_type == "oauth2")
        .await
        .map_err(|e| {
            if let Ok(conn) = db.lock() {
                let _ = crate::db::accounts::mark_sync(&conn, account_id, Some(&e));
            }
            e
        })?;

    let remote_folders = imap::list_folders(&mut session).await?;
    if remote_folders.is_empty() {
        return Err("未获取到任何邮件文件夹，请确认 IMAP 已开启".to_string());
    }
    {
        let conn = db.lock().map_err(|e| e.to_string())?;
        folders::upsert_remote(&conn, account_id, &remote_folders).map_err(|e| e.to_string())?;
    }

    // 本地文件夹映射 path -> (id, uid_validity)
    let local_map: Vec<(String, i64, Option<i64>)> = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        folders::list(&conn, Some(account_id))
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|f| (f.path, f.id, f.uid_validity))
            .collect()
    };

    let total = remote_folders.len();
    let attachments_root = db.attachments_dir();
    let mut total_new = 0i64;
    let mut inbox_new: Vec<InboxNew> = Vec::new();
    let mut last_error: Option<String> = None;

    for (index, remote) in remote_folders.iter().enumerate() {
        let Some((folder_id, local_validity)) = local_map
            .iter()
            .find(|(path, _, _)| path == &remote.path)
            .map(|(_, id, v)| (*id, *v))
        else {
            continue;
        };

        let _ = app.emit(
            "sync:progress",
            SyncProgress {
                account_id,
                folder: remote.name.clone(),
                index,
                total,
                new_messages: total_new,
            },
        );

        match sync_folder(
            &mut session,
            &db,
            account_id,
            folder_id,
            remote,
            local_validity,
            &attachments_root,
        )
        .await
        {
            Ok((new_count, items)) => {
                total_new += new_count;
                if remote.kind == "inbox" {
                    inbox_new.extend(items);
                }
            }
            Err(e) => {
                // 单文件夹失败不阻断其他文件夹
                tracing::warn!("同步文件夹 {} 失败: {e}", remote.path);
                last_error = Some(format!("{}: {e}", remote.name));
            }
        }
    }

    imap::logout(session).await;

    {
        let conn = db.lock().map_err(|e| e.to_string())?;
        let _ = folders::refresh_counts(&conn, Some(account_id));
        crate::db::accounts::mark_sync(&conn, account_id, last_error.as_deref())
            .map_err(|e| e.to_string())?;
    }

    let unread = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        messages::unread_count(&conn, None).unwrap_or(0)
    };
    update_badge(app, unread);

    let _ = app.emit(
        "sync:done",
        SyncDone {
            account_id,
            new_messages: total_new,
            folders: total,
        },
    );

    if total_new > 0 {
        let _ = app.emit(
            "sync:new-email",
            NewMailEvent {
                account_id,
                subject: inbox_new
                    .first()
                    .map(|i| i.subject.clone())
                    .unwrap_or_else(|| "新邮件".into()),
                from: inbox_new.first().map(|i| i.from.clone()).unwrap_or_default(),
                count: total_new,
            },
        );

        if notify_enabled && !inbox_new.is_empty() {
            show_notification(app, &account.display_name, &account.email, &inbox_new);
        }
    }

    if let Some(err) = last_error {
        let _ = app.emit("sync:error", SyncError { account_id, error: err.clone() });
        // 部分文件夹失败仍视为完成，但把错误回传给 UI
        tracing::warn!("账户 {account_id} 同步部分失败: {err}");
    }

    Ok(SyncOutcome {
        new_messages: total_new,
        folders: total,
    })
}

#[allow(clippy::too_many_arguments)]
async fn sync_folder(
    session: &mut imap::ImapSession,
    db: &Db,
    account_id: i64,
    folder_id: i64,
    remote: &RemoteFolder,
    local_validity: Option<i64>,
    attachments_root: &std::path::Path,
) -> Result<(i64, Vec<InboxNew>), String> {
    let mailbox = imap::select(session, &remote.path).await?;
    let remote_validity = mailbox.uid_validity.map(|v| v as i64);

    let (stored_validity, mut last_uid) = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        folders::sync_cursor(&conn, account_id, &remote.path).map_err(|e| e.to_string())?
    };

    let dir = attachments_root
        .join(account_id.to_string())
        .join(folder_id.to_string());

    let validity_changed = stored_validity.is_some() && stored_validity != remote_validity;
    let local_has_messages = local_validity.is_some();
    if validity_changed || (local_has_messages && local_validity != remote_validity) {
        // UIDVALIDITY 变化：本地缓存失效，全量重拉
        let conn = db.lock().map_err(|e| e.to_string())?;
        messages::clear_folder(&conn, folder_id).map_err(|e| e.to_string())?;
        folders::reset_sync_cursor(&conn, account_id, &remote.path, remote_validity)
            .map_err(|e| e.to_string())?;
        last_uid = 0;
        drop(conn);
        let _ = std::fs::remove_dir_all(&dir);
    }

    let from_uid = if last_uid <= 0 { 0 } else { last_uid + 1 };
    let fetched = imap::fetch_messages(session, from_uid).await?;

    let mut new_count = 0i64;
    let mut new_items = Vec::new();
    let mut max_uid = last_uid;

    for raw in fetched {
        max_uid = max_uid.max(raw.uid);
        let dest = dir.join(raw.uid.to_string());
        let seen = raw.flags.iter().any(|f| f == "seen");
        let flagged = raw.flags.iter().any(|f| f == "flagged");
        let Some(parsed) = parse::parse_message(&raw.body, &dest, seen, flagged, raw.size) else {
            continue;
        };
        let mut new = parsed.new;
        new.uid = raw.uid;

        let subject = new.subject.clone();
        let from_name = new.from_name.clone();
        let from_addr = new.from_addr.clone();
        let is_new = {
            let conn = db.lock().map_err(|e| e.to_string())?;
            let (_, is_new) = messages::upsert(&conn, account_id, folder_id, &new)
                .map_err(|e| e.to_string())?;
            is_new
        };
        if is_new {
            new_count += 1;
            if remote.kind == "inbox" {
                new_items.push(InboxNew {
                    subject,
                    from: if from_name.is_empty() { from_addr } else { from_name },
                });
            }
        }
    }

    {
        let conn = db.lock().map_err(|e| e.to_string())?;
        folders::update_state(
            &conn,
            folder_id,
            remote_validity,
            mailbox.uid_next.map(|v| v as i64),
        )
        .map_err(|e| e.to_string())?;
        if max_uid > last_uid {
            folders::set_sync_cursor(&conn, account_id, &remote.path, remote_validity, max_uid)
                .map_err(|e| e.to_string())?;
        }
    }

    Ok((new_count, new_items))
}

pub fn update_badge(app: &AppHandle, unread: i64) {
    if let Some(window) = app.get_webview_window("main") {
        #[cfg(desktop)]
        let _ = window.set_badge_count(if unread > 0 { Some(unread) } else { None });
        let title = if unread > 0 {
            format!("LiveEmail ({unread})")
        } else {
            "LiveEmail".to_string()
        };
        let _ = window.set_title(&title);
    }
}

fn show_notification(app: &AppHandle, display_name: &str, email: &str, items: &[InboxNew]) {
    use tauri_plugin_notification::NotificationExt;
    let first = &items[0];
    let title = if items.len() > 1 {
        format!("{} 封新邮件", items.len())
    } else {
        format!("新邮件 · {}", if display_name.is_empty() { email } else { display_name })
    };
    let body = format!("{}：{}", first.from, first.subject);
    let _ = app
        .notification()
        .builder()
        .title(title)
        .body(body)
        .show();
}
