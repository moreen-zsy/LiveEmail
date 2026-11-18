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
    pub is_default: bool,
    pub last_sync_at: Option<i64>,
    pub last_error: Option<String>,
    pub oauth_client_id: Option<String>,
    pub oauth_tenant: Option<String>,
    /// 桌面客户端的 client secret（Google 要求；非用户机密，按公开客户端对待）
    pub oauth_client_secret: Option<String>,
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
    pub oauth_client_id: Option<String>,
    pub oauth_tenant: Option<String>,
    pub oauth_client_secret: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Folder {
    pub id: i64,
    pub account_id: i64,
    pub name: String,
    pub path: String,
    pub delimiter: String,
    pub kind: String,
    pub unread_count: i64,
    pub total_count: i64,
    pub uid_validity: Option<i64>,
    pub uid_next: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MessageSummary {
    pub id: i64,
    pub account_id: i64,
    pub folder_id: i64,
    pub folder_name: String,
    pub account_email: String,
    pub uid: i64,
    pub subject: String,
    pub from_name: String,
    pub from_addr: String,
    pub to_addrs: String,
    pub preview: String,
    pub date: Option<i64>,
    pub is_read: bool,
    pub is_flagged: bool,
    pub has_attachment: bool,
    pub size: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AttachmentMeta {
    pub id: i64,
    pub filename: String,
    pub mime_type: String,
    pub size: i64,
    pub content_id: Option<String>,
    pub local_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MessageDetail {
    #[serde(flatten)]
    pub summary: MessageSummary,
    pub message_id: Option<String>,
    pub cc_addrs: String,
    pub bcc_addrs: String,
    pub in_reply_to: Option<String>,
    pub references: Option<String>,
    pub text_body: Option<String>,
    pub html_body: Option<String>,
    pub attachments: Vec<AttachmentMeta>,
}

/// 列表/搜索查询参数
#[derive(Debug, Clone, Default, Deserialize)]
pub struct MessageQuery {
    /// 账户 id；None 表示统一收件箱（全部账户）
    pub account_id: Option<i64>,
    /// 文件夹 id；None 且 unified=true 时查询各账户收件箱
    pub folder_id: Option<i64>,
    pub unified: Option<bool>,
    pub search: Option<String>,
    pub unread_only: Option<bool>,
    pub flagged_only: Option<bool>,
    pub has_attachment: Option<bool>,
    pub date_from: Option<i64>,
    pub date_to: Option<i64>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MessagePage {
    pub items: Vec<MessageSummary>,
    pub total: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FlagInput {
    pub message_ids: Vec<i64>,
    pub read: Option<bool>,
    pub flagged: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MoveInput {
    pub message_ids: Vec<i64>,
    /// 目标文件夹 id；delete/archive/junk 由后端自动选择
    pub target_folder_id: Option<i64>,
    /// delete | archive | junk | move
    pub action: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActionResult {
    pub updated: i64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Draft {
    pub id: i64,
    pub account_id: Option<i64>,
    pub subject: String,
    pub to_addrs: String,
    pub cc_addrs: String,
    pub bcc_addrs: String,
    pub body_html: String,
    pub body_text: String,
    pub updated_at: i64,
    pub attachments: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DraftInput {
    pub id: Option<i64>,
    pub account_id: Option<i64>,
    pub subject: String,
    pub to_addrs: String,
    pub cc_addrs: String,
    pub bcc_addrs: String,
    pub body_html: String,
    pub body_text: String,
    #[serde(default)]
    pub attachments: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OutboxItem {
    pub id: i64,
    pub account_id: i64,
    pub subject: String,
    pub to_addrs: String,
    pub status: String,
    pub error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ComposeInput {
    pub account_id: i64,
    pub to_addrs: String,
    pub cc_addrs: String,
    pub bcc_addrs: String,
    pub subject: String,
    pub body_html: String,
    pub body_text: String,
    pub attachments: Vec<String>,
    pub in_reply_to: Option<String>,
    pub references: Option<String>,
    /// 发送成功后删除的草稿 id
    pub draft_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SendResult {
    pub ok: bool,
    pub message: String,
    pub archived: bool,
    /// 发送失败时是否已进入发送队列
    pub queued: bool,
    /// 进入队列时的 outbox id
    pub outbox_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StorageStats {
    pub db_bytes: u64,
    pub attachments_bytes: u64,
    pub attachment_count: i64,
    pub message_count: i64,
    pub account_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub theme: String,
    pub font_size: String,
    pub sync_interval_secs: i64,
    pub signature: String,
    pub signature_per_account: bool,
    pub close_to_tray: bool,
    pub notifications_enabled: bool,
    pub remote_images_trusted: bool,
}
