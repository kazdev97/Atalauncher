
use discord_rich_presence::{activity, DiscordIpc, DiscordIpcClient};
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const DISCORD_CLIENT_ID: &str = "1553302286309134397";
pub const DISCORD_APP_ICON_URL: &str =
    "https://cdn.discordapp.com/app-icons/1553302286309134397/44bd09416dc7c58f2f241ddbc92a989c.png";
pub const DISCORD_PAUCALIPSIS_ASSET: &str = "paucalipsi2";

fn log_discord(msg: &str) {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    let line = format!("[{now}] [discord] {msg}");
    println!("{line}");

    let log_path = crate::config::launcher_log_file();
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&log_path) {
        let _ = writeln!(f, "{line}");
    }
}

#[derive(Debug, Clone)]
pub enum DiscordAction {
    MenuPage(String),
    Playing {
        instance_name: String,
        game_version: String,
        loader: String,
        started_at: i64,
    },
    StoppedPlaying,
    Clear,
    SetEnabled(bool),
}

struct DiscordManager {
    sender: Option<Sender<DiscordAction>>,
    enabled: bool,
}

static DISCORD_MGR: once_cell::sync::Lazy<Arc<Mutex<DiscordManager>>> =
    once_cell::sync::Lazy::new(|| {
        Arc::new(Mutex::new(DiscordManager {
            sender: None,
            enabled: true,
        }))
    });
pub fn init(enabled: bool) {
    let mut mgr = match DISCORD_MGR.lock() {
        Ok(m) => m,
        Err(_) => return,
    };

    mgr.enabled = enabled;
    if let Some(tx) = &mgr.sender {
        let _ = tx.send(DiscordAction::SetEnabled(enabled));
        return;
    }

    let (tx, rx) = channel::<DiscordAction>();
    mgr.sender = Some(tx.clone());

    thread::spawn(move || {
        log_discord("Iniciando hilo de presencia de Discord...");
        let mut client: Option<DiscordIpcClient> = None;
        let mut is_enabled = enabled;
        let mut current_page = "login".to_string();
        let mut active_playing: Option<DiscordAction> = None;

        loop {
            let received = rx.recv_timeout(Duration::from_secs(5));
            match received {
                Ok(DiscordAction::SetEnabled(en)) => {
                    log_discord(&format!(
                        "Estado de Discord cambiado a: {}",
                        if en { "activado" } else { "desactivado" }
                    ));
                    is_enabled = en;
                    if !en {
                        if let Some(ref mut c) = client {
                            let _ = c.clear_activity();
                            let _ = c.close();
                        }
                        client = None;
                    } else if let Some(ref play_act) = active_playing {
                        apply_action(&mut client, play_act);
                    } else {
                        apply_action(&mut client, &DiscordAction::MenuPage(current_page.clone()));
                    }
                }
                Ok(DiscordAction::Clear) => {
                    active_playing = None;
                    if let Some(ref mut c) = client {
                        let _ = c.clear_activity();
                    }
                }
                Ok(DiscordAction::MenuPage(page)) => {
                    current_page = page.clone();
                    if is_enabled && active_playing.is_none() {
                        apply_action(&mut client, &DiscordAction::MenuPage(page));
                    }
                }
                Ok(DiscordAction::Playing {
                    instance_name,
                    game_version,
                    loader,
                    started_at,
                }) => {
                    let play_act = DiscordAction::Playing {
                        instance_name,
                        game_version,
                        loader,
                        started_at,
                    };
                    active_playing = Some(play_act.clone());
                    if is_enabled {
                        apply_action(&mut client, &play_act);
                    }
                }
                Ok(DiscordAction::StoppedPlaying) => {
                    active_playing = None;
                    if is_enabled {
                        apply_action(&mut client, &DiscordAction::MenuPage(current_page.clone()));
                    }
                }
                Err(RecvTimeoutError::Timeout) => {
                    if is_enabled && client.is_none() {
                        if let Some(ref play_act) = active_playing {
                            apply_action(&mut client, play_act);
                        } else {
                            apply_action(&mut client, &DiscordAction::MenuPage(current_page.clone()));
                        }
                    }
                }
                Err(RecvTimeoutError::Disconnected) => {
                    log_discord("Canal de Discord cerrado; finalizando hilo.");
                    break;
                }
            }
        }
    });

    if enabled {
        let _ = tx.send(DiscordAction::MenuPage("login".to_string()));
    }
}

fn ensure_connected(client: &mut Option<DiscordIpcClient>) -> bool {
    if client.is_none() {
        let mut c = DiscordIpcClient::new(DISCORD_CLIENT_ID);
        match c.connect() {
            Ok(_) => {
                log_discord("Conectado con éxito al socket IPC local de Discord.");
                *client = Some(c);
            }
            Err(e) => {
                log_discord(&format!(
                    "No se pudo conectar a Discord IPC (reintentará en segundo plano): {e:?}"
                ));
                return false;
            }
        }
    }
    true
}

fn apply_action(client: &mut Option<DiscordIpcClient>, action: &DiscordAction) {
    if !ensure_connected(client) {
        return;
    }

    let c = match client.as_mut() {
        Some(c) => c,
        None => return,
    };

    let result = match action {
        DiscordAction::MenuPage(page) => {
            let (state, details) = match page.as_str() {
                "instances" => ("En el launcher", "Explorando instancias"),
                "settings" => ("En el launcher", "En ajustes"),
                "console" => ("En el launcher", "Viendo la consola"),
                "login" => ("En el launcher", "Gestionando cuenta"),
                _ => ("Listo para jugar", "En el menú principal"),
            };
            let assets = activity::Assets::new()
                .large_image(DISCORD_APP_ICON_URL)
                .large_text("Atalauncher");

            let act = activity::Activity::new()
                .state(state)
                .details(details)
                .assets(assets);

            match c.set_activity(act) {
                Ok(_) => {
                    log_discord(&format!("Actividad menú actualizada: {details}"));
                    Ok(())
                }
                Err(e) => {
                    log_discord(&format!("Aviso con asset de app: {e:?}, reintentando sin imagen"));
                    let fallback = activity::Activity::new().state(state).details(details);
                    c.set_activity(fallback)
                }
            }
        }
        DiscordAction::Playing {
            instance_name,
            game_version,
            loader,
            started_at,
        } => {
            let state_text = format!("Jugando {}", instance_name);
            let loader_display = if loader.is_empty() || loader == "vanilla" {
                "Vanilla".to_string()
            } else {
                let mut chars = loader.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                }
            };
            let details_text = format!("Minecraft {} · {}", game_version, loader_display);
            let image_key = if instance_name.to_lowercase().contains("paucalipsis") {
                DISCORD_PAUCALIPSIS_ASSET
            } else {
                DISCORD_APP_ICON_URL
            };

            let assets = activity::Assets::new()
                .large_image(image_key)
                .large_text("Atalauncher");

            let timestamps = activity::Timestamps::new().start(*started_at);

            let act = activity::Activity::new()
                .state(&state_text)
                .details(&details_text)
                .assets(assets)
                .timestamps(timestamps);

            match c.set_activity(act) {
                Ok(_) => {
                    log_discord(&format!(
                        "Actividad de juego establecida: {instance_name} (asset: {image_key})"
                    ));
                    Ok(())
                }
                Err(e) => {
                    log_discord(&format!("Error con asset {image_key}: {e:?}, reintentando sin imagen"));
                    let timestamps = activity::Timestamps::new().start(*started_at);
                    let fallback = activity::Activity::new()
                        .state(&state_text)
                        .details(&details_text)
                        .timestamps(timestamps);
                    c.set_activity(fallback)
                }
            }
        }
        DiscordAction::StoppedPlaying => Ok(()),
        DiscordAction::Clear => c.clear_activity(),
        DiscordAction::SetEnabled(_) => Ok(()),
    };

    if let Err(e) = result {
        log_discord(&format!(
            "Error crítico en Discord IPC: {e:?}. Cerrando conexión para reintentar."
        ));
        let _ = c.close();
        *client = None;
    }
}
pub fn set_menu_page(page: &str) {
    if let Ok(mgr) = DISCORD_MGR.lock() {
        if mgr.enabled {
            if let Some(tx) = &mgr.sender {
                let _ = tx.send(DiscordAction::MenuPage(page.to_string()));
            }
        }
    }
}
pub fn set_launcher_idle() {
    if let Ok(mgr) = DISCORD_MGR.lock() {
        if mgr.enabled {
            if let Some(tx) = &mgr.sender {
                let _ = tx.send(DiscordAction::StoppedPlaying);
            }
        }
    }
}
pub fn set_playing(instance_name: &str, game_version: &str, loader: &str) {
    if let Ok(mgr) = DISCORD_MGR.lock() {
        if mgr.enabled {
            if let Some(tx) = &mgr.sender {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);

                let _ = tx.send(DiscordAction::Playing {
                    instance_name: instance_name.to_string(),
                    game_version: game_version.to_string(),
                    loader: loader.to_string(),
                    started_at: now,
                });
            }
        }
    }
}
pub fn clear() {
    if let Ok(mgr) = DISCORD_MGR.lock() {
        if let Some(tx) = &mgr.sender {
            let _ = tx.send(DiscordAction::Clear);
        }
    }
}
pub fn set_enabled(enabled: bool) {
    if let Ok(mut mgr) = DISCORD_MGR.lock() {
        mgr.enabled = enabled;
        if let Some(tx) = &mgr.sender {
            let _ = tx.send(DiscordAction::SetEnabled(enabled));
        }
    }
}
