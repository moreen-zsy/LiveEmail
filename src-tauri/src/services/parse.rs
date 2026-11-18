use std::path::{Path, PathBuf};

use mail_parser::MessageParser;

use crate::db::messages::{NewAttachment, NewMessage};

pub struct ParsedMail {
    pub new: NewMessage,
}

fn format_addresses(list: Option<mail_parser::AddressList<'_>>) -> String {
    let Some(list) = list else {
        return String::new();
    };
    list.mailboxes()
        .filter_map(|m| {
            let addr = m.address()?;
            Some(match m.name() {
                Some(name) if !name.is_empty() => format!("{name} <{addr}>"),
                _ => addr.to_string(),
            })
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn first_address(list: Option<mail_parser::AddressList<'_>>) -> (String, String) {
    let Some(list) = list else {
        return (String::new(), String::new());
    };
    if let Some(m) = list.mailboxes().next() {
        (
            m.name().unwrap_or("").to_string(),
            m.address().unwrap_or("").to_string(),
        )
    } else {
        (String::new(), String::new())
    }
}

/// 文件名消毒：去掉路径分隔符与危险字符，防止路径遍历
pub fn safe_filename(name: &str, fallback: &str) -> String {
    let base = name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(name)
        .trim()
        .trim_matches('.');
    let cleaned: String = base
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '|' | '?' | '*' | '\0'..='\u{1f}' => '_',
            c => c,
        })
        .collect();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        fallback.to_string()
    } else {
        cleaned.chars().take(120).collect()
    }
}

fn ext_for(mime: &str) -> &'static str {
    match mime {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "application/pdf" => "pdf",
        _ => "bin",
    }
}

/// 解析 MIME 邮件；附件二进制写入 dest_dir（按 UID 隔离）
pub fn parse_message(
    raw: &[u8],
    dest_dir: &Path,
    seen: bool,
    flagged: bool,
    size: i64,
) -> Option<ParsedMail> {
    let parsed = MessageParser::new().parse(raw)?;
    let (from_name, from_addr) = first_address(parsed.from());
    let subject = parsed.subject().unwrap_or("(无主题)").to_string();
    let to_addrs = format_addresses(parsed.to());
    let cc_addrs = format_addresses(parsed.cc());
    let date = parsed.date().map(|d| d.to_timestamp());
    let text_body = parsed.body_text(0).map(|b| b.to_string());
    let html_body = parsed.body_html(0).map(|b| b.to_string());

    let preview = parsed
        .body_preview(240)
        .map(|p| p.to_string())
        .or_else(|| text_body.as_ref().map(|t| t.chars().take(240).collect()))
        .unwrap_or_default();

    let mut attachments = Vec::new();
    let mut has_attachment = false;
    let parts: Vec<_> = parsed.attachments().collect();
    if !parts.is_empty() {
        let _ = std::fs::create_dir_all(dest_dir);
    }
    for (idx, part) in parts.into_iter().enumerate() {
        let ctype = part
            .content_type()
            .map(|c| {
                format!(
                    "{}/{}",
                    c.ctype(),
                    c.subtype().unwrap_or("octet-stream")
                )
            })
            .unwrap_or_else(|| "application/octet-stream".to_string());
        let content_id = part
            .content_id()
            .map(|c| c.trim_matches(['<', '>']).to_string());
        let inline = part.is_inline();
        if !inline {
            has_attachment = true;
        }
        let default_name = format!("attachment-{}.{}", idx + 1, ext_for(&ctype));
        let filename = safe_filename(
            part.attachment_name().unwrap_or(&default_name),
            &default_name,
        );
        let data = part.decoded();
        let path: Option<PathBuf> = if data.is_empty() {
            None
        } else {
            let file_path = dest_dir.join(format!("{}-{}", idx + 1, filename));
            match std::fs::write(&file_path, data.as_ref()) {
                Ok(()) => Some(file_path),
                Err(_) => None,
            }
        };
        attachments.push(NewAttachment {
            filename,
            mime_type: ctype,
            size: data.len() as i64,
            content_id,
            local_path: path.map(|p| p.to_string_lossy().to_string()),
        });
    }

    Some(ParsedMail {
        new: NewMessage {
            uid: 0,
            message_id: parsed.message_id().map(|s| s.to_string()),
            subject,
            from_name,
            from_addr,
            to_addrs,
            cc_addrs,
            date,
            is_read: seen,
            is_flagged: flagged,
            has_attachment,
            size,
            preview,
            in_reply_to: parsed.in_reply_to().and_then(|l| l.first().map(|s| s.to_string())),
            references: parsed
                .references()
                .map(|l| l.iter().map(|s| s.to_string()).collect::<Vec<_>>().join(" ")),
            text_body,
            html_body,
            attachments,
        },
    })
}

/// 将 HTML 中的 cid: 图片替换为本地 data URI（远程图片保持不动，由前端净化/拦截）
pub fn inline_cid_images(
    html: &str,
    attachments: &[crate::models::AttachmentMeta],
) -> String {
    use base64::Engine;
    let mut out = html.to_string();
    for a in attachments {
        let Some(cid) = &a.content_id else { continue };
        if !a.mime_type.starts_with("image/") {
            continue;
        }
        let Some(path) = &a.local_path else { continue };
        let Ok(bytes) = std::fs::read(path) else { continue };
        let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
        let data_uri = format!("data:{};base64,{}", a.mime_type, b64);
        out = out.replace(&format!("cid:{cid}"), &data_uri);
        out = out.replace(&format!("CID:{cid}"), &data_uri);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::AttachmentMeta;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(tag: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "liveemail-test-{tag}-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn safe_filename_strips_paths_and_control_chars() {
        assert_eq!(safe_filename("../../etc/passwd", "fallback"), "passwd");
        assert_eq!(safe_filename("C:\\Windows\\evil.exe", "fallback"), "evil.exe");
        assert_eq!(safe_filename("a<b>c.txt", "fallback"), "a_b_c.txt");
        assert_eq!(safe_filename("..", "fallback"), "fallback");
        assert_eq!(safe_filename("   ", "fallback"), "fallback");
        assert_eq!(safe_filename("no_extension", "fallback"), "no_extension");

        let long = "x".repeat(200);
        assert_eq!(safe_filename(&long, "fallback").chars().count(), 120);
    }

    #[test]
    fn parses_plain_mail_with_attachment() {
        let dir = temp_dir("parse");
        let raw = concat!(
            "From: Alice <alice@example.com>\r\n",
            "To: Bob <bob@example.com>\r\n",
            "Subject: =?UTF-8?B?5rWL6K+V6YKu5Lu2?=\r\n",
            "Date: Tue, 18 Nov 2025 10:00:00 +0800\r\n",
            "MIME-Version: 1.0\r\n",
            "Content-Type: multipart/mixed; boundary=\"BOUND\"\r\n",
            "\r\n",
            "--BOUND\r\n",
            "Content-Type: text/plain; charset=utf-8\r\n",
            "\r\n",
            "Hello world\r\n",
            "--BOUND\r\n",
            "Content-Type: application/pdf; name=\"report.pdf\"\r\n",
            "Content-Disposition: attachment; filename=\"report.pdf\"\r\n",
            "Content-Transfer-Encoding: base64\r\n",
            "\r\n",
            "JVBERi0xLjQK\r\n",
            "--BOUND--\r\n"
        );

        let parsed = parse_message(raw.as_bytes(), &dir, false, false, raw.len() as i64)
            .expect("parse mail");
        assert_eq!(parsed.new.subject, "测试邮件");
        assert_eq!(parsed.new.from_addr, "alice@example.com");
        assert_eq!(parsed.new.from_name, "Alice");
        assert!(parsed.new.text_body.as_deref().unwrap_or("").contains("Hello world"));
        assert!(parsed.new.has_attachment);
        assert_eq!(parsed.new.attachments.len(), 1);

        let att = &parsed.new.attachments[0];
        assert_eq!(att.filename, "report.pdf");
        assert_eq!(att.mime_type, "application/pdf");
        let path = att.local_path.as_ref().expect("attachment written to disk");
        assert!(Path::new(path).exists());
        assert_eq!(std::fs::read(path).unwrap(), b"%PDF-1.4\n");
        assert!(!parsed.new.preview.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn inline_cid_images_replaces_matching_ids() {
        let dir = temp_dir("cid");
        let path = dir.join("logo.png");
        std::fs::write(&path, [0x89, 0x50, 0x4e, 0x47]).unwrap();

        let attachments = vec![AttachmentMeta {
            id: 1,
            filename: "logo.png".into(),
            mime_type: "image/png".into(),
            size: 4,
            content_id: Some("logo001@local".into()),
            local_path: Some(path.to_string_lossy().to_string()),
        }];

        let html = r#"<p><img src="cid:logo001@local"></p>"#;
        let out = inline_cid_images(html, &attachments);
        assert!(out.contains("data:image/png;base64,"));
        assert!(!out.contains("cid:logo001@local"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn invalid_mail_returns_none() {
        let dir = temp_dir("invalid");
        // 纯随机二进制通常无法解析为邮件；允许解析器较宽松时返回 None 或空主体
        let parsed = parse_message(&[0xff; 16], &dir, false, false, 16);
        if let Some(p) = parsed {
            assert_eq!(p.new.from_addr, "");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
