use crate::config::{runtime_dir, launcher_data_dir};
use crate::downloader::{download_file, get_json, http_client, get_text};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
#[derive(Debug, Clone, Serialize)]
pub struct JavaInfo {
    pub path: String,
    pub major: u32,
    pub is_valid: bool,
}
pub fn parse_mc_version_tuple(mc_version: &str) -> (u32, u32, u32) {
    let re = Regex::new(r"(\d+)\.(\d+)(?:\.(\d+))?").unwrap();
    if let Some(c) = re.captures(mc_version) {
        (
            c.get(1).and_then(|m| m.as_str().parse().ok()).unwrap_or(1),
            c.get(2).and_then(|m| m.as_str().parse().ok()).unwrap_or(21),
            c.get(3).and_then(|m| m.as_str().parse().ok()).unwrap_or(0),
        )
    } else {
        (1, 21, 0)
    }
}
pub fn required_java_major_heuristic(mc_version: &str) -> u32 {
    let (major, minor, patch) = parse_mc_version_tuple(mc_version);
    if major != 1 {
        21
    } else if minor < 17 {
        8
    } else if minor < 20 {
        17
    } else if minor == 20 && patch < 5 {
        17
    } else {
        21
    }
}
fn java_major_from_local_version_json(mc_version: &str, instance_dir: &Path) -> Option<u32> {
    let versions_dir = instance_dir.join("versions");
    let p = versions_dir.join(mc_version).join(format!("{mc_version}.json"));
    let text = fs::read_to_string(p).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    v.get("javaVersion")?.get("majorVersion")?.as_u64().map(|n| n as u32)
}
pub async fn resolve_java_major_online(mc_version: &str, instance_dir: &Path) -> u32 {
    if let Some(m) = java_major_from_local_version_json(mc_version, instance_dir) {
        return m;
    }
    if let Ok(m) = fetch_java_major_from_mojang(mc_version).await {
        return m;
    }
    required_java_major_heuristic(mc_version)
}
async fn fetch_java_major_from_mojang(mc_version: &str) -> Result<u32, String> {
    let client = http_client();
    let manifest_text = get_text(
        &client,
        "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json",
        Duration::from_secs(15),
    )
    .await?;
    let manifest: serde_json::Value =
        serde_json::from_str(&manifest_text).map_err(|e| format!("parse manifest: {e}"))?;
    let version_entry = manifest
        .get("versions")
        .and_then(|v| v.as_array())
        .and_then(|arr| {
            arr.iter()
                .find(|e| e.get("id").and_then(|i| i.as_str()) == Some(mc_version))
        });
    let url: Option<&str> = match version_entry {
        Some(entry) => entry.get("url").and_then(|u| u.as_str()),
        None => {
            let re = Regex::new(r"(\d+\.\d+(?:\.\d+)?)").unwrap();
            let base = re
                .captures(mc_version)
                .and_then(|c| c.get(1).map(|m| m.as_str()));
            base.and_then(|b| {
                manifest
                    .get("versions")
                    .and_then(|v| v.as_array())
                    .and_then(|arr| arr.iter().find(|e| e.get("id").and_then(|i| i.as_str()) == Some(b)))
                    .and_then(|e| e.get("url").and_then(|u| u.as_str()))
            })
        }
    };
    let url = url.ok_or_else(|| format!("no se encontró la versión {mc_version} en Mojang"))?;
    let version_json: serde_json::Value =
        get_json(&client, url, Duration::from_secs(15)).await?;
    version_json
        .get("javaVersion")
        .and_then(|v| v.get("majorVersion"))
        .and_then(|v| v.as_u64())
        .map(|n| n as u32)
        .ok_or_else(|| "no se encontró javaVersion.majorVersion en el JSON de Mojang".to_string())
}
fn extra_search_directories() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(pf) = std::env::var("ProgramFiles") {
        for sub in &["Java", "Microsoft", "Eclipse Adoptium", "Adoptium", "AdoptOpenJDK"] {
            dirs.push(PathBuf::from(&pf).join(sub));
        }
    }
    if let Ok(pfx) = std::env::var("ProgramFiles(x86)") {
        for sub in &["Java", "Microsoft", "Eclipse Adoptium"] {
            dirs.push(PathBuf::from(&pfx).join(sub));
        }
    }
    if let Ok(la) = std::env::var("LOCALAPPDATA") {
        for sub in &[
            "Programs/Eclipse Adoptium",
            "Programs/Microsoft",
            "Microsoft/OpenJDK",
        ] {
            dirs.push(PathBuf::from(&la).join(sub));
        }
    }
    if let Ok(java_home) = std::env::var("JAVA_HOME") {
        if !java_home.is_empty() {
            dirs.push(PathBuf::from(java_home));
        }
    }
    dirs
}
fn java_executable_in_dir(dir: &Path) -> Option<PathBuf> {
    let bin = dir.join("bin");
    let candidates = if cfg!(target_os = "windows") {
        vec!["javaw.exe", "java.exe"]
    } else {
        vec!["java"]
    };
    for name in candidates {
        let p = bin.join(name);
        if p.exists() {
            return Some(p);
        }
    }
    None
}
pub fn validate_java_executable(exe: &Path, min_major: u32) -> Result<u32, String> {
    let java_cmd = if exe.to_string_lossy().to_lowercase().ends_with("javaw.exe") {
        exe.parent().unwrap().join("java.exe")
    } else {
        exe.to_path_buf()
    };
    let mut cmd = Command::new(&java_cmd);
    cmd.arg("-version")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: no abrir consola
    let output = cmd
        .output()
        .map_err(|e| format!("ejecutar java -version: {e}"))?;
    let out = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let lower = out.to_lowercase();
    if cfg!(target_os = "windows") && (lower.contains("32-bit") || lower.contains("32 bit")) {
        return Err("Java de 32 bits no es compatible".to_string());
    }
    let re = Regex::new(r#"version "(\d+)"?"#).unwrap();
    let major = re
        .captures(&out)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<u32>().ok())
        .ok_or_else(|| format!("no se pudo detectar versión de Java en: {out}"))?;
    if major < min_major {
        return Err(format!("Java {major} es menor que la requerida ({min_major})"));
    }
    Ok(major)
}
pub fn find_bundled_java(major: u32) -> Option<PathBuf> {
    let root = runtime_dir().join(format!("jdk-{major}"));
    if !root.exists() {
        return None;
    }
    let exe = java_executable_in_dir(&root)?;
    if validate_java_executable(&exe, major).is_ok() {
        return Some(exe);
    }
    None
}
pub fn find_system_java(min_major: u32) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    for base in extra_search_directories() {
        if let Ok(entries) = fs::read_dir(&base) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    if let Some(exe) = java_executable_in_dir(&p) {
                        if validate_java_executable(&exe, min_major).is_ok() {
                            candidates.push((major_from_path(&exe).unwrap_or(0), exe));
                        }
                    }
                    if let Ok(subs) = fs::read_dir(&p) {
                        for sub in subs.flatten() {
                            let sub_p = sub.path();
                            if sub_p.is_dir() {
                                if let Some(exe) = java_executable_in_dir(&sub_p) {
                                    if validate_java_executable(&exe, min_major).is_ok() {
                                        candidates
                                            .push((major_from_path(&exe).unwrap_or(0), exe));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    candidates.sort_by(|a, b| b.0.cmp(&a.0));
    candidates.into_iter().next().map(|(_, exe)| exe)
}

fn major_from_path(exe: &Path) -> Option<u32> {
    let root = exe.parent()?.parent()?;
    let name = root.file_name()?.to_string_lossy().to_string();
    let re = Regex::new(r"jdk-?(\d+)").unwrap();
    re.captures(&name)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse().ok())
}
pub fn find_all_system_java() -> Result<Vec<JavaInfo>, String> {
    let mut found: Vec<JavaInfo> = Vec::new();
    for major in [21u32, 17, 11, 8] {
        if let Some(exe) = find_system_java(major) {
            let actual = validate_java_executable(&exe, major).unwrap_or(major);
            let path = exe.to_string_lossy().to_string();
            if !found.iter().any(|j| j.path == path) {
                found.push(JavaInfo { path, major: actual, is_valid: true });
            }
        }
    }
    Ok(found)
}
pub fn find_all_bundled_java() -> Vec<JavaInfo> {
    let mut found = Vec::new();
    let root = runtime_dir();
    if let Ok(entries) = fs::read_dir(&root) {
        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }
            if let Some(exe) = java_executable_in_dir(&dir) {
                if let Ok(major) = validate_java_executable(&exe, 8) {
                    found.push(JavaInfo {
                        path: exe.to_string_lossy().to_string(),
                        major,
                        is_valid: true,
                    });
                }
            }
        }
    }
    found
}
fn adoptium_os() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "mac"
    } else {
        "linux"
    }
}

#[derive(Deserialize)]
struct AdoptiumAsset {
    binary: AdoptiumBinary,
}

#[derive(Deserialize)]
struct AdoptiumBinary {
    package: AdoptiumPackage,
}

#[derive(Deserialize)]
struct AdoptiumPackage {
    link: String,
    name: String,
}
pub async fn install_adoptium(
    major: u32,
    on_status: &mut (dyn FnMut(String) + Send),
) -> Result<PathBuf, String> {
    let api = format!(
        "https://api.adoptium.net/v3/assets/latest/{major}/hotspot?architecture=x64&image_type=jdk&os={os}&heap_size=normal",
        os = adoptium_os()
    );
    let client = http_client();
    let assets: Vec<AdoptiumAsset> =
        get_json(&client, &api, Duration::from_secs(60)).await?;
    let asset = assets
        .first()
        .ok_or_else(|| "Adoptium no devolvió ningún JDK".to_string())?;
    let link = asset.binary.package.link.clone();
    let name = asset.binary.package.name.clone();
    on_status(format!("Descargando {name}..."));
    let tmp_dir = launcher_data_dir().join("_adoptium_tmp");
    if tmp_dir.exists() {
        fs::remove_dir_all(&tmp_dir).ok();
    }
    fs::create_dir_all(&tmp_dir).ok();
    let zip_path = tmp_dir.join(format!("jdk-{major}.zip"));
    let mut downloaded = 0u64;
    download_file(&client, &link, &zip_path, &mut |read, _total_dl| {
        downloaded = read;
        on_status(format!("Descargando JDK {major}... {downloaded} bytes"));
    })
    .await?;
    on_status(format!("Extrayendo JDK {major}..."));
    let extract_dir = tmp_dir.join(format!("jdk-{major}"));
    fs::create_dir_all(&extract_dir).ok();
    {
        let file = fs::File::open(&zip_path)
            .map_err(|e| format!("abrir zip adoptium: {e}"))?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| format!("leer zip adoptium: {e}"))?;
        archive
            .extract(&extract_dir)
            .map_err(|e| format!("extraer adoptium: {e}"))?;
    }
    let target_root = runtime_dir().join(format!("jdk-{major}"));
    if target_root.exists() {
        fs::remove_dir_all(&target_root).ok();
    }
    let inner_dir = fs::read_dir(&extract_dir)
        .map_err(|e| format!("leer dir extraído: {e}"))?
        .flatten()
        .find(|e| e.path().is_dir())
        .ok_or_else(|| "el zip no contiene un directorio JDK válido".to_string())?;
    fs::rename(inner_dir.path(), &target_root)
        .map_err(|e| format!("mover jdk a runtime dir: {e}"))?;
    fs::remove_dir_all(&tmp_dir).ok();
    let exe = java_executable_in_dir(&target_root)
        .ok_or_else(|| "no se encontró java tras extraer Adoptium".to_string())?;
    let _actual = validate_java_executable(&exe, major)
        .map_err(|e| format!("JDK descargado inválido: {e}"))?;
    on_status(format!("Java {major} listo en {}", target_root.display()));
    Ok(exe)
}
pub async fn ensure_java(
    mc_version: &str,
    instance_dir: &Path,
    on_status: &mut (dyn FnMut(String) + Send),
) -> Result<PathBuf, String> {
    let required = resolve_java_major_online(mc_version, instance_dir).await;
    on_status(format!("Buscando Java {required}..."));
    if let Some(exe) = find_bundled_java(required) {
        return Ok(exe);
    }
    if let Some(exe) = find_system_java(required) {
        return Ok(exe);
    }
    install_adoptium(required, on_status).await
}