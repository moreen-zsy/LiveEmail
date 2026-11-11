use std::sync::Arc;

use async_imap::Client;
use rustls::RootCertStore;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

/// 统一 ssl / starttls / 明文三种连接方式的流类型
type BoxedStream = Box<dyn AsyncRead + AsyncWrite + Unpin + Send>;

fn tls_connector() -> TlsConnector {
    let mut roots = RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    TlsConnector::from(Arc::new(config))
}

async fn tls_stream(host: &str, stream: TcpStream) -> Result<BoxedStream, String> {
    let server_name = rustls::pki_types::ServerName::try_from(host.to_string())
        .map_err(|e| format!("无效的主机名 {host}: {e}"))?;
    let tls = tls_connector()
        .connect(server_name, stream)
        .await
        .map_err(|e| format!("TLS 握手失败: {e}"))?;
    Ok(Box::new(tls))
}

async fn read_greeting<S>(client: &mut Client<S>) -> Result<(), String>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    let _ = client
        .read_response()
        .await
        .map_err(|e| format!("读取 IMAP 服务器问候失败: {e}"))?;
    Ok(())
}

/// IMAP 登录测试（ssl / starttls / none）
pub async fn imap_test(
    host: &str,
    port: u16,
    encryption: &str,
    email: &str,
    secret: &str,
) -> Result<(), String> {
    let tcp = TcpStream::connect((host, port))
        .await
        .map_err(|e| format!("无法连接 IMAP 服务器 {host}:{port}（{e}）"))?;

    let mut client: Client<BoxedStream> = match encryption {
        "ssl" => {
            let stream = tls_stream(host, tcp).await?;
            let mut c = Client::new(stream);
            read_greeting(&mut c).await?;
            c
        }
        "starttls" => {
            // STARTTLS 阶段仍在明文 TcpStream 上，升级后再装箱
            let mut c = Client::new(tcp);
            read_greeting(&mut c).await?;
            c.run_command_and_check_ok("STARTTLS", None)
                .await
                .map_err(|e| format!("STARTTLS 升级失败: {e}"))?;
            let upgraded = c.into_inner();
            let stream = tls_stream(host, upgraded).await?;
            Client::new(stream)
        }
        _ => {
            let mut c = Client::new(Box::new(tcp));
            read_greeting(&mut c).await?;
            c
        }
    };

    let session = client
        .login(email, secret)
        .await
        .map_err(|(e, _)| format!("IMAP 认证失败: {e}"))?;
    let _ = session.logout().await;
    Ok(())
}

/// SMTP 连接与认证测试（ssl / starttls / none）
pub async fn smtp_test(
    host: &str,
    port: u16,
    encryption: &str,
    email: &str,
    secret: &str,
) -> Result<(), String> {
    let implicit_tls = encryption == "ssl";
    let client = mail_send::SmtpClientBuilder::new(host, port)
        .map_err(|e| format!("无效的 SMTP 配置: {e}"))?
        .implicit_tls(implicit_tls)
        .credentials((email, secret))
        .connect()
        .await
        .map_err(|e| format!("SMTP 连接/认证失败: {e}"))?;
    drop(client);
    Ok(())
}
