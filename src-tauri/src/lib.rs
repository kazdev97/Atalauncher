pub mod antilag;
pub mod auth;
pub mod config;
pub mod downloader;
pub mod instances;
pub mod installer;
pub mod java;
pub mod jvm;
pub mod launch;
pub mod packager;
pub mod sync;
pub mod updater;
pub mod discord;

use config::{Account, AccountStore, Settings};
use serde::Serialize;
use tauri::Emitter;
#[derive(Clone, Serialize)]
pub struct ProgressMsg {
    pub percent: u32,
    pub status: String,
}
#[tauri::command]
fn login_get_state() -> Result<AccountStore, String> {
    Ok(config::AccountStore::load())
}

#[tauri::command]
async fn login_start(app: tauri::AppHandle) -> Result<Account, String> {
    auth::start_login_flow(app).await
}

#[tauri::command]
fn login_get_account(account_id: String) -> Result<Account, String> {
    let store = config::AccountStore::load();
    store
        .get(&account_id)
        .cloned()
        .ok_or_else(|| "cuenta no encontrada".to_string())
}

#[tauri::command]
fn login_set_selected(account_id: String) -> Result<(), String> {
    config::set_selected_account(&account_id)
}

#[tauri::command]
fn login_remove_account(account_id: String) -> Result<(), String> {
    config::remove_account(&account_id)
}
#[tauri::command]
fn login_set_offline(username: String) -> Result<(), String> {
    let name = username.trim().to_string();
    if name.is_empty() {
        return Err("Escribe un nombre de usuario para el modo No premium.".to_string());
    }
    let mut store = AccountStore::load();
    store.offline_username = name;
    store.selected_id = String::new();
    store.mode = "offline".to_string();
    store.save()
}
#[tauri::command]
async fn java_resolve(app: tauri::AppHandle, name: String) -> Result<java::JavaInfo, String> {
    let handle = app.clone();
    let dir = instances::instance_path(&name);
    let meta = instances::load_meta(&dir);
    let required = java::resolve_java_major_online(&meta.game_version, &dir).await;
    let exe = java::ensure_java(&meta.game_version, &dir, &mut |status| {
        let _ = handle.emit("java/progress", status);
    })
    .await?;
    let major = java::validate_java_executable(&exe, required).unwrap_or(required);
    Ok(java::JavaInfo { path: exe.to_string_lossy().to_string(), major, is_valid: true })
}

#[tauri::command]
async fn java_get_required(game_version: String) -> u32 {
    java::required_java_major_heuristic(&game_version)
}

#[tauri::command]
async fn java_find(major_version: u32) -> Result<java::JavaInfo, String> {
    if let Some(exe) = java::find_system_java(major_version) {
        let major = java::validate_java_executable(&exe, major_version).unwrap_or(major_version);
        return Ok(java::JavaInfo { path: exe.to_string_lossy().to_string(), major, is_valid: true });
    }
    if let Some(exe) = java::find_bundled_java(major_version) {
        let major = java::validate_java_executable(&exe, major_version).unwrap_or(major_version);
        return Ok(java::JavaInfo { path: exe.to_string_lossy().to_string(), major, is_valid: true });
    }
    Err(format!("No hay un Java {major_version}+ disponible"))
}

#[tauri::command]
async fn java_install(
    app: tauri::AppHandle,
    major_version: u32,
) -> Result<java::JavaInfo, String> {
    let handle = app.clone();
    let exe = java::install_adoptium(major_version, &mut move |status| {
        let _ = handle.emit("java/progress", status);
    })
    .await?;
    let major = java::validate_java_executable(&exe, major_version).unwrap_or(major_version);
    Ok(java::JavaInfo { path: exe.to_string_lossy().to_string(), major, is_valid: true })
}

#[tauri::command]
async fn java_get_system() -> Result<Vec<java::JavaInfo>, String> {
    java::find_all_system_java()
}

#[tauri::command]
async fn java_get_bundled() -> Vec<java::JavaInfo> {
    java::find_all_bundled_java()
}
#[tauri::command]
async fn update_check() -> Result<updater::UpdateInfo, String> {
    updater::check_update().await
}

#[tauri::command]
async fn update_apply() -> Result<(), String> {
    updater::apply_update().await
}
#[tauri::command]
fn instances_list() -> Vec<instances::InstanceInfo> {
    instances::scan_instances()
}

#[tauri::command]
async fn system_info() -> jvm::SystemInfo {
    jvm::system_info()
}

#[tauri::command]
fn ram_recommend(name: String) -> Result<jvm::RamRecommend, String> {
    jvm::ram_recommend(&name)
}

#[tauri::command]
async fn instances_create(
    name: String,
    game_version: String,
    loader: String,
    loader_version: String,
    manifest_url: String,
) -> Result<instances::InstanceInfo, String> {
    let req = instances::CreateInstanceRequest {
        name,
        game_version,
        loader,
        loader_version,
        manifest_url,
    };
    let dir = instances::create_instance_dir(&req)?;
    let clean = req.sanitized_name();
    let meta = instances::InstanceMeta {
        name: clean.clone(),
        game_version: req.game_version.clone(),
        loader: req.loader.clone(),
        loader_version: req.loader_version.clone(),
        manifest_url: req.manifest_url.clone(),
        ..Default::default()
    };
    instances::save_meta(&dir, &meta)?;
    instances::scan_instances()
        .into_iter()
        .find(|i| i.name == clean)
        .ok_or_else(|| "no se pudo crear la instancia".to_string())
}

#[tauri::command]
fn instances_get_meta(name: String) -> Result<instances::InstanceMeta, String> {
    let dir = instances::instance_path(&name);
    if !dir.exists() {
        return Err("La instancia no existe".into());
    }
    Ok(instances::load_meta(&dir))
}

#[tauri::command]
fn instances_delete(name: String) -> Result<(), String> {
    instances::delete_instance(&name)
}
#[tauri::command]
async fn sync_check(name: String) -> Result<sync::SyncDiff, String> {
    let dir = instances::instance_path(&name);
    let meta = instances::load_meta(&dir);
    if !meta.github_repo.trim().is_empty() {
        return sync::git_check_dir(&dir, &meta).await;
    }
    if meta.manifest_url.is_empty() {
        return Err("La instancia no tiene URL de manifest configurada".into());
    }
    sync::check_updates(&dir, &meta.manifest_url).await
}

#[tauri::command]
async fn sync_apply(app: tauri::AppHandle, name: String) -> Result<sync::SyncResult, String> {
    let dir = instances::instance_path(&name);
    let meta = instances::load_meta(&dir);
    let handle = app.clone();
    let mut on_status = |status: String| {
        let _ = handle.emit("sync/status", status);
    };
    let mut on_progress = |percent: u32, status: String| {
        let _ = handle.emit("sync/progress", ProgressMsg { percent, status });
    };
    if !meta.github_repo.trim().is_empty() {
        return sync::git_apply_dir(&dir, &mut on_status, &mut on_progress).await;
    }
    if meta.manifest_url.is_empty() {
        return Err("La instancia no tiene URL de manifest configurada".into());
    }
    sync::apply_updates(&dir, &meta.manifest_url, &mut on_status).await
}

#[tauri::command]
async fn apply_config_remote(app: tauri::AppHandle, name: String) -> Result<sync::SyncResult, String> {
    let dir = instances::instance_path(&name);
    let meta = instances::load_meta(&dir);
    if meta.manifest_url.is_empty() {
        return Err("La instancia no tiene URL de manifest configurada".into());
    }
    let handle = app.clone();
    let mut on_status = |status: String| {
        let _ = handle.emit("sync/status", status);
    };
    sync::apply_config_updates(&dir, &meta.manifest_url, &mut on_status).await
}

#[tauri::command]
fn sync_rollback(name: String, revision: String) -> Result<sync::SyncResult, String> {
    let dir = instances::instance_path(&name);
    sync::rollback(&dir, &revision)
}

#[tauri::command]
fn sync_list_backups(name: String) -> Vec<String> {
    sync::list_backups(&name)
}
#[tauri::command]
async fn auto_sync_check() -> Result<Vec<sync::InstanceUpdateState>, String> {
    sync::auto_check_all().await
}

#[tauri::command]
async fn auto_sync_apply(app: tauri::AppHandle) -> Result<sync::AutoSyncSummary, String> {
    let handle = app.clone();
    sync::auto_sync_all(&mut |status| {
        let _ = handle.emit("sync/status", status);
    })
    .await
}
#[tauri::command]
fn pack_export(
    instance_name: String,
    destination_dir: String,
    archive_url: String,
    file_base_url: String,
    extra_folders: Vec<String>,
    ram_min_mb: u32,
    ram_max_mb: u32,
) -> Result<packager::ExportResult, String> {
    let instance_dir = instances::instance_path(&instance_name);
    if !instance_dir.exists() {
        return Err("La instancia no existe".into());
    }
    let dest = if destination_dir.trim().is_empty() {
        dirs::desktop_dir()
            .unwrap_or_else(|| config::launcher_data_dir())
            .join("Atalauncher-packs")
    } else {
        std::path::PathBuf::from(destination_dir)
    };
    packager::export_instance(&instance_dir, &dest, &archive_url, &file_base_url, &extra_folders, ram_min_mb, ram_max_mb)
}

#[tauri::command]
fn pack_whitepaper() -> String {
    packager::export_whitepaper().to_string()
}
#[tauri::command]
async fn source_fetch(url: Option<String>) -> Result<Vec<sync::SourceInstance>, String> {
    let url = url
        .filter(|u| !u.trim().is_empty())
        .unwrap_or_else(|| config::Settings::load().source_url);
    let installed: Vec<String> = instances::scan_instances()
        .into_iter()
        .map(|i| i.name)
        .collect();
    let mut list = sync::fetch_source_index(&url).await?;
    let normalize = |s: &str| -> String {
        s.to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect()
    };
    for s in list.iter_mut() {
        if s.manifest_url.trim().is_empty() && s.github_repo.trim().is_empty() {
            s.manifest_url = url.clone();
        }
        let key = normalize(&s.name);
        s.installed = installed.iter().any(|n| normalize(n) == key);
    }
    Ok(list)
}

#[tauri::command]
async fn source_install(
    app: tauri::AppHandle,
    name: String,
    game_version: String,
    loader: String,
    loader_version: String,
    manifest_url: String,
    github_repo: String,
    github_path: String,
    github_branch: String,
) -> Result<(), String> {
    let handle = app.clone();
    sync::install_remote_instance(
        &name,
        &game_version,
        &loader,
        &loader_version,
        &manifest_url,
        &github_repo,
        &github_path,
        &github_branch,
        &mut |status| {
            let _ = handle.emit("sync/status", status);
        },
        &mut |percent, status| {
            let _ = handle.emit("sync/progress", ProgressMsg { percent, status });
        },
    )
    .await
}
#[tauri::command]
async fn install_vanilla_versions() -> Result<Vec<installer::VersionOverview>, String> {
    installer::get_vanilla_versions().await
}

#[tauri::command]
async fn install_fabric_loaders() -> Result<Vec<String>, String> {
    installer::get_fabric_loaders().await
}

#[tauri::command]
async fn install_forge_versions(game_version: String) -> Result<Vec<String>, String> {
    installer::get_forge_versions(&game_version).await
}

#[tauri::command]
async fn install_neoforge_versions(game_version: String) -> Result<Vec<String>, String> {
    installer::get_neoforge_versions(&game_version).await
}

#[tauri::command]
async fn install_instance(app: tauri::AppHandle, name: String) -> Result<String, String> {
    let dir = instances::instance_path(&name);
    let _ = &app;
    installer::install_instance(&dir, &mut |_, _| {}).await
}
#[tauri::command]
async fn launch_game(
    app: tauri::AppHandle,
    name: String,
    java_path: String,
) -> Result<u32, String> {
    let mut store = config::AccountStore::load();
    let account = if store.mode == "offline" && !store.offline_username.trim().is_empty() {
        config::offline_account(&store.offline_username)
    } else {
        let mut selected = store.selected().cloned().ok_or_else(|| {
            "No hay una cuenta seleccionada (inicia sesión con Microsoft o activa el modo No premium con un nombre de usuario)."
                .to_string()
        })?;
        if !selected.refresh_token.is_empty() {
            match auth::refresh_token(crate::auth::CLIENT_ID, &selected.refresh_token).await {
                Ok((fresh, _)) => {
                    selected = fresh.clone();
                    store.upsert(fresh);
                    let _ = store.save();
                }
                Err(e) => {
                    if e.contains("invalid_grant") {
                        selected.needs_relogin = true;
                        store.upsert(selected.clone());
                        let _ = store.save();
                        return Err(
                            "Tu sesión de Microsoft caducó. Vuelve a iniciar sesión con Relogin en la pestaña Cuenta."
                                .to_string(),
                        );
                    }
                    return Err(format!(
                        "No se pudo renovar tu sesión de Microsoft: {e}. Inténtalo de nuevo o usa Relogin en la pestaña Cuenta."
                    ));
                }
            }
        }
        selected
    };
    launch::launch_minecraft(app, &name, &java_path, account).await
}

#[tauri::command]
fn launch_kill() -> Result<(), String> {
    launch::kill_game()
}

#[tauri::command]
fn launch_is_running() -> bool {
    launch::is_running()
}
#[tauri::command]
fn settings_get() -> Result<Settings, String> {
    Ok(Settings::load())
}

#[tauri::command]
fn settings_save(
    java_path_override: String,
    streamer_mode: bool,
    ram_override_mb: u32,
    source_url: String,
    auto_update: bool,
    _discord_rpc_enabled: Option<bool>,
) -> Result<(), String> {
    let mut s = Settings::load();
    s.java_path_override = java_path_override;
    s.streamer_mode = streamer_mode;
    s.max_ram_mb = ram_override_mb;
    s.source_url = source_url;
    s.auto_update = auto_update;
    s.discord_rpc_enabled = true;
    discord::set_enabled(true);
    s.save()
}

#[tauri::command]
fn discord_set_enabled(enabled: bool) -> Result<(), String> {
    let mut s = Settings::load();
    s.discord_rpc_enabled = enabled;
    discord::set_enabled(enabled);
    s.save()
}

#[tauri::command]
fn discord_set_page(page: String) -> Result<(), String> {
    discord::set_menu_page(&page);
    Ok(())
}
#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    open::that(url).map_err(|e| format!("abrir URL: {e}"))
}
#[tauri::command]
async fn antilag_install(app: tauri::AppHandle) -> Result<antilag::AntilagStatus, String> {
    let handle = app.clone();
    antilag::antilag_install(move |percent, status| {
        let _ = handle.emit(
            "antilag/progress",
            ProgressMsg { percent, status },
        );
    })
    .await
}

#[tauri::command]
fn antilag_status() -> Result<antilag::AntilagStatus, String> {
    Ok(antilag::status())
}

#[tauri::command]
fn antilag_set_enabled(enabled: bool) -> Result<(), String> {
    let mut s = Settings::load();
    s.antilag_enabled = enabled;
    s.save()
}

#[tauri::command]
fn antilag_set_host(host: String) -> Result<(), String> {
    let mut s = Settings::load();
    s.antilag_host = host.trim().to_string();
    s.save()
}
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    discord::init(true);

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            login_get_state,
            login_start,
            login_get_account,
            login_set_selected,
            login_remove_account,
            login_set_offline,
            java_get_required,
            java_resolve,
            java_find,
            java_install,
            java_get_system,
            java_get_bundled,
            instances_list,
            instances_create,
            instances_get_meta,
            instances_delete,
            system_info,
            ram_recommend,
            source_fetch,
            source_install,
            sync_check,
            sync_apply,
            apply_config_remote,
            sync_rollback,
            sync_list_backups,
            auto_sync_check,
            auto_sync_apply,
            update_check,
            update_apply,
            pack_export,
            pack_whitepaper,
            install_vanilla_versions,
            install_fabric_loaders,
            install_forge_versions,
            install_neoforge_versions,
            install_instance,
            launch_game,
            launch_kill,
            launch_is_running,
            settings_get,
            settings_save,
            discord_set_enabled,
            discord_set_page,
            antilag_install,
            antilag_status,
            antilag_set_enabled,
            antilag_set_host,
            open_url,
        ])
        .run(tauri::generate_context!())
        .expect("error al iniciar Atalauncher");
}