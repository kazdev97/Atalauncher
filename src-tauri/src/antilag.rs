use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Read;
use std::net::{IpAddr, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::Command;
use x25519_dalek::{PublicKey, StaticSecret};

pub const TASK_ON: &str = "AtalauncherAntilagOn";
pub const TASK_OFF: &str = "AtalauncherAntilagOff";
pub const SING_BOX_VERSION: &str = "1.12.7";
pub const TUN_GATEWAY: &str = "172.16.0.1";
pub const SING_BOX_URL: &str = "https://github.com/SagerNet/sing-box/releases/download/v1.12.7/sing-box-1.12.7-windows-amd64.zip";
pub const WINTUN_URL: &str = "https://www.wintun.net/builds/wintun-0.14.1.zip";
const API_BASE: &str = "https://api.cloudflareclient.com/v0a884";
pub fn antilag_dir() -> PathBuf {
    let dir = crate::config::launcher_data_dir().join("antilag");
    fs::create_dir_all(&dir).ok();
    dir
}

pub fn warp_file() -> PathBuf {
    antilag_dir().join("warp.json")
}
pub fn sing_box_exe() -> PathBuf {
    antilag_dir().join("sing-box.exe")
}
pub fn wintun_dll() -> PathBuf {
    antilag_dir().join("wintun.dll")
}
pub fn on_script() -> PathBuf {
    antilag_dir().join("antilag-on.cmd")
}
pub fn off_script() -> PathBuf {
    antilag_dir().join("antilag-off.cmd")
}
pub fn session_config() -> PathBuf {
    antilag_dir().join("config.json")
}
pub fn targets_file() -> PathBuf {
    antilag_dir().join("targets.txt")
}
#[derive(Clone, Serialize)]
pub struct AntilagStatus {
    pub installed: bool,
    pub tasks_registered: bool,
    pub enabled: bool,
    pub host: String,
    pub tunnel_active: bool,
}

pub fn status() -> AntilagStatus {
    let s = crate::config::Settings::load();
    AntilagStatus {
        installed: installed(),
        tasks_registered: tasks_registered(),
        enabled: s.antilag_enabled,
        host: s.antilag_host.clone(),
        tunnel_active: tunnel_active(),
    }
}

fn installed() -> bool {
    sing_box_exe().exists()
        && wintun_dll().exists()
        && warp_file().exists()
        && on_script().exists()
        && off_script().exists()
}

fn tasks_registered() -> bool {
    task_exists(TASK_ON) && task_exists(TASK_OFF)
}

fn task_exists(name: &str) -> bool {
    let expr = format!("(Get-ScheduledTask -TaskName '{name}' -ErrorAction SilentlyContinue) -ne $null");
    powershell_hidden()
        .args(["-NoProfile", "-Command"])
        .arg(&expr)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "True")
        .unwrap_or(false)
}
fn powershell_hidden() -> Command {
    let mut cmd = Command::new("powershell");
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd
}

pub fn tunnel_active() -> bool {
    powershell_hidden()
        .args(["-NoProfile", "-Command"])
        .arg("(Get-Process sing-box -ErrorAction SilentlyContinue) -ne $null")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "True")
        .unwrap_or(false)
}
#[derive(Serialize, Deserialize)]
struct WarpIdentity {
    id: String,
    token: String,
    private_key: String,
    address_v4: String,
    address_v6: String,
    peer_pubkey: String,
    endpoint_host: String,
    endpoint_v4: String,
}

fn load_identity() -> Option<WarpIdentity> {
    let text = fs::read_to_string(warp_file()).ok()?;
    serde_json::from_str(&text).ok()
}
async fn register_identity(
    prog: impl FnMut(u32, String),
) -> Result<WarpIdentity, String> {
    let mut prog = prog;
    if let Some(existing) = load_identity() {
        return Ok(existing);
    }
    prog(10, "Registrando conexión Cloudflare WARP (identidad)...".into());

    let client = reqwest::Client::builder()
        .user_agent("okhttp/3.12.1")
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("cliente HTTP: {e}"))?;

    let secret = StaticSecret::random_from_rng(rand::rngs::OsRng);
    let pubkey = PublicKey::from(&secret);
    let tos = Utc::now().format("%Y-%m-%dT%H:%M:%S%.f%:z").to_string();

    prog(30, "Solicitando cuenta free en Cloudflare...".into());
    let reg: serde_json::Value = client
        .post(format!("{API_BASE}/reg"))
        .header("cf-client-version", "a-6.10-1910")
        .json(&serde_json::json!({
            "install_id": "",
            "tos": tos,
            "key": B64.encode(pubkey.as_bytes()),
            "fcm_token": "",
            "type": "Android",
            "model": "PC",
            "locale": "en_US"
        }))
        .send()
        .await
        .map_err(|e| format!("registro Cloudflare: {e}"))?
        .error_for_status()
        .map_err(|e| format!("Cloudflare rechazó el registro: {e}"))?
        .json()
        .await
        .map_err(|e| format!("respuesta de registro inválida: {e}"))?;

    let id = reg["id"]
        .as_str()
        .ok_or_else(|| "registro sin id".to_string())?
        .to_string();
    let token = reg["token"]
        .as_str()
        .ok_or_else(|| "registro sin token".to_string())?
        .to_string();

    prog(55, "Activando modo WARP (bypass)...".into());
    let patched: serde_json::Value = client
        .patch(format!("{API_BASE}/reg/{id}"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({ "warp_enabled": true }))
        .send()
        .await
        .map_err(|e| format!("activar WARP: {e}"))?
        .error_for_status()
        .map_err(|e| format!("Cloudflare rechazó warp_enabled: {e}"))?
        .json()
        .await
        .map_err(|e| format!("respuesta warp_enabled inválida: {e}"))?;
    if patched["warp_enabled"].as_bool() != Some(true) {
        return Err("Cloudflare no activó el modo WARP".to_string());
    }

    prog(75, "Descargando configuración del túnel...".into());
    let conf: serde_json::Value = client
        .get(format!("{API_BASE}/reg/{id}"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| format!("config WARP: {e}"))?
        .error_for_status()
        .map_err(|e| format!("error al leer la config: {e}"))?
        .json()
        .await
        .map_err(|e| format!("config inválida: {e}"))?;

    let addr = &conf["config"]["interface"]["addresses"];
    let peer = &conf["config"]["peers"][0];
    let endpoint = &peer["endpoint"];

    let ident = WarpIdentity {
        id,
        token,
        private_key: B64.encode(secret.to_bytes()),
        address_v4: addr["v4"]
            .as_str()
            .ok_or_else(|| "sin ipv4 de asignación".to_string())?
            .to_string(),
        address_v6: addr["v6"]
            .as_str()
            .ok_or_else(|| "sin ipv6 de asignación".to_string())?
            .to_string(),
        peer_pubkey: peer["public_key"]
            .as_str()
            .ok_or_else(|| "sin public key del peer".to_string())?
            .to_string(),
        endpoint_host: endpoint["host"]
            .as_str()
            .unwrap_or("engage.cloudflareclient.com")
            .to_string(),
        endpoint_v4: endpoint["v4"]
            .as_str()
            .unwrap_or("162.159.192.1")
            .split(':')
            .next()
            .unwrap_or("162.159.192.1")
            .to_string(),
    };

    let json = serde_json::to_string_pretty(&ident)
        .map_err(|e| format!("serializar identidad: {e}"))?;
    fs::write(warp_file(), json).map_err(|e| format!("guardar warp.json: {e}"))?;
    prog(100, "Identidad WARP lista.".into());
    Ok(ident)
}
async fn download_to_file(url: &str, dest: &Path) -> Result<(), String> {
    let mut resp = reqwest::Client::new()
        .get(url)
        .send()
        .await
        .map_err(|e| format!("descargar {url}: {e}"))?
        .error_for_status()
        .map_err(|e| format!("HTTP {} en {url}", e))?;
    let mut buf = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(|e| format!("leer chunk: {e}"))? {
        buf.extend_from_slice(&chunk);
    }
    fs::write(dest, &buf).map_err(|e| format!("guardar {:?}: {e}", dest))
}

pub async fn ensure_engine(mut prog: impl FnMut(u32, String)) -> Result<(), String> {
    if sing_box_exe().exists() && wintun_dll().exists() {
        return Ok(());
    }
    let dir = antilag_dir();
    let zip_path = dir.join("engine.zip");
    let wintun_zip = dir.join("wintun.zip");

    if !sing_box_exe().exists() {
        prog(5, "Descargando motor de túnel (sing-box)...".into());
        download_to_file(SING_BOX_URL, &zip_path).await?;
        prog(65, "Extrayendo sing-box...".into());
        extract_entry(&zip_path, "sing-box.exe", &sing_box_exe())?;
        let _ = fs::remove_file(&zip_path);
    }

    if !wintun_dll().exists() {
        prog(70, "Descargando driver de red (wintun)...".into());
        download_to_file(WINTUN_URL, &wintun_zip).await?;
        prog(90, "Extrayendo wintun...".into());
        extract_entry(&wintun_zip, "wintun.dll", &wintun_dll())?;
        let _ = fs::remove_file(&wintun_zip);
    }

    prog(100, "Motor de túnel listo.".into());
    Ok(())
}
fn extract_entry(zip_path: &Path, suffix: &str, dest: &Path) -> Result<(), String> {
    let file = fs::File::open(zip_path).map_err(|e| format!("abrir zip: {e}"))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("zip inválido: {e}"))?;
    for i in 0..zip.len() {
        let mut entry = zip
            .by_index(i)
            .map_err(|e| format!("entry: {e}"))?;
        let name = entry.name().replace('\\', "/");
        if name.to_lowercase().ends_with(suffix) {
            let mut buf = Vec::new();
            entry
                .read_to_end(&mut buf)
                .map_err(|e| format!("leer entry: {e}"))?;
            fs::write(dest, &buf).map_err(|e| format!("guardar {:?}: {e}", dest))?;
            return Ok(());
        }
    }
    Err(format!("no se encontró '{suffix}' en el zip"))
}
fn write_task_scripts() -> Result<(), String> {
    let base = crate::config::launcher_data_dir();
    let dir = antilag_dir();
    let base_s = base.to_string_lossy();
    let dir_s = dir.to_string_lossy();

    let on = format!(
        "@echo off\r\n\
         rem Modo antilag ON: arranca el túnel y mete la ruta de la sesión.\r\n\
         taskkill /f /im sing-box.exe >nul 2>&1\r\n\
         timeout /t 2 /nobreak >nul\r\n\
         set ENABLE_DEPRECATED_WIREGUARD_OUTBOUND=true\r\n\
         start /b \"\" \"{dir_s}\\sing-box.exe\" -D \"{dir_s}\" -c config.json run > \"{dir_s}\\sing-box.log\" 2>&1\r\n\
         timeout /t 4 /nobreak >nul\r\n\
         for /f \"usebackq delims=\" %%%%i in (\"{dir_s}\\targets.txt\") do route add %%%%i mask 255.255.255.255 {TUN_GATEWAY} >nul 2>&1\r\n"
    );
    let off = format!(
        "@echo off\r\n\
         rem Modo antilag OFF: apaga el túnel y quita las rutas de la sesión.\r\n\
         taskkill /f /im sing-box.exe >nul 2>&1\r\n\
         timeout /t 2 /nobreak >nul\r\n\
         for /f \"usebackq delims=\" %%%%i in (\"{dir_s}\\targets.txt\") do route delete %%%%i mask 255.255.255.255 {TUN_GATEWAY} >nul 2>&1\r\n"
    );

    let install_ps1 = format!(
        "$ErrorActionPreference='Stop'\r\n\
         $d='{dir_s}'\r\n\
         try {{ Unregister-ScheduledTask -TaskName '{TASK_ON}' -Confirm:$false -ErrorAction SilentlyContinue }} catch {{}}\r\n\
         try {{ Unregister-ScheduledTask -TaskName '{TASK_OFF}' -Confirm:$false -ErrorAction SilentlyContinue }} catch {{}}\r\n\
         $am = New-ScheduledTaskAction -Execute \"$d\\antilag-on.cmd\"\r\n\
         Register-ScheduledTask -TaskName '{TASK_ON}' -Action $am -RunLevel Highest -Force | Out-Null\r\n\
         $am2 = New-ScheduledTaskAction -Execute \"$d\\antilag-off.cmd\"\r\n\
         Register-ScheduledTask -TaskName '{TASK_OFF}' -Action $am2 -RunLevel Highest -Force | Out-Null\r\n\
         New-Item -ItemType File -Path '$dir_s\\tasks-installed.ok' -Force | Out-Null\r\n"
    );

    let run_ps1 = format!(
        "$ErrorActionPreference='Stop'\r\n\
         try {{\r\n\
           Remove-Item '{dir_s}\\tasks-installed.ok' -Force -ErrorAction SilentlyContinue\r\n\
           Start-Process -FilePath powershell -ArgumentList @('-NoProfile','-ExecutionPolicy','Bypass','-File','{dir_s}\\install-tasks.ps1') -Verb RunAs -Wait -WindowStyle Hidden\r\n\
           if (Test-Path '{dir_s}\\tasks-installed.ok') {{ Write-Output 'OK' }} else {{ throw 'Permiso cancelado o instalación incompleta.' }}\r\n\
         }} catch {{ Write-Error $_.Exception.Message; exit 1 }}\r\n"
    );

    fs::write(on_script(), on).map_err(|e| format!("escribir antilag-on.cmd: {e}"))?;
    fs::write(off_script(), off).map_err(|e| format!("escribir antilag-off.cmd: {e}"))?;
    fs::write(dir.join("install-tasks.ps1"), install_ps1)
        .map_err(|e| format!("escribir install-tasks.ps1: {e}"))?;
    fs::write(dir.join("run-tasks.ps1"), run_ps1)
        .map_err(|e| format!("escribir run-tasks.ps1: {e}"))?;
    let _ = base_s;
    Ok(())
}
pub fn install_tasks() -> Result<bool, String> {
    if tasks_registered() {
        return Ok(true);
    }
    write_task_scripts()?;
    let out = powershell_hidden()
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(antilag_dir().join("run-tasks.ps1"))
        .output()
        .map_err(|e| format!("lanzar instalación de tareas: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    if !stdout.trim().starts_with("OK") {
        let cause = stderr.lines().next().unwrap_or("sin detalle").to_string();
        if cause.to_lowercase().contains("cancel") {
            return Err("Cancelaste el permiso de administrador: sin él no se puede encender el túnel. Inténtalo de nuevo y acepta.".into());
        }
        return Err(format!("No se pudieron registrar las tareas: {cause}"));
    }
    Ok(tasks_registered())
}
pub fn resolve_public_v4(host: &str) -> Vec<String> {
    let host = host.trim();
    if host.is_empty() {
        return Vec::new();
    }
    let host = if host.starts_with('[') {
        host.trim_start_matches('[').split(']').next().unwrap_or(host).to_string()
    } else {
        let mut parts = host.rsplitn(2, ':');
        let last = parts.next().unwrap_or("");
        let head = parts.next().unwrap_or("");
        if head.is_empty() || last.parse::<u16>().is_err() {
            host.to_string()
        } else {
            head.to_string()
        }
    };
    if host.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<String> = Vec::new();
    let mut seen: Vec<IpAddr> = Vec::new();
    if let Ok(addrs) = (host.as_str(), 25565u16).to_socket_addrs() {
        for addr in addrs {
            let ip = addr.ip();
            if seen.contains(&ip) {
                continue;
            }
            seen.push(ip);
            let mut ok = false;
            if let IpAddr::V4(v4) = ip {
                if !v4.is_loopback()
                    && !v4.is_private()
                    && !v4.is_link_local()
                    && !v4.is_multicast()
                    && !v4.is_broadcast()
                    && !v4.is_unspecified()
                {
                    ok = true;
                }
            }
            if ok {
                out.push(ip.to_string());
            }
        }
    }
    out.sort();
    out.dedup();
    out
}
fn write_session_config(ips: &[String]) -> Result<(), String> {
    let ident = load_identity().ok_or_else(|| "no hay identidad WARP instalada".to_string())?;
    let ip_cidr: Vec<String> = ips.iter().map(|i| format!("{i}/32")).collect();
    let cfg = serde_json::json!({
        "log": { "level": "error", "timestamp": false },
        "inbounds": [{
            "type": "tun",
            "tag": "tun-in",
            "address": [format!("{TUN_GATEWAY}/30")],
            "auto_route": false,
            "strict_route": false,
            "stack": "mixed",
            "mtu": 1280
        }],
        "outbounds": [
            { "type": "direct", "tag": "direct" },
            {
                "type": "wireguard",
                "tag": "warp",
                "server": ident.endpoint_v4,
                "server_port": 2408,
                "local_address": [
                    format!("{}/12", ident.address_v4),
                    format!("{}/128", ident.address_v6)
                ],
                "private_key": ident.private_key,
                "peer_public_key": ident.peer_pubkey,
                "mtu": 1280
            }
        ],
        "route": {
            "rules": [ { "ip_cidr": ip_cidr, "outbound": "warp" } ],
            "final": "direct"
        },
        "dns": {
            "servers": [
                {
                    "tag": "cf-doh",
                    "address": "https://1.1.1.1/dns-query",
                    "detour": "direct"
                }
            ],
            "final": "cf-doh"
        }
    });
    fs::write(
        session_config(),
        serde_json::to_string(&cfg).map_err(|e| format!("serializar config: {e}"))?,
    )
    .map_err(|e| format!("escribir config.json: {e}"))?;
    fs::write(targets_file(), ips.join("\r\n"))
        .map_err(|e| format!("escribir targets.txt: {e}"))?;
    Ok(())
}

fn start_task(name: &str) -> Result<(), String> {
    let expr = format!("Start-ScheduledTask -TaskName '{name}'");
    let out = powershell_hidden()
        .args(["-NoProfile", "-Command"])
        .arg(&expr)
        .output()
        .map_err(|e| format!("arrancar tarea {name}: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).to_string();
        return Err(format!("arrancar tarea {name}: {err}"));
    }
    Ok(())
}
pub fn start_tunnel(host: &str) -> Result<u32, String> {
    let ips = resolve_public_v4(host);
    if ips.is_empty() {
        return Err(format!(
            "'{host}' no resuelve a una IP pública. Pon la dirección del servidor (la que usan para entrar) en Ajustes → Modo antilag."
        ));
    }
    if !tasks_registered() {
        return Err("El modo antilag no está instalado (faltan las tareas). Vuelve a pulsar 'Instalar' en Ajustes.".into());
    }
    write_session_config(&ips)?;
    start_task(TASK_ON)?;
    for _ in 0..20 {
        if tunnel_active() {
            std::thread::sleep(std::time::Duration::from_millis(1500));
            return Ok(ips.len() as u32);
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    Err("El túnel no llegó a arrancar (mira antilag\\sing-box.log)".into())
}
pub fn stop_tunnel() {
    let _ = start_task(TASK_OFF);
}
pub async fn antilag_install(
    mut prog: impl FnMut(u32, String),
) -> Result<AntilagStatus, String> {
    prog(5, "Comprobando componentes...".into());
    ensure_engine(&mut prog).await?;
    register_identity(&mut prog).await?;
    write_task_scripts()?;
    prog(80, "Registrando tareas (acepta el permiso de administrador)...".into());
    install_tasks()?;
    prog(100, "Modo antilag instalado.".into());
    Ok(status())
}