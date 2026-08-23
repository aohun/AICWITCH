//! App-layer glue: SQLite SSOT + Codex live write.

use std::path::{Path, PathBuf};

use adapters_codex::{
    read_live, resolve_codex_paths, write_live_for_provider, CodexAdapterError, CodexPaths,
};
use domain::{
    backfill_codex_settings, extract_codex_api_key, extract_codex_base_url, extract_codex_model,
    extract_codex_provider_name, new_provider_id, parse_codex_form, AppKind, CodexForm, CodexKind,
    CodexSettings, DomainError, Provider, ProviderSettings, OFFICIAL_CODEX_ID,
};
use store::{AppLanguage, AppSettings, Store, StoreError, ThemePreference};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SessionError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Adapter(#[from] CodexAdapterError),
    #[error(transparent)]
    Domain(#[from] DomainError),
    #[error("{0}")]
    Message(String),
}

pub struct Workspace {
    store: Store,
    paths: CodexPaths,
}

pub struct CodexSnapshot {
    pub providers: Vec<Provider>,
    pub current_id: Option<String>,
}

impl Workspace {
    pub fn open(db_path: impl AsRef<Path>, codex_home: Option<&Path>) -> Result<Self, SessionError> {
        let store = Store::open(db_path)?;
        let settings = store.settings()?;
        let override_home = codex_home
            .map(Path::to_path_buf)
            .or(settings.codex_home.clone());
        let paths = resolve_codex_paths(override_home.as_deref())?;
        Ok(Self { store, paths })
    }

    pub fn open_default() -> Result<Self, SessionError> {
        Self::open(store::default_db_path()?, None)
    }

    pub fn data_dir(&self) -> &Path {
        self.store.data_dir()
    }

    pub fn codex_home(&self) -> &Path {
        &self.paths.home
    }

    pub fn settings(&self) -> Result<AppSettings, SessionError> {
        Ok(self.store.settings()?)
    }

    pub fn save_settings(&self, settings: AppSettings) -> Result<(), SessionError> {
        self.store.save_settings(&settings)?;
        Ok(())
    }

    pub fn apply_codex_home(&mut self, home: Option<PathBuf>) -> Result<(), SessionError> {
        let mut settings = self.store.settings()?;
        settings.codex_home = home.clone();
        self.store.save_settings(&settings)?;
        self.paths = resolve_codex_paths(home.as_deref())?;
        Ok(())
    }

    pub fn set_theme(&self, theme: ThemePreference) -> Result<(), SessionError> {
        let mut settings = self.store.settings()?;
        settings.theme = theme;
        self.store.save_settings(&settings)?;
        Ok(())
    }

    pub fn set_language(&self, language: AppLanguage) -> Result<(), SessionError> {
        let mut settings = self.store.settings()?;
        settings.language = language;
        self.store.save_settings(&settings)?;
        Ok(())
    }

    pub fn toggle_main_app(&self, app_id: &str) -> Result<bool, SessionError> {
        let mut settings = self.store.settings()?;
        let is_enabled = if settings.main_apps.iter().any(|a| a == app_id) {
            settings.main_apps.retain(|a| a != app_id);
            false
        } else {
            settings.main_apps.push(app_id.to_string());
            true
        };
        self.store.save_settings(&settings)?;
        Ok(is_enabled)
    }

    pub fn reorder_main_apps(&self, new_order: Vec<String>) -> Result<(), SessionError> {
        let mut settings = self.store.settings()?;
        settings.main_apps = new_order;
        self.store.save_settings(&settings)?;
        Ok(())
    }

    pub fn set_launch_on_startup(&self, enabled: bool) -> Result<(), SessionError> {
        let mut settings = self.store.settings()?;
        settings.launch_on_startup = enabled;
        self.store.save_settings(&settings)?;
        Ok(())
    }

    pub fn set_minimize_to_tray(&self, enabled: bool) -> Result<(), SessionError> {
        let mut settings = self.store.settings()?;
        settings.minimize_to_tray = enabled;
        self.store.save_settings(&settings)?;
        Ok(())
    }

    pub fn snapshot(&self) -> Result<CodexSnapshot, SessionError> {
        Ok(CodexSnapshot {
            providers: self.store.list_providers(AppKind::Codex)?,
            current_id: self.store.current_id(AppKind::Codex)?,
        })
    }

    pub fn form_for(&self, id: &str) -> Result<CodexForm, SessionError> {
        let provider = self.require(id)?;
        let settings = require_codex(&provider)?;
        let mut settings = settings.clone();
        if self.store.current_id(AppKind::Codex)?.as_deref() == Some(id) {
            let live = read_live(&self.paths)?;
            settings = backfill_codex_settings(&settings, &live.auth, &live.config_toml);
        }
        Ok(settings.form_snapshot(&provider.name, provider.website_url.as_deref()))
    }

    pub fn save_form(
        &self,
        editing_id: Option<&str>,
        form: CodexForm,
    ) -> Result<Provider, SessionError> {
        let website_url = optional_url(&form.website_url);
        let settings = parse_codex_form(form.clone())?;
        let provider = if let Some(id) = editing_id {
            let mut existing = self.require(id)?;
            existing.name = form.name.trim().to_string();
            existing.website_url = website_url;
            existing.settings = ProviderSettings::Codex(settings);
            existing
        } else if form.kind.is_official() {
            match self.store.get_provider(OFFICIAL_CODEX_ID)? {
                Some(mut existing) => {
                    existing.name = form.name.trim().to_string();
                    existing.website_url = website_url;
                    existing.settings = ProviderSettings::Codex(settings);
                    existing
                }
                None => new_codex_provider(
                    OFFICIAL_CODEX_ID.to_string(),
                    form.name,
                    website_url,
                    settings,
                    0,
                ),
            }
        } else {
            let sort_index = next_sort(&self.store)?;
            new_codex_provider(
                new_provider_id(&form.name),
                form.name,
                website_url,
                settings,
                sort_index,
            )
        };
        self.store.upsert_provider(&provider)?;
        if self.store.current_id(AppKind::Codex)?.as_deref() == Some(provider.id.as_str()) {
            if let Some(settings) = provider.codex_settings() {
                write_live_for_provider(&self.paths, settings)?;
            }
        }
        Ok(provider)
    }

    pub fn enable(&self, id: &str) -> Result<(), SessionError> {
        let provider = self.require(id)?;
        let settings = require_codex(&provider)?;
        write_live_for_provider(&self.paths, settings)?;
        self.store.set_current(AppKind::Codex, id)?;
        Ok(())
    }

    pub fn delete(&self, id: &str) -> Result<(), SessionError> {
        self.store.delete_provider(id)?;
        Ok(())
    }

    pub fn duplicate(&self, id: &str) -> Result<Provider, SessionError> {
        let source = self.require(id)?;
        let mut copy = source.clone();
        copy.id = new_provider_id(&format!("{}-copy", source.name));
        copy.name = format!("{} copy", source.name);
        copy.created_at = now_secs();
        copy.sort_index = next_sort(&self.store)?;
        self.store.upsert_provider(&copy)?;
        Ok(copy)
    }

    pub fn import_live(&self) -> Result<Option<Provider>, SessionError> {
        let live = read_live(&self.paths)?;
        let Some(base_url) = extract_codex_base_url(&live.config_toml) else {
            return Ok(None);
        };
        let name = extract_codex_provider_name(&live.config_toml)
            .unwrap_or_else(|| "Imported Codex".into());
        let form = CodexForm {
            name,
            website_url: String::new(),
            kind: CodexKind::ResponsesThirdParty,
            api_key: extract_codex_api_key(&live.auth).unwrap_or_default(),
            base_url,
            model: extract_codex_model(&live.config_toml).unwrap_or_default(),
            model_mappings: Vec::new(),
        };
        Ok(Some(self.save_form(None, form)?))
    }

    fn require(&self, id: &str) -> Result<Provider, SessionError> {
        self.store
            .get_provider(id)?
            .ok_or_else(|| SessionError::Message("供应商不存在".into()))
    }
}

fn require_codex(provider: &Provider) -> Result<&CodexSettings, SessionError> {
    provider
        .codex_settings()
        .ok_or_else(|| SessionError::Message("不是 Codex 供应商".into()))
}

fn optional_url(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn new_codex_provider(
    id: String,
    name: String,
    website_url: Option<String>,
    settings: CodexSettings,
    sort_index: i64,
) -> Provider {
    Provider {
        id,
        app: AppKind::Codex,
        name: name.trim().to_string(),
        website_url,
        settings: ProviderSettings::Codex(settings),
        created_at: now_secs(),
        sort_index,
    }
}

fn next_sort(store: &Store) -> Result<i64, SessionError> {
    let max = store
        .list_providers(AppKind::Codex)?
        .into_iter()
        .map(|provider| provider.sort_index)
        .max()
        .unwrap_or(0);
    Ok(max + 1)
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{extract_codex_api_key, extract_codex_base_url, extract_codex_model};
    use serde_json::json;

    fn temp_workspace() -> (tempfile::TempDir, Workspace) {
        let dir = tempfile::tempdir().unwrap();
        let ws = Workspace::open(
            dir.path().join("app.db"),
            Some(&dir.path().join("codex")),
        )
        .unwrap();
        (dir, ws)
    }

    fn third_party_form(name: &str, key: &str, url: &str, model: &str) -> CodexForm {
        CodexForm {
            name: name.into(),
            website_url: String::new(),
            kind: CodexKind::ResponsesThirdParty,
            api_key: key.into(),
            base_url: url.into(),
            model: model.into(),
            model_mappings: Vec::new(),
        }
    }

    #[test]
    fn enable_third_party_writes_live_files() {
        let (_dir, ws) = temp_workspace();
        let provider = ws
            .save_form(
                None,
                third_party_form(
                    "PackyCode",
                    "sk-live",
                    "https://www.packyapi.ai/v1",
                    "gpt-5.6-sol",
                ),
            )
            .unwrap();
        ws.enable(&provider.id).unwrap();

        let live = read_live(&ws.paths).unwrap();
        assert_eq!(live.auth["OPENAI_API_KEY"], "sk-live");
        assert!(live.config_toml.contains("wire_api = \"responses\""));
        assert_eq!(
            extract_codex_base_url(&live.config_toml).as_deref(),
            Some("https://www.packyapi.ai/v1")
        );
        assert_eq!(ws.snapshot().unwrap().current_id.as_deref(), Some(provider.id.as_str()));
    }

    #[test]
    fn official_enable_keeps_oauth_and_clears_custom_provider() {
        let (_dir, ws) = temp_workspace();
        std::fs::create_dir_all(&ws.paths.home).unwrap();
        std::fs::write(
            &ws.paths.auth,
            r#"{"tokens":{"access_token":"chatgpt-oauth"}}"#,
        )
        .unwrap();
        std::fs::write(
            &ws.paths.config,
            "model_provider = \"custom\"\n\n[model_providers.custom]\nname = \"relay\"\n",
        )
        .unwrap();

        ws.enable(OFFICIAL_CODEX_ID).unwrap();
        let live = read_live(&ws.paths).unwrap();
        assert_eq!(live.auth["tokens"]["access_token"], "chatgpt-oauth");
        assert!(!live.config_toml.contains("model_providers"));
    }

    #[test]
    fn edit_current_backfills_live_key_and_endpoint() {
        let (_dir, ws) = temp_workspace();
        let provider = ws
            .save_form(
                None,
                third_party_form(
                    "Packy",
                    "old-key",
                    "https://old.example/v1",
                    "old-model",
                ),
            )
            .unwrap();
        ws.enable(&provider.id).unwrap();

        std::fs::write(
            &ws.paths.auth,
            serde_json::to_string(&json!({"OPENAI_API_KEY": "live-key"})).unwrap(),
        )
        .unwrap();
        std::fs::write(
            &ws.paths.config,
            domain::generate_third_party_config("Packy", "https://live.example/v1", "live-model"),
        )
        .unwrap();

        let form = ws.form_for(&provider.id).unwrap();
        assert_eq!(form.api_key, "live-key");
        assert_eq!(form.base_url, "https://live.example/v1");
        assert_eq!(form.model, "live-model");
    }

    #[test]
    fn saving_current_provider_rewrites_live() {
        let (_dir, ws) = temp_workspace();
        let provider = ws
            .save_form(
                None,
                third_party_form("Relay", "sk-1", "https://one.example/v1", "m1"),
            )
            .unwrap();
        ws.enable(&provider.id).unwrap();
        ws.save_form(
            Some(&provider.id),
            third_party_form("Relay", "sk-2", "https://two.example/v1", "m2"),
        )
        .unwrap();

        let live = read_live(&ws.paths).unwrap();
        assert_eq!(extract_codex_api_key(&live.auth).as_deref(), Some("sk-2"));
        assert_eq!(
            extract_codex_base_url(&live.config_toml).as_deref(),
            Some("https://two.example/v1")
        );
        assert_eq!(extract_codex_model(&live.config_toml).as_deref(), Some("m2"));
    }

    #[test]
    fn cannot_delete_current_provider() {
        let (_dir, ws) = temp_workspace();
        ws.enable(OFFICIAL_CODEX_ID).unwrap();
        let err = ws.delete(OFFICIAL_CODEX_ID).unwrap_err();
        assert!(err.to_string().contains("不能删除"));
    }

    #[test]
    fn persist_codex_home_override() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("app.db");
        let first = dir.path().join("codex-a");
        let second = dir.path().join("codex-b");
        let mut ws = Workspace::open(&db, Some(&first)).unwrap();
        ws.apply_codex_home(Some(second.clone())).unwrap();
        drop(ws);

        let ws = Workspace::open(&db, None).unwrap();
        assert_eq!(ws.codex_home(), second.as_path());
        assert_eq!(ws.settings().unwrap().codex_home.as_deref(), Some(second.as_path()));
    }

    #[test]
    fn reorder_main_apps_persists() {
        let (dir, ws) = temp_workspace();
        let initial = ws.settings().unwrap().main_apps;
        assert_eq!(initial, vec!["codex", "claude", "claude-desktop", "grok"]);

        let custom_order = vec!["codex".to_string(), "grok".to_string(), "claude".to_string()];
        ws.reorder_main_apps(custom_order.clone()).unwrap();

        let loaded = ws.settings().unwrap().main_apps;
        assert_eq!(loaded, custom_order);

        drop(ws);
        let ws2 = Workspace::open(dir.path().join("app.db"), None).unwrap();
        assert_eq!(ws2.settings().unwrap().main_apps, custom_order);
    }
}