use crate::config::Account;
use crate::downloader::{http_client, post_form_json, post_json};
use base64::Engine;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
include!(concat!(env!("OUT_DIR"), "/client_id.rs"));
const TOKEN_ENDPOINT: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const AUTH_ENDPOINT: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize";
const SCOPE: &str = "XboxLive.signin offline_access";
const MINECRAFT_NOT_OWNED: &str = "__MINECRAFT_NOT_OWNED__";

#[derive(Debug, Clone, Serialize)]
pub struct AuthState {
    pub logged_in: bool,
    pub mode: String,
    pub name: String,
    pub uuid: String,
    pub has_refresh: bool,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    #[serde(default)]
    access_token: Option<String>,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    error_description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct XblResponse {
    #[serde(rename = "Token")]
    token: String,
    #[serde(rename = "DisplayClaims")]
    display_claims: XblClaims,
}

#[derive(Debug, Deserialize)]
struct XblClaims {
    xui: Vec<XblXui>,
}

#[derive(Debug, Deserialize)]
struct XblXui {
    uhs: String,
}

#[derive(Debug, Deserialize)]
struct XstsResponse {
    #[serde(rename = "Token")]
    #[serde(default)]
    token: Option<String>,
    #[serde(rename = "XErr")]
    #[serde(default)]
    xerr: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct McLoginResponse {
    #[serde(default)]
    access_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MinecraftProfile {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    error: Option<String>,
}
fn find_free_port() -> Result<u16, String> {
    for port in 8080u16..8090 {
        if let Ok(listener) = std::net::TcpListener::bind(("127.0.0.1", port)) {
            drop(listener);
            return Ok(port);
        }
    }
    Err("No hay puertos disponibles (8080-8089)".to_string())
}
fn pkce_pair() -> (String, String) {
    use sha2::{Digest, Sha256};
    let mut raw = [0u8; 48];
    rand::thread_rng().fill_bytes(&mut raw);
    let verifier = base64::engine::general_purpose::URL_SAFE
        .encode(raw)
        .trim_end_matches('=')
        .to_string();
    let digest = Sha256::digest(verifier.as_bytes());
    let challenge = base64::engine::general_purpose::URL_SAFE
        .encode(digest)
        .trim_end_matches('=')
        .to_string();
    (verifier, challenge)
}

fn build_auth_url(redirect_uri: &str, challenge: &str, state: &str, client_id: &str) -> String {
    let params = serde_urlencoded::to_string([
        ("client_id", client_id.to_string()),
        ("response_type", "code".to_string()),
        ("redirect_uri", redirect_uri.to_string()),
        ("response_mode", "query".to_string()),
        ("scope", SCOPE.to_string()),
        ("state", state.to_string()),
        ("prompt", "select_account".to_string()),
        ("code_challenge", challenge.to_string()),
        ("code_challenge_method", "S256".to_string()),
    ])
    .unwrap_or_default();
    format!("{AUTH_ENDPOINT}?{params}")
}
async fn wait_for_callback(port: u16) -> Result<(String, String, bool), String> {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .map_err(|e| format!("bind OAuth: {e}"))?;
    let html_ok = "<html><body><h3 style=\"font-family:sans-serif\">Login recibido. Ya puedes cerrar esta pestaña y volver al launcher.</h3></body></html>";
    let body = html_ok.as_bytes();
    let response_head = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let response = format!("{response_head}");
    let (mut socket, _) = listener
        .accept()
        .await
        .map_err(|e| format!("accept OAuth: {e}"))?;
    let mut buf = [0u8; 8192];
    let n = tokio::time::timeout(Duration::from_secs(30), socket.read(&mut buf))
        .await
        .map_err(|_| "timeout esperando redirect de Microsoft".to_string())?
        .map_err(|e| format!("leer OAuth: {e}"))?;
    let _ = socket.write_all(response.as_bytes()).await;
    let _ = tokio::time::sleep(Duration::from_millis(200)).await;
    let _ = socket.write_all(body).await;
    let _ = socket.shutdown().await;
    let request = String::from_utf8_lossy(&buf[..n]).to_string();
    let first_line = request.lines().next().unwrap_or("");
    let mut tokens = first_line.split_whitespace();
    let _method = tokens.next();
    let path = tokens.next().unwrap_or("");
    let url = url::Url::parse(&format!("http://localhost:{port}{path}"))
        .map_err(|e| format!("parse callback url ({first_line}): {e}"))?;
    let query: std::collections::HashMap<String, String> =
        url.query_pairs().into_owned().collect();
    let code = query.get("code").cloned().unwrap_or_default();
    let state = query.get("state").cloned().unwrap_or_default();
    let has_error = query.get("error").is_some();
    if code.is_empty() && !has_error {
        return Err("Microsoft no devolvió un código de autorización.".to_string());
    }
    Ok((code, state, has_error))
}
async fn fetch_minecraft_profile(msa_access_token: &str) -> Result<Account, String> {
    let client = http_client();
    let xbl_body = serde_json::json!({
        "Properties": {
            "AuthMethod": "RPS",
            "SiteName": "user.auth.xboxlive.com",
            "RpsTicket": format!("d={msa_access_token}")
        },
        "RelyingParty": "http://auth.xboxlive.com",
        "TokenType": "JWT"
    });
    let xbl: XblResponse = post_json(
        &client,
        "https://user.auth.xboxlive.com/user/authenticate",
        &xbl_body,
        Duration::from_secs(20),
    )
    .await
    .map_err(|e| format!("XBL auth: {e}"))?;
    let userhash = xbl
        .display_claims
        .xui
        .first()
        .map(|x| x.uhs.clone())
        .ok_or_else(|| "XBL: sin uhs".to_string())?;

    let xsts_body = serde_json::json!({
        "Properties": {
            "SandboxId": "RETAIL",
            "UserTokens": [xbl.token]
        },
        "RelyingParty": "rp://api.minecraftservices.com/",
        "TokenType": "JWT"
    });
    let xsts: XstsResponse = post_json(
        &client,
        "https://xsts.auth.xboxlive.com/xsts/authorize",
        &xsts_body,
        Duration::from_secs(20),
    )
    .await
    .map_err(|e| format!("XSTS auth: {e}"))?;
    let xsts_token = match &xsts.token {
        Some(t) => t.clone(),
        None => {
            let err = xsts.xerr.unwrap_or(0);
            if err == 2148916233 {
                return Err(MINECRAFT_NOT_OWNED.to_string());
            }
            return Err(format!("XSTS error (XErr={err})"));
        }
    };

    let mc_body = serde_json::json!({
        "identityToken": format!("XBL3.0 x={userhash};{xsts_token}")
    });
    let mc: McLoginResponse = post_json(
        &client,
        "https://api.minecraftservices.com/authentication/login_with_xbox",
        &mc_body,
        Duration::from_secs(20),
    )
    .await
    .map_err(|e| format!("Minecraft auth: {e}"))?;
    let mc_token = mc
        .access_token
        .ok_or_else(|| "Minecraft auth: sin access_token".to_string())?;

    let profile: MinecraftProfile = crate::downloader::get_json_auth(
        &client,
        "https://api.minecraftservices.com/minecraft/profile",
        &mc_token,
        Duration::from_secs(20),
    )
    .await
    .map_err(|e| format!("perfil Minecraft: {e}"))?;
    if profile.error.as_deref() == Some("NOT_FOUND") || profile.id.is_none() {
        return Err(MINECRAFT_NOT_OWNED.to_string());
    }
    Ok(Account {
        id: profile.id.unwrap_or_default(),
        name: profile.name.unwrap_or_default(),
        access_token: mc_token,
        refresh_token: String::new(),
        needs_relogin: false,
    })
}

pub async fn exchange_code(
    client_id: &str,
    redirect_uri: &str,
    code: &str,
    verifier: &str,
) -> Result<(Account, String), String> {
    let client = http_client();
    let form = [
        ("client_id", client_id.to_string()),
        ("scope", SCOPE.to_string()),
        ("code", code.to_string()),
        ("redirect_uri", redirect_uri.to_string()),
        ("grant_type", "authorization_code".to_string()),
        ("code_verifier", verifier.to_string()),
    ];
    let tok: TokenResponse = post_form_json(&client, TOKEN_ENDPOINT, &form, Duration::from_secs(20))
        .await
        .map_err(|e| {
            if e.contains("invalid_grant") {
                e
            } else {
                e
            }
        })?;
    let access = tok
        .access_token
        .ok_or_else(|| tok.error_description.clone().unwrap_or_else(|| "token_error".to_string()))?;
    let mut account = fetch_minecraft_profile(&access).await?;
    account.refresh_token = tok.refresh_token.unwrap_or_default();
    Ok((account, access))
}
pub async fn refresh_token(client_id: &str, refresh: &str) -> Result<(Account, String), String> {
    let client = http_client();
    let form = [
        ("client_id", client_id.to_string()),
        ("scope", SCOPE.to_string()),
        ("refresh_token", refresh.to_string()),
        ("grant_type", "refresh_token".to_string()),
    ];
    let tok: TokenResponse = post_form_json(&client, TOKEN_ENDPOINT, &form, Duration::from_secs(20))
        .await
        .map_err(|e| format!("refresh: {e}"))?;
    let access = tok.access_token.ok_or_else(|| {
        if tok.error.as_deref() == Some("invalid_grant") {
            "invalid_grant".to_string()
        } else {
            tok.error_description.unwrap_or_else(|| "token_error".to_string())
        }
    })?;
    let mut account = fetch_minecraft_profile(&access).await?;
    account.refresh_token = tok
        .refresh_token
        .unwrap_or_else(|| refresh.to_string());
    Ok((account, access))
}
pub async fn login(client_id: &str) -> Result<Account, String> {
    if client_id.trim().is_empty() {
        return Err("Atalauncher no tiene configurado su CLIENT_ID de Microsoft (falta client_id.env al compilar).".to_string());
    }
    let port = find_free_port()?;
    let redirect_uri = format!("http://localhost:{port}/callback");
    let state = uuid::Uuid::new_v4().simple().to_string();
    let (verifier, challenge) = pkce_pair();
    let auth_url = build_auth_url(&redirect_uri, &challenge, &state, client_id);
    let wait_handle = tokio::spawn(wait_for_callback(port));

    open::that(&auth_url).map_err(|e| format!("no se pudo abrir el navegador: {e}"))?;

    let (code, received_state, has_error) = wait_handle
        .await
        .map_err(|e| format!("callback server: {e}"))??;
    if has_error {
        return Err("login_cancelled".to_string());
    }
    if !state.is_empty() && received_state != state {
        return Err("state_mismatch".to_string());
    }
    let (account, _access) = exchange_code(client_id, &redirect_uri, &code, &verifier).await?;
    Ok(account)
}
pub async fn start_login_flow(_app: tauri::AppHandle) -> Result<Account, String> {
    let mut store = crate::config::AccountStore::load();
    if let Some(selected) = store.selected() {
        if !selected.refresh_token.is_empty() {
            if let Ok((account, _)) = refresh_token(CLIENT_ID, &selected.refresh_token).await {
                store.upsert(account.clone());
                let _ = store.save();
                return Ok(account);
            }
        }
    }
    let account = login(CLIENT_ID).await?;
    store.upsert(account.clone());
    if let Err(e) = store.save() {
        crate::launch::log_launch(&format!("[auth] no se pudo guardar la cuenta: {e}"));
    }
    Ok(account)
}