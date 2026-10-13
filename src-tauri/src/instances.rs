use crate::config::{instances_dir, INSTANCE_META_FILE};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct InstanceMeta {
    pub name: String,
    pub version_id: String,
    pub loader: String,        // vanilla | forge | fabric | neoforge
    pub game_version: String,
    pub loader_version: String,
    pub source: String,        // remote | local | mrpack | pau
    pub manifest_url: String,  // donde el owner publica las actualizaciones
    pub manifest_type: String, // "pau" | "modpack" | ""
    pub manifest_revision: String,
    pub server_version: String,
    pub actualizacion: bool,
    pub extra_info: String,
    pub github_repo: String,     // "owner/repo"
    pub github_branch: String,   // rama (por defecto "main")
    pub github_path: String,     // carpeta del pack dentro de la repo ("" = raíz)
    pub github_base_sha: String, // último HEAD aplicado (para el diff por commits)
    pub github_tracked: Vec<String>, // paths gestionados remotamente en el último sync
}

impl Default for InstanceMeta {
    fn default() -> Self {
        InstanceMeta {
            name: String::new(),
            version_id: String::new(),
            loader: "vanilla".to_string(),
            game_version: String::new(),
            loader_version: String::new(),
            source: "remote".to_string(),
            manifest_url: String::new(),
            manifest_type: String::new(),
            manifest_revision: String::new(),
            server_version: String::new(),
            actualizacion: true,
            extra_info: String::new(),
            github_repo: String::new(),
            github_branch: String::new(),
            github_path: String::new(),
            github_base_sha: String::new(),
            github_tracked: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct InstanceInfo {
    pub name: String,
    pub version_id: String,
    pub loader: String,
    pub game_version: String,
    pub loader_version: String,
    pub source: String,
    pub manifest_url: String,
    pub manifest_type: String,
    pub server_version: String,
    pub actualizacion: bool,
    pub installed: bool,
    pub instance_dir: String,
    pub last_launched: String,
    pub session_based: bool,
    pub remote: bool, // tiene manifest_url o carpeta de repo
}

pub fn instance_path(name: &str) -> PathBuf {
    instances_dir().join(name)
}

pub fn load_meta(instance_dir: &Path) -> InstanceMeta {
    let path = instance_dir.join(INSTANCE_META_FILE);
    if let Ok(text) = fs::read_to_string(&path) {
        if let Ok(meta) = serde_json::from_str::<InstanceMeta>(&text) {
            return meta;
        }
    }
    InstanceMeta::default()
}

pub fn save_meta(instance_dir: &Path, meta: &InstanceMeta) -> Result<(), String> {
    fs::create_dir_all(instance_dir).map_err(|e| format!("crear instancia: {e}"))?;
    let json =
        serde_json::to_string_pretty(meta).map_err(|e| format!("serialize meta: {e}"))?;
    fs::write(instance_dir.join(INSTANCE_META_FILE), json)
        .map_err(|e| format!("guardar meta: {e}"))
}
pub fn installed_version_id(instance_dir: &Path, meta: &InstanceMeta) -> Option<String> {
    let versions_dir = instance_dir.join("versions");
    if !versions_dir.is_dir() {
        return None;
    }
    let ids: Vec<String> = fs::read_dir(&versions_dir)
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect()
        })
        .unwrap_or_default();
    let game = &meta.game_version;
    let loader = meta.loader.to_lowercase();
    if loader != "vanilla" && !loader.is_empty() {
        let candidates: Vec<String> = ids
            .iter()
            .filter(|id| {
                let lower = id.to_lowercase();
                if !lower.contains(&loader) || !lower.contains(&game.to_lowercase()) {
                    return false;
                }
                if !meta.loader_version.is_empty()
                    && !lower.contains(&meta.loader_version.to_lowercase())
                {
                    return false;
                }
                true
            })
            .cloned()
            .collect();
        if let Some(found) = candidates.into_iter().next() {
            return Some(found);
        }
    }
    if ids.iter().any(|id| id == game) {
        return Some(game.clone());
    }
    None
}

pub fn is_installed(instance_dir: &Path, meta: &InstanceMeta) -> bool {
    installed_version_id(instance_dir, meta).is_some()
        || instance_dir.join(INSTANCE_META_FILE).exists()
}
pub fn scan_instances() -> Vec<InstanceInfo> {
    let base = instances_dir();
    let mut results = Vec::new();
    if let Ok(entries) = fs::read_dir(&base) {
        for entry in entries.flatten() {
            if !entry.path().is_dir() {
                continue;
            }
            let dir = entry.path();
            let meta = load_meta(&dir);
            let name = if meta.name.is_empty() {
                entry.file_name().to_string_lossy().to_string()
            } else {
                meta.name.clone()
            };
            let version_id = if !meta.version_id.is_empty() {
                meta.version_id.clone()
            } else {
                installed_version_id(&dir, &meta).unwrap_or_default()
            };
            let installed = is_installed(&dir, &meta);
            let last_launched = fs::read_to_string(dir.join("last_launched.txt")).unwrap_or_default();
            results.push(InstanceInfo {
                name,
                version_id,
                loader: meta.loader.clone(),
                game_version: meta.game_version.clone(),
                loader_version: meta.loader_version.clone(),
                source: meta.source.clone(),
                manifest_url: meta.manifest_url.clone(),
                manifest_type: meta.manifest_type.clone(),
                server_version: meta.server_version.clone(),
                actualizacion: meta.actualizacion,
                installed,
                instance_dir: dir.to_string_lossy().to_string(),
                last_launched,
                session_based: false,
                remote: !meta.manifest_url.trim().is_empty() || !meta.github_repo.trim().is_empty(),
            });
        }
    }
    results
}

#[derive(Serialize)]
pub struct CreateInstanceRequest {
    pub name: String,
    pub game_version: String,
    pub loader: String,
    pub loader_version: String,
    pub manifest_url: String,
}

impl CreateInstanceRequest {
    pub fn sanitized_name(&self) -> String {
        let cleaned: String = self
            .name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let trimmed = cleaned.trim_matches('_').to_string();
        if trimmed.is_empty() {
            "Modpack".to_string()
        } else {
            trimmed
        }
    }
}

pub fn create_instance_dir(req: &CreateInstanceRequest) -> Result<PathBuf, String> {
    let dir = instance_path(&req.sanitized_name());
    if dir.exists() {
        return Err(format!("La instancia '{}' ya existe", req.sanitized_name()));
    }
    fs::create_dir_all(&dir).map_err(|e| format!("crear instancia: {e}"))?;
    for sub in ["versions", "libraries", "assets/indexes", "assets/objects", "mods", "config", "saves", "resourcepacks"] {
        fs::create_dir_all(dir.join(sub)).ok();
    }
    Ok(dir)
}

pub fn delete_instance(name: &str) -> Result<(), String> {
    let dir = instance_path(name);
    if !dir.exists() {
        return Err("La instancia no existe".to_string());
    }
    fs::remove_dir_all(&dir).map_err(|e| format!("eliminar instancia: {e}"))
}

pub fn touch_last_launched(name: &str) {
    let dir = instance_path(name);
    let stamp = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
    fs::write(dir.join("last_launched.txt"), stamp).ok();
}