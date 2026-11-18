use std::collections::HashMap;

use tauri::{AppHandle, State};

use crate::db::{accounts, folders, messages, Db};
use crate::error::AppResult;
use crate::models::{
    ActionResult, FlagInput, Folder, MessageDetail, MessagePage, MessageQuery, MoveInput,
};
use crate::services::{imap, oauth, parse, sync};

async fn login_account(db: &Db, account_id: i64) -> Result<(crate::models::Account, imap::ImapSession), String> {
    let account = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        accounts::get(&conn, account_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "账户不存在".to_string())?
    };
    let secret = oauth::account_secret(db, &account).await?;
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
    Ok((account, session))
}

#[tauri::command]
pub fn list_messages(state: State<'_, Db>, query: MessageQuery) -> AppResult<MessagePage> {
    let conn = state.lock()?;
    messages::list(&conn, &query)
}

#[tauri::command]
pub fn get_message(state: State<'_, Db>, id: i64) -> AppResult<Option<MessageDetail>> {
    let conn = state.lock()?;
    let mut detail = messages::get_detail(&conn, id)?;
    if let Some(d) = detail.as_mut() {
        if let Some(html) = &d.html_body {
            d.html_body = Some(parse::inline_cid_images(html, &d.attachments));
        }
    }
    Ok(detail)
}

#[tauri::command]
pub fn unread_count(state: State<'_, Db>, account_id: Option<i64>) -> AppResult<i64> {
    let conn = state.lock()?;
    messages::unread_count(&conn, account_id)
}

#[tauri::command]
pub async fn sync_account(
    app: AppHandle,
    state: State<'_, Db>,
    account_id: i64,
) -> Result<sync::SyncOutcome, String> {
    let db = state.inner().clone();
    sync::sync_account(&app, db, account_id).await
}

/// 触发全部账户同步（后台并发执行，立即返回账户数量）
#[tauri::command]
pub fn sync_all(app: AppHandle, state: State<'_, Db>) -> AppResult<usize> {
    let account_ids: Vec<i64> = {
        let conn = state.lock()?;
        accounts::list(&conn)?.into_iter().map(|a| a.id).collect()
    };
    for id in &account_ids {
        let app = app.clone();
        let db = state.inner().clone();
        let id = *id;
        tauri::async_runtime::spawn(async move {
            if let Err(e) = sync::sync_account(&app, db, id).await {
                tracing::warn!("账户 {id} 同步失败: {e}");
            }
        });
    }
    Ok(account_ids.len())
}

#[tauri::command]
pub fn list_folders(state: State<'_, Db>, account_id: Option<i64>) -> AppResult<Vec<Folder>> {
    let conn = state.lock()?;
    folders::list(&conn, account_id)
}

#[tauri::command]
pub async fn create_folder(
    state: State<'_, Db>,
    account_id: i64,
    name: String,
    parent_path: Option<String>,
) -> Result<Folder, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("文件夹名称不能为空".to_string());
    }
    let db = state.inner().clone();
    let (_account, mut session) = login_account(&db, account_id).await?;

    let delimiter = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        folders::list(&conn, Some(account_id))
            .map_err(|e| e.to_string())?
            .into_iter()
            .find_map(|f| {
                if f.delimiter.is_empty() {
                    None
                } else {
                    Some(f.delimiter)
                }
            })
            .unwrap_or_else(|| "/".to_string())
    };

    let path = match parent_path.filter(|p| !p.is_empty()) {
        Some(parent) => format!("{parent}{delimiter}{name}"),
        None => name.clone(),
    };

    let result = imap::create_folder(&mut session, &path).await;
    imap::logout(session).await;
    result?;

    let conn = db.lock().map_err(|e| e.to_string())?;
    folders::insert_local(&conn, account_id, &name, &path, &delimiter).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_folder(
    state: State<'_, Db>,
    account_id: i64,
    folder_id: i64,
) -> Result<(), String> {
    let folder = {
        let conn = state.lock().map_err(|e| e.to_string())?;
        folders::get(&conn, folder_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "文件夹不存在".to_string())?
    };
    if folder.account_id != account_id {
        return Err("文件夹不属于该账户".to_string());
    }
    if matches!(
        folder.kind.as_str(),
        "inbox" | "sent" | "drafts" | "trash" | "junk" | "archive"
    ) {
        return Err("系统文件夹不可删除".to_string());
    }

    let db = state.inner().clone();
    let (_account, mut session) = login_account(&db, account_id).await?;
    let result = imap::delete_folder(&mut session, &folder.path).await;
    imap::logout(session).await;
    result?;

    let conn = db.lock().map_err(|e| e.to_string())?;
    folders::delete_by_path(&conn, account_id, &folder.path).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn set_message_flags(
    state: State<'_, Db>,
    input: FlagInput,
) -> Result<ActionResult, String> {
    if input.message_ids.is_empty() {
        return Ok(ActionResult {
            updated: 0,
            message: "未选择邮件".to_string(),
        });
    }
    if input.read.is_none() && input.flagged.is_none() {
        return Err("未指定要修改的状态".to_string());
    }

    let db = state.inner().clone();
    let locs = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        messages::locations(&conn, &input.message_ids).map_err(|e| e.to_string())?
    };

    // 按 (账户, 文件夹) 分组，减少 IMAP 往返
    let mut groups: HashMap<(i64, String), Vec<i64>> = HashMap::new();
    for (_id, loc) in &locs {
        groups
            .entry((loc.account_id, loc.folder_path.clone()))
            .or_default()
            .push(loc.uid);
    }

    let mut sessions: HashMap<i64, imap::ImapSession> = HashMap::new();
    for ((account_id, folder_path), uids) in groups {
        if !sessions.contains_key(&account_id) {
            let (_account, session) = login_account(&db, account_id).await?;
            sessions.insert(account_id, session);
        }
        let session = sessions.get_mut(&account_id).expect("session inserted");
        imap::select(session, &folder_path).await?;
        if let Some(read) = input.read {
            imap::set_seen(session, &uids, read).await?;
        }
        if let Some(flagged) = input.flagged {
            imap::set_flagged(session, &uids, flagged).await?;
        }
    }

    for (_, session) in sessions {
        imap::logout(session).await;
    }

    let conn = db.lock().map_err(|e| e.to_string())?;
    let updated = messages::set_flags(&conn, &input.message_ids, input.read, input.flagged)?;
    let _ = folders::refresh_counts(&conn, None);
    Ok(ActionResult {
        updated,
        message: format!("已更新 {updated} 封邮件的状态"),
    })
}

#[derive(Clone)]
struct MoveTarget {
    path: String,
    folder_id: i64,
}

async fn ensure_special_folder(
    db: &Db,
    session: &mut imap::ImapSession,
    account_id: i64,
    kind: &str,
    default_name: &str,
) -> Result<MoveTarget, String> {
    let existing = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        folders::find_by_kind(&conn, account_id, kind).map_err(|e| e.to_string())?
    };
    if let Some(folder) = existing {
        return Ok(MoveTarget {
            path: folder.path,
            folder_id: folder.id,
        });
    }

    let delimiter = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        folders::list(&conn, Some(account_id))
            .map_err(|e| e.to_string())?
            .into_iter()
            .find_map(|f| (!f.delimiter.is_empty()).then_some(f.delimiter))
            .unwrap_or_else(|| "/".to_string())
    };

    // 本地尚无记录时，先看远端是否已有对应特殊文件夹（如 [Gmail]/Trash）
    if let Ok(remote) = imap::list_folders(session).await {
        if let Some(found) = remote.into_iter().find(|f| f.kind == kind) {
            let conn = db.lock().map_err(|e| e.to_string())?;
            let folder = folders::insert_local(
                &conn,
                account_id,
                &found.name,
                &found.path,
                &found.delimiter,
            )
            .map_err(|e| e.to_string())?;
            conn.execute(
                "UPDATE folders SET kind = ?1 WHERE id = ?2",
                rusqlite::params![kind, folder.id],
            )
            .map_err(|e| e.to_string())?;
            return Ok(MoveTarget {
                path: found.path,
                folder_id: folder.id,
            });
        }
    }

    let path = default_name.to_string();
    imap::create_folder(session, &path).await.ok();
    let conn = db.lock().map_err(|e| e.to_string())?;
    let folder = folders::insert_local(&conn, account_id, default_name, &path, &delimiter)
        .map_err(|e| e.to_string())?;
    // 新文件夹归类为归档/垃圾
    conn.execute(
        "UPDATE folders SET kind = ?1 WHERE id = ?2",
        rusqlite::params![kind, folder.id],
    )
    .map_err(|e| e.to_string())?;
    Ok(MoveTarget {
        path,
        folder_id: folder.id,
    })
}

#[tauri::command]
pub async fn move_messages(
    app: AppHandle,
    state: State<'_, Db>,
    input: MoveInput,
) -> Result<ActionResult, String> {
    if input.message_ids.is_empty() {
        return Ok(ActionResult {
            updated: 0,
            message: "未选择邮件".to_string(),
        });
    }
    let action = input.action.as_str();
    if !matches!(action, "delete" | "archive" | "junk" | "move") {
        return Err(format!("不支持的操作：{action}"));
    }

    let db = state.inner().clone();
    let (locs, folder_kinds, explicit_target) = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        let locs = messages::locations(&conn, &input.message_ids).map_err(|e| e.to_string())?;
        let mut kinds: HashMap<i64, String> = HashMap::new();
        for loc in locs.values() {
            if !kinds.contains_key(&loc.folder_id) {
                let kind = folders::get(&conn, loc.folder_id)
                    .map_err(|e| e.to_string())?
                    .map(|f| f.kind)
                    .unwrap_or_else(|| "other".to_string());
                kinds.insert(loc.folder_id, kind);
            }
        }
        let target = match input.target_folder_id {
            Some(id) => folders::get(&conn, id).map_err(|e| e.to_string())?,
            None => None,
        };
        (locs, kinds, target)
    };

    if action == "move" {
        let target = explicit_target.ok_or_else(|| "请选择目标文件夹".to_string())?;
        for loc in locs.values() {
            if loc.account_id != target.account_id {
                return Err("不能跨账户移动邮件".to_string());
            }
        }
        // 同一目标文件夹
        let mut moved = 0i64;
        let mut sessions: HashMap<i64, imap::ImapSession> = HashMap::new();
        let account_ids: Vec<i64> = locs.values().map(|l| l.account_id).collect();
        for account_id in account_ids {
            if !sessions.contains_key(&account_id) {
                let (_a, session) = login_account(&db, account_id).await?;
                sessions.insert(account_id, session);
            }
            let session = sessions.get_mut(&account_id).expect("session inserted");
            let mut by_folder: HashMap<String, Vec<i64>> = HashMap::new();
            for loc in locs.values().filter(|l| l.account_id == account_id) {
                if loc.folder_path == target.path {
                    continue;
                }
                by_folder
                    .entry(loc.folder_path.clone())
                    .or_default()
                    .push(loc.uid);
            }
            for (folder_path, uids) in by_folder {
                imap::select(session, &folder_path).await?;
                imap::copy_to(session, &uids, &target.path).await?;
                imap::delete_messages(session, &uids).await?;
                moved += uids.len() as i64;
            }
        }
        for (_, session) in sessions {
            imap::logout(session).await;
        }

        {
            let conn = db.lock().map_err(|e| e.to_string())?;
            messages::delete_ids(&conn, &input.message_ids)?;
            let _ = folders::refresh_counts(&conn, None);
        }
        let db2 = db.clone();
        let app2 = app.clone();
        tauri::async_runtime::spawn(async move {
            let _ = sync::sync_account(&app2, db2, target.account_id).await;
        });
        return Ok(ActionResult {
            updated: moved,
            message: format!("已移动 {moved} 封邮件"),
        });
    }

    // 按账户准备目标文件夹
    let mut sessions: HashMap<i64, imap::ImapSession> = HashMap::new();
    let mut created_targets: HashMap<i64, MoveTarget> = HashMap::new();
    let mut removed_ids: Vec<i64> = Vec::new();
    let mut affected_accounts: Vec<i64> = Vec::new();
    let mut updated = 0i64;

    let mut per_account_folder: HashMap<(i64, String), Vec<(i64, i64, String)>> = HashMap::new();
    for (id, loc) in &locs {
        let kind = folder_kinds
            .get(&loc.folder_id)
            .cloned()
            .unwrap_or_else(|| "other".to_string());
        per_account_folder
            .entry((loc.account_id, loc.folder_path.clone()))
            .or_default()
            .push((*id, loc.uid, kind));
    }

    for ((account_id, folder_path), items) in per_account_folder {
        let source_kind = items
            .first()
            .map(|(_, _, kind)| kind.clone())
            .unwrap_or_else(|| "other".to_string());
        let uids: Vec<i64> = items.iter().map(|(_, uid, _)| *uid).collect();
        let ids: Vec<i64> = items.iter().map(|(id, _, _)| *id).collect();

        if !sessions.contains_key(&account_id) {
            let (_a, session) = login_account(&db, account_id).await?;
            sessions.insert(account_id, session);
        }

        if action == "delete" && source_kind == "trash" {
            let session = sessions.get_mut(&account_id).expect("session inserted");
            imap::select(session, &folder_path).await?;
            imap::delete_messages(session, &uids).await?;
            removed_ids.extend(ids);
            updated += uids.len() as i64;
            if !affected_accounts.contains(&account_id) {
                affected_accounts.push(account_id);
            }
            continue;
        }

        let target = if let Some(t) = created_targets.get(&account_id) {
            t.clone()
        } else {
            let session = sessions.get_mut(&account_id).expect("session inserted");
            let t = match action {
                "delete" => {
                    ensure_special_folder(&db, session, account_id, "trash", "Trash").await?
                }
                "archive" => {
                    ensure_special_folder(&db, session, account_id, "archive", "Archive").await?
                }
                _ => ensure_special_folder(&db, session, account_id, "junk", "Junk").await?,
            };
            created_targets.insert(account_id, t.clone());
            t
        };

        if target.path == folder_path {
            removed_ids.extend(ids);
            continue;
        }

        let session = sessions.get_mut(&account_id).expect("session inserted");
        imap::select(session, &folder_path).await?;
        imap::copy_to(session, &uids, &target.path).await?;
        imap::delete_messages(session, &uids).await?;
        removed_ids.extend(ids);
        updated += uids.len() as i64;
        if !affected_accounts.contains(&account_id) {
            affected_accounts.push(account_id);
        }
    }

    for (_, session) in sessions {
        imap::logout(session).await;
    }

    {
        let conn = db.lock().map_err(|e| e.to_string())?;
        messages::delete_ids(&conn, &removed_ids)?;
        let _ = folders::refresh_counts(&conn, None);
    }

    for account_id in affected_accounts {
        let db2 = db.clone();
        let app2 = app.clone();
        tauri::async_runtime::spawn(async move {
            let _ = sync::sync_account(&app2, db2, account_id).await;
        });
    }

    let label = match action {
        "delete" => "删除",
        "archive" => "归档",
        "junk" => "标记垃圾",
        _ => "移动",
    };
    Ok(ActionResult {
        updated,
        message: format!("已{label} {updated} 封邮件"),
    })
}

#[tauri::command]
pub fn attachment_path(state: State<'_, Db>, attachment_id: i64) -> AppResult<Option<String>> {
    let conn = state.lock()?;
    Ok(messages::attachment_path(&conn, attachment_id)?
        .ok_or_else(|| crate::error::AppError::Message("附件不存在".to_string()))?
        .2)
}

#[tauri::command]
pub fn save_attachment(
    state: State<'_, Db>,
    attachment_id: i64,
    dest_path: String,
) -> Result<String, String> {
    let conn = state.lock().map_err(|e| e.to_string())?;
    let (filename, _mime, local) = messages::attachment_path(&conn, attachment_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "附件不存在".to_string())?;
    let local = local.ok_or_else(|| "附件文件未缓存到本地".to_string())?;
    let dest = std::path::Path::new(&dest_path);
    if dest.is_dir() {
        let safe = parse::safe_filename(&filename, "attachment.bin");
        std::fs::copy(&local, dest.join(safe))
            .map_err(|e| format!("保存附件失败: {e}"))?;
    } else {
        std::fs::copy(&local, dest).map_err(|e| format!("保存附件失败: {e}"))?;
    }
    Ok(dest_path)
}

/// 供前端在刷新时同步更新托盘/任务栏未读数
#[tauri::command]
pub fn refresh_badge(app: AppHandle, state: State<'_, Db>) -> AppResult<i64> {
    let conn = state.lock()?;
    let unread = messages::unread_count(&conn, None)?;
    drop(conn);
    sync::update_badge(&app, unread);
    Ok(unread)
}
