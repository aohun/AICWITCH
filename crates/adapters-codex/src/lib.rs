//! Read/write ~/.codex live files. Official vs third-party policy lives here.

use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

use domain::{
    generate_catalog_json, has_login_material, CodexKind, CodexSettings,
};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CodexAdapterError {
    #[error("无法解析用户主目录")]
    HomeDir,
    #[error("读写 Codex 配置失败: {0}")]
    Io(#[from] io::Error),
    #[error("auth.json 不是合法 JSON: {0}")]
    AuthJson(#[from] serde_json::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexPaths {
    pub home: PathBuf,
    pub auth: PathBuf,
    pub config: PathBuf,
    pub catalog: PathBuf,
}

impl CodexPaths {
    pub fn from_home(home: impl Into<PathBuf>) -> Self {
        let home = home.into();
        Self {
            auth: home.join("auth.json"),
            config: home.join("config.toml"),
            catalog: home.join("router-switch-model-catalog.json"),
            home,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LiveCodex {
    pub auth: Value,
    pub config_toml: String,
}

pub fn resolve_codex_paths(override_home: Option<&Path>) -> Result<CodexPaths, CodexAdapterError> {
    if let Some(home) = override_home {
        return Ok(CodexPaths::from_home(home));
    }
    if let Ok(home) = std::env::var("CODEX_HOME") {
        let trimmed = home.trim();
        if !trimmed.is_empty() {
            return Ok(CodexPaths::from_home(trimmed));
        }
    }
    let home = dirs::home_dir().ok_or(CodexAdapterError::HomeDir)?;
    Ok(CodexPaths::from_home(home.join(".codex")))
}

pub fn read_live(paths: &CodexPaths) -> Result<LiveCodex, CodexAdapterError> {
    let auth = match fs::read_to_string(&paths.auth) {
        Ok(text) if text.trim().is_empty() => serde_json::json!({}),
        Ok(text) => serde_json::from_str(&text)?,
        Err(err) if err.kind() == ErrorKind::NotFound => serde_json::json!({}),
        Err(err) => return Err(err.into()),
    };
    let config_toml = match fs::read_to_string(&paths.config) {
        Ok(text) => text,
        Err(err) if err.kind() == ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err.into()),
    };
    Ok(LiveCodex { auth, config_toml })
}

/// Official without stored login material must not clobber ChatGPT OAuth in auth.json.
/// Third-party always writes both files atomically.
pub fn write_live_for_provider(
    paths: &CodexPaths,
    settings: &CodexSettings,
) -> Result<(), CodexAdapterError> {
    fs::create_dir_all(&paths.home)?;
    // Write or remove router-switch-model-catalog.json
    if let Some(catalog_json) = generate_catalog_json(&settings.model_mappings) {
        write_text_atomic(&paths.catalog, &catalog_json)?;
    } else if paths.catalog.exists() {
        let _ = fs::remove_file(&paths.catalog);
    }

    match settings.kind {
        CodexKind::Official => {
            write_text_atomic(&paths.config, &settings.config_toml)?;
            if has_login_material(&settings.auth) {
                write_json_atomic(&paths.auth, &settings.auth)?;
            }
            Ok(())
        }
        CodexKind::ResponsesThirdParty => {
            write_codex_live_atomic(paths, &settings.auth, &settings.config_toml)
        }
    }
}

pub fn write_codex_live_atomic(
    paths: &CodexPaths,
    auth: &Value,
    config_toml: &str,
) -> Result<(), CodexAdapterError> {
    fs::create_dir_all(&paths.home)?;
    let previous_auth = read_optional(&paths.auth)?;
    write_json_atomic(&paths.auth, auth)?;
    if let Err(err) = write_text_atomic(&paths.config, config_toml) {
        restore_previous(&paths.auth, previous_auth.as_deref())?;
        return Err(err);
    }
    Ok(())
}

fn write_json_atomic(path: &Path, value: &Value) -> Result<(), CodexAdapterError> {
    let mut body = serde_json::to_string_pretty(value)?;
    if !body.ends_with('\n') {
        body.push('\n');
    }
    write_text_atomic(path, &body)
}

fn write_text_atomic(path: &Path, contents: &str) -> Result<(), CodexAdapterError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = tmp_path(path);
    fs::write(&tmp, contents)?;
    replace_file(&tmp, path)?;
    Ok(())
}

fn replace_file(tmp: &Path, dest: &Path) -> io::Result<()> {
    match fs::rename(tmp, dest) {
        Ok(()) => Ok(()),
        Err(err) if dest.exists() => {
            fs::remove_file(dest)?;
            fs::rename(tmp, dest)
        }
        Err(err) => {
            let _ = fs::remove_file(tmp);
            Err(err)
        }
    }
}

fn tmp_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|name| {
            let mut name = name.to_os_string();
            name.push(".tmp");
            name
        })
        .unwrap_or_else(|| "file.tmp".into());
    path.with_file_name(name)
}

fn read_optional(path: &Path) -> io::Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

fn restore_previous(path: &Path, previous: Option<&[u8]>) -> io::Result<()> {
    match previous {
        Some(bytes) => {
            let tmp = tmp_path(path);
            fs::write(&tmp, bytes)?;
            replace_file(&tmp, path)
        }
        None => match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{generate_third_party_auth, generate_third_party_config, official_codex_settings};
    use serde_json::json;

    fn temp_paths() -> (tempfile::TempDir, CodexPaths) {
        let dir = tempfile::tempdir().unwrap();
        let paths = CodexPaths::from_home(dir.path().join("codex"));
        (dir, paths)
    }

    fn third_party() -> CodexSettings {
        CodexSettings {
            kind: CodexKind::ResponsesThirdParty,
            auth: generate_third_party_auth("sk-live"),
            config_toml: generate_third_party_config(
                "PackyCode",
                "https://www.packyapi.ai/v1",
                "gpt-5.6-sol",
            ),
            model_mappings: Vec::new(),
        }
    }

    #[test]
    fn third_party_writes_auth_and_config() {
        let (_dir, paths) = temp_paths();
        write_live_for_provider(&paths, &third_party()).unwrap();
        let live = read_live(&paths).unwrap();
        assert_eq!(live.auth["OPENAI_API_KEY"], "sk-live");
        assert!(live.config_toml.contains("wire_api = \"responses\""));
        assert!(live.config_toml.contains("https://www.packyapi.ai/v1"));
    }

    #[test]
    fn official_without_login_keeps_existing_oauth() {
        let (_dir, paths) = temp_paths();
        fs::create_dir_all(&paths.home).unwrap();
        fs::write(
            &paths.auth,
            r#"{"tokens":{"access_token":"chatgpt-oauth"}}"#,
        )
        .unwrap();
        fs::write(
            &paths.config,
            "model_provider = \"custom\"\n\n[model_providers.custom]\nname = \"relay\"\n",
        )
        .unwrap();

        write_live_for_provider(&paths, &official_codex_settings()).unwrap();

        let live = read_live(&paths).unwrap();
        assert_eq!(live.auth["tokens"]["access_token"], "chatgpt-oauth");
        assert!(!live.config_toml.contains("model_providers"));
    }

    #[test]
    fn official_with_stored_key_overwrites_auth() {
        let (_dir, paths) = temp_paths();
        let mut settings = official_codex_settings();
        settings.auth = json!({"OPENAI_API_KEY": "sk-official"});
        write_live_for_provider(&paths, &settings).unwrap();
        let live = read_live(&paths).unwrap();
        assert_eq!(live.auth["OPENAI_API_KEY"], "sk-official");
    }

    #[test]
    fn config_failure_rolls_back_auth() {
        let (_dir, paths) = temp_paths();
        fs::create_dir_all(&paths.home).unwrap();
        fs::write(&paths.auth, r#"{"OPENAI_API_KEY":"old-key"}"#).unwrap();
        fs::create_dir_all(&paths.config).unwrap();

        let err = write_live_for_provider(&paths, &third_party()).unwrap_err();
        assert!(matches!(err, CodexAdapterError::Io(_)));
        let auth = fs::read_to_string(&paths.auth).unwrap();
        assert!(auth.contains("old-key"));
        assert!(!auth.contains("sk-live"));
    }
}