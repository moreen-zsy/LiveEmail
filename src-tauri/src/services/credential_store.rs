use keyring::v1::Entry;

use crate::error::AppResult;

const SERVICE: &str = "com.liveemail.app";

fn entry(account_id: i64) -> AppResult<Entry> {
    let key = format!("account:{account_id}");
    Ok(Entry::new(SERVICE, &key)?)
}

fn token_entry(account_id: i64) -> AppResult<Entry> {
    let key = format!("account:{account_id}:oauth");
    Ok(Entry::new(SERVICE, &key)?)
}

fn refresh_entry(account_id: i64) -> AppResult<Entry> {
    let key = format!("account:{account_id}:oauth_refresh");
    Ok(Entry::new(SERVICE, &key)?)
}

/// 保存密码 / 授权码 / OAuth 令牌（写入系统凭据管理器）
pub fn save_secret(account_id: i64, secret: &str) -> AppResult<()> {
    entry(account_id)?.set_password(secret)?;
    Ok(())
}

/// 读取凭据
pub fn get_secret(account_id: i64) -> AppResult<Option<String>> {
    match entry(account_id)?.get_password() {
        Ok(secret) => Ok(Some(secret)),
        Err(keyring::v1::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// 删除凭据
pub fn delete_secret(account_id: i64) -> AppResult<()> {
    match entry(account_id)?.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::v1::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// 保存 OAuth2 access token（与密码/授权码分开存放）
pub fn save_token(account_id: i64, token: &str) -> AppResult<()> {
    token_entry(account_id)?.set_password(token)?;
    Ok(())
}

/// 读取 OAuth2 access token
pub fn get_token(account_id: i64) -> AppResult<Option<String>> {
    match token_entry(account_id)?.get_password() {
        Ok(token) => Ok(Some(token)),
        Err(keyring::v1::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// 删除 OAuth2 access token
pub fn delete_token(account_id: i64) -> AppResult<()> {
    match token_entry(account_id)?.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::v1::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// 保存 OAuth2 refresh token（用于静默续期）
pub fn save_refresh_token(account_id: i64, token: &str) -> AppResult<()> {
    refresh_entry(account_id)?.set_password(token)?;
    Ok(())
}

/// 读取 OAuth2 refresh token
pub fn get_refresh_token(account_id: i64) -> AppResult<Option<String>> {
    match refresh_entry(account_id)?.get_password() {
        Ok(token) => Ok(Some(token)),
        Err(keyring::v1::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// 删除 OAuth2 refresh token
pub fn delete_refresh_token(account_id: i64) -> AppResult<()> {
    match refresh_entry(account_id)?.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::v1::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.into()),
    }
}
