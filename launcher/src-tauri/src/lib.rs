mod game;
mod install;
mod manifest;
mod state;

use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::AppHandle;

/// GitHub repository `owner/name` that publishes translation releases.
pub const TRANSLATION_REPO: &str = "htdealwme/wuwarus";

static BUSY: AtomicBool = AtomicBool::new(false);

struct BusyGuard;
impl BusyGuard {
    fn acquire() -> Result<Self, String> {
        BUSY.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| BusyGuard)
            .map_err(|_| "Уже идёт другая операция".to_string())
    }
}
impl Drop for BusyGuard {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::Release);
    }
}

#[derive(Serialize)]
struct Status {
    game_path: Option<String>,
    game_ok: bool,
    installed_version: Option<String>,
    installed_ok: bool,
    risk_accepted: bool,
    repo: &'static str,
}

fn status_of(s: &state::Saved) -> Status {
    let game_ok = s.game_path.as_deref().is_some_and(game::is_game_dir);
    let installed_ok = match (&s.game_path, &s.installed) {
        (Some(g), Some(i)) if game_ok => install::files_present(g, i),
        _ => false,
    };
    Status {
        game_path: s.game_path.as_ref().map(|p| p.display().to_string()),
        game_ok,
        installed_version: s.installed.as_ref().map(|i| i.version.clone()),
        installed_ok,
        risk_accepted: s.risk_accepted,
        repo: TRANSLATION_REPO,
    }
}

fn require_game(s: &state::Saved) -> Result<PathBuf, String> {
    s.game_path
        .clone()
        .filter(|p| game::is_game_dir(p))
        .ok_or_else(|| "Сначала укажите папку с игрой".to_string())
}

#[tauri::command]
fn get_status(app: AppHandle) -> Status {
    status_of(&state::load(&app))
}

#[tauri::command]
fn detect_game(app: AppHandle) -> Result<Status, String> {
    let mut s = state::load(&app);
    let found = game::detect().ok_or("Игра не найдена автоматически, выберите папку вручную")?;
    s.game_path = Some(found);
    state::save(&app, &s)?;
    Ok(status_of(&s))
}

#[tauri::command]
fn set_game_path(app: AppHandle, path: String) -> Result<Status, String> {
    let dir = game::normalize(&PathBuf::from(&path))
        .ok_or("В этой папке нет Wuthering Waves (ищу Client\\Binaries\\Win64\\Client-Win64-Shipping.exe)")?;
    let mut s = state::load(&app);
    s.game_path = Some(dir);
    state::save(&app, &s)?;
    Ok(status_of(&s))
}

#[tauri::command]
fn accept_risk(app: AppHandle) -> Result<Status, String> {
    let mut s = state::load(&app);
    s.risk_accepted = true;
    state::save(&app, &s)?;
    Ok(status_of(&s))
}

#[tauri::command]
async fn fetch_manifest() -> Result<manifest::Manifest, String> {
    manifest::fetch().await
}

#[tauri::command]
async fn install_translation(app: AppHandle) -> Result<Status, String> {
    let _busy = BusyGuard::acquire()?;
    let mut s = state::load(&app);
    if !s.risk_accepted {
        return Err("Сначала подтвердите предупреждение о рисках".into());
    }
    let game_dir = require_game(&s)?;
    let m = manifest::fetch().await?;
    let installed = install::install(&app, &game_dir, &m, s.installed.as_ref()).await?;
    s.installed = Some(installed);
    state::save(&app, &s)?;
    Ok(status_of(&s))
}

#[tauri::command]
fn uninstall_translation(app: AppHandle) -> Result<Status, String> {
    let _busy = BusyGuard::acquire()?;
    let mut s = state::load(&app);
    let game_dir = require_game(&s)?;
    if let Some(i) = &s.installed {
        install::uninstall(&game_dir, i)?;
    }
    s.installed = None;
    state::save(&app, &s)?;
    Ok(status_of(&s))
}

#[tauri::command]
fn launch_game(app: AppHandle) -> Result<(), String> {
    let s = state::load(&app);
    game::launch(&require_game(&s)?)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            get_status,
            detect_game,
            set_game_path,
            accept_risk,
            fetch_manifest,
            install_translation,
            uninstall_translation,
            launch_game
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
