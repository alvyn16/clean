use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use base64::Engine;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::secrets;

const AUTH_ENDPOINT: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";
const SCOPE: &str = "openid email profile https://mail.google.com/";

pub const CLIENT_SECRET_KIND: &str = "google_client_secret";

#[derive(Debug, Clone)]
pub struct GoogleSession {
    pub email: String,
    pub picture: Option<String>,
    pub refresh_token: String,
    pub access_token: String,
    pub expires_at: i64,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
    id_token: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Deserialize)]
struct IdTokenClaims {
    email: Option<String>,
    picture: Option<String>,
}

fn id_token_claims(id_token: &str) -> Option<IdTokenClaims> {
    let payload = id_token.split('.').nth(1)?;
    let raw = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .ok()?;
    serde_json::from_slice(&raw).ok()
}
fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn verifier() -> String {
    let mut bytes = Vec::with_capacity(32);
    bytes.extend_from_slice(uuid::Uuid::new_v4().as_bytes());
    bytes.extend_from_slice(uuid::Uuid::new_v4().as_bytes());
    b64(&bytes)
}

fn challenge(verifier: &str) -> String {
    b64(&Sha256::digest(verifier.as_bytes()))
}

fn email_from_id_token(id_token: &str) -> Option<String> {
    id_token_claims(id_token)?.email
}

fn http() -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .context("preparing the OAuth HTTP client")
}

fn exchange(
    client: &reqwest::blocking::Client,
    client_id: &str,
    client_secret: &str,
    code: &str,
    redirect_uri: &str,
    verifier: &str,
) -> Result<TokenResponse> {
    let response = client
        .post(TOKEN_ENDPOINT)
        .form(&[
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("code", code),
            ("code_verifier", verifier),
            ("grant_type", "authorization_code"),
            ("redirect_uri", redirect_uri),
        ])
        .send()
        .context("exchanging the Google sign-in code")?;

    let body: TokenResponse = response
        .json()
        .context("reading the Google token response")?;

    if let Some(error) = body.error.as_deref() {
        return Err(anyhow!(
            "Google sign-in failed: {}",
            body.error_description.as_deref().unwrap_or(error)
        ));
    }
    Ok(body)
}

fn refresh(
    client: &reqwest::blocking::Client,
    client_id: &str,
    client_secret: &str,
    refresh_token: &str,
) -> Result<TokenResponse> {
    let response = client
        .post(TOKEN_ENDPOINT)
        .form(&[
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
        ])
        .send()
        .context("refreshing the Google access token")?;

    let body: TokenResponse = response
        .json()
        .context("reading the Google refresh response")?;

    if let Some(error) = body.error.as_deref() {
        return Err(anyhow!(
            "Google token refresh failed: {}",
            body.error_description.as_deref().unwrap_or(error)
        ));
    }
    Ok(body)
}

/// A prepared Google OAuth flow. Creating one opens the browser right away;
/// `finish` then waits for the redirect in the background.
pub struct GoogleAuthFlow {
    listener: TcpListener,
    verifier: String,
    state: String,
    redirect_uri: String,
}

/// Build the consent URL and open the browser immediately (call on the UI thread).
pub fn begin_google_auth(client_id: &str) -> Result<GoogleAuthFlow> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).context("opening the local OAuth port")?;
    let port = listener.local_addr()?.port();
    let redirect_uri = format!("http://127.0.0.1:{port}");
    let verifier = verifier();
    let challenge = challenge(&verifier);
    let state = uuid::Uuid::new_v4().to_string();

    let mut url = url::Url::parse(AUTH_ENDPOINT)?;
    url.query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", &redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", SCOPE)
        .append_pair("access_type", "offline")
        .append_pair("prompt", "consent")
        .append_pair("login_hint", "me")
        .append_pair("code_challenge", &challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", &state);

    println!("Open this URL to sign in with Google:\n\n{}\n", url);
    let _ = std::process::Command::new("xdg-open").arg(url.as_str()).spawn();

    Ok(GoogleAuthFlow {
        listener,
        verifier,
        state,
        redirect_uri,
    })
}

impl GoogleAuthFlow {
    /// Wait for the browser redirect and exchange the code for tokens.
    pub fn finish(
        self,
        client_id: &str,
        client_secret: &str,
        cancel: Option<Arc<AtomicBool>>,
    ) -> Result<GoogleSession> {
        let (code, returned_state) =
            wait_for_code(&self.listener, Duration::from_secs(120), cancel)?;
        if returned_state.as_deref() != Some(self.state.as_str()) {
            return Err(anyhow!("the Google sign-in response had a mismatched state"));
        }

        let client = http()?;
        let tokens = exchange(
            &client,
            client_id,
            client_secret,
            &code,
            &self.redirect_uri,
            &self.verifier,
        )?;

        let access_token = tokens
            .access_token
            .ok_or_else(|| anyhow!("Google did not return an access token"))?;
        let refresh_token = tokens
            .refresh_token
            .ok_or_else(|| anyhow!("Google did not return a refresh token; try signing in again"))?;
        let claims = tokens
            .id_token
            .as_deref()
            .and_then(id_token_claims)
            .ok_or_else(|| anyhow!("Google did not return an ID token"))?;
        let email = claims
            .email
            .ok_or_else(|| anyhow!("Google did not return an email address"))?;
        let picture = claims.picture;
        let expires_at = now() + tokens.expires_in.unwrap_or(3600);

        Ok(GoogleSession {
            email,
            picture,
            refresh_token,
            access_token,
            expires_at,
        })
    }
}

/// Run the loopback OAuth flow (used by the CLI). Opens the browser immediately.
pub fn connect_google(
    client_id: &str,
    client_secret: &str,
    cancel: Option<Arc<AtomicBool>>,
) -> Result<GoogleSession> {
    let flow = begin_google_auth(client_id)?;
    flow.finish(client_id, client_secret, cancel)
}

fn wait_for_code(
    listener: &TcpListener,
    timeout: Duration,
    cancel: Option<Arc<AtomicBool>>,
) -> Result<(String, Option<String>)> {
    listener.set_nonblocking(true)?;
    let deadline = Instant::now() + timeout;

    while Instant::now() < deadline {
        if cancel.as_ref().is_some_and(|flag| flag.load(Ordering::Relaxed)) {
            return Err(anyhow!("Google sign-in was cancelled"));
        }
        match listener.accept() {
            Ok((mut stream, _)) => {
                stream.set_nonblocking(false).ok();
                stream.set_read_timeout(Some(Duration::from_secs(5))).ok();

                let mut buffer = [0u8; 8192];
                let read = stream.read(&mut buffer).unwrap_or(0);
                let request = String::from_utf8_lossy(&buffer[..read]);
                let target = request
                    .lines()
                    .next()
                    .and_then(|line| line.split_whitespace().nth(1))
                    .unwrap_or("/");

                let parsed = url::Url::parse(&format!("http://127.0.0.1{target}")).ok();
                let mut code = None;
                let mut state = None;
                if let Some(parsed) = parsed {
                    for (key, value) in parsed.query_pairs() {
                        match key.as_ref() {
                            "code" => code = Some(value.into_owned()),
                            "state" => state = Some(value.into_owned()),
                            _ => {}
                        }
                    }
                }

                let ok = code.is_some();
                let body = if ok {
                    "<h1>You're signed in.</h1><p>Return to clean to finish setting up.</p>"
                } else {
                    "<h1>Sign-in was cancelled.</h1><p>Return to clean and try again.</p>"
                };
                let page = format!(
                    "<!doctype html><html><head><meta charset=\"utf-8\"><title>clean</title></head>\
                     <body style=\"margin:0;display:grid;place-items:center;height:100vh;\
                     background:#181d20;color:#d3c6aa;font-family:system-ui,sans-serif\">\
                     <div style=\"text-align:center\">{body}</div></body></html>"
                );
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    page.len(),
                    page
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();

                if let Some(code) = code {
                    return Ok((code, state));
                }
                return Err(anyhow!("Google sign-in was cancelled"));
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(error) => return Err(error.into()),
        }
    }

    Err(anyhow!("timed out waiting for the Google sign-in"))
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

/// A valid access token for an account, refreshed and cached when needed.
pub fn access_token(
    account_id: &str,
    client_id: &str,
    client_secret: &str,
    refresh_token: &str,
) -> Result<String> {
    let cached = secrets::get(account_id, "oauth_access")?;
    let expiry = secrets::get(account_id, "oauth_expiry")?
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(0);

    if let Some(token) = cached
        && now() < expiry - 60 {
            return Ok(token);
        }

    let client = http()?;
    let tokens = refresh(&client, client_id, client_secret, refresh_token)?;
    let access = tokens
        .access_token
        .ok_or_else(|| anyhow!("Google did not return an access token"))?;
    let expires_at = now() + tokens.expires_in.unwrap_or(3600);

    secrets::set(account_id, "oauth_access", &access)?;
    secrets::set(account_id, "oauth_expiry", &expires_at.to_string())?;

    Ok(access)
}

/// Fetch the Google profile photo URL for an already-connected account.
pub fn fetch_profile_picture(
    account_id: &str,
    client_id: &str,
    client_secret: &str,
    refresh_token: &str,
) -> Result<Option<String>> {
    let token = access_token(account_id, client_id, client_secret, refresh_token)?;
    #[derive(Deserialize)]
    struct UserInfo {
        picture: Option<String>,
    }
    let info: UserInfo = http()?
        .get("https://www.googleapis.com/oauth2/v3/userinfo")
        .bearer_auth(token)
        .send()?
        .error_for_status()?
        .json()?;
    Ok(info.picture)
}

/// Download an avatar to a local cache file so GPUI can load it (GIF/WebP included).
pub fn cache_avatar(account_id: &str, url: &str) -> Result<std::path::PathBuf> {
    use std::io::Write;
    let bytes = http()?.get(url).send()?.error_for_status()?.bytes()?.to_vec();
    let path = crate::config::data_dir().join("avatars").join(format!("{account_id}.img"));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::File::create(&path)?;
    file.write_all(&bytes)?;
    Ok(path)
}

pub fn client_secret() -> Result<Option<String>> {
    secrets::get(secrets::APP, CLIENT_SECRET_KIND)
}

pub fn store_client_secret(secret: &str) -> Result<()> {
    secrets::set(secrets::APP, CLIENT_SECRET_KIND, secret)
}

pub fn forget(account_id: &str) {
    let _ = secrets::delete(account_id, "oauth_access");
    let _ = secrets::delete(account_id, "oauth_expiry");
    let _ = secrets::delete(account_id, "oauth_refresh");
}
