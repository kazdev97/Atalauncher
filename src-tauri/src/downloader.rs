use reqwest::header::{HeaderMap, USER_AGENT};
use reqwest::Client;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::time::Duration;

pub const USER_AGENT_STRING: &str = "Atalauncher/0.1";

pub fn http_client() -> Client {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, USER_AGENT_STRING.parse().unwrap());
    Client::builder()
        .default_headers(headers)
        .connect_timeout(Duration::from_secs(20))
        .build()
        .expect("fail to build reqwest client")
}

pub type ProgressFn<'a> = Option<&'a mut (dyn FnMut(u64, u64) + Send)>;
pub async fn download_file(
    client: &Client,
    url: &str,
    dest: &std::path::Path,
    on_progress: &mut (dyn FnMut(u64, u64) + Send),
) -> Result<(), String> {
    let mut last_err: Option<String> = None;
    for attempt in 1..=4 {
        match download_one(client, url, dest, on_progress).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                last_err = Some(e.clone());
                if attempt < 4 {
                    tokio::time::sleep(Duration::from_millis(700 * attempt)).await;
                }
            }
        }
    }
    Err(last_err.unwrap_or_else(|| "descarga fallida".to_string()))
}

async fn download_one(
    client: &Client,
    url: &str,
    dest: &std::path::Path,
    on_progress: &mut (dyn FnMut(u64, u64) + Send),
) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("crear dir {e}"))?;
    }
    let tmp = dest.with_extension(format!(
        "tmp{}",
        std::process::id()
    ));
    let resp = client
        .get(url)
        .timeout(Duration::from_secs(300))
        .send()
        .await
        .map_err(|e| format!("request {url}: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!("HTTP {} para {url}", status.as_u16()));
    }
    let total = resp.content_length().unwrap_or(0);
    let mut file = File::create(&tmp).map_err(|e| format!("crear archivo: {e}"))?;
    let mut stream = resp.bytes_stream();
    let mut written: u64 = 0;
    use futures_util::StreamExt;
    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| format!("stream: {e}"))?;
        std::io::Write::write_all(&mut file, &bytes).map_err(|e| format!("write: {e}"))?;
        written += bytes.len() as u64;
        on_progress(written, total);
    }
    drop(file);
    std::fs::rename(&tmp, dest).map_err(|e| format!("guardar archivo: {e}"))?;
    Ok(())
}
pub async fn get_text(client: &Client, url: &str, timeout: Duration) -> Result<String, String> {
    let resp = client
        .get(url)
        .timeout(timeout)
        .send()
        .await
        .map_err(|e| format!("request {url}: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {} para {url}", resp.status().as_u16()));
    }
    resp.text()
        .await
        .map_err(|e| format!("leer respuesta {url}: {e}"))
}
pub async fn get_json<T: serde::de::DeserializeOwned>(
    client: &Client,
    url: &str,
    timeout: Duration,
) -> Result<T, String> {
    let resp = client
        .get(url)
        .timeout(timeout)
        .send()
        .await
        .map_err(|e| format!("request {url}: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {} para {url}", resp.status().as_u16()));
    }
    resp.json::<T>()
        .await
        .map_err(|e| format!("JSON {url}: {e}"))
}
pub async fn get_json_auth<T: serde::de::DeserializeOwned>(
    client: &Client,
    url: &str,
    token: &str,
    timeout: Duration,
) -> Result<T, String> {
    let resp = client
        .get(url)
        .bearer_auth(token)
        .timeout(timeout)
        .send()
        .await
        .map_err(|e| format!("request {url}: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {} para {url}", resp.status().as_u16()));
    }
    resp.json::<T>()
        .await
        .map_err(|e| format!("JSON {url}: {e}"))
}
pub async fn post_form_json<T: serde::de::DeserializeOwned>(
    client: &Client,
    url: &str,
    form: &[(&str, String)],
    timeout: Duration,
) -> Result<T, String> {
    let resp = client
        .post(url)
        .form(form)
        .timeout(timeout)
        .send()
        .await
        .map_err(|e| format!("post {url}: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {} para {url}", resp.status().as_u16()));
    }
    resp.json::<T>()
        .await
        .map_err(|e| format!("JSON {url}: {e}"))
}
pub async fn post_json<T: serde::de::DeserializeOwned>(
    client: &Client,
    url: &str,
    body: &serde_json::Value,
    timeout: Duration,
) -> Result<T, String> {
    let resp = client
        .post(url)
        .json(body)
        .timeout(timeout)
        .send()
        .await
        .map_err(|e| format!("post {url}: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {} para {url}", resp.status().as_u16()));
    }
    resp.json::<T>()
        .await
        .map_err(|e| format!("JSON {url}: {e}"))
}
pub async fn post_form_no_response(
    client: &Client,
    url: &str,
    form: &[(&str, String)],
    timeout: Duration,
) -> Result<(), String> {
    let resp = client
        .post(url)
        .form(form)
        .timeout(timeout)
        .send()
        .await
        .map_err(|e| format!("post {url}: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {} para {url}", resp.status().as_u16()));
    }
    Ok(())
}
pub fn sha256_file(path: &std::path::Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| format!("abrir para hash: {e}"))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let read = file
            .read(&mut buf)
            .map_err(|e| format!("leer para hash: {e}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}
pub fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
pub fn folder_size(path: &std::path::Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                total += folder_size(&p);
            } else if let Ok(md) = std::fs::metadata(&p) {
                total += md.len();
            }
        }
    }
    total
}