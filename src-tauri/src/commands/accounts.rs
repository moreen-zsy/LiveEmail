use tauri::{AppHandle, State};

use crate::db::{accounts, Db};
use crate::error::AppResult;
use crate::models::{Account, AccountInput, ProviderPreset, TestConnectionInput, TestConnectionResult};
use crate::services;

#[tauri::command]
pub fn list_accounts(state: State<'_, Db>) -> AppResult<Vec<Account>> {
    let conn = state.lock()?;
    crate::db::accounts::list(&conn)
}

#[tauri::command]
pub fn add_account(state: State<'_, Db>, input: AccountInput) -> AppResult<Account> {
    let conn = state.lock()?;
    let account = crate::db::accounts::insert(&conn, &input)?;
    drop(conn);
    if let Some(secret) = &input.secret {
        if !secret.is_empty() {
            services::credential_store::save_secret(account.id, secret)?;
        }
    }
    Ok(account)
}

#[tauri::command]
pub fn update_account(
    state: State<'_, Db>,
    id: i64,
    input: AccountInput,
) -> AppResult<Option<Account>> {
    let conn = state.lock()?;
    let updated = crate::db::accounts::update(&conn, id, &input)?;
    drop(conn);
    if updated.is_some() {
        if let Some(secret) = &input.secret {
            if !secret.is_empty() {
                services::credential_store::save_secret(id, secret)?;
            }
        }
    }
    Ok(updated)
}

#[tauri::command]
pub fn delete_account(state: State<'_, Db>, id: i64) -> AppResult<()> {
    {
        let conn = state.lock()?;
        crate::db::accounts::delete(&conn, id)?;
    }
    let _ = services::credential_store::delete_secret(id);
    let _ = services::credential_store::delete_token(id);
    let _ = services::credential_store::delete_refresh_token(id);
    let dir = state.attachments_dir().join(id.to_string());
    let _ = std::fs::remove_dir_all(dir);
    Ok(())
}

/// 启动 OAuth2 授权：打开系统浏览器并等待 loopback 回调
#[tauri::command]
pub async fn oauth_authorize(
    app: AppHandle,
    state: State<'_, Db>,
    account_id: i64,
) -> Result<services::oauth::OAuthResult, String> {
    let account = {
        let conn = state.lock().map_err(|e| e.to_string())?;
        accounts::get(&conn, account_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "账户不存在".to_string())?
    };
    if account.auth_type != "oauth2" {
        return Err("该账户未启用 OAuth2 认证".to_string());
    }
    services::oauth::authorize(&app, state.inner(), &account).await
}

#[tauri::command]
pub fn set_default_account(state: State<'_, Db>, id: i64) -> AppResult<()> {
    let conn = state.lock()?;
    crate::db::accounts::set_default(&conn, id)?;
    Ok(())
}

#[tauri::command]
pub fn provider_presets() -> Vec<ProviderPreset> {
    services::providers::all()
}

#[tauri::command]
pub async fn test_connection(
    state: State<'_, Db>,
    input: TestConnectionInput,
) -> Result<TestConnectionResult, String> {
    let oauth2 = input.auth_type == "oauth2";

    let secret = match input.secret.clone().filter(|s| !s.is_empty()) {
        Some(s) => s,
        None => {
            let account_id = input
                .account_id
                .ok_or_else(|| "缺少凭据：既未提供 secret，也未提供 account_id".to_string())?;
            if oauth2 {
                // 已保存的 OAuth2 账户：读取/刷新令牌（凭据不经过前端）
                let account = {
                    let conn = state.lock().map_err(|e| e.to_string())?;
                    accounts::get(&conn, account_id)
                        .map_err(|e| e.to_string())?
                        .ok_or_else(|| "账户不存在".to_string())?
                };
                services::oauth::account_secret(state.inner(), &account).await?
            } else {
                services::credential_store::get_secret(account_id)
                    .map_err(|e| e.to_string())?
                    .ok_or_else(|| "未找到已保存的凭据，请先重新输入密码/授权码".to_string())?
            }
        }
    };

    let imap = services::imap::login(
        &services::imap::ImapConfig {
            host: input.imap_host.clone(),
            port: input.imap_port,
            encryption: input.imap_encryption.clone(),
            email: input.email.clone(),
        },
        &secret,
        oauth2,
    )
    .await
    .map(|session| {
        tauri::async_runtime::spawn(async move {
            services::imap::logout(session).await;
        });
    });

    let smtp = services::mail::smtp_test(
        &input.smtp_host,
        input.smtp_port,
        &input.smtp_encryption,
        &input.email,
        &secret,
        oauth2,
    )
    .await;

    let (imap_ok, imap_error) = match imap {
        Ok(()) => (true, None),
        Err(e) => (false, Some(e)),
    };
    let (smtp_ok, smtp_error) = match smtp {
        Ok(()) => (true, None),
        Err(e) => (false, Some(e)),
    };

    Ok(TestConnectionResult {
        ok: imap_ok && smtp_ok,
        imap_ok,
        smtp_ok,
        imap_error,
        smtp_error,
    })
}
