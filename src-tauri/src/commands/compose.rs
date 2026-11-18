use tauri::State;

use crate::db::{accounts, drafts, folders, outbox, Db};
use crate::error::AppResult;
use crate::models::{
    ComposeInput, Draft, DraftInput, OutboxItem, SendResult,
};
use crate::services::{imap, oauth, smtp};

const MAX_ATTACHMENT_BYTES: u64 = 25 * 1024 * 1024;

fn load_account(db: &Db, account_id: i64) -> Result<crate::models::Account, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    accounts::get(&conn, account_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "账户不存在".to_string())
}

fn validate_recipients(to: &[String], cc: &[String], bcc: &[String]) -> Result<(), String> {
    if to.is_empty() {
        return Err("请至少填写一个收件人".to_string());
    }
    for addr in to.iter().chain(cc.iter()).chain(bcc.iter()) {
        if !smtp::valid_address(addr) {
            return Err(format!("邮箱地址格式不正确：{addr}"));
        }
    }
    Ok(())
}

fn check_attachments(paths: &[String]) -> Result<u64, String> {
    let mut total = 0u64;
    for path in paths {
        let meta = std::fs::metadata(path)
            .map_err(|e| format!("附件不存在或无法读取：{path}（{e}）"))?;
        total += meta.len();
    }
    if total > MAX_ATTACHMENT_BYTES {
        return Err(format!(
            "附件总大小超过 25MB（当前 {:.1}MB）",
            total as f64 / 1024.0 / 1024.0
        ));
    }
    Ok(total)
}

/// 发送成功后尝试追加到「已发送」文件夹；失败不影响发送结果
fn sent_path(db: &Db, account_id: i64) -> Option<String> {
    let conn = db.lock().ok()?;
    folders::find_by_kind(&conn, account_id, "sent")
        .ok()
        .flatten()
        .map(|f| f.path)
}

async fn archive_to_sent(
    db: &Db,
    account: &crate::models::Account,
    secret: &str,
    raw: &[u8],
) -> bool {
    let Some(path) = sent_path(db, account.id) else {
        return false;
    };
    let session = imap::login(
        &imap::ImapConfig {
            host: account.imap_host.clone(),
            port: account.imap_port,
            encryption: account.imap_encryption.clone(),
            email: account.email.clone(),
        },
        secret,
        account.auth_type == "oauth2",
    )
    .await;
    let Ok(mut session) = session else {
        return false;
    };
    let ok = imap::append_to(&mut session, &path, raw).await.is_ok();
    imap::logout(session).await;
    ok
}

fn enqueue_failed(
    db: &Db,
    input: &ComposeInput,
) -> Result<i64, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let id = outbox::enqueue(
        &conn,
        input.account_id,
        &input.subject,
        &input.to_addrs,
        &input.cc_addrs,
        &input.bcc_addrs,
        &input.body_html,
        &input.body_text,
    )
    .map_err(|e| e.to_string())?;
    if !input.attachments.is_empty() {
        outbox::add_attachments(&conn, id, &input.attachments).map_err(|e| e.to_string())?;
    }
    outbox::set_status(&conn, id, "failed", Some("等待重试")).map_err(|e| e.to_string())?;
    Ok(id)
}

#[tauri::command]
pub async fn send_mail(state: State<'_, Db>, input: ComposeInput) -> Result<SendResult, String> {
    let db = state.inner().clone();
    let account = load_account(&db, input.account_id)?;
    let secret = oauth::account_secret(&db, &account).await?;

    let to = smtp::parse_addresses(&input.to_addrs);
    let cc = smtp::parse_addresses(&input.cc_addrs);
    let bcc = smtp::parse_addresses(&input.bcc_addrs);
    validate_recipients(&to, &cc, &bcc)?;
    let _ = check_attachments(&input.attachments)?;

    let outgoing = smtp::Outgoing {
        host: &account.smtp_host,
        port: account.smtp_port,
        encryption: &account.smtp_encryption,
        email: &account.email,
        display_name: &account.display_name,
        secret: &secret,
        oauth2: account.auth_type == "oauth2",
        to,
        cc,
        bcc,
        subject: &input.subject,
        body_html: &input.body_html,
        body_text: &input.body_text,
        in_reply_to: input.in_reply_to.as_deref(),
        references: input.references.as_deref(),
        attachments: input.attachments.clone(),
    };

    match smtp::send(&outgoing).await {
        Ok(raw) => {
            let archived = archive_to_sent(&db, &account, &secret, &raw).await;
            if let Some(draft_id) = input.draft_id {
                if let Ok(conn) = db.lock() {
                    let _ = drafts::delete(&conn, draft_id);
                }
            }
            Ok(SendResult {
                ok: true,
                message: if archived {
                    "邮件已发送".to_string()
                } else {
                    "邮件已发送（未能归档到服务器「已发送」，稍后同步可恢复）".to_string()
                },
                archived,
                queued: false,
                outbox_id: None,
            })
        }
        Err(error) => {
            let outbox_id = enqueue_failed(&db, &input)?;
            Ok(SendResult {
                ok: false,
                message: format!("发送失败，已进入发送队列：{error}"),
                archived: false,
                queued: true,
                outbox_id: Some(outbox_id),
            })
        }
    }
}

#[tauri::command]
pub async fn retry_outbox(state: State<'_, Db>, id: i64) -> Result<SendResult, String> {
    let db = state.inner().clone();
    let full = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        outbox::full(&conn, id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "队列中的邮件不存在".to_string())?
    };
    let account = load_account(&db, full.account_id)?;
    let secret = oauth::account_secret(&db, &account).await?;

    let to = smtp::parse_addresses(&full.to_addrs);
    let cc = smtp::parse_addresses(&full.cc_addrs);
    let bcc = smtp::parse_addresses(&full.bcc_addrs);
    validate_recipients(&to, &cc, &bcc)?;

    let outgoing = smtp::Outgoing {
        host: &account.smtp_host,
        port: account.smtp_port,
        encryption: &account.smtp_encryption,
        email: &account.email,
        display_name: &account.display_name,
        secret: &secret,
        oauth2: account.auth_type == "oauth2",
        to,
        cc,
        bcc,
        subject: &full.subject,
        body_html: &full.body_html,
        body_text: &full.body_text,
        in_reply_to: None,
        references: None,
        attachments: full.attachments.clone(),
    };

    match smtp::send(&outgoing).await {
        Ok(raw) => {
            let archived = archive_to_sent(&db, &account, &secret, &raw).await;
            if let Ok(conn) = db.lock() {
                let _ = outbox::remove(&conn, id);
            }
            Ok(SendResult {
                ok: true,
                message: "重试发送成功".to_string(),
                archived,
                queued: false,
                outbox_id: None,
            })
        }
        Err(error) => {
            if let Ok(conn) = db.lock() {
                let _ = outbox::set_status(&conn, id, "failed", Some(&error));
            }
            Ok(SendResult {
                ok: false,
                message: format!("重试失败：{error}"),
                archived: false,
                queued: true,
                outbox_id: Some(id),
            })
        }
    }
}

#[tauri::command]
pub fn list_outbox(state: State<'_, Db>) -> AppResult<Vec<OutboxItem>> {
    let conn = state.lock()?;
    outbox::list(&conn)
}

#[tauri::command]
pub fn delete_outbox(state: State<'_, Db>, id: i64) -> AppResult<()> {
    let conn = state.lock()?;
    outbox::remove(&conn, id)
}

#[tauri::command]
pub fn save_draft(state: State<'_, Db>, input: DraftInput) -> AppResult<Draft> {
    let conn = state.lock()?;
    drafts::save(&conn, &input)
}

#[tauri::command]
pub fn list_drafts(state: State<'_, Db>) -> AppResult<Vec<Draft>> {
    let conn = state.lock()?;
    drafts::list(&conn)
}

#[tauri::command]
pub fn delete_draft(state: State<'_, Db>, id: i64) -> AppResult<()> {
    let conn = state.lock()?;
    drafts::delete(&conn, id)
}
