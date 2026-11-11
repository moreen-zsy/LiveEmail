use keyring::v1::Entry;

use crate::error::AppResult;

const SERVICE: &str = "com.liveemail.app";

fn entry(account_id: i64) -> AppResult<Entry> {
    let key = format!("account:{account_id}");
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
    match entry(account_id)?.delete_password() {
        Ok(()) => Ok(()),
        Err(keyring::v1::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.into()),
    }
}
