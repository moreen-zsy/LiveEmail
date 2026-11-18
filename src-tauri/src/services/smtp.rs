use std::path::Path;
use std::time::Duration;

use mail_builder::headers::address::Address;
use mail_builder::headers::message_id::MessageId;
use mail_builder::MessageBuilder;
use mail_send::Credentials;

pub fn parse_addresses(raw: &str) -> Vec<String> {
    raw.split([',', ';'])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

pub fn valid_address(addr: &str) -> bool {
    let addr = addr.trim();
    if addr.contains(' ') {
        // 允许 "Name <a@b.com>" 形式
        if let (Some(start), Some(end)) = (addr.rfind('<'), addr.rfind('>')) {
            if start < end {
                return valid_address(&addr[start + 1..end]);
            }
        }
        return false;
    }
    match addr.rsplit_once('@') {
        Some((local, domain)) => {
            !local.is_empty() && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.')
        }
        None => false,
    }
}

fn guess_mime(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "pdf" => "application/pdf",
        "txt" => "text/plain",
        "html" | "htm" => "text/html",
        "csv" => "text/csv",
        "zip" => "application/zip",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        _ => "application/octet-stream",
    }
}

pub struct Outgoing<'a> {
    pub host: &'a str,
    pub port: u16,
    pub encryption: &'a str,
    pub email: &'a str,
    pub display_name: &'a str,
    pub secret: &'a str,
    pub oauth2: bool,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: &'a str,
    pub body_html: &'a str,
    pub body_text: &'a str,
    pub in_reply_to: Option<&'a str>,
    pub references: Option<&'a str>,
    pub attachments: Vec<String>,
}

/// 解析 "Name <a@b.com>" 或裸地址，避免把显示名整体当成邮箱地址
fn to_address(raw: &str) -> Address<'static> {
    let s = raw.trim();
    if let (Some(start), Some(end)) = (s.rfind('<'), s.rfind('>')) {
        if start < end {
            let name = s[..start].trim().trim_matches('"').trim();
            let email = s[start + 1..end].trim();
            return if name.is_empty() {
                Address::new_address(None::<String>, email.to_string())
            } else {
                Address::new_address(Some(name.to_string()), email.to_string())
            };
        }
    }
    Address::new_address(None::<String>, s.to_string())
}

fn to_addresses(list: &[String]) -> Vec<Address<'static>> {
    list.iter().map(|s| to_address(s)).collect()
}

pub fn build_message(og: &Outgoing<'_>) -> Result<(MessageBuilder<'static>, Vec<u8>), String> {
    let from = if og.display_name.trim().is_empty() {
        Address::new_address(None::<String>, og.email.to_string())
    } else {
        Address::new_address(
            Some(og.display_name.trim().to_string()),
            og.email.to_string(),
        )
    };
    let mut builder = MessageBuilder::new()
        .from(from)
        .subject(og.subject.to_string())
        .message_id(MessageId::new(format!(
            "<{}.{}@liveemail>",
            chrono_stamp(),
            rand_suffix()
        )));

    if !og.to.is_empty() {
        builder = builder.to(to_addresses(&og.to));
    }
    if !og.cc.is_empty() {
        builder = builder.cc(to_addresses(&og.cc));
    }
    if !og.bcc.is_empty() {
        builder = builder.bcc(to_addresses(&og.bcc));
    }
    if let Some(reply) = og.in_reply_to.filter(|s| !s.is_empty()) {
        builder = builder.in_reply_to(reply.to_string());
    }
    if let Some(references) = og.references.filter(|s| !s.is_empty()) {
        let ids: Vec<String> = references
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();
        builder = builder.references(MessageId::new_list(ids.into_iter()));
    }

    if !og.body_text.is_empty() {
        builder = builder.text_body(og.body_text.to_string());
    }
    if !og.body_html.is_empty() {
        builder = builder.html_body(og.body_html.to_string());
    }

    for path in &og.attachments {
        let p = Path::new(path);
        let data = std::fs::read(p).map_err(|e| format!("读取附件 {} 失败: {e}", p.display()))?;
        let filename = p
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "attachment".to_string());
        builder = builder.attachment(guess_mime(p), filename, data);
    }

    let raw = builder
        .clone()
        .write_to_vec()
        .map_err(|e| format!("构造邮件失败: {e}"))?;
    Ok((builder, raw))
}

fn chrono_stamp() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn rand_suffix() -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    chrono_stamp().hash(&mut hasher);
    std::process::id().hash(&mut hasher);
    hasher.finish()
}

/// 发送邮件并返回已构造的原始 MIME 字节（用于已发送归档）
pub async fn send(og: &Outgoing<'_>) -> Result<Vec<u8>, String> {
    let (builder, raw) = build_message(og)?;

    let credentials = if og.oauth2 {
        Credentials::XOauth2 {
            username: og.email.to_string(),
            secret: og.secret.to_string(),
        }
    } else {
        Credentials::Plain {
            username: og.email.to_string(),
            secret: og.secret.to_string(),
        }
    };

    let builder_setup = mail_send::SmtpClientBuilder::<String>::new(og.host.to_string(), og.port)
        .map_err(|e| format!("无效的 SMTP 配置: {e}"))?
        .timeout(Duration::from_secs(30))
        .credentials(credentials);

    let result = match og.encryption {
        "ssl" => {
            let client = builder_setup
                .implicit_tls(true)
                .connect()
                .await
                .map_err(|e| format!("SMTP 连接/认证失败: {e}"))?;
            send_with(client, builder).await
        }
        "starttls" => {
            let client = builder_setup
                .implicit_tls(false)
                .connect()
                .await
                .map_err(|e| format!("SMTP STARTTLS 连接/认证失败: {e}"))?;
            send_with(client, builder).await
        }
        _ => {
            let client = builder_setup
                .connect_plain()
                .await
                .map_err(|e| format!("SMTP 连接/认证失败: {e}"))?;
            send_with(client, builder).await
        }
    };

    result?;
    Ok(raw)
}

async fn send_with<T>(
    mut client: mail_send::SmtpClient<T>,
    message: MessageBuilder<'static>,
) -> Result<(), String>
where
    T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    client
        .send(message)
        .await
        .map(|_| ())
        .map_err(|e| format!("SMTP 发送失败: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mail_parser::MessageParser;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn validates_addresses() {
        assert!(valid_address("alice@example.com"));
        assert!(valid_address("Alice <alice@example.com>"));
        assert!(valid_address("  alice@example.com  "));

        assert!(!valid_address(""));
        assert!(!valid_address("no-at-sign"));
        assert!(!valid_address("alice@localhost"));
        assert!(!valid_address("alice@.com"));
        assert!(!valid_address("alice@example."));
        assert!(!valid_address("two words@example.com"));
    }

    #[test]
    fn splits_recipient_lists() {
        assert_eq!(
            parse_addresses("a@x.com, b@y.com; c@z.com ,, "),
            vec!["a@x.com", "b@y.com", "c@z.com"]
        );
        assert!(parse_addresses("  ").is_empty());
    }

    #[test]
    fn builds_mime_message_with_headers_and_attachment() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "liveemail-smtp-test-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let attachment = dir.join("note.txt");
        std::fs::write(&attachment, b"attachment body").unwrap();

        let og = Outgoing {
            host: "smtp.example.com",
            port: 465,
            encryption: "ssl",
            email: "alice@example.com",
            display_name: "Alice",
            secret: "unused",
            oauth2: false,
            to: vec!["bob@example.com".into()],
            cc: vec!["carol@example.com".into()],
            bcc: vec![],
            subject: "测试主题",
            body_html: "<p>Hello <b>world</b></p>",
            body_text: "Hello world",
            in_reply_to: Some("<parent@example.com>"),
            references: Some("<root@example.com> <parent@example.com>"),
            attachments: vec![attachment.to_string_lossy().to_string()],
        };

        let (_, raw) = build_message(&og).expect("build mime message");
        let parsed = MessageParser::new().parse(&raw).expect("re-parse mime");

        assert_eq!(parsed.subject().unwrap(), "测试主题");
        assert_eq!(parsed.from().unwrap().mailboxes().next().unwrap().address().unwrap(), "alice@example.com");
        assert_eq!(parsed.to().unwrap().mailboxes().next().unwrap().address().unwrap(), "bob@example.com");
        assert_eq!(parsed.cc().unwrap().mailboxes().next().unwrap().address().unwrap(), "carol@example.com");
        assert_eq!(
            parsed.in_reply_to().and_then(|l| l.first().map(|s| s.to_string())).unwrap(),
            "parent@example.com"
        );
        let msg_id = parsed.message_id().unwrap();
        assert!(msg_id.contains("@liveemail"));

        let attachments: Vec<_> = parsed.attachments().collect();
        assert_eq!(attachments.len(), 1);
        assert_eq!(attachments[0].attachment_name().unwrap(), "note.txt");
        assert_eq!(
            attachments[0].decoded().to_vec(),
            b"attachment body".to_vec()
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn build_message_reports_missing_attachment() {
        let og = Outgoing {
            host: "smtp.example.com",
            port: 465,
            encryption: "ssl",
            email: "alice@example.com",
            display_name: "",
            secret: "unused",
            oauth2: false,
            to: vec!["bob@example.com".into()],
            cc: vec![],
            bcc: vec![],
            subject: "s",
            body_html: "",
            body_text: "t",
            in_reply_to: None,
            references: None,
            attachments: vec!["/definitely/not/exists/file.bin".into()],
        };
        let err = build_message(&og).unwrap_err();
        assert!(err.contains("读取附件") && err.contains("失败"));
    }
}
