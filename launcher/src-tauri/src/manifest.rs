use serde::{Deserialize, Serialize};

use crate::TRANSLATION_REPO;

/// Hard cap per file, so a broken manifest cannot fill the disk.
pub const MAX_FILE_SIZE: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub url: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub schema: u32,
    pub version: String,
    #[serde(default)]
    pub game_version: Option<String>,
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    pub files: Vec<FileEntry>,
}

pub fn manifest_url() -> String {
    format!("https://github.com/{TRANSLATION_REPO}/releases/latest/download/manifest.json")
}

pub fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(concat!("wuwarus-launcher/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())
}

/// File names land inside the game folder, so only plain `*.pak` names are allowed.
pub fn is_safe_name(name: &str) -> bool {
    name.len() <= 100
        && name.ends_with(".pak")
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

fn is_allowed_url(raw: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(raw) else { return false };
    let host = url.host_str().unwrap_or_default();
    url.scheme() == "https" && (host == "github.com" || host.ends_with(".githubusercontent.com"))
}

fn validate(m: &Manifest) -> Result<(), String> {
    if m.schema != 1 {
        return Err(format!(
            "Манифест версии {} не поддерживается, обновите лаунчер",
            m.schema
        ));
    }
    if m.files.is_empty() {
        return Err("В манифесте нет файлов".into());
    }
    for f in &m.files {
        if !is_safe_name(&f.name) {
            return Err(format!("Недопустимое имя файла в манифесте: {}", f.name));
        }
        if !is_allowed_url(&f.url) {
            return Err(format!("Недопустимый адрес загрузки: {}", f.url));
        }
        if f.sha256.len() != 64 || !f.sha256.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(format!("Неверный SHA-256 у {}", f.name));
        }
        if f.size == 0 || f.size > MAX_FILE_SIZE {
            return Err(format!("Неверный размер у {}", f.name));
        }
    }
    Ok(())
}

pub async fn fetch() -> Result<Manifest, String> {
    let resp = http_client()?
        .get(manifest_url())
        .send()
        .await
        .map_err(|e| format!("Нет связи с GitHub: {e}"))?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Err("Релиз перевода пока не опубликован".into());
    }
    let m: Manifest = resp
        .error_for_status()
        .map_err(|e| format!("GitHub ответил ошибкой: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Битый manifest.json: {e}"))?;
    validate(&m)?;
    Ok(m)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert!(is_safe_name("wuwarus_ru_P.pak"));
        assert!(!is_safe_name("../evil.pak"));
        assert!(!is_safe_name("a\\b.pak"));
        assert!(!is_safe_name("x.dll"));
        assert!(!is_safe_name(".pak"));
    }

    #[test]
    fn urls() {
        assert!(is_allowed_url("https://github.com/o/r/releases/download/v1/a.pak"));
        assert!(is_allowed_url("https://objects.githubusercontent.com/x"));
        assert!(!is_allowed_url("http://github.com/x"));
        assert!(!is_allowed_url("https://github.com.evil.io/x"));
    }
}
