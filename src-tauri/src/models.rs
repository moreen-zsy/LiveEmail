use serde::{Deserialize, Serialize};

/// 邮件账户（不含凭据，凭据存系统凭据管理器）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: i64,
    pub email: String,
    pub display_name: String,
    pub provider: String,
    pub imap_host: String,
    pub imap_port: u16,
    pub imap_encryption: String,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_encryption: String,
    pub auth_type: String,
    pub created_at: i64,
}

/// 新增/更新账户的输入
#[derive(Debug, Clone, Deserialize)]
pub struct AccountInput {
    pub email: String,
    pub display_name: String,
    pub provider: String,
    pub imap_host: String,
    pub imap_port: u16,
    pub imap_encryption: String,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_encryption: String,
    pub auth_type: String,
    /// 密码 / 授权码（OAuth2 账户可为 None，令牌由授权流程写入）
    pub secret: Option<String>,
}

/// 服务商预设
#[derive(Debug, Clone, Serialize)]
pub struct ProviderPreset {
    pub id: String,
    pub name: String,
    pub imap_host: String,
    pub imap_port: u16,
    pub imap_encryption: String,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_encryption: String,
    pub auth_type: String,
    pub note: Option<String>,
}

/// 连接测试输入
#[derive(Debug, Clone, Deserialize)]
pub struct TestConnectionInput {
    pub account_id: Option<i64>,
    pub email: String,
    pub imap_host: String,
    pub imap_port: u16,
    pub imap_encryption: String,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_encryption: String,
    pub auth_type: String,
    /// 保存账户前的即时测试使用；已保存账户可留空，由后端从凭据管理器读取
    pub secret: Option<String>,
}

/// 连接测试结果
#[derive(Debug, Clone, Serialize)]
pub struct TestConnectionResult {
    pub ok: bool,
    pub imap_ok: bool,
    pub smtp_ok: bool,
    pub imap_error: Option<String>,
    pub smtp_error: Option<String>,
}
