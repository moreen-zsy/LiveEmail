use crate::models::ProviderPreset;

/// 内置服务商预设（主机 / 端口 / 加密方式）
pub fn all() -> Vec<ProviderPreset> {
    vec![
        ProviderPreset {
            id: "gmail".into(),
            name: "Gmail".into(),
            imap_host: "imap.gmail.com".into(),
            imap_port: 993,
            imap_encryption: "ssl".into(),
            smtp_host: "smtp.gmail.com".into(),
            smtp_port: 465,
            smtp_encryption: "ssl".into(),
            auth_type: "oauth2".into(),
            note: Some("需要 OAuth2 授权登录（推荐）".into()),
        },
        ProviderPreset {
            id: "outlook".into(),
            name: "Outlook.com".into(),
            imap_host: "outlook.office365.com".into(),
            imap_port: 993,
            imap_encryption: "ssl".into(),
            smtp_host: "smtp.office365.com".into(),
            smtp_port: 587,
            smtp_encryption: "starttls".into(),
            auth_type: "oauth2".into(),
            note: Some("需要 OAuth2 授权登录".into()),
        },
        ProviderPreset {
            id: "qq".into(),
            name: "QQ 邮箱".into(),
            imap_host: "imap.qq.com".into(),
            imap_port: 993,
            imap_encryption: "ssl".into(),
            smtp_host: "smtp.qq.com".into(),
            smtp_port: 465,
            smtp_encryption: "ssl".into(),
            auth_type: "password".into(),
            note: Some("需在 QQ 邮箱开启 IMAP/SMTP 并生成授权码".into()),
        },
        ProviderPreset {
            id: "163".into(),
            name: "163 邮箱".into(),
            imap_host: "imap.163.com".into(),
            imap_port: 993,
            imap_encryption: "ssl".into(),
            smtp_host: "smtp.163.com".into(),
            smtp_port: 465,
            smtp_encryption: "ssl".into(),
            auth_type: "password".into(),
            note: Some("需开启客户端授权码，使用授权码登录".into()),
        },
        ProviderPreset {
            id: "custom".into(),
            name: "自定义（手动配置）".into(),
            imap_host: String::new(),
            imap_port: 993,
            imap_encryption: "ssl".into(),
            smtp_host: String::new(),
            smtp_port: 465,
            smtp_encryption: "ssl".into(),
            auth_type: "password".into(),
            note: None,
        },
    ]
}

/// 按 id 查找预设
pub fn find(id: &str) -> Option<ProviderPreset> {
    all().into_iter().find(|p| p.id == id)
}
