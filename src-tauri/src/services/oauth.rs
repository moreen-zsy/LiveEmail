//! OAuth2 授权流程（Gmail / Outlook.com）
//!
//! 采用 RFC 8252 原生应用推荐方式：系统浏览器 + 127.0.0.1 随机端口 loopback + PKCE(S256)。
//! 访问令牌与刷新令牌写入系统凭据管理器，过期时间写入 settings 表。

use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::db::Db;
use crate::models::Account;
use crate::services::credential_store;

const AUTH_TIMEOUT: Duration = Duration::from_secs(300);
const HTTP_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Serialize)]
pub struct OAuthResult {
    pub ok: bool,
    pub email: String,
    pub message: String,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
    error: Option<String>,
    error_description: Option<String>,
}

struct ProviderOAuth {
    auth_endpoint: String,
    token_endpoint: String,
    scope: String,
    client_id: String,
    client_secret: Option<String>,
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn expiry_key(account_id: i64) -> String {
    format!("oauth_expiry:{account_id}")
}

fn random_urlsafe(len: usize) -> String {
    let mut buf = vec![0u8; len];
    rand::rng().fill_bytes(&mut buf);
    URL_SAFE_NO_PAD.encode(buf)
}

fn pkce_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

fn parse_query(query: &str) -> HashMap<String, String> {
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .filter_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            let key = urlencoding::decode(key).ok()?.into_owned();
            let value = urlencoding::decode(value).ok()?.into_owned();
            Some((key, value))
        })
        .collect()
}

fn config_for(account: &Account) -> Result<ProviderOAuth, String> {
    let client_id = account
        .oauth_client_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "OAuth2 账户缺少 Client ID，请先在账户设置中填写".to_string())?
        .to_string();
    let client_secret = account
        .oauth_client_secret
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    match account.provider.as_str() {
        "gmail" => Ok(ProviderOAuth {
            auth_endpoint: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
            token_endpoint: "https://oauth2.googleapis.com/token".to_string(),
            scope: "https://mail.google.com/ openid email".to_string(),
            client_id,
            client_secret,
        }),
        "outlook" => {
            let tenant = account
                .oauth_tenant
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("common");
            Ok(ProviderOAuth {
                auth_endpoint: format!(
                    "https://login.microsoftonline.com/{tenant}/oauth2/v2.0/authorize"
                ),
                token_endpoint: format!(
                    "https://login.microsoftonline.com/{tenant}/oauth2/v2.0/token"
                ),
                scope: "offline_access openid email \
                        https://outlook.office.com/IMAP.AccessAsUser.All \
                        https://outlook.office.com/SMTP.Send"
                    .to_string(),
                client_id,
                client_secret,
            })
        }
        other => Err(format!(
            "服务商 {other} 暂不支持 OAuth2，请改用授权码/密码方式"
        )),
    }
}

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .build()
        .map_err(|e| format!("初始化网络客户端失败: {e}"))
}

fn token_error(resp: TokenResponse, status: reqwest::StatusCode) -> String {
    match resp.error {
        Some(err) => {
            let desc = resp.error_description.unwrap_or_default();
            if desc.is_empty() {
                format!("授权失败：{err}")
            } else {
                format!("授权失败：{err}（{desc}）")
            }
        }
        None => format!("令牌交换失败（HTTP {status}）"),
    }
}

async fn exchange_code(
    cfg: &ProviderOAuth,
    code: &str,
    verifier: &str,
    redirect_uri: &str,
) -> Result<TokenResponse, String> {
    let client = http_client()?;
    let mut form: Vec<(&str, &str)> = vec![
        ("client_id", cfg.client_id.as_str()),
        ("code", code),
        ("code_verifier", verifier),
        ("grant_type", "authorization_code"),
        ("redirect_uri", redirect_uri),
    ];
    if let Some(secret) = cfg.client_secret.as_deref() {
        form.push(("client_secret", secret));
    }

    let resp = client
        .post(&cfg.token_endpoint)
        .form(&form)
        .send()
        .await
        .map_err(|e| format!("请求令牌接口失败: {e}"))?;
    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| format!("读取令牌响应失败: {e}"))?;
    let parsed: TokenResponse = serde_json::from_str(&text)
        .map_err(|_| format!("令牌接口返回异常（HTTP {status}）: {text}"))?;
    if parsed.access_token.is_none() || parsed.error.is_some() {
        return Err(token_error(parsed, status));
    }
    Ok(parsed)
}

fn store_tokens(db: &Db, account_id: i64, tokens: &TokenResponse) -> Result<(), String> {
    let access = tokens
        .access_token
        .as_deref()
        .filter(|t| !t.is_empty())
        .ok_or_else(|| "令牌接口未返回 access_token".to_string())?;
    credential_store::save_token(account_id, access).map_err(|e| e.to_string())?;
    if let Some(refresh) = tokens.refresh_token.as_deref().filter(|t| !t.is_empty()) {
        credential_store::save_refresh_token(account_id, refresh).map_err(|e| e.to_string())?;
    }
    let expires_at = now() + tokens.expires_in.unwrap_or(3600).max(60);
    let conn = db.lock().map_err(|e| e.to_string())?;
    crate::db::settings::set(&conn, &expiry_key(account_id), &expires_at.to_string())
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// 确保返回可用的 access token；临近过期或已过期时用 refresh token 静默续期
pub async fn ensure_access_token(db: &Db, account: &Account) -> Result<String, String> {
    let expires_at = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        crate::db::settings::get(&conn, &expiry_key(account.id))
            .map_err(|e| e.to_string())?
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(0)
    };

    if expires_at > now() + 60 {
        if let Some(token) = credential_store::get_token(account.id).map_err(|e| e.to_string())? {
            if !token.is_empty() {
                return Ok(token);
            }
        }
    }

    let refresh = credential_store::get_refresh_token(account.id)
        .map_err(|e| e.to_string())?
        .filter(|t| !t.is_empty())
        .ok_or_else(|| "OAuth2 授权已过期，请重新授权".to_string())?;

    let cfg = config_for(account)?;
    let client = http_client()?;
    let mut form: Vec<(&str, &str)> = vec![
        ("client_id", cfg.client_id.as_str()),
        ("refresh_token", refresh.as_str()),
        ("grant_type", "refresh_token"),
        ("scope", cfg.scope.as_str()),
    ];
    if let Some(secret) = cfg.client_secret.as_deref() {
        form.push(("client_secret", secret));
    }

    let resp = client
        .post(&cfg.token_endpoint)
        .form(&form)
        .send()
        .await
        .map_err(|e| format!("刷新 OAuth2 令牌失败: {e}"))?;
    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| format!("读取令牌响应失败: {e}"))?;
    let parsed: TokenResponse = serde_json::from_str(&text)
        .map_err(|_| format!("令牌接口返回异常（HTTP {status}）: {text}"))?;
    if parsed.access_token.is_none() || parsed.error.is_some() {
        return Err(token_error(parsed, status));
    }
    store_tokens(db, account.id, &parsed)?;
    Ok(parsed.access_token.expect("checked above"))
}

/// 读取账户凭据：优先密码/授权码，OAuth2 账户自动刷新 access token
pub async fn account_secret(db: &Db, account: &Account) -> Result<String, String> {
    if let Some(secret) = credential_store::get_secret(account.id).map_err(|e| e.to_string())? {
        if !secret.is_empty() {
            return Ok(secret);
        }
    }
    if account.auth_type == "oauth2" {
        return ensure_access_token(db, account).await;
    }
    Err("未找到账户凭据，请重新输入密码/授权码".to_string())
}

fn extract_code(target: &str, expected_state: &str) -> Result<String, String> {
    let query = target.split_once('?').map(|(_, q)| q).unwrap_or("");
    let params = parse_query(query);

    if let Some(error) = params.get("error") {
        let desc = params.get("error_description").cloned().unwrap_or_default();
        return Err(if desc.is_empty() {
            format!("授权失败：{error}")
        } else {
            format!("授权失败：{error}（{desc}）")
        });
    }
    if params.get("state").map(String::as_str) != Some(expected_state) {
        return Err("授权回调校验失败（state 不匹配），请重试".to_string());
    }
    params
        .get("code")
        .filter(|c| !c.is_empty())
        .cloned()
        .ok_or_else(|| "授权回调缺少 code 参数".to_string())
}

fn success_page() -> String {
    r#"<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><title>LiveEmail 授权完成</title>
<style>body{font-family:system-ui,"Segoe UI",sans-serif;display:flex;align-items:center;justify-content:center;height:100vh;margin:0;background:#f7f7f8;color:#171717}
main{background:#fff;border-radius:12px;padding:32px 40px;box-shadow:0 8px 30px rgba(0,0,0,.08);text-align:center}
h1{font-size:20px;margin:0 0 8px}p{color:#666;margin:0;font-size:14px}</style></head>
<body><main><h1>授权完成</h1><p>请关闭此页面并返回 LiveEmail。</p></main></body></html>"#
        .to_string()
}

/// 执行完整授权流程：打开浏览器 → 等待 loopback 回调 → 交换并保存令牌
pub async fn authorize(app: &AppHandle, db: &Db, account: &Account) -> Result<OAuthResult, String> {
    let cfg = config_for(account)?;
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| format!("无法启动本地回调服务: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("读取本地回调端口失败: {e}"))?
        .port();
    let redirect_uri = format!("http://127.0.0.1:{port}");
    let verifier = random_urlsafe(48);
    let challenge = pkce_challenge(&verifier);
    let state = random_urlsafe(24);

    let url = format!(
        "{}?response_type=code&client_id={}&redirect_uri={}&scope={}&state={}\
         &code_challenge={}&code_challenge_method=S256&access_type=offline&prompt=consent",
        cfg.auth_endpoint,
        urlencoding::encode(&cfg.client_id),
        urlencoding::encode(&redirect_uri),
        urlencoding::encode(&cfg.scope),
        urlencoding::encode(&state),
        urlencoding::encode(&challenge),
    );

    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| format!("无法打开系统浏览器: {e}"))?;

    let (mut socket, _) = tokio::time::timeout(AUTH_TIMEOUT, listener.accept())
        .await
        .map_err(|_| "授权超时（5 分钟），请重试".to_string())?
        .map_err(|e| format!("接收授权回调失败: {e}"))?;

    let mut buf = vec![0u8; 16 * 1024];
    let n = tokio::time::timeout(Duration::from_secs(15), socket.read(&mut buf))
        .await
        .map_err(|_| "读取授权回调超时".to_string())?
        .map_err(|e| format!("读取授权回调失败: {e}"))?;
    let request = String::from_utf8_lossy(&buf[..n]);
    let target = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/");

    let result = extract_code(target, &state);

    // 无论成功失败都给浏览器一个明确结果页
    let (title, detail) = match &result {
        Ok(_) => ("授权完成", "请关闭此页面并返回 LiveEmail。".to_string()),
        Err(e) => ("授权未完成", e.clone()),
    };
    let body = success_page()
        .replace("授权完成", title)
        .replace("请关闭此页面并返回 LiveEmail。", &detail);
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.as_bytes().len(),
        body
    );
    let _ = socket.write_all(response.as_bytes()).await;
    let _ = socket.flush().await;

    let code = result?;
    let tokens = exchange_code(&cfg, &code, &verifier, &redirect_uri).await?;
    store_tokens(db, account.id, &tokens)?;

    Ok(OAuthResult {
        ok: true,
        email: account.email.clone(),
        message: "授权成功，令牌已保存到系统凭据管理器".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_matches_rfc7636_vector() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert_eq!(
            pkce_challenge(verifier),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn parses_callback_query() {
        let params = parse_query("code=abc%2Fdef&state=s1&scope=a%20b");
        assert_eq!(params.get("code").map(String::as_str), Some("abc/def"));
        assert_eq!(params.get("state").map(String::as_str), Some("s1"));
        assert_eq!(params.get("scope").map(String::as_str), Some("a b"));
    }

    #[test]
    fn extracts_code_and_rejects_bad_callbacks() {
        assert_eq!(
            extract_code("/?code=xyz&state=s1", "s1").unwrap(),
            "xyz"
        );
        assert!(extract_code("/?code=xyz&state=other", "s1")
            .unwrap_err()
            .contains("state"));
        assert!(extract_code(
            "/?error=access_denied&error_description=user%20denied&state=s1",
            "s1"
        )
        .unwrap_err()
        .contains("access_denied"));
        assert!(extract_code("/?state=s1", "s1")
            .unwrap_err()
            .contains("缺少 code"));
    }

    #[test]
    fn random_values_have_expected_shape() {
        let a = random_urlsafe(32);
        let b = random_urlsafe(32);
        assert_eq!(a.len(), 43);
        assert_ne!(a, b);
        assert!(!a.contains('='));
    }
}
