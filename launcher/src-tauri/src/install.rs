use futures_util::StreamExt;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;

use crate::game;
use crate::manifest::{self, FileEntry, Manifest};
use crate::state::Installed;

#[derive(Clone, Serialize)]
struct Progress<'a> {
    file: &'a str,
    done: u64,
    total: u64,
}

fn part_path(mods: &Path, name: &str) -> PathBuf {
    mods.join(format!("{name}.part"))
}

async fn download(
    app: &AppHandle,
    client: &reqwest::Client,
    f: &FileEntry,
    dest: &Path,
    done_before: u64,
    total: u64,
) -> Result<(), String> {
    let resp = client
        .get(&f.url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("Не удалось скачать {}: {e}", f.name))?;

    let mut out = tokio::fs::File::create(dest)
        .await
        .map_err(|e| format!("Не удалось создать файл в папке игры: {e}"))?;
    let mut hasher = Sha256::new();
    let mut got: u64 = 0;
    let mut last_emit: u64 = 0;
    let mut stream = resp.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Обрыв загрузки {}: {e}", f.name))?;
        got += chunk.len() as u64;
        if got > f.size {
            return Err(format!("{} больше заявленного размера", f.name));
        }
        hasher.update(&chunk);
        out.write_all(&chunk).await.map_err(|e| e.to_string())?;
        if got - last_emit >= 256 * 1024 || got == f.size {
            last_emit = got;
            let _ = app.emit(
                "install-progress",
                Progress { file: &f.name, done: done_before + got, total },
            );
        }
    }
    out.flush().await.map_err(|e| e.to_string())?;
    drop(out);

    if got != f.size {
        return Err(format!("{}: скачано {got} из {} байт", f.name, f.size));
    }
    let actual = hex::encode(hasher.finalize());
    if !actual.eq_ignore_ascii_case(&f.sha256) {
        return Err(format!("{}: контрольная сумма не совпала, файл повреждён", f.name));
    }
    Ok(())
}

/// Downloads every file to `*.part`, verifies it, and only then swaps them in.
/// Files from the previous install that are not in the new manifest are removed.
pub async fn install(
    app: &AppHandle,
    game_dir: &Path,
    m: &Manifest,
    prev: Option<&Installed>,
) -> Result<Installed, String> {
    let mods = game::mods_dir(game_dir);
    std::fs::create_dir_all(&mods).map_err(|e| format!("Не удалось создать {}: {e}", mods.display()))?;

    let client = manifest::http_client()?;
    let total: u64 = m.files.iter().map(|f| f.size).sum();
    let mut done = 0;

    for f in &m.files {
        let part = part_path(&mods, &f.name);
        if let Err(e) = download(app, &client, f, &part, done, total).await {
            for f in &m.files {
                let _ = std::fs::remove_file(part_path(&mods, &f.name));
            }
            return Err(e);
        }
        done += f.size;
    }

    for f in &m.files {
        std::fs::rename(part_path(&mods, &f.name), mods.join(&f.name))
            .map_err(|e| format!("Не удалось записать {} (игра запущена?): {e}", f.name))?;
    }

    let keep: HashSet<&str> = m.files.iter().map(|f| f.name.as_str()).collect();
    if let Some(prev) = prev {
        for old in prev.files.iter().filter(|n| !keep.contains(n.as_str())) {
            if manifest::is_safe_name(old) {
                let _ = std::fs::remove_file(mods.join(old));
            }
        }
    }

    Ok(Installed {
        version: m.version.clone(),
        files: m.files.iter().map(|f| f.name.clone()).collect(),
    })
}

pub fn uninstall(game_dir: &Path, installed: &Installed) -> Result<(), String> {
    let mods = game::mods_dir(game_dir);
    for name in installed.files.iter().filter(|n| manifest::is_safe_name(n)) {
        for p in [mods.join(name), part_path(&mods, name)] {
            match std::fs::remove_file(&p) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(format!("Не удалось удалить {name} (игра запущена?): {e}")),
            }
        }
    }
    Ok(())
}

pub fn files_present(game_dir: &Path, installed: &Installed) -> bool {
    let mods = game::mods_dir(game_dir);
    installed.files.iter().all(|n| mods.join(n).is_file())
}
