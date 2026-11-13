//! Cloudflare Access login + WebSocket tunnel (Phase 5, ADR-001).
//!
//! Protocol ported from upstream FishTerm's `cloudflared` behaviour (see
//! `docs/adr/ADR-001-rewrite-in-tauri-rust.md`):
//!
//! 1. `HEAD https://<host>` (cloudflared UA) and follow redirects; the final URL
//!    carries `kid`, the final response carries `cf-access-domain`.
//! 2. A cached token (keychain, keyed by domain+kid) is reused until its JWT `exp`.
//! 3. Otherwise: generate a NaCl box keypair, open
//!    `https://<domain>/cdn-cgi/access/cli?...&token=<pubkey>` in the system
//!    browser (validated first: https, no userinfo, host must equal the app domain).
//! 4. Poll `GET https://login.cloudflareaccess.org/transfer/<pubkey>`; the service
//!    answers with a box-encrypted token blob and its `service-public-key` header.
//! 5. `wss://<domain>` with header `Cf-Access-Token: <jwt>` is a raw byte pipe to
//!    sshd behind the tunnel; SSH runs over [`WsPipe`] unchanged.
//!
//! Upstream accepted every host key (`verify(true)`); here the same strict
//! known_hosts TOFU check as direct SSH applies to the domain identity.

use crate::error::{AppError, AppResult};
use crate::secrets::SecretStore;
use base64::Engine;
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
use crypto_box::aead::{Aead, OsRng};
use crypto_box::{Nonce, PublicKey, SalsaBox, SecretKey};
use futures_util::{Sink as FutSink, Stream as FutStream};
use serde::Deserialize;
use std::io::ErrorKind;
use std::pin::Pin;
use std::task::{Context, Poll, ready};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::{Bytes, Error as WsError, Message};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};
use url::Url;

/// Mirrors `cloudflared`; Cloudflare keys some behaviour on this prefix.
const UA: &str = "cloudflared/2022.10.0";
const LOGIN_BASE: &str = "https://login.cloudflareaccess.org";
const LOGIN_PATH: &str = "/cdn-cgi/access/cli";
const NONCE_LEN: usize = 24;
const POLY1305_TAG: usize = 16;
/// Upstream polled 10x60s; keep the same window for the browser round-trip.
const TRANSFER_RETRIES: u8 = 10;
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(60);
const PROBE_TIMEOUT: Duration = Duration::from_secs(15);
const WS_CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const JWT_PAYLOAD_MAX: usize = 16 * 1024;

// ---------------------------------------------------------------- base64 ----

/// Upstream encoded keys as standard base64 with `+`/`/` swapped for `-`/`_`
/// and the padding kept. `URL_SAFE` pads, so it produces the same shape.
fn b64url_encode(data: &[u8]) -> String {
    URL_SAFE.encode(data)
}

/// Node's `Buffer.from(x, 'base64')` accepted both alphabets; the service mixes
/// them across body/header/query, so decode leniently.
fn b64_flexible(s: &str) -> Option<Vec<u8>> {
    let s = s.trim();
    [URL_SAFE, URL_SAFE_NO_PAD, STANDARD, STANDARD_NO_PAD]
        .iter()
        .find_map(|e| e.decode(s).ok())
}

// ------------------------------------------------------------- validation ----

fn host_ok(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 253
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

/// The webview-saved `host` field before anything network or browser happens.
fn validate_app_host(host: &str) -> AppResult<()> {
    if !host_ok(host) || host.contains(':') {
        return Err(AppError::Invalid("host must be a plain hostname"));
    }
    Ok(())
}

fn valid_host_shape(s: &str) -> bool {
    host_ok(s) && !s.contains(':')
}

/// Threat-model §7 control: the login URL is opened by the OS browser, so it is
/// validated here — https only, no userinfo, host must equal the app domain,
/// exactly the cloudflared CLI path, default port, no fragment.
fn validate_login_url(u: &Url, app_domain: &str) -> AppResult<()> {
    let fail = |what: &'static str| {
        tracing::warn!(url_host = %u.host_str().unwrap_or("?"), "login URL rejected: {what}");
        AppError::CfLogin
    };
    if u.scheme() != "https" {
        return Err(fail("scheme"));
    }
    if !u.username().is_empty() || u.password().is_some() {
        return Err(fail("userinfo"));
    }
    let host = u
        .host_str()
        .ok_or_else(|| fail("host"))?
        .to_ascii_lowercase();
    if !valid_host_shape(&host) || host != app_domain.to_ascii_lowercase() {
        return Err(fail("host mismatch"));
    }
    if u.port().is_some_and(|p| p != 443) {
        return Err(fail("port"));
    }
    if u.path() != LOGIN_PATH {
        return Err(fail("path"));
    }
    if u.fragment().is_some() {
        return Err(fail("fragment"));
    }
    Ok(())
}

// ------------------------------------------------------------------ probe ----

/// What a successful probe pins down: the Access app id and its canonical domain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Probe {
    pub kid: String,
    pub domain: String,
}

fn kid_ok(kid: &str) -> bool {
    !kid.is_empty()
        && kid.len() <= 128
        && kid
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

async fn probe_url(base: &str) -> AppResult<Probe> {
    let client = reqwest::Client::builder()
        .user_agent(UA)
        .build()
        .map_err(|e| {
            tracing::error!(error = ?e, "http client");
            AppError::Internal
        })?;
    let resp = client
        .head(base)
        .timeout(PROBE_TIMEOUT)
        .send()
        .await
        .map_err(|e| {
            tracing::warn!(error = ?e, "cf probe request failed");
            AppError::CfProbe
        })?;
    let final_url = resp.url().clone();
    // Production always probes an https base (ensure_token builds it), so a
    // downgrade mid-redirect is refused. Tests use an http mock instead.
    if base.starts_with("https://") && final_url.scheme() != "https" {
        tracing::warn!(url = %final_url, "cf probe landed on non-https");
        return Err(AppError::CfProbe);
    }
    let kid = final_url
        .query_pairs()
        .find(|(k, _)| k == "kid")
        .map(|(_, v)| v.into_owned())
        .ok_or_else(|| {
            tracing::warn!(url = %final_url, "cf probe: no kid in final query");
            AppError::CfProbe
        })?;
    if !kid_ok(&kid) {
        tracing::warn!("cf probe: kid has an unexpected shape");
        return Err(AppError::CfProbe);
    }
    let domain = resp
        .headers()
        .get("cf-access-domain")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            tracing::warn!("cf probe: cf-access-domain header missing");
            AppError::CfProbe
        })?;
    if !valid_host_shape(domain) {
        tracing::warn!(domain, "cf probe: cf-access-domain is not a hostname");
        return Err(AppError::CfProbe);
    }
    Ok(Probe {
        kid,
        domain: domain.to_owned(),
    })
}

// ------------------------------------------------------------------ login ----

pub struct LoginStart {
    /// Validated login URL for the system browser.
    pub url: String,
    pub secret: SecretKey,
    pub public_b64: String,
}

fn generate_login(app_domain: &str) -> AppResult<LoginStart> {
    validate_app_host(app_domain)?;
    let secret = SecretKey::generate(&mut OsRng);
    let public_b64 = b64url_encode(secret.public_key().as_bytes());

    let mut u = Url::parse(&format!("https://{app_domain}{LOGIN_PATH}"))
        .map_err(|_| AppError::Invalid("invalid host"))?;
    u.query_pairs_mut()
        .append_pair(
            "redirect_url",
            &format!("https://{app_domain}?token={public_b64}"),
        )
        .append_pair("edge_token_transfer", "true")
        .append_pair("send_org_token", "true")
        .append_pair("token", &public_b64);
    validate_login_url(&u, app_domain)?;
    Ok(LoginStart {
        url: u.to_string(),
        secret,
        public_b64,
    })
}

// --------------------------------------------------------------- transfer ----

fn decrypt_transfer(body: &[u8], svc_pub: &str, secret: &SecretKey) -> AppResult<String> {
    if body.len() < NONCE_LEN + POLY1305_TAG {
        tracing::warn!(len = body.len(), "cf transfer body too short");
        return Err(AppError::CfTransfer);
    }
    let peer = b64_flexible(svc_pub)
        .and_then(|k| PublicKey::from_slice(&k).ok())
        .ok_or_else(|| {
            tracing::warn!("cf transfer: undecodable service-public-key");
            AppError::CfTransfer
        })?;
    let (nonce_bytes, ct) = body.split_at(NONCE_LEN);
    let box_ = SalsaBox::new(&peer, secret);
    let plain = box_
        .decrypt(Nonce::from_slice(nonce_bytes), ct)
        .map_err(|e| {
            tracing::warn!(error = ?e, "cf transfer: decryption failed");
            AppError::CfTransfer
        })?;
    let v: serde_json::Value = serde_json::from_slice(&plain).map_err(|e| {
        tracing::warn!(error = ?e, "cf transfer: payload is not json");
        AppError::CfTransfer
    })?;
    let token = v
        .get("app_token")
        .and_then(serde_json::Value::as_str)
        .filter(|t| !t.is_empty() && t.len() <= 8192)
        .ok_or_else(|| {
            tracing::warn!("cf transfer: no app_token in payload");
            AppError::CfTransfer
        })?;
    Ok(token.to_owned())
}

/// Poll the transfer endpoint up to `retries` times. Each attempt is bounded by
/// [`TRANSFER_TIMEOUT`], so the whole wait is `retries` minutes (browser login).
async fn transfer_on(
    client: &reqwest::Client,
    base: &str,
    login: &LoginStart,
    retries: u8,
    mut on_attempt: impl FnMut(u8),
) -> AppResult<String> {
    for attempt in 1..=retries {
        on_attempt(attempt);
        let resp = client
            .get(format!("{base}/transfer/{}", login.public_b64))
            .timeout(TRANSFER_TIMEOUT)
            .send()
            .await;
        // The service answers non-200 until the browser login completes.
        let resp = match resp {
            Ok(r) if r.status().is_success() => r,
            Ok(r) => {
                tracing::debug!(status = %r.status(), attempt, "cf transfer not ready");
                continue;
            }
            Err(e) => {
                tracing::debug!(error = ?e, attempt, "cf transfer request failed");
                continue;
            }
        };
        let svc = resp
            .headers()
            .get("service-public-key")
            .and_then(|v| v.to_str().ok())
            .ok_or(AppError::CfTransfer)?
            .to_owned();
        let body = resp.bytes().await.map_err(|e| {
            tracing::warn!(error = ?e, "cf transfer body read failed");
            AppError::CfTransfer
        })?;
        return decrypt_transfer(&body, &svc, &login.secret);
    }
    tracing::warn!(retries, "cf transfer: browser sign-in never completed");
    Err(AppError::CfLogin)
}

// ------------------------------------------------------------------ token ----

fn token_account(domain: &str, kid: &str) -> String {
    format!("cf-access:{domain}:{kid}")
}

#[derive(Deserialize)]
struct JwtPayload {
    #[serde(default)]
    exp: Option<u64>,
}

/// True when the JWT cannot be trusted to still be valid (malformed, undated,
/// or `exp` passed). Never panics on hostile input; bounded payload size.
fn jwt_expired(token: &str, now_unix: u64) -> bool {
    let mut parts = token.split('.');
    let payload = match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some(_), Some(p), Some(_), None) if p.len() <= JWT_PAYLOAD_MAX => p,
        _ => return true,
    };
    let bytes = match b64_flexible(payload) {
        Some(b) => b,
        None => return true,
    };
    let Ok(p) = serde_json::from_slice::<JwtPayload>(&bytes) else {
        return true;
    };
    match p.exp {
        Some(exp) => now_unix >= exp,
        None => true,
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Resolve a usable token for the Access app behind `app_host`: keychain cache
/// first, browser login + transfer otherwise. Returns the canonical domain (use
/// it for both the WebSocket and the SSH host identity) and the token.
///
/// `open_browser` receives the already-validated login URL; `on_status` drives
/// UI progress as `("browser"|"waiting"|"cache"|"token", attempt)`.
async fn ensure_token_on(
    probe_base: String,
    transfer_base: &str,
    secrets: &dyn SecretStore,
    open_browser: &(dyn Fn(String) -> AppResult<()> + Send + Sync),
    mut on_status: impl FnMut(&str, u8),
) -> AppResult<(String, String)> {
    let probe = probe_url(&probe_base).await?;
    let account = token_account(&probe.domain, &probe.kid);

    if let Some(cached) = secrets.get(&account)? {
        if !jwt_expired(&cached, now_unix()) {
            on_status("cache", 0);
            return Ok((probe.domain, cached));
        }
        // Expired: remove so a failed login below cannot leave a dead token behind.
        secrets.delete(&account)?;
    }

    let login = generate_login(&probe.domain)?;
    on_status("browser", 0);
    open_browser(login.url.clone())?;

    let client = reqwest::Client::builder()
        .user_agent(UA)
        .build()
        .map_err(|e| {
            tracing::error!(error = ?e, "http client");
            AppError::Internal
        })?;
    let token = transfer_on(
        &client,
        transfer_base,
        &login,
        TRANSFER_RETRIES,
        |attempt| on_status("waiting", attempt),
    )
    .await?;
    secrets.set(&account, &token)?;
    on_status("token", 0);
    Ok((probe.domain, token))
}

async fn ensure_token(
    app_host: &str,
    secrets: &dyn SecretStore,
    open_browser: &(dyn Fn(String) -> AppResult<()> + Send + Sync),
    on_status: impl FnMut(&str, u8),
) -> AppResult<(String, String)> {
    validate_app_host(app_host)?;
    ensure_token_on(
        format!("https://{app_host}"),
        LOGIN_BASE,
        secrets,
        open_browser,
        on_status,
    )
    .await
}

// ---------------------------------------------------------- websocket pipe ----

type Ws = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

fn ws_request(domain: &str, token: &str) -> AppResult<http::Request<()>> {
    let mut req = format!("wss://{domain}/")
        .into_client_request()
        .map_err(|e| {
            tracing::warn!(error = ?e, "cf ws request build failed");
            AppError::CfWs
        })?;
    let headers = req.headers_mut();
    let token_value = http::HeaderValue::from_str(token).map_err(|_| {
        tracing::warn!("cf token contains invalid header characters");
        AppError::CfWs
    })?;
    headers.insert("Cf-Access-Token", token_value);
    headers.insert("User-Agent", http::HeaderValue::from_static(UA));
    Ok(req)
}

/// Open the tunnel as a byte pipe. The same strict SSH host-key check then runs
/// over it (threat-model §5) — upstream accepted any key here.
async fn connect_ws(domain: &str, token: &str) -> AppResult<WsPipe> {
    let req = ws_request(domain, token)?;
    let ws = tokio::time::timeout(WS_CONNECT_TIMEOUT, connect_async(req))
        .await
        .map_err(|_| {
            tracing::warn!("cf ws connect timed out");
            AppError::CfWs
        })?
        .map_err(|e| {
            tracing::warn!(error = ?e, "cf ws connect failed");
            AppError::CfWs
        })?
        .0;
    Ok(WsPipe::new(ws))
}

/// `AsyncRead + AsyncWrite` over the tunnel WebSocket, so russh's
/// `connect_stream` can run SSH over it unchanged.
///
/// Writes: `poll_ready` drains previously queued frames, `start_send` queues,
/// and an opportunistic `poll_flush` pushes them out immediately; russh also
/// drives `poll_flush` itself. Reads: message payloads are buffered as a byte
/// stream; `Close`/stream end becomes EOF, so russh sees a normal disconnect.
pub struct WsPipe {
    ws: Pin<Box<Ws>>,
    buf: Vec<u8>,
    pos: usize,
    eof: bool,
    closed: bool,
}

fn ws_io_err(e: WsError) -> std::io::Error {
    std::io::Error::new(ErrorKind::ConnectionReset, e.to_string())
}

impl WsPipe {
    pub fn new(ws: Ws) -> Self {
        Self {
            ws: Box::pin(ws),
            buf: Vec::new(),
            pos: 0,
            eof: false,
            closed: false,
        }
    }

    /// Test hook: build a pipe over any websocket endpoint (e.g. `ws://`).
    #[cfg(test)]
    async fn connect_test(url: &str) -> AppResult<Self> {
        let (ws, _) = connect_async(url).await.map_err(|e| {
            tracing::warn!(error = ?e, "test ws connect failed");
            AppError::CfWs
        })?;
        Ok(Self::new(ws))
    }
}

impl AsyncRead for WsPipe {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        out: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        loop {
            if self.pos < self.buf.len() {
                let n = (self.buf.len() - self.pos).min(out.remaining());
                out.put_slice(&self.buf[self.pos..self.pos + n]);
                self.pos += n;
                if self.pos == self.buf.len() {
                    self.buf.clear();
                    self.pos = 0;
                }
                return Poll::Ready(Ok(()));
            }
            if self.eof {
                // Nothing written + Ok == EOF(0) for the caller.
                return Poll::Ready(Ok(()));
            }
            match self.ws.as_mut().poll_next(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Some(Ok(Message::Binary(b)))) => self.buf.extend_from_slice(&b),
                Poll::Ready(Some(Ok(Message::Close(_)))) | Poll::Ready(None) => {
                    self.eof = true;
                    return Poll::Ready(Ok(()));
                }
                Poll::Ready(Some(Ok(_))) => continue, // Ping/Pong are handled by tungstenite
                Poll::Ready(Some(Err(e))) => {
                    self.eof = true;
                    tracing::warn!(error = ?e, "cf websocket read failed");
                    return Poll::Ready(Err(ws_io_err(e)));
                }
            }
        }
    }
}

impl AsyncWrite for WsPipe {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        if self.closed {
            return Poll::Ready(Err(std::io::Error::new(
                ErrorKind::BrokenPipe,
                "websocket closed",
            )));
        }
        // poll_ready first: it writes out any frames queued by earlier calls.
        ready!(self.ws.as_mut().poll_ready(cx)).map_err(ws_io_err)?;
        let data = Bytes::copy_from_slice(buf);
        self.ws
            .as_mut()
            .start_send(Message::Binary(data))
            .map_err(ws_io_err)?;
        // Opportunistic flush; Pending is fine, later flushes drain the queue.
        let _ = self.ws.as_mut().poll_flush(cx).map_err(ws_io_err)?;
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        self.ws.as_mut().poll_flush(cx).map_err(ws_io_err)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        if !self.closed {
            ready!(self.ws.as_mut().poll_ready(cx)).map_err(ws_io_err)?;
            self.ws
                .as_mut()
                .start_send(Message::Close(None))
                .map_err(ws_io_err)?;
            self.closed = true;
        }
        self.ws.as_mut().poll_close(cx).map_err(ws_io_err)
    }
}

// --------------------------------------------------------------- entrypoint ----

/// Everything `session_open` needs for a Cloudflare Access host: the SSH target
/// identity plus the tunnel as a byte pipe. `known_hosts` applies to `domain`.
pub struct CfSession {
    pub domain: String,
    pub pipe: WsPipe,
}

pub async fn open_session_stream(
    app_host: &str,
    secrets: &dyn SecretStore,
    open_browser: &(dyn Fn(String) -> AppResult<()> + Send + Sync),
    mut on_status: impl FnMut(&str, u8),
) -> AppResult<CfSession> {
    let (domain, token) = ensure_token(app_host, secrets, open_browser, &mut on_status).await?;
    on_status("connecting", 0);
    let pipe = connect_ws(&domain, &token).await?;
    Ok(CfSession { domain, pipe })
}

// ------------------------------------------------------------------- tests ----

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::memory::MemoryStore;
    use base64::engine::general_purpose::URL_SAFE as B64;
    use futures_util::{SinkExt, StreamExt};
    use russh::Channel;
    use russh::keys::decode_secret_key;
    use russh::server::{self, Auth, Msg};
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU8, Ordering};

    const HOST_A: &str = include_str!("../tests/fixtures/host_a");

    fn mem() -> Arc<MemoryStore> {
        Arc::new(MemoryStore::default())
    }

    // ---- validation ---------------------------------------------------------

    #[test]
    fn app_host_rules() {
        for ok in ["ssh.example.com", "a-b.c", "team.cfargotunnel.com"] {
            assert!(validate_app_host(ok).is_ok(), "{ok}");
        }
        for bad in [
            "",
            "https://x.com",
            "x.com:2222",
            "a b.com",
            "u@x.com",
            "x/y",
            &"a".repeat(254),
        ] {
            assert!(validate_app_host(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn login_url_validator() {
        let good = "https://ssh.example.com/cdn-cgi/access/cli?token=x";
        validate_login_url(&Url::parse(good).unwrap(), "ssh.example.com").unwrap();

        let cases: Vec<(&str, &str)> = vec![
            ("file", "file:///etc/passwd"),
            ("javascript", "javascript:alert(1)"),
            (
                "http downgrade",
                "http://ssh.example.com/cdn-cgi/access/cli",
            ),
            (
                "userinfo",
                "https://evil.com@ssh.example.com/cdn-cgi/access/cli",
            ),
            ("wrong host", "https://evil.com/cdn-cgi/access/cli"),
            ("wrong path", "https://ssh.example.com/other"),
            ("port", "https://ssh.example.com:8080/cdn-cgi/access/cli"),
            ("fragment", "https://ssh.example.com/cdn-cgi/access/cli#z"),
        ];
        for (label, u) in cases {
            assert!(
                matches!(
                    validate_login_url(&Url::parse(u).unwrap(), "ssh.example.com"),
                    Err(AppError::CfLogin)
                ),
                "{label} must be rejected"
            );
        }
    }

    // ---- login URL ----------------------------------------------------------

    #[test]
    fn generated_login_is_valid_and_carries_the_key() {
        let login = generate_login("ssh.example.com").unwrap();
        let u = Url::parse(&login.url).unwrap();
        validate_login_url(&u, "ssh.example.com").unwrap();
        assert_eq!(u.path(), LOGIN_PATH);

        let pairs: HashMap<String, String> = u
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        assert_eq!(
            pairs.get("edge_token_transfer").map(String::as_str),
            Some("true")
        );
        assert_eq!(
            pairs.get("send_org_token").map(String::as_str),
            Some("true")
        );
        // The redirect_url embeds the same key as the `token` parameter.
        let redirect = Url::parse(pairs.get("redirect_url").unwrap()).unwrap();
        let redirect_token = redirect
            .query_pairs()
            .find(|(k, _)| k == "token")
            .map(|(_, v)| v.into_owned())
            .unwrap();
        assert_eq!(pairs.get("token").unwrap(), &redirect_token);
        // The embedded key is 32 bytes of key material in padded URL-safe base64.
        let key = b64_flexible(pairs.get("token").unwrap()).unwrap();
        assert_eq!(key.len(), 32);
    }

    // ---- JWT expiry ---------------------------------------------------------

    #[test]
    fn jwt_expiry_rules() {
        let payload = URL_SAFE_NO_PAD.encode(br#"{"exp":100}"#);
        let token = format!("a.{payload}.c");
        assert!(jwt_expired(&token, 100));
        assert!(jwt_expired(&token, 101));
        assert!(!jwt_expired(&token, 99));

        // Padded base64url, like Cloudflare actually emits.
        let padded = B64.encode(br#"{"exp":5000000000}"#);
        assert!(!jwt_expired(&format!("h.{padded}.s"), 100));

        for bad in [
            "",
            "a.b",
            "a.b.c.d",
            "a.!!!.c",
            &format!("a.{}.c", "x".repeat(JWT_PAYLOAD_MAX + 1)),
        ] {
            assert!(jwt_expired(bad, 0), "{bad:?} must count as expired");
        }
    }

    // ---- transfer crypto ----------------------------------------------------

    /// Simulate the transfer service: encrypt `{"app_token":..}` to our pubkey
    /// with a fresh service keypair, nonce first, ciphertext after.
    fn service_encrypt(pub_b64: &str, plain: &[u8]) -> (String, Vec<u8>) {
        let svc_secret = SecretKey::generate(&mut OsRng);
        let peer = PublicKey::from_slice(&b64_flexible(pub_b64).unwrap()).unwrap();
        let box_ = SalsaBox::new(&peer, &svc_secret);
        let nonce = Nonce::from_slice(&[7u8; NONCE_LEN]);
        let mut blob = nonce.to_vec();
        blob.extend_from_slice(&box_.encrypt(nonce, plain).unwrap());
        (b64url_encode(svc_secret.public_key().as_bytes()), blob)
    }

    #[tokio::test]
    async fn transfer_decrypts_service_payload() {
        let login = generate_login("ssh.example.com").unwrap();
        let plain = br#"{"app_token":"eyJ.abc.def"}"#;
        let (svc_pub, blob) = service_encrypt(&login.public_b64, plain);

        let (svc_pub2, blob2) = (svc_pub.clone(), blob.clone());
        let got = decrypt_transfer(&blob2, &svc_pub2, &login.secret).unwrap();
        assert_eq!(got, "eyJ.abc.def");

        // Standard base64 header (both alphabets must decode).
        let got2 = decrypt_transfer(
            &blob,
            &B64.encode(B64.decode(&svc_pub).unwrap()),
            &login.secret,
        )
        .unwrap();
        assert_eq!(got2, "eyJ.abc.def");

        // Tampered ciphertext is refused.
        let mut tampered = blob.clone();
        tampered[NONCE_LEN] ^= 1;
        assert!(matches!(
            decrypt_transfer(&tampered, &svc_pub, &login.secret),
            Err(AppError::CfTransfer)
        ));
        assert!(matches!(
            decrypt_transfer(&blob[..10], &svc_pub, &login.secret),
            Err(AppError::CfTransfer)
        ));
    }

    // ---- HTTP endpoints (wiremock) -------------------------------------------

    #[tokio::test]
    async fn probe_follows_redirect_and_reads_kid_and_domain() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("HEAD"))
            .and(wiremock::matchers::path("/"))
            .respond_with(wiremock::ResponseTemplate::new(302).insert_header(
                "Location",
                format!(
                    "{}/cdn-cgi/access/login/ssh.example.com?kid=kid-123",
                    server.uri()
                ),
            ))
            .mount(&server)
            .await;
        wiremock::Mock::given(wiremock::matchers::method("HEAD"))
            .and(wiremock::matchers::path(
                "/cdn-cgi/access/login/ssh.example.com",
            ))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .insert_header("cf-access-domain", "ssh.example.com"),
            )
            .mount(&server)
            .await;

        let probe = probe_url(&server.uri()).await.unwrap();
        assert_eq!(
            probe,
            Probe {
                kid: "kid-123".into(),
                domain: "ssh.example.com".into()
            }
        );
    }

    #[tokio::test]
    async fn probe_without_domain_header_is_refused() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("HEAD"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).insert_header("cf-access-domain", "bad host!"),
            )
            .mount(&server)
            .await;
        assert!(matches!(
            probe_url(&server.uri()).await,
            Err(AppError::CfProbe)
        ));
    }

    /// Matches any `/transfer/<key>`: `ensure_token_on` generates a fresh keypair
    /// internally, so the exact key is only known when the caller drives
    /// `transfer_on` itself.
    fn transfer_mock_ok(svc_pub: &str, body: &[u8]) -> wiremock::Mock {
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path_regex(r"^/transfer/"))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .insert_header("service-public-key", svc_pub)
                    .set_body_bytes(body.to_vec()),
            )
    }

    #[tokio::test]
    async fn transfer_success_flow() {
        let login = generate_login("ssh.example.com").unwrap();
        let plain = br#"{"app_token":"tok"}"#;
        let (svc_pub, blob) = service_encrypt(&login.public_b64, plain);
        let server = wiremock::MockServer::start().await;
        transfer_mock_ok(&svc_pub, &blob).mount(&server).await;

        let client = reqwest::Client::builder().user_agent(UA).build().unwrap();
        let seen = AtomicU8::new(0);
        let token = transfer_on(&client, &server.uri(), &login, 3, |a| {
            seen.store(a, Ordering::SeqCst)
        })
        .await
        .unwrap();
        assert_eq!(token, "tok");
        assert_eq!(seen.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn transfer_gives_up_after_retries() {
        let login = generate_login("ssh.example.com").unwrap();
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .respond_with(wiremock::ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let client = reqwest::Client::builder().user_agent(UA).build().unwrap();
        let err = transfer_on(&client, &server.uri(), &login, 2, |_| {})
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::CfLogin), "{err:?}");
    }

    #[tokio::test]
    async fn transfer_with_wrong_service_key_fails() {
        let login = generate_login("ssh.example.com").unwrap();
        let (svc_pub, blob) = service_encrypt(&login.public_b64, br#"{"app_token":"x"}"#);
        // Hand over a peer key that does not match the encryption.
        let stranger = SecretKey::generate(&mut OsRng);
        let stranger_pub = b64url_encode(stranger.public_key().as_bytes());
        let server = wiremock::MockServer::start().await;
        transfer_mock_ok(&svc_pub, &blob).mount(&server).await;
        assert!(matches!(
            decrypt_transfer(&blob, &stranger_pub, &login.secret),
            Err(AppError::CfTransfer)
        ));
    }

    // ---- token orchestration -----------------------------------------------

    fn noop_browser() -> impl Fn(String) -> AppResult<()> {
        |_url| Ok(())
    }

    /// Dynamic transfer service: encrypts a fixed token to whatever public key
    /// the client generated (the real service reads the key from the request
    /// path), so `ensure_token_on`'s fresh keypair decrypts successfully.
    fn dynamic_transfer_responder(
        app_token: &'static str,
    ) -> impl Fn(&wiremock::Request) -> wiremock::ResponseTemplate {
        move |req: &wiremock::Request| {
            let key = req.url.path().strip_prefix("/transfer/").unwrap_or("");
            let Some(peer) = b64_flexible(key).and_then(|k| PublicKey::from_slice(&k).ok()) else {
                return wiremock::ResponseTemplate::new(400);
            };
            let secret = SecretKey::generate(&mut OsRng);
            let box_ = SalsaBox::new(&peer, &secret);
            let nonce = Nonce::from_slice(&[9u8; NONCE_LEN]);
            let plain = format!(r#"{{"app_token":"{app_token}"}}"#);
            let mut blob = nonce.to_vec();
            blob.extend_from_slice(&box_.encrypt(nonce, plain.as_bytes()).unwrap());
            wiremock::ResponseTemplate::new(200)
                .insert_header(
                    "service-public-key",
                    b64url_encode(secret.public_key().as_bytes()),
                )
                .set_body_bytes(blob)
        }
    }

    #[tokio::test]
    async fn ensure_token_logs_in_then_uses_cache() {
        // The mock must mint a well-formed JWT with a future `exp`: the cache
        // path below re-validates it.
        let jwt: &'static str = Box::leak(
            format!("h.{}.s", URL_SAFE_NO_PAD.encode(br#"{"exp":9999999999}"#)).into_boxed_str(),
        );
        // The probe reads `kid` from the *request's* final URL query, so the mock
        // only needs to answer with the cf-access-domain header.
        let probe_server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("HEAD"))
            .and(wiremock::matchers::path("/"))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .insert_header("cf-access-domain", "ssh.example.com"),
            )
            .mount(&probe_server)
            .await;
        let transfer_server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path_regex(r"^/transfer/"))
            .respond_with(dynamic_transfer_responder(jwt))
            .mount(&transfer_server)
            .await;

        let mem = mem();
        let statuses = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let opened = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let opened_sink = opened.clone();

        let probe_base = format!("{}/?kid=kid-9", probe_server.uri());
        let (domain, token) = ensure_token_on(
            probe_base.clone(),
            &transfer_server.uri(),
            mem.as_ref(),
            &|url: String| {
                opened_sink.lock().unwrap().push(url);
                Ok(())
            },
            |s, _| statuses.lock().unwrap().push(s.into()),
        )
        .await
        .unwrap();
        assert_eq!(domain, "ssh.example.com");
        assert_eq!(token, jwt);
        assert_eq!(opened.lock().unwrap().len(), 1);
        {
            let st = statuses.lock().unwrap();
            assert!(st.contains(&"browser".to_string()));
            assert!(st.contains(&"token".to_string()));
        } // guard dropped before the awaits below
        // Cached in the keychain under the cf-access account.
        let cached = mem
            .get(&token_account("ssh.example.com", "kid-9"))
            .unwrap()
            .unwrap();
        assert_eq!(cached, jwt);

        // Second call: valid cache short-circuits, no browser, no transfer.
        let mut saw_cache = false;
        let (d2, t2) = ensure_token_on(
            probe_base,
            &transfer_server.uri(),
            mem.as_ref(),
            &noop_browser(),
            |s, _| {
                if s == "cache" {
                    saw_cache = true;
                }
            },
        )
        .await
        .unwrap();
        assert_eq!((d2.as_str(), t2.as_str()), ("ssh.example.com", jwt));
        assert!(saw_cache);
        assert_eq!(opened.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn expired_cache_triggers_fresh_login() {
        let probe_server = wiremock::MockServer::start().await;
        // Redirect carries the kid; the follow-up HEAD answers the domain.
        wiremock::Mock::given(wiremock::matchers::method("HEAD"))
            .and(wiremock::matchers::path("/"))
            .respond_with(wiremock::ResponseTemplate::new(302).insert_header(
                "Location",
                format!("{}/login?kid=kid-1", probe_server.uri()),
            ))
            .mount(&probe_server)
            .await;
        wiremock::Mock::given(wiremock::matchers::method("HEAD"))
            .and(wiremock::matchers::path("/login"))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .insert_header("cf-access-domain", "ssh.example.com"),
            )
            .mount(&probe_server)
            .await;
        let transfer_server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path_regex(r"^/transfer/"))
            .respond_with(dynamic_transfer_responder("fresh"))
            .mount(&transfer_server)
            .await;

        let mem = mem();
        // An expired JWT sits in the keychain.
        let payload = URL_SAFE_NO_PAD.encode(br#"{"exp":1}"#);
        mem.set(
            &token_account("ssh.example.com", "kid-1"),
            &format!("h.{payload}.s"),
        )
        .unwrap();

        let (_, token) = ensure_token_on(
            probe_server.uri(),
            &transfer_server.uri(),
            mem.as_ref(),
            &noop_browser(),
            |_, _| {},
        )
        .await
        .unwrap();
        assert_eq!(token, "fresh");
    }

    // ---- WebSocket pipe end-to-end over russh ------------------------------
    //
    // A real handshake + password auth + echo through a ws:// bridge proves the
    // pipe preserves SSH bytes in both directions; the same pipe fronts the
    // production wss:// tunnel.

    #[derive(Clone)]
    struct EchoServer;

    impl russh::server::Handler for EchoServer {
        type Error = russh::Error;
        async fn auth_password(&mut self, user: &str, pw: &str) -> Result<Auth, Self::Error> {
            Ok(if user == "leo" && pw == "right" {
                Auth::Accept
            } else {
                Auth::reject()
            })
        }
        async fn channel_open_session(
            &mut self,
            _ch: Channel<Msg>,
            reply: server::ChannelOpenHandle,
            _s: &mut russh::server::Session,
        ) -> Result<(), Self::Error> {
            reply.accept().await;
            Ok(())
        }
        async fn data(
            &mut self,
            ch: russh::ChannelId,
            data: &[u8],
            s: &mut russh::server::Session,
        ) -> Result<(), Self::Error> {
            s.data(ch, data.to_vec())?;
            Ok(())
        }
    }

    /// sshd that keeps accepting connections (one russh session per accept).
    async fn serve_echo(port_handle: tokio::sync::oneshot::Sender<u16>) {
        let config = Arc::new(server::Config {
            keys: vec![decode_secret_key(HOST_A, None).unwrap()],
            ..Default::default()
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let _ = port_handle.send(port);
        while let Ok((sock, _)) = listener.accept().await {
            if let Ok(running) = server::run_stream(config.clone(), sock, EchoServer).await {
                let _ = running.await;
            }
        }
    }

    /// ws:// bridge that keeps accepting: each websocket is piped to one fresh
    /// TCP connection to the SSH server, both directions.
    async fn ws_bridge(ssh_port: u16, ws_port_handle: tokio::sync::oneshot::Sender<u16>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let ws_port = listener.local_addr().unwrap().port();
        let _ = ws_port_handle.send(ws_port);
        while let Ok((sock, _)) = listener.accept().await {
            tokio::spawn(async move {
                let ws = match tokio_tungstenite::accept_async(sock).await {
                    Ok(ws) => ws,
                    Err(_) => return,
                };
                let tcp = match tokio::net::TcpStream::connect(("127.0.0.1", ssh_port)).await {
                    Ok(t) => t,
                    Err(_) => return,
                };
                let (mut ws_tx, mut ws_rx) = futures_util::stream::StreamExt::split(ws);
                let (mut t_rx, mut t_tx) = tcp.into_split();
                let to_ssh = tokio::spawn(async move {
                    while let Some(msg) = ws_rx.next().await {
                        match msg {
                            Ok(Message::Binary(b)) => {
                                if tokio::io::AsyncWriteExt::write_all(&mut t_tx, &b)
                                    .await
                                    .is_err()
                                {
                                    break;
                                }
                            }
                            Ok(Message::Close(_)) | Err(_) => break,
                            Ok(_) => {}
                        }
                    }
                    let _ = tokio::io::AsyncWriteExt::shutdown(&mut t_tx).await;
                });
                let to_ws = tokio::spawn(async move {
                    let mut buf = [0u8; 8192];
                    loop {
                        match tokio::io::AsyncReadExt::read(&mut t_rx, &mut buf).await {
                            Ok(0) | Err(_) => break,
                            Ok(n) => {
                                if ws_tx
                                    .send(Message::Binary(buf[..n].to_vec().into()))
                                    .await
                                    .is_err()
                                {
                                    break;
                                }
                            }
                        }
                    }
                    let _ = ws_tx.close().await;
                });
                let _ = tokio::join!(to_ssh, to_ws);
            });
        }
    }

    fn answer(accept: bool) -> crate::ssh::Ask {
        Box::new(move |_| {
            let (tx, rx) = tokio::sync::oneshot::channel();
            tx.send(accept).unwrap();
            rx
        })
    }

    #[tokio::test]
    async fn ssh_runs_through_ws_pipe_end_to_end() {
        let (ssh_tx, ssh_rx) = tokio::sync::oneshot::channel();
        tokio::spawn(serve_echo(ssh_tx));
        let ssh_port = ssh_rx.await.unwrap();
        let (ws_tx, ws_rx) = tokio::sync::oneshot::channel();
        tokio::spawn(ws_bridge(ssh_port, ws_tx));
        let ws_port = ws_rx.await.unwrap();
        let ws_url = format!("ws://127.0.0.1:{ws_port}");

        let kh = tempfile::tempdir().unwrap();
        let target = crate::ssh::Target {
            // In production the identity is the cf-access-domain; here the test
            // endpoint so a later connect can trust without prompting.
            host: "127.0.0.1".into(),
            port: ws_port,
            username: "leo".into(),
            credential: crate::ssh::Credential::Password("right".into()),
            known_hosts: kh.path().join("known_hosts"),
        };

        // 1. Unknown key: TOFU prompt, rejected -> nothing is learned.
        let pipe = WsPipe::connect_test(&ws_url).await.unwrap();
        let err = crate::ssh::connect_on(target.clone(), answer(false), pipe)
            .await
            .err()
            .unwrap();
        assert!(matches!(err, AppError::HostKeyRejected), "{err:?}");
        assert!(!kh.path().join("known_hosts").exists());

        // 2. Accepted: key learned; echo flows both ways through the pipe.
        let pipe = WsPipe::connect_test(&ws_url).await.unwrap();
        let handle = crate::ssh::connect_on(target.clone(), answer(true), pipe)
            .await
            .unwrap();
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let (out_tx, mut out_rx) = tokio::sync::mpsc::unbounded_channel();
        let pump = tokio::spawn(crate::ssh::run_shell(handle.into(), 80, 24, rx, move |b| {
            let _ = out_tx.send(b);
        }));
        tx.send(crate::ssh::Cmd::Data(b"ping\n".to_vec())).unwrap();
        assert_eq!(out_rx.recv().await.unwrap(), b"ping\n");
        tx.send(crate::ssh::Cmd::Close).unwrap();
        pump.await.unwrap().unwrap();

        // 3. Learned identity: a fresh pipe connects with no prompt at all.
        let pipe = WsPipe::connect_test(&ws_url).await.unwrap();
        let must_not_ask: crate::ssh::Ask = Box::new(|p| {
            panic!("unexpected prompt: {p:?}");
        });
        let handle = crate::ssh::connect_on(target, must_not_ask, pipe)
            .await
            .unwrap();
        let _ = handle
            .disconnect(russh::Disconnect::ByApplication, "", "en")
            .await;
    }

    #[tokio::test]
    async fn ws_request_carries_token_and_ua() {
        let req = ws_request("ssh.example.com", "tok.en.sec").unwrap();
        assert_eq!(req.uri().to_string(), "wss://ssh.example.com/");
        assert_eq!(req.headers().get("Cf-Access-Token").unwrap(), "tok.en.sec");
        assert_eq!(
            req.headers().get("User-Agent").unwrap(),
            "cloudflared/2022.10.0"
        );
        assert!(ws_request("ssh.example.com", "bad\nheader").is_err());
    }
}
