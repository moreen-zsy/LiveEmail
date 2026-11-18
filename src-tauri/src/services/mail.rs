use std::time::Duration;

use mail_send::Credentials;

use crate::services::imap::{self, ImapConfig};

/// IMAP 登录测试（ssl / starttls / none，支持密码、授权码与 OAuth2）
pub async fn imap_test(
    host: &str,
    port: u16,
    encryption: &str,
    email: &str,
    secret: &str,
    oauth2: bool,
) -> Result<(), String> {
    let config = ImapConfig {
        host: host.to_string(),
        port,
        encryption: encryption.to_string(),
        email: email.to_string(),
    };
    let session = imap::login(&config, secret, oauth2).await?;
    imap::logout(session).await;
    Ok(())
}

/// SMTP 连接与认证测试（ssl / starttls / none）
pub async fn smtp_test(
    host: &str,
    port: u16,
    encryption: &str,
    email: &str,
    secret: &str,
    oauth2: bool,
) -> Result<(), String> {
    let credentials = if oauth2 {
        Credentials::XOauth2 {
            username: email.to_string(),
            secret: secret.to_string(),
        }
    } else {
        Credentials::Plain {
            username: email.to_string(),
            secret: secret.to_string(),
        }
    };

    let setup = mail_send::SmtpClientBuilder::<String>::new(host.to_string(), port)
        .map_err(|e| format!("无效的 SMTP 配置: {e}"))?
        .timeout(Duration::from_secs(30))
        .credentials(credentials);

    match encryption {
        "ssl" => {
            setup
                .implicit_tls(true)
                .connect()
                .await
                .map_err(|e| format!("SMTP 连接/认证失败: {e}"))?;
        }
        "starttls" => {
            setup
                .implicit_tls(false)
                .connect()
                .await
                .map_err(|e| format!("SMTP STARTTLS 连接/认证失败: {e}"))?;
        }
        _ => {
            setup
                .connect_plain()
                .await
                .map_err(|e| format!("SMTP 连接/认证失败: {e}"))?;
        }
    }
    Ok(())
}
