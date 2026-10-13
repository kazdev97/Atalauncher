use crate::downloader::{download_file, get_json, get_text, http_client};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

const VERSIONS_MANIFEST: &str = "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json";
const FABRIC_META: &str = "https://meta.fabricmc.net/v2";
pub type InstallProgress<'a> = dyn FnMut(u32, String) + Send + 'a;
struct PctThrottle<'a> {
    cb: &'a mut (dyn FnMut(u32, String) + Send),
    last: u32,
}
impl<'a> PctThrottle<'a> {
    fn new(cb: &'a mut (dyn FnMut(u32, String) + Send)) -> Self {
        PctThrottle { cb, last: u32::MAX }
    }
    fn emit(&mut self, pct: u32, msg: String) {
        if pct != self.last {
            self.last = pct;
            (self.cb)(pct, msg);
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VersionOverview {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub release_time: String,
}
pub async fn get_vanilla_versions() -> Result<Vec<VersionOverview>, String> {
    let client = http_client();
    let root: Value = get_json(&client, VERSIONS_MANIFEST, Duration::from_secs(30))
        .await
        .map_err(|e| format!("obtener manifest de versiones: {e}"))?;
    let versions = root
        .get("versions")
        .and_then(|v| v.as_array())
        .ok_or("manifest sin lista de versiones")?;
    let mut out = Vec::new();
    for v in versions {
        if let (Some(id), Some(kind)) = (v.get("id").and_then(|x| x.as_str()), v.get("type").and_then(|x| x.as_str())) {
            out.push(VersionOverview {
                id: id.to_string(),
                kind: kind.to_string(),
                release_time: v
                    .get("releaseTime")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string(),
            });
        }
    }
    Ok(out)
}
pub async fn get_fabric_loaders() -> Result<Vec<String>, String> {
    let client = http_client();
    let list: Vec<Value> = get_json::<Vec<Value>>(
        &client,
        &format!("{FABRIC_META}/versions/loader"),
        Duration::from_secs(30),
    )
    .await
    .map_err(|e| format!("obtener loaders fabric: {e}"))?;
    let mut out: Vec<String> = list
        .iter()
        .filter_map(|v| v.get("loader").and_then(|l| l.get("version")).and_then(|x| x.as_str()))
        .map(|s| s.to_string())
        .collect();
    out.dedup();
    Ok(out)
}

fn ensure_subdirs(instance_dir: &Path) {
    for sub in ["versions", "libraries", "assets/indexes", "assets/objects", "mods", "config", "saves", "resourcepacks"] {
        fs::create_dir_all(instance_dir.join(sub)).ok();
    }
}
async fn fetch_version_json(instance_dir: &Path, url: &str, id: &str) -> Result<Value, String> {
    let client = http_client();
    let text = get_text(&client, url, Duration::from_secs(60))
        .await
        .map_err(|e| format!("descargar json de version ({id}): {e}"))?;
    let dir = instance_dir.join("versions").join(id);
    fs::create_dir_all(&dir).map_err(|e| format!("crear dir de versión: {e}"))?;
    fs::write(dir.join(format!("{id}.json")), &text).map_err(|e| format!("guardar json: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("json de version inválido: {e}"))
}

async fn download_asset_index(
    instance_dir: &Path,
    index_url: &str,
    id: &str,
    on_progress: &mut InstallProgress<'_>,
) -> Result<(), String> {
    let client = http_client();
    let json: Value = get_json(&client, index_url, Duration::from_secs(60)).await
        .map_err(|e| format!("descargar asset index: {e}"))?;
    fs::create_dir_all(instance_dir.join("assets").join("indexes")).ok();
    fs::write(
        instance_dir.join("assets").join("indexes").join(format!("{id}.json")),
        serde_json::to_string_pretty(&json).unwrap_or_default(),
    )
    .map_err(|e| format!("guardar asset index: {e}"))?;

    let objects = json
        .get("objects")
        .and_then(|o| o.as_object())
        .ok_or("asset index sin objects")?;
    let total = objects.len().max(1);
    let mut throttle = PctThrottle::new(on_progress);
    for (i, (_, obj)) in objects.iter().enumerate() {
        let hash = obj.get("hash").and_then(|h| h.as_str()).ok_or("objeto sin hash")?.to_string();
        let sub1 = &hash[..2];
        let url = format!("https://resources.download.minecraft.net/{sub1}/{hash}");
        let dest = instance_dir.join("assets").join("objects").join(sub1).join(&hash);
        if dest.exists() {
            continue;
        }
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).ok();
        }
        let _ = download_file(&client, &url, &dest, &mut |_, _| {}).await;
        throttle.emit(
            12 + 28 * (i as u32) / (total as u32),
            format!("Descargando assets... {}/{}", i + 1, total),
        );
    }
    Ok(())
}

fn maven_path_from_name(name: &str) -> Option<PathBuf> {
    let parts: Vec<&str> = name.splitn(3, ':').collect();
    if parts.len() < 3 {
        return None;
    }
    let group = parts[0].replace('.', "/");
    let artifact = parts[1];
    let ver = parts[2];
    if ver.contains(':') {
        let vp: Vec<&str> = ver.splitn(2, ':').collect();
        let (ver_only, classifier) = (vp[0], vp[1]);
        return Some(
            PathBuf::from(group).join(artifact).join(ver_only).join(format!(
                "{artifact}-{ver_only}-{classifier}.jar"
            )),
        );
    }
    Some(PathBuf::from(group).join(artifact).join(ver).join(format!("{artifact}-{ver}.jar")))
}

async fn download_library(client: &reqwest::Client, lib: &Value, base: &Path) -> Result<(), String> {
    let name = lib.get("name").and_then(|n| n.as_str()).unwrap_or("");
    let artifact = lib.get("downloads").and_then(|d| d.get("artifact"));
    let url = artifact.and_then(|a| a.get("url")).and_then(|u| u.as_str()).unwrap_or("");
    let rel = artifact
        .and_then(|a| a.get("path"))
        .and_then(|p| p.as_str())
        .map(PathBuf::from)
        .or_else(|| maven_path_from_name(name));
    if let Some(rel) = rel {
        let dest = base.join(&rel);
        if dest.exists() {
            return Ok(());
        }
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).ok();
        }
        let mut sources: Vec<String> = Vec::new();
        if !url.is_empty() {
            sources.push(url.to_string());
        }
        if !name.is_empty() {
            if let Some(rel2) = maven_path_from_name(name) {
                let p = rel2.to_string_lossy().to_string();
                for host in [
                    "https://libraries.minecraft.net/",
                    "https://repo1.maven.org/maven2/",
                    "https://maven.minecraftforge.net/",
                    "https://maven.neoforged.net/releases/",
                ] {
                    sources.push(format!("{host}{p}"));
                }
            }
        }
        for src in sources {
            if download_file(client, &src, &dest, &mut |_, _| {}).await.is_ok() {
                return Ok(());
            }
        }
    }
    Ok(())
}
async fn download_natives(client: &reqwest::Client, lib: &Value, libs_base: &Path, natives_dir: &Path) {
    let Some(classifiers) = lib.get("downloads").and_then(|d| d.get("classifiers")) else {
        return;
    };
    let key = if cfg!(target_os = "windows") {
        "natives-windows"
    } else if cfg!(target_os = "linux") {
        "natives-linux"
    } else {
        "natives-osx"
    };
    let Some(entry) = classifiers.get(key) else { return };
    let Some(url) = entry.get("url").and_then(|u| u.as_str()) else { return };
    if url.is_empty() {
        return;
    }
    let rel = entry
        .get("path")
        .and_then(|p| p.as_str())
        .map(PathBuf::from);
    let Some(rel) = rel else { return };
    let jar = libs_base.join(&rel);
    if !jar.exists() {
        if let Some(parent) = jar.parent() {
            fs::create_dir_all(parent).ok();
        }
        if download_file(client, url, &jar, &mut |_, _| {}).await.is_err() {
            return;
        }
    }
    extract_zip_to(&jar, natives_dir);
}
fn extract_zip_to(jar: &Path, dest: &Path) {
    let Ok(file) = std::fs::File::open(jar) else { return };
    let Ok(mut archive) = zip::ZipArchive::new(file) else { return };
    fs::create_dir_all(dest).ok();
    for i in 0..archive.len() {
        let mut entry = match archive.by_index(i) {
            Ok(e) => e,
            Err(_) => continue,
        };
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().replace('\\', "/");
        if name.starts_with("META-INF") || name.contains("..") || name.starts_with('/') {
            continue;
        }
        let out = dest.join(&name);
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent).ok();
        }
        if let Ok(mut f) = fs::File::create(&out) {
            let _ = std::io::copy(&mut entry, &mut f);
        }
    }
}
pub async fn install_game(
    instance_dir: &Path,
    game_version: &str,
    on_progress: &mut InstallProgress<'_>,
) -> Result<String, String> {
    ensure_subdirs(instance_dir);
    let client = http_client();
    let mut throttle = PctThrottle::new(on_progress);
    throttle.emit(1, "Obteniendo manifiesto de versiones...".to_string());
    let root: Value = get_json(&client, VERSIONS_MANIFEST, Duration::from_secs(60)).await
        .map_err(|e| format!("obtener manifest mojang: {e}"))?;
    let url = root
        .get("versions")
        .and_then(|v| v.as_array())
        .and_then(|arr| {
            arr.iter().find(|v| {
                v.get("id").and_then(|x| x.as_str()).map(|s| s == game_version).unwrap_or(false)
            })
        })
        .and_then(|v| v.get("url").and_then(|u| u.as_str()))
        .ok_or_else(|| format!("Versión de Minecraft '{game_version}' no encontrada"))?;
    let mut current = fetch_version_json(instance_dir, url, game_version).await?;
    let mut inherits = current.get("inheritsFrom").and_then(|i| i.as_str()).map(String::from);
    while let Some(parent_id) = inherits {
        let parent_url = root
            .get("versions")
            .and_then(|v| v.as_array())
            .and_then(|arr| {
                arr.iter().find(|v| v.get("id").and_then(|x| x.as_str()).map(|s| s == parent_id).unwrap_or(false))
            })
            .and_then(|v| v.get("url").and_then(|u| u.as_str()))
            .ok_or_else(|| format!("versión base {parent_id} no encontrada"))?;
        current = fetch_version_json(instance_dir, parent_url, &parent_id).await?;
        inherits = current
            .get("inheritsFrom")
            .and_then(|i| i.as_str())
            .map(String::from);
    }
    if let Some(client_url) = current
        .get("downloads")
        .and_then(|d| d.get("client"))
        .and_then(|c| c.get("url"))
        .and_then(|u| u.as_str())
    {
        let size_hint = current
            .get("downloads")
            .and_then(|d| d.get("client"))
            .and_then(|c| c.get("size"))
            .and_then(|s| s.as_u64());
        let dest = instance_dir.join("versions").join(game_version).join(format!("{game_version}.jar"));
        if !dest.exists() {
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).ok();
            }
            throttle.emit(5, "Descargando el jar del juego...".to_string());
            let cb = &mut throttle.cb;
            let mut last = 0u32;
            download_file(&client, client_url, &dest, &mut move |read, total| {
                let t = size_hint.unwrap_or(total);
                if t > 0 {
                    let pct = 5 + 7 * (read as u32).min(t as u32) / (t as u32);
                    if pct != last {
                        last = pct;
                        (cb)(pct, format!("Descargando el jar del juego... {pct}%"));
                    }
                }
            })
            .await
            .map_err(|e| format!("descargar client jar: {e}"))?;
        }
    }
    if let Some(index) = current.get("assetIndex").and_then(|i| i.get("url")).and_then(|u| u.as_str()) {
        let asset_id = current
            .get("assets")
            .and_then(|a| a.as_str())
            .map(String::from)
            .unwrap_or_else(|| game_version.to_string());
        throttle.emit(12, "Descargando índice de assets...".to_string());
        download_asset_index(instance_dir, index, &asset_id, &mut *throttle.cb).await?;
    }
    if let Some(libs) = current.get("libraries").and_then(|l| l.as_array()) {
        let n = libs.len().max(1);
        for (i, lib) in libs.iter().enumerate() {
            if rules_allow(lib) {
                let _ = download_library(&client, lib, &instance_dir.join("libraries")).await;
                download_natives(
                    &client,
                    lib,
                    &instance_dir.join("libraries"),
                    &instance_dir.join("natives"),
                )
                .await;
            }
            throttle.emit(
                40 + 30 * (i as u32) / (n as u32),
                format!("Descargando librerías... {}/{}", i + 1, n),
            );
        }
    }
    throttle.emit(70, "Juego base instalado".to_string());
    Ok(format!("{game_version}.json"))
}

pub fn rules_allow(lib: &Value) -> bool {
    let Some(rules) = lib.get("rules").and_then(|r| r.as_array()) else {
        return true;
    };
    let mut allow = false;
    let mut has_pos_allow = false;
    for rule in rules {
        let action = rule.get("action").and_then(|a| a.as_str()).unwrap_or("");
        let os_name = rule.get("os").and_then(|o| o.get("name")).and_then(|n| n.as_str()).unwrap_or("");
        let matches_os = rule.get("os").is_none()
            || os_name == "windows"
            || (os_name == "linux" && cfg!(target_os = "linux"));
        if !matches_os {
            continue;
        }
        if action == "allow" {
            allow = true;
            has_pos_allow = true;
        } else if action == "disallow" {
            allow = false;
        }
    }
    has_pos_allow && allow
}
pub async fn install_fabric(
    instance_dir: &Path,
    game_version: &str,
    loader_version: &str,
    on_progress: &mut InstallProgress<'_>,
) -> Result<String, String> {
    let client = http_client();
    let mut throttle = PctThrottle::new(on_progress);
    throttle.emit(72, "Descargando loader fabric...".to_string());
    let url = format!(
        "{FABRIC_META}/versions/loader/{game_version}/{loader_version}/profile/json"
    );
    let text = get_text(&client, &url, Duration::from_secs(60)).await
        .map_err(|e| format!("descargar perfil fabric: {e}"))?;
    let profile: Value = serde_json::from_str(&text).map_err(|e| format!("perfil fabric inválido: {e}"))?;
    let id = profile.get("id").and_then(|i| i.as_str()).unwrap_or(game_version).to_string();
    let dir = instance_dir.join("versions").join(&id);
    fs::create_dir_all(&dir).map_err(|e| format!("crear dir fabric: {e}"))?;
    fs::write(dir.join(format!("{id}.json")), &text).map_err(|e| format!("guardar perfil fabric: {e}"))?;
    if let Some(libs) = profile.get("libraries").and_then(|l| l.as_array()) {
        for lib in libs {
            let name = lib.get("name").and_then(|n| n.as_str()).unwrap_or("");
            if name.starts_with("net.fabricmc:fabric-loader") {
                if let Some(dl) = lib.get("downloads").and_then(|d| d.get("artifact").and_then(|a| a.get("url")).and_then(|u| u.as_str())) {
                    let jar = dir.join(format!("{id}.jar"));
                    if !jar.exists() {
                        download_file(&client, dl, &jar, &mut |_, _| {}).await
                            .map_err(|e| format!("descargar loader fabric: {e}"))?;
                    }
                }
            }
        }
    }
    throttle.emit(85, "Loader fabric instalado".to_string());
    Ok(format!("{id}.json"))
}
pub async fn get_forge_versions(game_version: &str) -> Result<Vec<String>, String> {
    let mut out = fetch_maven_versions(
        "https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml",
    )
    .await?;
    let prefix = format!("{game_version}-");
    out.retain(|v| v.starts_with(&prefix));
    Ok(out)
}
pub async fn get_neoforge_versions(game_version: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut transitional = fetch_maven_versions(
        "https://maven.neoforged.net/releases/net/neoforged/forge/maven-metadata.xml",
    )
    .await
    .unwrap_or_default();
    let prefix = format!("{game_version}-");
    transitional.retain(|v| v.starts_with(&prefix));
    out.extend(transitional);
    if let Some(neo_prefix) = neoforge_zero_prefix(game_version) {
        let mut stable = fetch_maven_versions(
            "https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml",
        )
        .await
        .unwrap_or_default();
        stable.retain(|v| v.starts_with(&neo_prefix));
        out.extend(stable);
    }
    out.dedup();
    Ok(out)
}
fn neoforge_zero_prefix(game_version: &str) -> Option<String> {
    let rest = game_version.strip_prefix("1.")?;
    Some(format!("{rest}."))
}
async fn fetch_maven_versions(url: &str) -> Result<Vec<String>, String> {
    let client = http_client();
    let xml = get_text(&client, url, Duration::from_secs(30))
        .await
        .map_err(|e| format!("obtener metadata maven: {e}"))?;
    let tag = "<version>";
    let end_tag = "</version>";
    let mut out = Vec::new();
    let mut i = 0usize;
    while let Some(rel) = xml[i..].find(tag) {
        let start = i + rel + tag.len();
        let Some(end_rel) = xml[start..].find(end_tag) else { break };
        let end = start + end_rel;
        out.push(xml[start..end].to_string());
        i = end + end_tag.len();
    }
    Ok(out)
}
pub async fn install_forge(
    instance_dir: &Path,
    game_version: &str,
    loader_version: &str,
    on_progress: &mut InstallProgress<'_>,
) -> Result<String, String> {
    install_simple_installer("forge", instance_dir, game_version, loader_version, on_progress).await
}
pub async fn install_neoforge(
    instance_dir: &Path,
    game_version: &str,
    loader_version: &str,
    on_progress: &mut InstallProgress<'_>,
) -> Result<String, String> {
    install_simple_installer("neoforge", instance_dir, game_version, loader_version, on_progress)
        .await
}
async fn install_simple_installer(
    kind: &str,
    instance_dir: &Path,
    game_version: &str,
    loader_version: &str,
    on_progress: &mut InstallProgress<'_>,
) -> Result<String, String> {
    let label = if kind == "neoforge" { "NeoForge" } else { "Forge" };
    let client = http_client();
    let mut throttle = PctThrottle::new(on_progress);
    let version = format!("{game_version}-{loader_version}");
    throttle.emit(72, format!("Descargando el instalador de {label}..."));
    let cache_dir = instance_dir.join(".cache");
    fs::create_dir_all(&cache_dir).ok();
    let mut version_variants = vec![version.clone()];
    if !version_variants.contains(&loader_version.to_string()) {
        version_variants.push(loader_version.to_string());
    }
    let mut candidates: Vec<(String, PathBuf)> = Vec::new();
    if kind == "neoforge" {
        for v in &version_variants {
            candidates.push((
                format!("https://maven.neoforged.net/releases/net/neoforged/neoforge/{v}/neoforge-{v}-installer.jar"),
                cache_dir.join(format!("neoforge-{v}-installer.jar")),
            ));
        }
    }
    for v in &version_variants {
        candidates.push((
            format!(
                "https://maven.neoforged.net/releases/net/neoforged/forge/{v}/forge-{v}-installer.jar"
            ),
            cache_dir.join(format!("forge-{v}-installer.jar")),
        ));
    }
    for v in &version_variants {
        candidates.push((
            format!("https://maven.minecraftforge.net/net/minecraftforge/forge/{v}/forge-{v}-installer.jar"),
            cache_dir.join(format!("forge-{v}-installer.jar")),
        ));
    }

    let mut installer_jar: Option<PathBuf> = None;
    for (url, jar) in candidates {
        if jar.exists() {
            installer_jar = Some(jar);
            break;
        }
        if download_file(&client, &url, &jar, &mut |_, _| {}).await.is_ok() {
            installer_jar = Some(jar);
            break;
        }
        let _ = fs::remove_file(&jar);
    }
    let installer_jar = installer_jar.ok_or_else(|| {
        format!("no se pudo descargar el instalador de {label} para '{loader_version}'")
    })?;
    let mut profile_text: Option<String> = None;
    {
        use std::io::Read;
        let file = std::fs::File::open(&installer_jar)
            .map_err(|e| format!("abrir instalador {label}: {e}"))?;
        let mut archive =
            zip::ZipArchive::new(file).map_err(|e| format!("instalador {label} inválido: {e}"))?;
        if let Ok(mut entry) = archive.by_name("version.json") {
            let mut s = String::new();
            if entry.read_to_string(&mut s).is_ok() && !s.trim().is_empty() {
                profile_text = Some(s);
            }
        }
        if profile_text.is_none() {
            if let Ok(mut entry) = archive.by_name("install_profile.json") {
                let mut s = String::new();
                if entry.read_to_string(&mut s).is_ok() {
                    if let Ok(v) = serde_json::from_str::<Value>(&s) {
                        if let Some(vi) = v.get("versionInfo") {
                            profile_text = serde_json::to_string_pretty(vi).ok();
                        }
                    }
                }
            }
        }
    }
    let profile_text = profile_text
        .ok_or_else(|| format!("el instalador de {label} no trae perfil de lanzamiento"))?;

    let profile: Value =
        serde_json::from_str(&profile_text).map_err(|e| format!("perfil {label} inválido: {e}"))?;
    let id = profile
        .get("id")
        .and_then(|i| i.as_str())
        .unwrap_or(&version)
        .to_string();
    let dir = instance_dir.join("versions").join(&id);
    fs::create_dir_all(&dir).map_err(|e| format!("crear dir {label}: {e}"))?;
    fs::write(dir.join(format!("{id}.json")), &profile_text)
        .map_err(|e| format!("guardar perfil {label}: {e}"))?;

    if let Some(libs) = profile.get("libraries").and_then(|l| l.as_array()) {
        let n = libs.len().max(1);
        for (i, lib) in libs.iter().enumerate() {
            if rules_allow(lib) {
                let _ = download_library(&client, lib, &instance_dir.join("libraries")).await;
                download_natives(
                    &client,
                    lib,
                    &instance_dir.join("libraries"),
                    &instance_dir.join("natives"),
                )
                .await;
            }
            throttle.emit(
                72 + 13 * (i as u32) / (n as u32),
                format!("Descargando librerías de {label}... {}/{}", i + 1, n),
            );
        }
    }
    throttle.emit(86, format!("Generando los jars de {label} (instalador)..."));
    let java = crate::java::ensure_java(
        game_version,
        instance_dir,
        &mut |msg| {
            throttle.emit(86, format!("Preparando Java: {msg}"));
        },
    )
    .await?;
    throttle.emit(87, format!("Ejecutando el instalador de {label}..."));
    let launcher_profiles = instance_dir.join("launcher_profiles.json");
    if !launcher_profiles.exists() {
        fs::write(
            &launcher_profiles,
            r#"{"profiles":{},"settings":{},"selectedProfile":""}"#,
        )
        .map_err(|e| format!("crear launcher_profiles.json: {e}"))?;
    }
    let mut cmd = std::process::Command::new(&java);
    cmd.arg("-Djava.awt.headless=true")
        .arg("-jar")
        .arg(&installer_jar)
        .arg("--installClient")
        .arg(instance_dir)
        .current_dir(instance_dir)
        .stdin(std::process::Stdio::null());
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    let out = cmd
        .output()
        .map_err(|e| format!("ejecutar instalador {label}: {e}"))?;
    if !out.status.success() {
        let combined = [
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr),
        ]
        .join("\n");
        let lines: Vec<&str> = combined.lines().collect();
        let snippet =
            lines.iter().rev().take(14).rev().cloned().collect::<Vec<_>>().join("\n");
        return Err(format!("El instalador de {label} falló:\n{snippet}"));
    }
    let srg_ok = fs::read_dir(
        instance_dir.join("libraries").join("net").join("minecraft").join("client"),
    )
    .map(|rd| {
        rd.flatten().any(|e| {
            e.path()
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.ends_with("-srg.jar"))
                .unwrap_or(false)
        })
    })
    .unwrap_or(false);
    let mut loader_jar_ok = false;
    for base in ["net/minecraftforge/forge", "net/neoforged/forge", "net/neoforged/neoforge"] {
        if loader_jar_ok {
            break;
        }
        loader_jar_ok = fs::read_dir(instance_dir.join("libraries").join(base)).map(|rd| {
            rd.flatten().any(|e| {
                let p = e.path();
                if !p.is_file() {
                    return false;
                }
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.ends_with("-universal.jar") || n.ends_with("-client.jar"))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false);
    }
    if !srg_ok && !loader_jar_ok {
        return Err(format!(
            "El instalador de {label} terminó pero no generó los jars de runtime (srg/universal/client)."
        ));
    }
    throttle.emit(90, format!("{label} instalado"));
    Ok(format!("{id}.json"))
}
pub async fn install_instance(
    instance_dir: &Path,
    on_progress: &mut InstallProgress<'_>,
) -> Result<String, String> {
    let meta = crate::instances::load_meta(instance_dir);
    ensure_subdirs(instance_dir);
    install_game(instance_dir, &meta.game_version, on_progress).await?;
    let loader = meta.loader.to_lowercase();
    let loader_version = if meta.loader_version.trim().is_empty() {
        match loader.as_str() {
            "forge" => get_forge_versions(&meta.game_version)
                .await?
                .into_iter()
                .next_back()
                .unwrap_or_default(),
            "neoforge" => get_neoforge_versions(&meta.game_version)
                .await?
                .into_iter()
                .next_back()
                .unwrap_or_default(),
            _ => String::new(),
        }
    } else {
        meta.loader_version.clone()
    };

    let installed_id = if loader == "fabric" && !loader_version.is_empty() {
        install_fabric(instance_dir, &meta.game_version, &loader_version, on_progress).await?
    } else if loader == "forge" && !loader_version.is_empty() {
        install_forge(instance_dir, &meta.game_version, &loader_version, on_progress).await?
    } else if loader == "neoforge" && !loader_version.is_empty() {
        install_neoforge(instance_dir, &meta.game_version, &loader_version, on_progress).await?
    } else {
        format!("{}.json", meta.game_version)
    };
    Ok(installed_id)
}
