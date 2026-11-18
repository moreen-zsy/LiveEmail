use std::sync::Arc;
use std::time::Duration;

use async_imap::extensions::idle::Handle;
use async_imap::types::{Fetch, Flag, Mailbox, Name, NameAttribute};
use async_imap::{Authenticator, Client, Session};
use futures_util::TryStreamExt;
use rustls::RootCertStore;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

use crate::db::folders::RemoteFolder;

pub trait AsyncStream: AsyncRead + AsyncWrite + Unpin + Send + std::fmt::Debug {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send + std::fmt::Debug> AsyncStream for T {}
pub type BoxedStream = Box<dyn AsyncStream>;
pub type ImapSession = Session<BoxedStream>;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const OP_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone)]
pub struct ImapConfig {
    pub host: String,
    pub port: u16,
    pub encryption: String,
    pub email: String,
}

struct XOAuth2 {
    response: Vec<u8>,
}

impl Authenticator for XOAuth2 {
    type Response = Vec<u8>;

    fn process(&mut self, _challenge: &[u8]) -> Self::Response {
        std::mem::take(&mut self.response)
    }
}

fn tls_connector() -> TlsConnector {
    let mut roots = RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    TlsConnector::from(Arc::new(config))
}

async fn tls_stream<S>(host: &str, stream: S) -> Result<tokio_rustls::client::TlsStream<S>, String>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let server_name = rustls::pki_types::ServerName::try_from(host.to_string())
        .map_err(|e| format!("无效的主机名 {host}: {e}"))?;
    tls_connector()
        .connect(server_name, stream)
        .await
        .map_err(|e| format!("TLS 握手失败（证书或网络问题）: {e}"))
}

async fn read_greeting<S>(client: &mut Client<S>) -> Result<(), String>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + std::fmt::Debug,
{
    tokio::time::timeout(OP_TIMEOUT, client.read_response())
        .await
        .map_err(|_| "读取 IMAP 服务器问候超时".to_string())?
        .map_err(|e| format!("读取 IMAP 服务器问候失败: {e}"))?;
    Ok(())
}

/// 建立连接并登录（password / 授权码 / OAuth2 access token）
pub async fn login(config: &ImapConfig, secret: &str, oauth2: bool) -> Result<ImapSession, String> {
    let tcp = tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect((config.host.as_str(), config.port)))
        .await
        .map_err(|_| format!("连接 IMAP 服务器 {}:{} 超时", config.host, config.port))?
        .map_err(|e| format!("无法连接 IMAP 服务器 {}:{}（{e}）", config.host, config.port))?;
    let _ = tcp.set_nodelay(true);

    let client: Client<BoxedStream> = match config.encryption.as_str() {
        "ssl" => {
            let tls = tls_stream(&config.host, tcp).await?;
            let mut c = Client::new(Box::new(tls) as BoxedStream);
            read_greeting(&mut c).await?;
            c
        }
        "starttls" => {
            let boxed: BoxedStream = Box::new(tcp);
            let mut c = Client::new(boxed);
            read_greeting(&mut c).await?;
            c.run_command_and_check_ok("STARTTLS", None)
                .await
                .map_err(|e| format!("STARTTLS 升级失败: {e}"))?;
            let upgraded = c.into_inner();
            let tls = tls_stream(&config.host, upgraded).await?;
            Client::new(Box::new(tls) as BoxedStream)
        }
        _ => {
            let boxed: BoxedStream = Box::new(tcp);
            let mut c = Client::new(boxed);
            read_greeting(&mut c).await?;
            c
        }
    };

    let result = if oauth2 {
        let response = format!(
            "user={}\x01auth=Bearer {}\x01\x01",
            config.email, secret
        )
        .into_bytes();
        tokio::time::timeout(
            OP_TIMEOUT,
            client.authenticate("XOAUTH2", XOAuth2 { response }),
        )
        .await
        .map_err(|_| "IMAP XOAUTH2 认证超时".to_string())?
    } else {
        tokio::time::timeout(OP_TIMEOUT, client.login(&config.email, secret))
            .await
            .map_err(|_| "IMAP 登录超时".to_string())?
    };

    match result {
        Ok(session) => Ok(session),
        Err((e, _)) => Err(classify_auth_error(&e.to_string())),
    }
}

fn classify_auth_error(err: &str) -> String {
    let lower = err.to_lowercase();
    if lower.contains("authentication") || lower.contains("auth") || lower.contains("login") {
        format!("IMAP 认证失败，请检查账号与密码/授权码，并确认已开启 IMAP：{err}")
    } else if lower.contains("certificate") || lower.contains("tls") {
        format!("IMAP 证书校验失败：{err}")
    } else {
        format!("IMAP 登录失败：{err}")
    }
}

fn kind_of(name: &str, attrs: &[NameAttribute<'_>]) -> String {
    for attr in attrs {
        match attr {
            NameAttribute::Sent => return "sent".into(),
            NameAttribute::Drafts => return "drafts".into(),
            NameAttribute::Trash => return "trash".into(),
            NameAttribute::Junk => return "junk".into(),
            NameAttribute::Archive => return "archive".into(),
            _ => {}
        }
    }
    let lower = name.to_lowercase();
    if name.eq_ignore_ascii_case("INBOX") {
        "inbox".into()
    } else if lower.contains("sent") || name.contains("已发送") {
        "sent".into()
    } else if lower.contains("draft") || name.contains("草稿") {
        "drafts".into()
    } else if lower.contains("trash") || lower.contains("deleted") || name.contains("已删除") {
        "trash".into()
    } else if lower.contains("junk") || lower.contains("spam") || name.contains("垃圾") {
        "junk".into()
    } else if lower.contains("archive") || name.contains("归档") {
        "archive".into()
    } else {
        "other".into()
    }
}

pub async fn list_folders(session: &mut ImapSession) -> Result<Vec<RemoteFolder>, String> {
    let names: Vec<Name> = tokio::time::timeout(OP_TIMEOUT, session.list(None, Some("*")))
        .await
        .map_err(|_| "获取文件夹列表超时".to_string())?
        .map_err(|e| format!("获取文件夹列表失败: {e}"))?
        .try_collect()
        .await
        .map_err(|e| format!("解析文件夹列表失败: {e}"))?;

    let mut out = Vec::new();
    for name in names {
        let path = name.name().to_string();
        if path.is_empty() {
            continue;
        }
        if name.attributes().iter().any(|a| matches!(a, NameAttribute::NoSelect)) {
            continue;
        }
        let attrs: Vec<NameAttribute<'_>> = name.attributes().to_vec();
        let kind = kind_of(&path, &attrs);
        let display = path
            .rsplit(['/', '.'])
            .next()
            .unwrap_or(path.as_str())
            .to_string();
        out.push(RemoteFolder {
            name: display,
            path,
            delimiter: name.delimiter().unwrap_or("").to_string(),
            kind,
            uid_validity: None,
            uid_next: None,
        });
    }
    Ok(out)
}

pub async fn select(session: &mut ImapSession, path: &str) -> Result<Mailbox, String> {
    tokio::time::timeout(OP_TIMEOUT, session.select(path))
        .await
        .map_err(|_| format!("选择文件夹 {path} 超时"))?
        .map_err(|e| format!("选择文件夹 {path} 失败: {e}"))
}

pub struct FetchedMessage {
    pub uid: i64,
    pub flags: Vec<String>,
    pub size: i64,
    pub body: Vec<u8>,
}

/// 拉取 UID >= from_uid 的邮件（分批，避免单次 FETCH 过大）
pub async fn fetch_messages(
    session: &mut ImapSession,
    from_uid: i64,
) -> Result<Vec<FetchedMessage>, String> {
    let search = if from_uid <= 1 {
        "1:*".to_string()
    } else {
        format!("UID {from_uid}:*")
    };
    let mut uids: Vec<u32> = tokio::time::timeout(OP_TIMEOUT, session.uid_search(&search))
        .await
        .map_err(|_| "搜索新邮件超时".to_string())?
        .map_err(|e| format!("搜索新邮件失败: {e}"))?
        .into_iter()
        .filter(|uid| (*uid as i64) >= from_uid.max(1))
        .collect();
    uids.sort_unstable();

    let mut out = Vec::new();
    for chunk in uids.chunks(30) {
        let set = chunk
            .iter()
            .map(|u| u.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let fetches: Vec<Fetch> = tokio::time::timeout(
            OP_TIMEOUT,
            session.uid_fetch(&set, "(UID FLAGS RFC822.SIZE BODY.PEEK[])"),
        )
        .await
        .map_err(|_| "拉取邮件超时".to_string())?
        .map_err(|e| format!("拉取邮件失败: {e}"))?
        .try_collect()
        .await
        .map_err(|e| format!("解析邮件响应失败: {e}"))?;

        for fetch in fetches {
            let Some(uid) = fetch.uid else { continue };
            let mut flags = Vec::new();
            for flag in fetch.flags() {
                match flag {
                    Flag::Seen => flags.push("seen".to_string()),
                    Flag::Flagged => flags.push("flagged".to_string()),
                    Flag::Draft => flags.push("draft".to_string()),
                    _ => {}
                }
            }
            out.push(FetchedMessage {
                uid: uid as i64,
                flags,
                size: fetch.size.unwrap_or(0) as i64,
                body: fetch.body().map(|b| b.to_vec()).unwrap_or_default(),
            });
        }
    }
    Ok(out)
}

fn uid_set(uids: &[i64]) -> String {
    uids.iter()
        .map(|u| u.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

pub async fn set_seen(session: &mut ImapSession, uids: &[i64], seen: bool) -> Result<(), String> {
    let op = if seen { "+FLAGS (\\Seen)" } else { "-FLAGS (\\Seen)" };
    let stream = session
        .uid_store(uid_set(uids), op)
        .await
        .map_err(|e| format!("同步已读状态失败: {e}"))?;
    let _: Vec<Fetch> = stream.try_collect().await.map_err(|e| format!("同步已读状态失败: {e}"))?;
    Ok(())
}

pub async fn set_flagged(session: &mut ImapSession, uids: &[i64], flagged: bool) -> Result<(), String> {
    let op = if flagged { "+FLAGS (\\Flagged)" } else { "-FLAGS (\\Flagged)" };
    let stream = session
        .uid_store(uid_set(uids), op)
        .await
        .map_err(|e| format!("同步星标失败: {e}"))?;
    let _: Vec<Fetch> = stream.try_collect().await.map_err(|e| format!("同步星标失败: {e}"))?;
    Ok(())
}

pub async fn copy_to(session: &mut ImapSession, uids: &[i64], target: &str) -> Result<(), String> {
    tokio::time::timeout(OP_TIMEOUT, session.uid_copy(uid_set(uids), target))
        .await
        .map_err(|_| "复制邮件超时".to_string())?
        .map_err(|e| format!("复制邮件到 {target} 失败: {e}"))
}

pub async fn delete_messages(session: &mut ImapSession, uids: &[i64]) -> Result<(), String> {
    let stream = session
        .uid_store(uid_set(uids), "+FLAGS (\\Deleted)")
        .await
        .map_err(|e| format!("标记删除失败: {e}"))?;
    let _: Vec<Fetch> = stream.try_collect().await.map_err(|e| format!("标记删除失败: {e}"))?;
    let expunge = session
        .uid_expunge(uid_set(uids))
        .await
        .map_err(|e| format!("彻底删除失败: {e}"))?;
    let _: Vec<_> = expunge
        .try_collect()
        .await
        .map_err(|e| format!("彻底删除失败: {e}"))?;
    Ok(())
}

pub async fn create_folder(session: &mut ImapSession, path: &str) -> Result<(), String> {
    session
        .create(path)
        .await
        .map_err(|e| format!("创建文件夹失败: {e}"))
}

pub async fn delete_folder(session: &mut ImapSession, path: &str) -> Result<(), String> {
    session
        .delete(path)
        .await
        .map_err(|e| format!("删除文件夹失败: {e}"))
}

pub async fn append_to(session: &mut ImapSession, mailbox: &str, raw: &[u8]) -> Result<(), String> {
    tokio::time::timeout(OP_TIMEOUT, session.append(mailbox, Some("(\\Seen)"), None, raw))
        .await
        .map_err(|_| "归档到服务器超时".to_string())?
        .map_err(|e| format!("归档到 {mailbox} 失败: {e}"))
}

pub async fn start_idle(mut session: ImapSession, path: &str) -> Result<Handle<BoxedStream>, String> {
    select(&mut session, path).await?;
    let mut handle = session.idle();
    handle
        .init()
        .await
        .map_err(|e| format!("IDLE 初始化失败: {e}"))?;
    Ok(handle)
}

pub async fn logout(mut session: ImapSession) {
    let _ = tokio::time::timeout(Duration::from_secs(5), session.logout()).await;
}
