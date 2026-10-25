use crate::config::instances_dir;
use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use tauri::{AppHandle, Emitter};
pub struct LaunchProcess {
    pub pid: u32,
    pub child: std::process::Child,
}

pub struct LaunchState {
    pub process: Option<LaunchProcess>,
}

pub static LAUNCH_STATE: once_cell::sync::Lazy<Arc<Mutex<LaunchState>>> =
    once_cell::sync::Lazy::new(|| Arc::new(Mutex::new(LaunchState { process: None })));

#[derive(Debug, Clone)]
struct LibraryEntry {
    path: String,
}

#[derive(Debug, Clone, Default)]
struct ResolvedVersion {
    id: String,
    main_class: String,
    libraries: Vec<LibraryEntry>,
    jvm_args: Vec<String>,
    game_args: Vec<String>,
    client_jar_path: String,
    extra_jars: Vec<String>,
    asset_index: String,
}

fn resolve_libraries(json: &serde_json::Value) -> Vec<LibraryEntry> {
    let mut out = Vec::new();
    if let Some(libs) = json.get("libraries").and_then(|l| l.as_array()) {
        for lib in libs {
            if !crate::installer::rules_allow(lib) {
                continue;
            }
            let name = lib.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let path = lib
                .get("downloads")
                .and_then(|d| d.get("artifact"))
                .and_then(|a| a.get("path"))
                .and_then(|p| p.as_str())
                .map(String::from)
                .or_else(|| crate::installer::maven_rel_path_str(name));
            if let Some(p) = path {
                out.push(LibraryEntry { path: p });
            }
        }
    }
    out
}
fn extract_args(json: &serde_json::Value) -> (Vec<String>, Vec<String>) {
    let mut jvm = Vec::new();
    let mut game = Vec::new();
    if let Some(arguments) = json.get("arguments") {
        if let Some(arr) = arguments.get("jvm").and_then(|a| a.as_array()) {
            for arg in arr {
                jvm.extend(arg_values(arg));
            }
        }
        if let Some(arr) = arguments.get("game").and_then(|a| a.as_array()) {
            for arg in arr {
                game.extend(arg_values(arg));
            }
        }
    }
    let legacy = json
        .get("minecraftArguments")
        .and_then(|a| a.as_str())
        .unwrap_or("")
        .to_string();

    if game.is_empty() && !legacy.is_empty() {
        game.extend(legacy.split_whitespace().map(String::from));
    }
    (jvm, game)
}
fn mojang_os() -> &'static str {
    match std::env::consts::OS {
        "windows" => "windows",
        "macos" => "osx",
        _ => "linux",
    }
}
fn arg_rules_allow(rules: &serde_json::Value) -> bool {
    let Some(arr) = rules.as_array() else { return true };
    let mut allowed = false;
    let mut matched = false;
    for rule in arr {
        let action = rule.get("action").and_then(|a| a.as_str());
        let os = rule.get("os");
        let os_name = os.and_then(|o| o.get("name")).and_then(|n| n.as_str()).unwrap_or("");
        let arch = os.and_then(|o| o.get("arch")).and_then(|n| n.as_str()).unwrap_or("");
        let os_ok = os.is_none() || os_name.is_empty() || os_name == mojang_os();
        let arch_ok = arch.is_empty() || arch == std::env::consts::ARCH;
        if !os_ok || !arch_ok {
            continue;
        }
        if let Some(features) = rule.get("features").and_then(|f| f.as_object()) {
            let mut feat_ok = true;
            for (k, v) in features {
                let v_bool = v.as_bool().unwrap_or(false);
                let actual = match k.as_str() {
                    "is_demo_user" => false,           // Launcher con cuenta completa, nunca demo
                    "has_custom_resolution" => true,  // Especificamos width y height
                    _ => false,                        // quickPlay u otras features no activas
                };
                if actual != v_bool {
                    feat_ok = false;
                    break;
                }
            }
            if !feat_ok {
                continue;
            }
        }
        matched = true;
        allowed = action == Some("allow");
    }
    matched && allowed
}
fn arg_values(arg: &serde_json::Value) -> Vec<String> {
    if let Some(s) = arg.as_str() {
        return vec![s.to_string()];
    }
    let Some(obj) = arg.as_object() else { return Vec::new() };
    if let Some(rs) = obj.get("rules") {
        if !arg_rules_allow(rs) {
            return Vec::new();
        }
    }
    let mut out = Vec::new();
    if let Some(val) = obj.get("value") {
        if let Some(arr) = val.as_array() {
            for v in arr {
                if let Some(s) = v.as_str() {
                    out.push(s.to_string());
                }
            }
        } else if let Some(s) = val.as_str() {
            out.push(s.to_string());
        }
    }
    out
}
fn load_version_chain(
    instance_dir: &Path,
    version_id: &str,
) -> Result<Vec<serde_json::Value>, String> {
    let versions_dir = instance_dir.join("versions");
    let mut chain = Vec::new();
    let mut current = version_id.to_string();
    let mut seen: HashSet<String> = HashSet::new();
    loop {
        let json_path = versions_dir.join(&current).join(format!("{current}.json"));
        let text = fs::read_to_string(&json_path)
            .map_err(|e| format!("no se encuentra el JSON de la versión '{version_id}': {e}"))?;
        let json: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| format!("JSON de versión inválido ({current}): {e}"))?;
        chain.push(json.clone());
        seen.insert(current.clone());
        let next = json
            .get("inheritsFrom")
            .and_then(|i| i.as_str())
            .map(String::from);
        match next {
            Some(parent) if !seen.contains(&parent) => current = parent,
            _ => break,
        }
    }
    Ok(chain)
}

fn resolve_version(instance_dir: &Path, version_id: &str) -> Result<ResolvedVersion, String> {
    let chain = load_version_chain(instance_dir, version_id)?;

    let mut main_class = String::new();
    let mut libraries: Vec<LibraryEntry> = Vec::new();
    let mut jvm_args: Vec<String> = Vec::new();
    let mut game_args: Vec<String> = Vec::new();
    let mut client_jar_path = String::new();
    let mut extra_jars: Vec<String> = Vec::new();
    for json in chain.iter().rev() {
        let (j, g) = extract_args(json);
        jvm_args.extend(j);
        game_args.extend(g);
        libraries.extend(resolve_libraries(json));
    }
    for json in chain.iter() {
        if let Some(mc) = json.get("mainClass").and_then(|m| m.as_str()) {
            if !mc.is_empty() {
                main_class = mc.to_string();
                break;
            }
        }
    }
    let versions_dir = instance_dir.join("versions");
    let mut asset_index = String::new();
    if let Some(base) = chain.last() {
        let base_id = base.get("id").and_then(|i| i.as_str()).unwrap_or(version_id);
        client_jar_path = versions_dir
            .join(base_id)
            .join(format!("{base_id}.jar"))
            .to_string_lossy()
            .to_string();
        asset_index = base
            .get("assets")
            .and_then(|a| a.as_str())
            .map(String::from)
            .unwrap_or_default();
        if asset_index.is_empty() {
            asset_index = base
                .get("assetIndex")
                .and_then(|ai| ai.get("id"))
                .and_then(|id| id.as_str())
                .map(String::from)
                .unwrap_or_default();
        }
    }
    if let Some(hijo) = chain.first() {
        let id = hijo.get("id").and_then(|i| i.as_str()).unwrap_or(version_id);
        if id != chain.last().and_then(|b| b.get("id")).and_then(|i| i.as_str()).unwrap_or("") {
            let lp = versions_dir.join(id).join(format!("{id}.jar"));
            if lp.exists() {
                extra_jars.push(lp.to_string_lossy().to_string());
            }
        }
    }
    jvm_args.dedup();
    game_args.dedup();

    Ok(ResolvedVersion {
        id: version_id.to_string(),
        main_class,
        libraries,
        jvm_args,
        game_args,
        client_jar_path,
        extra_jars,
        asset_index,
    })
}

fn build_classpath(resolved: &ResolvedVersion, instance_dir: &Path) -> String {
    let mut cp_paths = Vec::new();
    let libs_base = instance_dir.join("libraries");
    for lib in &resolved.libraries {
        let full = libs_base.join(Path::new(&lib.path));
        if full.exists() {
            cp_paths.push(full.to_string_lossy().to_string());
        }
    }
    let is_forge_or_neoforge = resolved.main_class.contains("bootstraplauncher")
        || resolved.main_class.contains("modlauncher")
        || resolved.id.to_lowercase().contains("forge");

    let has_srg = {
        let client_dir = libs_base.join("net").join("minecraft").join("client");
        if let Ok(entries) = fs::read_dir(&client_dir) {
            entries.flatten().any(|e| {
                let p = e.path();
                if p.is_dir() {
                    if let Ok(sub) = fs::read_dir(&p) {
                        return sub.flatten().any(|se| {
                            se.path()
                                .file_name()
                                .and_then(|n| n.to_str())
                                .map(|n| n.ends_with("-srg.jar") || n.ends_with("-extra.jar"))
                                .unwrap_or(false)
                        });
                    }
                }
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.ends_with("-srg.jar") || n.ends_with("-extra.jar"))
                    .unwrap_or(false)
            })
        } else {
            false
        }
    };

    if !has_srg
        && !is_forge_or_neoforge
        && !resolved.client_jar_path.is_empty()
        && Path::new(&resolved.client_jar_path).exists()
    {
        cp_paths.push(resolved.client_jar_path.clone());
    }
    for j in &resolved.extra_jars {
        if Path::new(j).exists() {
            cp_paths.push(j.clone());
        }
    }
    cp_paths.join(if cfg!(target_os = "windows") { ";" } else { ":" })
}

fn build_launch_command(
    java_path: &str,
    resolved: &ResolvedVersion,
    instance_dir: &Path,
    account: &crate::config::Account,
    ram_min_mb: u32,
    ram_max_mb: u32,
) -> Vec<String> {
    let mut cmd = Vec::new();
    cmd.push(java_path.to_string());
    let meta = crate::instances::load_meta(instance_dir);
    let mod_count = crate::jvm::count_mods(instance_dir);
    let java_major = crate::java::required_java_major_heuristic(&meta.game_version);
    let flags =
        crate::jvm::generate_jvm_flags(java_major, ram_max_mb / 1024, mod_count, false);
    cmd.extend(flags);
    cmd.push(format!("-Xmx{ram_max_mb}M"));
    cmd.push(format!("-Xms{ram_min_mb}M"));
    let cp = build_classpath(resolved, instance_dir);

    let game_dir = instance_dir.to_string_lossy().to_string();
    let assets_root = instance_dir.join("assets").to_string_lossy().to_string();
    let natives_dir = instance_dir.join("natives").to_string_lossy().to_string();
    let libraries_dir = instance_dir.join("libraries").to_string_lossy().to_string();
    let expander = |arg: &mut String| {
        *arg = arg.replace("${auth_player_name}", &account.name);
        *arg = arg.replace("${auth_uuid}", &account.id.replace('-', ""));
        *arg = arg.replace("${auth_access_token}", &account.access_token);
        *arg = arg.replace("${user_properties}", "{}");
        let user_type = if account.access_token.is_empty() {
            "legacy"
        } else {
            "msa"
        };
        *arg = arg.replace("${user_type}", user_type);
        *arg = arg.replace("${version_type}", "release");
        *arg = arg.replace("${game_directory}", &game_dir);
        *arg = arg.replace("${assets_root}", &assets_root);
        *arg = arg.replace("${game_assets}", &assets_root);
        *arg = arg.replace("${version_name}", &resolved.id);
        *arg = arg.replace("${fabric_classpath_path}", &cp);
        *arg = arg.replace("${launcher_name}", "Atalauncher");
        *arg = arg.replace("${launcher_version}", "0.1");
        *arg = arg.replace("${natives_directory}", &natives_dir);
        *arg = arg.replace("${classpath}", &cp);
        *arg = arg.replace(
            "${classpath_separator}",
            if cfg!(target_os = "windows") { ";" } else { ":" },
        );
        *arg = arg.replace("${library_directory}", &libraries_dir);
        *arg = arg.replace("${assets_index_name}", &resolved.asset_index);
        *arg = arg.replace("${clientid}", "Atalauncher");
        *arg = arg.replace("${auth_xuid}", "");
        *arg = arg.replace("${resolution_width}", "1280");
        *arg = arg.replace("${resolution_height}", "720");
        *arg = arg.replace("${quickPlayPath}", "");
        *arg = arg.replace("${quickPlaySingleplayer}", "");
        *arg = arg.replace("${quickPlayMultiplayer}", "");
        *arg = arg.replace("${quickPlayRealms}", "");
    };
    let mut jvm_args = resolved.jvm_args.clone();
    for arg in &mut jvm_args {
        expander(arg);
    }
    cmd.extend(jvm_args);

    if !cp.is_empty() {
        cmd.push("-cp".to_string());
        cmd.push(cp.clone());
    }
    if !resolved.main_class.is_empty() {
        cmd.push(resolved.main_class.clone());
    }
    let mut game_args = resolved.game_args.clone();
    for arg in &mut game_args {
        expander(arg);
    }
    let mut filtered: Vec<String> = Vec::with_capacity(game_args.len());
    let mut skip_next = false;
    for a in game_args {
        if a == "--width" || a == "--height" {
            skip_next = true;
            continue;
        }
        if skip_next {
            skip_next = false;
            continue;
        }
        filtered.push(a);
    }
    cmd.extend(filtered);
    cmd
}

#[derive(Clone, Serialize)]
pub struct LogLine {
    pub line: String,
    pub stream: String, // stdout | stderr
}

/// Re-descarga los archivos de lanzamiento que falten (p. ej. si se eliminó algún archivo o la descarga quedó incompleta).
async fn repair_launch_files(
    instance_dir: &Path,
    meta: &crate::instances::InstanceMeta,
) -> Result<(), String> {
    let mut noop = |_: u32, _: String| {};
    let progress: &mut crate::installer::InstallProgress<'_> = &mut noop;
    let loader = meta.loader.to_lowercase();
    let mut loader_version = meta.loader_version.trim().to_string();
    if loader_version.is_empty() {
        if let Some(v) = meta.version_id.strip_prefix("fabric-loader-") {
            if let Some(first_part) = v.split('-').next() {
                loader_version = first_part.to_string();
            }
        }
    }
    // Asegurar juego base
    crate::installer::install_game(instance_dir, &meta.game_version, progress)
        .await
        .map(|_| ())?;

    if loader.contains("fabric") || loader.contains("quilt") {
        let lv = if loader_version.is_empty() {
            crate::installer::get_fabric_loaders()
                .await
                .ok()
                .and_then(|list| list.into_iter().next())
                .unwrap_or_default()
        } else {
            loader_version
        };
        if !lv.is_empty() {
            crate::installer::install_fabric(
                instance_dir,
                &meta.game_version,
                &lv,
                progress,
            )
            .await
            .map(|_| ())?;
        }
    }
    Ok(())
}

pub(crate) fn log_launch(msg: &str) {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    let line = format!("[{now}] [launch] {msg}");
    println!("{line}");
    let log_path = crate::config::launcher_log_file();
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&log_path) {
        let _ = writeln!(f, "{line}");
    }
}

/// Comprueba que los archivos imprescindibles existen antes de invocar a Java.
/// Sin esto, un jar ausente produce un ClassNotFoundException críptico en la consola.
fn validate_launch_files(
    app: &AppHandle,
    instance_name: &str,
    instance_dir: &Path,
    version_id: &str,
    loader: &str,
    resolved: &ResolvedVersion,
) -> Result<(), String> {
    if resolved.main_class.trim().is_empty() {
        log_launch(&format!(
            "ERROR: '{instance_name}' sin clase principal (version_id='{version_id}')"
        ));
        return Err("No se pudo determinar la clase principal del juego: la instalación parece incompleta. Elimina la instancia e instálala de nuevo.".to_string());
    }
    let loader_lc = loader.to_lowercase();
    let is_fabric_like = loader_lc.contains("fabric")
        || loader_lc.contains("quilt")
        || resolved.main_class.to_lowercase().contains("knot");

    let is_forge_like = resolved.main_class.contains("bootstraplauncher")
        || resolved.main_class.contains("modlauncher")
        || resolved.id.to_lowercase().contains("forge");
    if !is_forge_like
        && !resolved.client_jar_path.is_empty()
        && !Path::new(&resolved.client_jar_path).exists()
    {
        log_launch(&format!(
            "ERROR: '{instance_name}' sin client jar (falta {})",
            resolved.client_jar_path
        ));
        return Err("Falta el archivo principal de Minecraft (client jar): la instalación quedó incompleta. Elimina la instancia e instálala de nuevo.".to_string());
    }
    let libs_base = instance_dir.join("libraries");
    let missing: Vec<&str> = resolved
        .libraries
        .iter()
        .filter(|lib| !libs_base.join(Path::new(&lib.path)).exists())
        .map(|lib| lib.path.as_str())
        .collect();

    if is_fabric_like {
        let missing_loader = missing
            .iter()
            .find(|p| p.contains("fabric-loader") || p.contains("quilt-loader"));
        if let Some(loader_rel) = missing_loader {
            let full = libs_base.join(Path::new(loader_rel));
            log_launch(&format!(
                "ERROR: '{instance_name}' sin librería del loader fabric (falta {})",
                full.to_string_lossy()
            ));
            return Err(format!(
                "Falta el archivo del loader fabric ({}): la descarga quedó incompleta.",
                full.to_string_lossy()
            ));
        }
    }
    if !missing.is_empty() {
        log_launch(&format!(
            "AVISO: '{instance_name}' tiene {} librerías sin descargar, ej: {}",
            missing.len(),
            missing.iter().take(3).cloned().collect::<Vec<_>>().join(", ")
        ));
        let _ = app.emit(
            "launch/line",
            LogLine {
                line: format!(
                    "[aviso] Faltan {} librerías por descargar; si el juego falla, elimina la instancia e instálala de nuevo.",
                    missing.len()
                ),
                stream: "stderr".to_string(),
            },
        );
    }
    log_launch(&format!(
        "lanzando '{instance_name}' version_id='{version_id}' main='{}' libs={} faltan={}",
        resolved.main_class,
        resolved.libraries.len(),
        missing.len()
    ));
    Ok(())
}

fn copy_missing_tree(src: &Path, dst: &Path, copied: &mut u64) {
    if !src.is_dir() {
        return;
    }
    if let Ok(entries) = std::fs::read_dir(src) {
        for entry in entries.flatten() {
            let p = entry.path();
            let name = p.file_name().unwrap_or_default().to_string_lossy().into_owned();
            let rel = dst.join(&name);
            if p.is_dir() {
                if let Err(e) = std::fs::create_dir_all(&rel) {
                    log_launch(&format!("journeymap: no se pudo crear {rel:?}: {e}"));
                    continue;
                }
                copy_missing_tree(&p, &rel, copied);
            } else if !rel.exists() {
                if let Err(e) = std::fs::copy(&p, &rel) {
                    log_launch(&format!("journeymap: no se pudo copiar {name}: {e}"));
                } else {
                    *copied += 1;
                }
            }
        }
    }
}

fn count_files_tree(dir: &Path) -> u64 {
    let mut n = 0u64;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                n += count_files_tree(&p);
            } else {
                n += 1;
            }
        }
    }
    n
}

fn merge_journeymap_servers(instance_dir: &Path) {
    let mp = instance_dir.join("journeymap").join("data").join("mp");
    if !mp.is_dir() {
        return;
    }
    let mut groups: std::collections::HashMap<String, Vec<std::path::PathBuf>> =
        std::collections::HashMap::new();
    if let Ok(entries) = std::fs::read_dir(&mp) {
        for entry in entries.flatten() {
            let p = entry.path();
            if !p.is_dir() {
                continue;
            }
            let name = p.file_name().unwrap_or_default().to_string_lossy().into_owned();
            let uuid = name.rsplit('_').next().unwrap_or(&name).to_string();
            groups.entry(uuid).or_default().push(p);
        }
    }
    let mut total = 0u64;
    for (uuid, mut folders) in groups.into_iter() {
        if folders.len() < 2 {
            continue;
        }
        folders.sort_by(|a, b| {
            let ca = count_files_tree(a);
            let cb = count_files_tree(b);
            cb.cmp(&ca)
        });
        let count = folders.len();
        let target = folders.remove(0);
        let target_count = count_files_tree(&target);
        for src in folders {
            let mut copied = 0u64;
            copy_missing_tree(&src, &target, &mut copied);
            total += copied;
        }
        log_launch(&format!(
            "journeymap: combinadas {} carpetas del servidor (uuid {uuid}), archivos totales ~{}",
            count + 1,
            target_count + total
        ));
    }
}

fn ensure_default_resourcepacks(instance_dir: &Path) {
    let mut names: Vec<String> = crate::sync::load_manifest_cache(instance_dir)
        .map(|m| m.default_resourcepacks)
        .unwrap_or_default();
    if names.is_empty() {
        names = crate::config::DEFAULT_RESOURCE_PACKS.iter().map(|s| s.to_string()).collect();
    }
    let packs_dir = instance_dir.join("resourcepacks");
    let mut wanted: Vec<String> = Vec::new();
    for raw in names {
        let raw = raw.trim().to_string();
        if raw.is_empty() {
            continue;
        }
        let entry = if raw.starts_with("file/") || raw == "vanilla" || raw.contains(':') {
            raw.clone()
        } else {
            format!("file/{raw}")
        };
        let rel = entry.strip_prefix("file/").unwrap_or(&entry);
        if packs_dir.join(rel).exists() {
            wanted.push(entry);
        }
    }
    if wanted.is_empty() {
        return;
    }
    let options_path = instance_dir.join("options.txt");
    let existing = fs::read_to_string(&options_path).unwrap_or_default();
    let eol = if existing.contains("\r\n") { "\r\n" } else { "\n" };
    let mut lines: Vec<String> = existing.lines().map(|l| l.trim_end().to_string()).collect();

    let mut packs: Vec<String> = Vec::new();
    let mut packs_idx: Option<usize> = None;
    for (i, line) in lines.iter().enumerate() {
        if let Some(rest) = line.strip_prefix("resourcePacks:") {
            packs = serde_json::from_str::<Vec<String>>(rest.trim()).unwrap_or_default();
            packs_idx = Some(i);
            break;
        }
    }
    if packs.is_empty() {
        packs.push("vanilla".to_string());
    }
    let mut changed = false;
    for w in &wanted {
        if !packs.contains(w) {
            packs.push(w.clone());
            changed = true;
        }
    }
    let pack_line = format!("resourcePacks:{}", serde_json::to_string(&packs).unwrap_or_default());
    match packs_idx {
        Some(i) => {
            if lines[i] != pack_line {
                lines[i] = pack_line;
                changed = true;
            }
        }
        None => {
            lines.push(pack_line);
            changed = true;
        }
    }
    for line in lines.iter_mut() {
        if let Some(rest) = line.strip_prefix("incompatibleResourcePacks:") {
            if let Ok(mut inc) = serde_json::from_str::<Vec<String>>(rest.trim()) {
                let before = inc.len();
                inc.retain(|p| !wanted.contains(p));
                if inc.len() != before {
                    *line = format!(
                        "incompatibleResourcePacks:{}",
                        serde_json::to_string(&inc).unwrap_or_default()
                    );
                    changed = true;
                }
            }
        }
    }
    if !changed {
        return;
    }
    let mut out = lines.join(eol);
    out.push_str(eol);
    if let Err(e) = fs::write(&options_path, out) {
        log_launch(&format!("no se pudo activar el texture pack por defecto: {e}"));
    } else {
        log_launch(&format!("texture pack por defecto activado: {}", wanted.join(", ")));
    }
}

pub async fn launch_minecraft(
    app: AppHandle,
    instance_name: &str,
    java_path: &str,
    account: crate::config::Account,
) -> Result<u32, String> {
    let instance_dir = instances_dir().join(instance_name);
    if !instance_dir.exists() {
        return Err("La instancia no existe".to_string());
    }
    let meta = crate::instances::load_meta(&instance_dir);

    let version_id = if !meta.version_id.is_empty() {
        meta.version_id.clone()
    } else {
        crate::instances::installed_version_id(&instance_dir, &meta)
            .unwrap_or_else(|| meta.game_version.clone())
    };

    let mut resolved = resolve_version(&instance_dir, &version_id)?;
    if let Err(first_err) = validate_launch_files(
        &app,
        instance_name,
        &instance_dir,
        &version_id,
        &meta.loader,
        &resolved,
    ) {
        // Auto-reparación: descarga lo que falte y reintenta una sola vez.
        let _ = app.emit(
            "launch/line",
            LogLine {
                line: "[info] Faltan archivos de la instalación, descargando faltantes..."
                    .to_string(),
                stream: "stdout".to_string(),
            },
        );
        log_launch(&format!("reparando '{instance_name}' tras: {first_err}"));
        match repair_launch_files(&instance_dir, &meta).await {
            Ok(()) => {
                resolved = resolve_version(&instance_dir, &version_id)?;
                validate_launch_files(
                    &app,
                    instance_name,
                    &instance_dir,
                    &version_id,
                    &meta.loader,
                    &resolved,
                )?;
            }
            Err(e) => {
                log_launch(&format!("reparación fallida de '{instance_name}': {e}"));
                return Err(first_err);
            }
        }
    }
        ensure_default_resourcepacks(&instance_dir);
        merge_journeymap_servers(&instance_dir);
        let (ram_min_mb, ram_max_mb) = crate::jvm::effective_max_ram(&instance_dir);
    let command = build_launch_command(
        java_path,
        &resolved,
        &instance_dir,
        &account,
        ram_min_mb,
        ram_max_mb,
    );
    let settings = crate::config::Settings::load();
    if settings.antilag_enabled && !settings.antilag_host.trim().is_empty() {
        match crate::antilag::start_tunnel(&settings.antilag_host) {
            Ok(n) => {
                println!(
                    "[antilag] túnel activo para {n} IP(s) del servidor '{}'",
                    settings.antilag_host
                );
            }
            Err(e) => {
                println!("[antilag] aviso: {e}");
            }
        }
    }

    let mut cmd = Command::new(&command[0]);
    cmd.args(&command[1..])
        .current_dir(&instance_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: sin ventana de consola
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("error al lanzar el juego: {e}"))?;

    let pid = child.id();

    let stamp = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
    fs::write(instance_dir.join("last_launched.txt"), stamp).ok();
    crate::discord::set_playing(instance_name, &meta.game_version, &meta.loader);

    let started = std::time::Instant::now();
    if let Some(stdout) = child.stdout.take() {
        use std::io::BufRead;
        let reader = std::io::BufReader::new(stdout);
        let app2 = app.clone();
        let instance_dir2 = instance_dir.clone();
        std::thread::spawn(move || {
            for line in reader.lines().flatten() {
                let _ = app2.emit(
                    "launch/line",
                    LogLine { line: line.clone(), stream: "stdout".to_string() },
                );
            }
            if started.elapsed().as_secs() < 6 {
                let _ = app2.emit(
                    "launch/error",
                    "El proceso de Java terminó nada más arrancar. Suele deberse a la memoria asignada o a un flag de JVM: prueba a ajustar la RAM en Ajustes, o a quitar el Java manual si lo tenías puesto.",
                );
            }
            let _ = app2.emit("launch/exit", pid);
            ensure_default_resourcepacks(&instance_dir2);
            merge_journeymap_servers(&instance_dir2);
            crate::antilag::stop_tunnel();
            crate::discord::set_launcher_idle();
        });
    }
    if let Some(stderr) = child.stderr.take() {
        use std::io::BufRead;
        let reader = std::io::BufReader::new(stderr);
        let app2 = app.clone();
        std::thread::spawn(move || {
            for line in reader.lines().flatten() {
                let _ = app2.emit(
                    "launch/line",
                    LogLine { line: line.clone(), stream: "stderr".to_string() },
                );
            }
        });
    }

    {
        let mut state = LAUNCH_STATE.lock().map_err(|_| "lock launch state")?;
        state.process = Some(LaunchProcess { pid, child });
    }

    Ok(pid)
}

pub fn kill_game() -> Result<(), String> {
    let mut state = LAUNCH_STATE.lock().map_err(|_| "lock launch state")?;
    if let Some(mut proc) = state.process.take() {
        let _ = proc.child.kill();
    }
    crate::discord::set_launcher_idle();
    Ok(())
}

pub fn is_running() -> bool {
    LAUNCH_STATE.lock().map(|s| s.process.is_some()).unwrap_or(false)
}