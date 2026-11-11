use std::path::{Path, PathBuf};
use std::process::Command;

const GAME_SUBDIR: &str = "Wuthering Waves Game";

/// Folders (relative to a drive root) where the game usually lives.
const CANDIDATES: &[&str] = &[
    "Wuthering Waves",
    "Games\\Wuthering Waves",
    "Program Files\\Wuthering Waves",
    "Program Files (x86)\\Wuthering Waves",
    "SteamLibrary\\steamapps\\common\\Wuthering Waves",
    "Program Files (x86)\\Steam\\steamapps\\common\\Wuthering Waves",
    "Program Files\\Epic Games\\WutheringWavesj3oFh",
];

fn shipping_exe(game: &Path) -> PathBuf {
    game.join("Client")
        .join("Binaries")
        .join("Win64")
        .join("Client-Win64-Shipping.exe")
}

pub fn is_game_dir(p: &Path) -> bool {
    shipping_exe(p).is_file()
}

/// Accepts either the game folder itself or the folder above it
/// (the one the official launcher installs into).
pub fn normalize(p: &Path) -> Option<PathBuf> {
    if is_game_dir(p) {
        return Some(p.to_path_buf());
    }
    let sub = p.join(GAME_SUBDIR);
    is_game_dir(&sub).then_some(sub)
}

pub fn detect() -> Option<PathBuf> {
    for letter in b'C'..=b'Z' {
        let root = PathBuf::from(format!("{}:\\", letter as char));
        if !root.exists() {
            continue;
        }
        for rel in CANDIDATES {
            if let Some(found) = normalize(&root.join(rel)) {
                return Some(found);
            }
        }
    }
    None
}

pub fn mods_dir(game: &Path) -> PathBuf {
    game.join("Client").join("Content").join("Paks").join("~mods")
}

pub fn launch(game: &Path) -> Result<(), String> {
    let bootstrap = game.join("Wuthering Waves.exe");
    let exe = if bootstrap.is_file() { bootstrap } else { shipping_exe(game) };
    if !exe.is_file() {
        return Err(format!("Не найден исполняемый файл игры: {}", exe.display()));
    }
    let cwd = exe.parent().unwrap_or(game);
    Command::new(&exe)
        .current_dir(cwd)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Не удалось запустить игру: {e}"))
}
