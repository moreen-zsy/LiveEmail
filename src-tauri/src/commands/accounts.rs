use tauri::State;

use crate::db::Db;
use crate::error::AppResult;
use crate::models::{Account, AccountInput, ProviderPreset, TestConnectionInput, TestConnectionResult};
use crate::services;

#[tauri::command]
pub fn list_accounts(state: State<'_, Db>) -> AppResult<Vec<Account>> {
    let conn = state.0.lock().map_err(|e| crate::error::AppError::Message(e.to_string()))?;
    crate::db::accounts::list(&conn)
}

#[tauri::command]
pub fn add_account(state: State<'_, Db>, input: AccountInput) -> AppResult<Account> {
    let conn = state.0.lock().map_err(|e| crate::error::AppError::Message(e.to_string()))?;
    let account = crate::db::accounts::insert(&conn, &input)?;
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
    let conn = state.0.lock().map_err(|e| crate::error::AppError::Message(e.to_string()))?;
    let updated = crate::db::accounts::update(&conn, id, &input)?;
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
    let conn = state.0.lock().map_err(|e| crate::error::AppError::Message(e.to_string()))?;
    crate::db::accounts::delete(&conn, id)?;
    let _ = services::credential_store::delete_secret(id);
    Ok(())
}

#[tauri::command]
pub fn provider_presets() -> Vec<ProviderPreset> {
    services::providers::all()
}

#[tauri::command]
pub async fn test_connection(input: TestConnectionInput) -> Result<TestConnectionResult, String> {
    // 优先使用调用方提供的 secret；否则从系统凭据管理器读取（凭据不经过前端）
    let secret = match input.secret.filter(|s| !s.is_empty()) {
        Some(s) => s,
        None => {
            let account_id = input
                .account_id
                .ok_or_else(|| "缺少凭据：既未提供 secret，也未提供 account_id".to_string())?;
            services::credential_store::get_secret(account_id)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| "未找到已保存的凭据，请先重新输入密码/授权码".to_string())?
        }
    };

    let imap = services::mail::imap_test(
        &input.imap_host,
        input.imap_port,
        &input.imap_encryption,
        &input.email,
        &secret,
    )
    .await;
    let smtp = services::mail::smtp_test(
        &input.smtp_host,
        input.smtp_port,
        &input.smtp_encryption,
        &input.email,
        &secret,
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
