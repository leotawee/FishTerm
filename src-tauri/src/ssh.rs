//! SSH sessions over russh. Host keys are checked strictly against known_hosts
//! (threat model §5: MITM). Upstream FishTerm accepted every key.

use crate::error::{AppError, AppResult};
use russh::client::{self, Handle};
// Re-export so other modules can name the shared-session type without a russh dep.
pub use russh::client::Handle as SessionHandle;
use russh::keys::known_hosts::{known_host_keys_path, learn_known_hosts_path};
use russh::keys::{HashAlg, PrivateKey, PrivateKeyWithHashAlg, PublicKey, PublicKeyOrCertificate};
use russh::{ChannelMsg, Disconnect};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

// Longest a user may think about a host-key prompt; also bounds a stalled handshake/auth
// via `inactivity_timeout`. Idle shells survive: keepalives count as activity.
const PROMPT_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, PartialEq, Eq)]
pub enum HostKeyStatus {
    Known,
    Unknown,
    Mismatch,
}

/// Stricter than `russh::keys::check_known_hosts_path`: a host with recorded keys whose
/// presented key matches none of them is a mismatch, even if the algorithm differs.
/// WHY: otherwise an attacker sidesteps the check by offering a different key type.
pub fn host_key_status(
    path: &Path,
    host: &str,
    port: u16,
    key: &PublicKey,
) -> AppResult<HostKeyStatus> {
    let recorded = known_host_keys_path(host, port, path).map_err(|e| {
        tracing::error!(error = ?e, "known_hosts unreadable");
        AppError::KnownHosts
    })?;
    Ok(if recorded.is_empty() {
        HostKeyStatus::Unknown
    } else if recorded.iter().any(|(_, k)| k.key_data() == key.key_data()) {
        HostKeyStatus::Known
    } else {
        HostKeyStatus::Mismatch
    })
}

/// Shown to the user before trusting a new host (TOFU).
#[derive(Serialize, Clone, Debug)]
pub struct HostKeyPrompt {
    pub host: String,
    pub port: u16,
    pub algorithm: String,
    pub fingerprint: String,
}

/// Asks the user; the receiver yields `true` to trust the key.
pub type Ask = Box<dyn Fn(HostKeyPrompt) -> oneshot::Receiver<bool> + Send + Sync>;

#[derive(Clone)]
pub enum Credential {
    Password(String),
    Key {
        path: PathBuf,
        passphrase: Option<String>,
    },
}

#[derive(Clone)]
pub struct Target {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub credential: Credential,
    pub known_hosts: PathBuf,
}

pub struct Client {
    host: String,
    port: u16,
    known_hosts: PathBuf,
    ask: Ask,
}

impl client::Handler for Client {
    type Error = AppError;

    async fn check_server_key(
        &mut self,
        server_key: &PublicKeyOrCertificate,
    ) -> Result<bool, AppError> {
        // shortcut: host certificates (CA-signed host keys) are refused; add @cert-authority support if needed.
        let PublicKeyOrCertificate::PublicKey { key, .. } = server_key else {
            return Err(AppError::Invalid(
                "server uses a host certificate, which is not supported yet",
            ));
        };
        let fingerprint = key.fingerprint(HashAlg::Sha256).to_string();
        match host_key_status(&self.known_hosts, &self.host, self.port, key)? {
            HostKeyStatus::Known => Ok(true),
            HostKeyStatus::Mismatch => {
                tracing::warn!(host = %self.host, port = self.port, %fingerprint, "host key mismatch");
                Err(AppError::HostKeyChanged(fingerprint))
            }
            HostKeyStatus::Unknown => {
                let rx = (self.ask)(HostKeyPrompt {
                    host: self.host.clone(),
                    port: self.port,
                    algorithm: key.algorithm().to_string(),
                    fingerprint,
                });
                match tokio::time::timeout(PROMPT_TIMEOUT, rx).await {
                    Ok(Ok(true)) => {
                        if let Some(dir) = self.known_hosts.parent() {
                            crate::store::create_private_dir(dir)?;
                        }
                        learn_known_hosts_path(&self.host, self.port, key, &self.known_hosts)
                            .map_err(|e| {
                                tracing::error!(error = ?e, "could not record host key");
                                AppError::KnownHosts
                            })?;
                        Ok(true)
                    }
                    _ => Err(AppError::HostKeyRejected),
                }
            }
        }
    }
}

fn load_key(path: &Path, passphrase: Option<&str>) -> AppResult<PrivateKey> {
    russh::keys::load_secret_key(path, passphrase).map_err(|e| match e {
        russh::keys::Error::KeyIsEncrypted => {
            AppError::Invalid("key is encrypted: save its passphrase first")
        }
        russh::keys::Error::IO(io) if io.kind() == std::io::ErrorKind::NotFound => {
            AppError::Invalid("key file not found")
        }
        other => {
            tracing::warn!(error = ?other, "key load failed");
            AppError::Invalid("could not read key: wrong passphrase or unsupported format")
        }
    })
}

/// TCP connect, verify host key, authenticate.
pub async fn connect(t: Target, ask: Ask) -> AppResult<Handle<Client>> {
    // Fail before touching the network if the key is unusable.
    let key = match &t.credential {
        Credential::Key { path, passphrase } => {
            Some(Arc::new(load_key(path, passphrase.as_deref())?))
        }
        Credential::Password(_) => None,
    };
    let config = Arc::new(client::Config {
        // A stalled auth or blackholed host now dies here instead of hanging forever;
        // post-auth idle shells are safe because keepalives count as activity.
        inactivity_timeout: Some(PROMPT_TIMEOUT),
        keepalive_interval: Some(Duration::from_secs(30)),
        keepalive_max: 3,
        ..Default::default()
    });
    let handler = Client {
        host: t.host.clone(),
        port: t.port,
        known_hosts: t.known_hosts.clone(),
        ask,
    };
    // WHY: this must cover the interactive host-key prompt, so it cannot be a short 10s;
    // a dead network is bounded by `inactivity_timeout` above instead.
    let stream = tokio::time::timeout(
        PROMPT_TIMEOUT,
        tokio::net::TcpStream::connect((t.host.as_str(), t.port)),
    )
    .await
    .map_err(|_| AppError::Timeout)?
    .map_err(AppError::from)?;
    run_handshake(config, stream, handler, t, key).await
}

/// Same handshake and auth over an already-open stream (Cloudflare tunnel).
/// Host-key identity is still `t.host`/`t.port` in known_hosts.
pub async fn connect_on<R>(t: Target, ask: Ask, stream: R) -> AppResult<Handle<Client>>
where
    R: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    // Fail before touching the network if the key is unusable.
    let key = match &t.credential {
        Credential::Key { path, passphrase } => {
            Some(Arc::new(load_key(path, passphrase.as_deref())?))
        }
        Credential::Password(_) => None,
    };
    let config = Arc::new(client::Config {
        inactivity_timeout: Some(PROMPT_TIMEOUT),
        keepalive_interval: Some(Duration::from_secs(30)),
        keepalive_max: 3,
        ..Default::default()
    });
    let handler = Client {
        host: t.host.clone(),
        port: t.port,
        known_hosts: t.known_hosts.clone(),
        ask,
    };
    run_handshake(config, stream, handler, t, key).await
}

/// Shared tail of `connect`/`connect_on`: handshake, then authenticate.
async fn run_handshake<R>(
    config: Arc<client::Config>,
    stream: R,
    handler: Client,
    t: Target,
    key: Option<Arc<PrivateKey>>,
) -> AppResult<Handle<Client>>
where
    R: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let mut handle = client::connect_stream(config, stream, handler).await?;

    let res = match (&t.credential, key) {
        (Credential::Password(pw), _) => handle.authenticate_password(&t.username, pw).await?,
        (Credential::Key { .. }, Some(key)) => {
            let hash = handle.best_supported_rsa_hash().await?.flatten();
            handle
                .authenticate_publickey(&t.username, PrivateKeyWithHashAlg::new(key, hash))
                .await?
        }
        (Credential::Key { .. }, None) => unreachable!("key loaded above"),
    };
    if !res.success() {
        return Err(AppError::AuthFailed);
    }
    Ok(handle)
}

pub enum Cmd {
    Data(Vec<u8>),
    Resize {
        cols: u32,
        rows: u32,
    },
    Close,
    /// One-shot command on a fresh exec channel; the reply carries its captured output.
    Exec {
        command: String,
        reply: oneshot::Sender<AppResult<String>>,
    },
}

const EXEC_TIMEOUT: Duration = Duration::from_secs(5);

/// Run a command on a separate exec channel of the same connection. Its output is
/// captured for the caller and never reaches the user's PTY.
pub async fn run_capture(handle: &Handle<Client>, command: &str) -> AppResult<String> {
    let mut ch = handle.channel_open_session().await?;
    ch.exec(true, command.as_bytes().to_vec()).await?;
    let mut out = Vec::new();
    let _ = tokio::time::timeout(EXEC_TIMEOUT, async {
        while let Some(msg) = ch.wait().await {
            match msg {
                ChannelMsg::Data { data } => out.extend_from_slice(&data),
                ChannelMsg::Eof | ChannelMsg::Close => break,
                _ => {}
            }
        }
    })
    .await;
    Ok(String::from_utf8_lossy(&out).into_owned())
}

/// Open a PTY shell and pump data until either side closes. `out` gets raw terminal bytes.
/// WHY Arc: SFTP and future channel users share this handle from other tasks;
/// `Handle` itself is not `Clone` (it owns the reply receiver).
pub async fn run_shell(
    handle: std::sync::Arc<Handle<Client>>,
    cols: u32,
    rows: u32,
    mut cmds: mpsc::UnboundedReceiver<Cmd>,
    mut out: impl FnMut(Vec<u8>) + Send,
) -> AppResult<()> {
    let mut ch = handle.channel_open_session().await?;
    ch.request_pty(false, "xterm-256color", cols, rows, 0, 0, &[])
        .await?;
    ch.request_shell(false).await?;

    let result = loop {
        tokio::select! {
            cmd = cmds.recv() => match cmd {
                Some(Cmd::Data(d)) => if let Err(e) = ch.data_bytes(d).await { break Err(e.into()) },
                Some(Cmd::Resize { cols, rows }) => if let Err(e) = ch.window_change(cols, rows, 0, 0).await { break Err(e.into()) },
                Some(Cmd::Exec { command, reply }) => {
                    // Inline await: the shell PTY buffers briefly (channel windowing) while
                    // the exec runs on a separate channel of the shared handle.
                    let _ = reply.send(run_capture(&handle, &command).await);
                }
                Some(Cmd::Close) | None => break Ok(()),
            },
            msg = ch.wait() => match msg {
                Some(ChannelMsg::Data { data }) | Some(ChannelMsg::ExtendedData { data, .. }) => out(data.to_vec()),
                Some(ChannelMsg::Eof | ChannelMsg::Close) | None => break Ok(()),
                Some(_) => {}
            },
        }
    };
    let _ = handle.disconnect(Disconnect::ByApplication, "", "en").await;
    result
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use russh::keys::decode_secret_key;

    /// In-process SSH server + authenticated handle, for other modules' tests.
    pub(crate) async fn test_handle(d: &tempfile::TempDir) -> Handle<Client> {
        let kh = d.path().join(".ssh/known_hosts");
        let port = serve_once(HOST_A).await;
        // TOFU: answer the first-use prompt, like the interactive flow does.
        connect(target(port, "right", &kh), answer(true))
            .await
            .unwrap()
    }
    use russh::server::{self, Auth, Msg, Session};
    use russh::{Channel, ChannelId};
    use std::sync::atomic::{AtomicBool, Ordering};

    const HOST_A: &str = include_str!("../tests/fixtures/host_a");
    const HOST_B: &str = include_str!("../tests/fixtures/host_b");
    const HOST_ECDSA_PUB: &str = include_str!("../tests/fixtures/host_ecdsa.pub");
    const CLIENT_ENC: &str = include_str!("../tests/fixtures/client_enc");

    fn pubkey(private: &str) -> PublicKey {
        decode_secret_key(private, None)
            .unwrap()
            .public_key()
            .clone()
    }

    #[derive(Clone)]
    struct EchoServer;

    impl server::Handler for EchoServer {
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
            _s: &mut Session,
        ) -> Result<(), Self::Error> {
            reply.accept().await;
            Ok(())
        }
        async fn data(
            &mut self,
            ch: ChannelId,
            data: &[u8],
            s: &mut Session,
        ) -> Result<(), Self::Error> {
            s.data(ch, data.to_vec())?;
            Ok(())
        }
        // Accept direct-tcpip so the forward listener has a peer to talk to;
        // the global `data` echo above stands in for the remote target.
        async fn channel_open_direct_tcpip(
            &mut self,
            _channel: Channel<Msg>,
            _host: &str,
            _port: u32,
            _originator: &str,
            _originator_port: u32,
            reply: server::ChannelOpenHandle,
            _s: &mut Session,
        ) -> Result<(), Self::Error> {
            reply.accept().await;
            Ok(())
        }
    }

    /// One-connection sshd on 127.0.0.1 presenting `host_key`. Returns the port.
    async fn serve_once(host_key: &str) -> u16 {
        let config = Arc::new(server::Config {
            keys: vec![decode_secret_key(host_key, None).unwrap()],
            auth_rejection_time: Duration::from_millis(1),
            auth_rejection_time_initial: Some(Duration::ZERO),
            ..Default::default()
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let (sock, _) = listener.accept().await.unwrap();
            if let Ok(running) = server::run_stream(config, sock, EchoServer).await {
                let _ = running.await;
            }
        });
        port
    }

    fn target(port: u16, pw: &str, known_hosts: &Path) -> Target {
        Target {
            host: "127.0.0.1".into(),
            port,
            username: "leo".into(),
            credential: Credential::Password(pw.into()),
            known_hosts: known_hosts.to_path_buf(),
        }
    }

    fn answer(accept: bool) -> Ask {
        Box::new(move |_| {
            let (tx, rx) = oneshot::channel();
            tx.send(accept).unwrap();
            rx
        })
    }

    fn must_not_ask() -> Ask {
        Box::new(|p| panic!("unexpected host key prompt: {p:?}"))
    }

    #[test]
    fn status_unknown_known_mismatch_and_other_algorithm() {
        let d = tempfile::tempdir().unwrap();
        let kh = d.path().join("known_hosts");
        let (a, b) = (pubkey(HOST_A), pubkey(HOST_B));
        assert_eq!(
            host_key_status(&kh, "h", 2222, &a).unwrap(),
            HostKeyStatus::Unknown
        );

        learn_known_hosts_path("h", 2222, &a, &kh).unwrap();
        assert_eq!(
            host_key_status(&kh, "h", 2222, &a).unwrap(),
            HostKeyStatus::Known
        );
        assert_eq!(
            host_key_status(&kh, "h", 2222, &b).unwrap(),
            HostKeyStatus::Mismatch
        );
        // Same host on another port is a different entry.
        assert_eq!(
            host_key_status(&kh, "h", 22, &a).unwrap(),
            HostKeyStatus::Unknown
        );

        // Only an ECDSA key recorded; presenting ed25519 must not fall back to "unknown".
        let kh2 = d.path().join("kh2");
        let ecdsa = PublicKey::from_openssh(HOST_ECDSA_PUB.trim()).unwrap();
        learn_known_hosts_path("h", 22, &ecdsa, &kh2).unwrap();
        assert_eq!(
            host_key_status(&kh2, "h", 22, &a).unwrap(),
            HostKeyStatus::Mismatch
        );
    }

    #[tokio::test]
    async fn rejected_prompt_aborts_and_records_nothing() {
        let d = tempfile::tempdir().unwrap();
        let kh = d.path().join(".ssh/known_hosts");
        let port = serve_once(HOST_A).await;
        let err = connect(target(port, "right", &kh), answer(false))
            .await
            .err()
            .unwrap();
        assert!(matches!(err, AppError::HostKeyRejected), "{err:?}");
        assert!(!kh.exists());
    }

    #[tokio::test]
    async fn accepted_key_is_learned_then_trusted_without_prompt() {
        let d = tempfile::tempdir().unwrap();
        let kh = d.path().join(".ssh/known_hosts");

        let port = serve_once(HOST_A).await;
        let asked = Arc::new(AtomicBool::new(false));
        let flag = asked.clone();
        let ask: Ask = Box::new(move |p| {
            assert!(p.fingerprint.starts_with("SHA256:"));
            flag.store(true, Ordering::SeqCst);
            let (tx, rx) = oneshot::channel();
            tx.send(true).unwrap();
            rx
        });
        connect(target(port, "right", &kh), ask).await.unwrap();
        assert!(asked.load(Ordering::SeqCst));
        assert!(
            std::fs::read_to_string(&kh)
                .unwrap()
                .contains(&format!("[127.0.0.1]:{port} ssh-ed25519"))
        );

        // Same key on the same port: known, so asking would panic.
        let line = std::fs::read_to_string(&kh).unwrap();
        let port2 = serve_once(HOST_A).await;
        std::fs::write(&kh, line.replace(&port.to_string(), &port2.to_string())).unwrap();
        let handle = connect(target(port2, "right", &kh), must_not_ask())
            .await
            .unwrap();

        let (tx, rx) = mpsc::unbounded_channel();
        let (out_tx, mut out_rx) = mpsc::unbounded_channel();
        let pump = tokio::spawn(run_shell(std::sync::Arc::new(handle), 80, 24, rx, move |b| {
            let _ = out_tx.send(b);
        }));
        tx.send(Cmd::Data(b"ping".to_vec())).unwrap();
        assert_eq!(out_rx.recv().await.unwrap(), b"ping");
        tx.send(Cmd::Close).unwrap();
        pump.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn changed_key_is_refused_without_prompt() {
        let d = tempfile::tempdir().unwrap();
        let kh = d.path().join("known_hosts");
        let port = serve_once(HOST_B).await;
        learn_known_hosts_path("127.0.0.1", port, &pubkey(HOST_A), &kh).unwrap();
        let err = connect(target(port, "right", &kh), must_not_ask())
            .await
            .err()
            .unwrap();
        assert!(matches!(err, AppError::HostKeyChanged(_)), "{err:?}");
    }

    #[tokio::test]
    async fn wrong_password_is_auth_failed() {
        let d = tempfile::tempdir().unwrap();
        let kh = d.path().join("known_hosts");
        let port = serve_once(HOST_A).await;
        learn_known_hosts_path("127.0.0.1", port, &pubkey(HOST_A), &kh).unwrap();
        let err = connect(target(port, "wrong", &kh), must_not_ask())
            .await
            .err()
            .unwrap();
        assert!(matches!(err, AppError::AuthFailed), "{err:?}");
    }

    #[test]
    fn encrypted_key_needs_correct_passphrase() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("id");
        std::fs::write(&p, CLIENT_ENC).unwrap();
        assert!(load_key(&p, Some("test-pass")).is_ok());
        assert!(
            matches!(load_key(&p, None), Err(AppError::Invalid(m)) if m.contains("passphrase"))
        );
        assert!(matches!(
            load_key(&p, Some("nope")),
            Err(AppError::Invalid(_))
        ));
        assert!(
            matches!(load_key(&d.path().join("missing"), None), Err(AppError::Invalid(m)) if m.contains("not found"))
        );
    }

    #[tokio::test]
    async fn local_forward_pipes_bytes_end_to_end() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let d = tempfile::tempdir().unwrap();
        let kh = d.path().join(".ssh/known_hosts");
        let port = serve_once(HOST_A).await;
        let handle = Arc::new(connect(target(port, "right", &kh), answer(true)).await.unwrap());

        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let local_port = listener.local_addr().unwrap().port();
        let progress = tauri::ipc::Channel::new(|_| Ok(()));
        let fw = tokio::spawn(crate::forward::run_listener(
            handle,
            crate::store::PortForward {
                local_port,
                remote_host: "db.internal".into(),
                remote_port: 5432,
            },
            progress,
            listener,
        ));

        let mut tcp = tokio::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, local_port))
            .await
            .unwrap();
        tcp.write_all(b"ping").await.unwrap();
        let mut buf = [0u8; 4];
        tcp.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"ping");
        fw.abort();
    }
}
