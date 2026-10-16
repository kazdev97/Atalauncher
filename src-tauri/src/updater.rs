use crate::downloader::{download_file, get_json, http_client};
use serde::Serialize;
use std::fs;
use std::time::Duration;
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const UPDATE_METADATA_URL: &str =
    "https://raw.githubusercontent.com/kazdev97/Atalauncher/main/version.json";

#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    pub current: String,
    pub latest: String,
    pub available: bool,
    pub notes: String,
    pub download_url: String,
    pub error: Option<String>,
}

#[derive(serde::Deserialize)]
struct RemoteMeta {
    #[serde(default)]
    version: String,
    #[serde(default)]
    notes: String,
    #[serde(default)]
    url: String,
}
pub fn version_gt(a: &str, b: &str) -> bool {
    let num = |s: &str| -> Vec<i64> {
        s.split(|c: char| !c.is_ascii_digit())
            .filter_map(|p| p.parse().ok())
            .collect()
    };
    let an = num(a);
    let bn = num(b);
    let n = an.len().max(bn.len());
    for i in 0..n {
        let x = *an.get(i).unwrap_or(&0);
        let y = *bn.get(i).unwrap_or(&0);
        if x != y {
            return x > y;
        }
    }
    false
}
pub async fn check_update() -> Result<UpdateInfo, String> {
    let client = http_client();
    let meta: RemoteMeta =
        get_json(&client, UPDATE_METADATA_URL, Duration::from_secs(30)).await.map_err(|e| {
            format!("no se pudo consultar versiones ({UPDATE_METADATA_URL}): {e}")
        })?;
    let latest = meta.version.trim().to_string();
    let available = !latest.is_empty() && version_gt(&latest, APP_VERSION);
    Ok(UpdateInfo {
        current: APP_VERSION.to_string(),
        latest,
        available,
        notes: meta.notes,
        download_url: meta.url,
        error: None,
    })
}
pub async fn apply_update() -> Result<(), String> {
    let info = check_update().await?;
    if !info.available || info.download_url.trim().is_empty() {
        return Err("No hay una versión más reciente para instalar.".to_string());
    }

    let updates_dir = crate::config::launcher_data_dir().join("updates");
    fs::create_dir_all(&updates_dir).map_err(|e| format!("crear carpeta de updates: {e}"))?;
    let new_exe = updates_dir.join(format!("atalauncher-{}.new.exe", info.latest));
    let client = http_client();
    download_file(&client, &info.download_url, &new_exe, &mut |_, _| {})
        .await
        .map_err(|e| format!("descargar la actualización: {e}"))?;
    let cur_exe = std::env::current_exe().map_err(|e| format!("localizar el exe actual: {e}"))?;
    // Guardia anti-bucle: si el binario descargado es idéntico al actual
    // (p. ej. version.json apunta al exe viejo), no reiniciar.
    if let (Ok(a), Ok(b)) = (
        crate::downloader::sha256_file(&new_exe),
        crate::downloader::sha256_file(&cur_exe),
    ) {
        if a == b {
            fs::remove_file(&new_exe).ok();
            return Err("La actualización descargada es idéntica a la versión actual; se omite el reinicio.".to_string());
        }
    }
    let cur_str = cur_exe.to_string_lossy().replace('\'', "''");
    let new_str = new_exe.to_string_lossy().replace('\'', "''");
    let ps = format!(
        "Start-Sleep -Seconds 3; $ErrorActionPreference='Stop'; \
Stop-Process -Name 'atalauncher' -Force -ErrorAction SilentlyContinue; \
Start-Sleep -Milliseconds 800; \
Copy-Item -LiteralPath '{new_str}' -Destination '{cur_str}' -Force; \
Remove-Item -LiteralPath '{new_str}' -Force -ErrorAction SilentlyContinue; \
Start-Process -FilePath '{cur_str}'"
    );
    let script = updates_dir.join("apply_update.ps1");
    fs::write(&script, &ps).map_err(|e| format!("escribir script de actualización: {e}"))?;

    let mut cmd = std::process::Command::new("powershell");
    cmd.arg("-NoProfile")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-WindowStyle")
        .arg("Hidden")
        .arg("-File")
        .arg(&script);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd.spawn()
        .map_err(|e| format!("lanzar el instalador de la actualización: {e}"))?;

    Ok(())
}