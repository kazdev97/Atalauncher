use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
pub const APP_NAME: &str = "Atalauncher";
pub const INSTANCE_META_FILE: &str = "kazu_instance.json";
pub const DEFAULT_RESOURCE_PACKS: &[&str] = &["HalloGeey_TexturePack.zip"];
pub const DEFAULT_SOURCE_URL: &str =
    "https://raw.githubusercontent.com/kazdev97/Hallogeey/main/index.json";

fn candidate_dirs() -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(d) = dirs::document_dir() {
        candidates.push(d.join(APP_NAME));
    }
    if let Some(d) = dirs::data_dir() {
        candidates.push(d.join(APP_NAME));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            candidates.push(parent.join(APP_NAME));
        }
    }
    if let Some(d) = dirs::home_dir() {
        candidates.push(d.join(APP_NAME));
    }
    candidates.push(PathBuf::from(".").join(APP_NAME));
    candidates
}

pub fn launcher_data_dir() -> PathBuf {
    static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let candidates = candidate_dirs();
        for dir in &candidates {
            if dir_is_usable(dir) {
                return dir.clone();
            }
        }
        candidates[0].clone()
    })
    .clone()
}

/// Lee un archivo de datos probando todas las ubicaciones y quedandose con el
/// mas reciente, para no perder lo guardado en una carpeta de respaldo.
fn read_newest(name: &str) -> Option<String> {
    let mut best: Option<(std::time::SystemTime, String)> = None;
    for dir in candidate_dirs() {
        let path = dir.join(name);
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let stamp = fs::metadata(&path)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::UNIX_EPOCH);
        if best.as_ref().map(|(s, _)| stamp > *s).unwrap_or(true) {
            best = Some((stamp, text));
        }
    }
    best.map(|(_, text)| text)
}

/// Guarda un archivo de datos probando todas las ubicaciones hasta que una
/// funcione (Documentos puede estar redirigido a OneDrive o a una unidad caida).
fn write_data_file(name: &str, text: &str) -> Result<(), String> {
    let mut last_err = String::from("no hay ninguna carpeta disponible");
    for dir in candidate_dirs() {
        if !dir_is_usable(&dir) {
            continue;
        }
        match fs::write(dir.join(name), text) {
            Ok(()) => return Ok(()),
            Err(e) => last_err = format!("{}: {e}", dir.display()),
        }
    }
    Err(last_err)
}

/// Comprueba que la carpeta se pueda crear y escribir de verdad. Si la ruta de
/// Documentos esta redirigida a una unidad que no existe (OneDrive desconectado,
/// disco extraible) create_dir_all falla con 'os error 3' y el guardado reventaba.
fn dir_is_usable(dir: &Path) -> bool {
    if fs::create_dir_all(dir).is_err() {
        return false;
    }
    let probe = dir.join(".write_probe");
    if fs::write(&probe, b"ok").is_err() {
        return false;
    }
    fs::remove_file(&probe).ok();
    true
}

pub fn instances_dir() -> PathBuf {
    let dir = launcher_data_dir().join("instancias");
    fs::create_dir_all(&dir).ok();
    dir
}
pub fn runtime_dir() -> PathBuf {
    let dir = launcher_data_dir().join("runtime");
    fs::create_dir_all(&dir).ok();
    dir
}

pub fn accounts_file() -> PathBuf {
    launcher_data_dir().join("accounts.json")
}

pub fn settings_file() -> PathBuf {
    launcher_data_dir().join("settings.json")
}

pub fn launcher_log_file() -> PathBuf {
    launcher_data_dir().join("atalauncher.log")
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub ram_mb: u32,
    pub max_ram_mb: u32,
    pub java_path_override: String,
    pub streamer_mode: bool,
    pub close_launcher_after_launch: bool,
    pub game_res_width: u32,
    pub game_res_height: u32,
    pub fullscreen: bool,
    pub last_username: String,
    pub source_url: String,
    pub auto_update: bool,
    pub antilag_enabled: bool,
    pub antilag_host: String,
    pub discord_rpc_enabled: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            ram_mb: 4096,
            max_ram_mb: 0,
            java_path_override: String::new(),
            streamer_mode: true,
            close_launcher_after_launch: false,
            game_res_width: 0,
            game_res_height: 0,
            fullscreen: false,
            last_username: String::new(),
            source_url: DEFAULT_SOURCE_URL.to_string(),
            auto_update: true,
            antilag_enabled: false,
            antilag_host: String::new(),
            discord_rpc_enabled: true,
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        if let Some(text) = read_newest("settings.json") {
            if let Ok(s) = serde_json::from_str(&text) {
                return s;
            }
        }
        Settings::default()
    }

    pub fn save(&self) -> Result<(), String> {
        let json =
            serde_json::to_string_pretty(self).map_err(|e| format!("serialize settings: {e}"))?;
        write_data_file("settings.json", &json).map_err(|e| format!("guardar settings: {e}"))
    }
    pub fn effective_ram_mb(&self) -> u32 {
        if self.max_ram_mb > 0 {
            self.max_ram_mb
        } else {
            self.ram_mb
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub name: String,
    pub access_token: String,
    pub refresh_token: String,
    #[serde(default)]
    pub needs_relogin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AccountStore {
    pub accounts: Vec<Account>,
    pub selected_id: String,
    pub mode: String, // "offline" | "online"
    #[serde(default)]
    pub offline_username: String,
}

impl AccountStore {
    pub fn load() -> Self {
        if let Some(text) = read_newest("accounts.json") {
            if let Ok(store) = serde_json::from_str(&text) {
                return store;
            }
        }
        AccountStore {
            accounts: Vec::new(),
            selected_id: String::new(),
            mode: "offline".to_string(),
            offline_username: String::new(),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("serialize accounts: {e}"))?;
        write_data_file("accounts.json", &json).map_err(|e| format!("guardar cuentas: {e}"))
    }

    pub fn selected(&self) -> Option<&Account> {
        self.accounts.iter().find(|a| a.id == self.selected_id)
    }

    pub fn get(&self, account_id: &str) -> Option<&Account> {
        self.accounts.iter().find(|a| a.id == account_id)
    }
    pub fn set_selected(&mut self, account_id: &str) -> Result<(), String> {
        if !self.accounts.iter().any(|a| a.id == account_id) {
            return Err("La cuenta no está en el almacén".to_string());
        }
        self.selected_id = account_id.to_string();
        self.mode = "online".to_string();
        self.save()
    }
    pub fn upsert(&mut self, account: Account) {
        let id = account.id.clone();
        if let Some(existing) = self.accounts.iter_mut().find(|a| a.id == id) {
            *existing = account;
        } else {
            self.accounts.push(account);
        }
        self.selected_id = id;
        self.mode = "online".to_string();
    }

    pub fn remove(&mut self, account_id: &str) {
        self.accounts.retain(|a| a.id != account_id);
        if self.selected_id == account_id {
            if let Some(first) = self.accounts.first() {
                self.selected_id = first.id.clone();
                self.mode = "online".to_string();
            } else {
                self.selected_id = String::new();
                self.mode = "offline".to_string();
            }
        }
    }
}
pub fn offline_account(username: &str) -> Account {
    let hash = crate::downloader::sha256_bytes(username.trim().as_bytes());
    let hex: String = hash.chars().take(32).collect();
    let uuid = format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    );
    Account {
        id: uuid,
        name: username.trim().to_string(),
        access_token: String::new(),
        refresh_token: String::new(),
        needs_relogin: false,
    }
}
pub fn path_exists(p: impl AsRef<Path>) -> bool {
    p.as_ref().exists()
}
pub fn set_selected_account(account_id: &str) -> Result<(), String> {
    let mut store = AccountStore::load();
    store.set_selected(account_id)
}
pub fn remove_account(account_id: &str) -> Result<(), String> {
    let mut store = AccountStore::load();
    store.remove(account_id);
    store.save()
}