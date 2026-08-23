//! App-layer glue: SQLite SSOT + live config adapters for Codex, Claude, and Grok.

use std::path::{Path, PathBuf};

use adapters_claude::{
    read_live as read_claude_live, resolve_claude_paths,
    write_live_for_provider as write_claude_live, ClaudeAdapterError, ClaudePaths,
};
use adapters_codex::{
    read_live as read_codex_live, resolve_codex_paths,
    write_live_for_provider as write_codex_live, CodexAdapterError, CodexPaths,
};
use adapters_grok::{
    read_live as read_grok_live, resolve_grok_paths,
    write_live_for_provider as write_grok_live, GrokAdapterError, GrokPaths,
};
use domain::{
    backfill_claude_settings, backfill_codex_settings, backfill_grok_settings,
    new_provider_id, parse_claude_form, parse_codex_form, parse_grok_form, AppKind,
    ClaudeForm, CodexForm, DomainError, GrokForm, Provider, ProviderForm, ProviderSettings,
    OFFICIAL_CLAUDE_ID, OFFICIAL_CODEX_ID, OFFICIAL_GROK_ID,
};
use store::{AppLanguage, AppSettings, Store, StoreError, ThemePreference};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SessionError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    CodexAdapter(#[from] CodexAdapterError),
    #[error(transparent)]
    ClaudeAdapter(#[from] ClaudeAdapterError),
    #[error(transparent)]
    GrokAdapter(#[from] GrokAdapterError),
    #[error(transparent)]
    Domain(#[from] DomainError),
    #[error("{0}")]
    Message(String),
}

pub struct Workspace {
    store: Store,
    codex_paths: CodexPaths,
    claude_paths: ClaudePaths,
    grok_paths: GrokPaths,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppSnapshot {
    pub app: AppKind,
    pub providers: Vec<Provider>,
    pub current_id: Option<String>,
}

// Retain CodexSnapshot alias for compatibility if needed
pub type CodexSnapshot = AppSnapshot;

impl Workspace {
    pub fn open(
        db_path: impl AsRef<Path>,
        codex_home: Option<&Path>,
    ) -> Result<Self, SessionError> {
        let store = Store::open(db_path)?;
        let settings = store.settings()?;
        let override_codex = codex_home
            .map(Path::to_path_buf)
            .or(settings.codex_home.clone());
        let codex_paths = resolve_codex_paths(override_codex.as_deref())?;
        let claude_paths = resolve_claude_paths(settings.claude_home.as_deref())?;
        let grok_paths = resolve_grok_paths(settings.grok_home.as_deref())?;
        Ok(Self {
            store,
            codex_paths,
            claude_paths,
            grok_paths,
        })
    }

    pub fn open_default() -> Result<Self, SessionError> {
        Self::open(store::default_db_path()?, None)
    }

    pub fn data_dir(&self) -> &Path {
        self.store.data_dir()
    }

    pub fn codex_home(&self) -> &Path {
        &self.codex_paths.home
    }

    pub fn claude_home(&self) -> &Path {
        &self.claude_paths.home
    }

    pub fn grok_home(&self) -> &Path {
        &self.grok_paths.home
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
        self.codex_paths = resolve_codex_paths(home.as_deref())?;
        Ok(())
    }

    pub fn apply_claude_home(&mut self, home: Option<PathBuf>) -> Result<(), SessionError> {
        let mut settings = self.store.settings()?;
        settings.claude_home = home.clone();
        self.store.save_settings(&settings)?;
        self.claude_paths = resolve_claude_paths(home.as_deref())?;
        Ok(())
    }

    pub fn apply_grok_home(&mut self, home: Option<PathBuf>) -> Result<(), SessionError> {
        let mut settings = self.store.settings()?;
        settings.grok_home = home.clone();
        self.store.save_settings(&settings)?;
        self.grok_paths = resolve_grok_paths(home.as_deref())?;
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

    pub fn snapshot(&self) -> Result<AppSnapshot, SessionError> {
        self.snapshot_for(AppKind::Codex)
    }

    pub fn snapshot_for(&self, app: AppKind) -> Result<AppSnapshot, SessionError> {
        Ok(AppSnapshot {
            app,
            providers: self.store.list_providers(app)?,
            current_id: self.store.current_id(app)?,
        })
    }

    pub fn form_for(&self, id: &str) -> Result<ProviderForm, SessionError> {
        let provider = self.require(id)?;
        match &provider.settings {
            ProviderSettings::Codex(settings) => {
                let mut settings = settings.clone();
                if self.store.current_id(AppKind::Codex)?.as_deref() == Some(id) {
                    let live = read_codex_live(&self.codex_paths)?;
                    settings = backfill_codex_settings(&settings, &live.auth, &live.config_toml);
                }
                Ok(ProviderForm::Codex(settings.form_snapshot(
                    &provider.name,
                    provider.website_url.as_deref(),
                )))
            }
            ProviderSettings::Claude(settings) => {
                let mut settings = settings.clone();
                if self.store.current_id(AppKind::Claude)?.as_deref() == Some(id) {
                    let live = read_claude_live(&self.claude_paths)?;
                    settings = backfill_claude_settings(&settings, &live.settings);
                }
                Ok(ProviderForm::Claude(settings.form_snapshot(
                    &provider.name,
                    provider.website_url.as_deref(),
                )))
            }
            ProviderSettings::Grok(settings) => {
                let mut settings = settings.clone();
                if self.store.current_id(AppKind::Grok)?.as_deref() == Some(id) {
                    let live = read_grok_live(&self.grok_paths)?;
                    settings = backfill_grok_settings(&settings, &live.config_toml);
                }
                Ok(ProviderForm::Grok(settings.form_snapshot(
                    &provider.name,
                    provider.website_url.as_deref(),
                )))
            }
            ProviderSettings::Unsupported { app } => Err(SessionError::Message(format!(
                "暂不支持应用 {} 的表单配置",
                app.display_name()
            ))),
        }
    }

    pub fn save_codex_form(
        &self,
        editing_id: Option<&str>,
        form: CodexForm,
    ) -> Result<Provider, SessionError> {
        self.save_form(AppKind::Codex, editing_id, ProviderForm::Codex(form))
    }

    pub fn save_claude_form(
        &self,
        editing_id: Option<&str>,
        form: ClaudeForm,
    ) -> Result<Provider, SessionError> {
        self.save_form(AppKind::Claude, editing_id, ProviderForm::Claude(form))
    }

    pub fn save_grok_form(
        &self,
        editing_id: Option<&str>,
        form: GrokForm,
    ) -> Result<Provider, SessionError> {
        self.save_form(AppKind::Grok, editing_id, ProviderForm::Grok(form))
    }

    pub fn save_form(
        &self,
        app: AppKind,
        editing_id: Option<&str>,
        form: ProviderForm,
    ) -> Result<Provider, SessionError> {
        let (name, website_url, settings, official_id, is_official) = match form {
            ProviderForm::Codex(f) => {
                let name = f.name.trim().to_string();
                let url = optional_url(&f.website_url);
                let is_off = f.kind.is_official();
                let s = parse_codex_form(f)?;
                (name, url, ProviderSettings::Codex(s), OFFICIAL_CODEX_ID, is_off)
            }
            ProviderForm::Claude(f) => {
                let name = f.name.trim().to_string();
                let url = optional_url(&f.website_url);
                let is_off = f.kind.is_official();
                let s = parse_claude_form(f)?;
                (name, url, ProviderSettings::Claude(s), OFFICIAL_CLAUDE_ID, is_off)
            }
            ProviderForm::Grok(f) => {
                let name = f.name.trim().to_string();
                let url = optional_url(&f.website_url);
                let is_off = f.kind.is_official();
                let s = parse_grok_form(f)?;
                (name, url, ProviderSettings::Grok(s), OFFICIAL_GROK_ID, is_off)
            }
        };

        let provider = if let Some(id) = editing_id {
            let mut existing = self.require(id)?;
            existing.name = name;
            existing.website_url = website_url;
            existing.settings = settings;
            existing
        } else if is_official {
            match self.store.get_provider(official_id)? {
                Some(mut existing) => {
                    existing.name = name;
                    existing.website_url = website_url;
                    existing.settings = settings;
                    existing
                }
                None => Provider {
                    id: official_id.to_string(),
                    app,
                    name,
                    website_url,
                    settings,
                    created_at: now_secs(),
                    sort_index: 0,
                },
            }
        } else {
            let sort_index = next_sort(&self.store, app)?;
            Provider {
                id: new_provider_id(&name),
                app,
                name,
                website_url,
                settings,
                created_at: now_secs(),
                sort_index,
            }
        };

        self.store.upsert_provider(&provider)?;
        if self.store.current_id(app)?.as_deref() == Some(provider.id.as_str()) {
            self.write_live(&provider)?;
        }
        Ok(provider)
    }

    pub fn enable(&self, id: &str) -> Result<(), SessionError> {
        let provider = self.require(id)?;
        self.write_live(&provider)?;
        self.store.set_current(provider.app, id)?;
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
        copy.sort_index = next_sort(&self.store, source.app)?;
        self.store.upsert_provider(&copy)?;
        Ok(copy)
    }

    fn write_live(&self, provider: &Provider) -> Result<(), SessionError> {
        match &provider.settings {
            ProviderSettings::Codex(settings) => {
                write_codex_live(&self.codex_paths, settings)?;
            }
            ProviderSettings::Claude(settings) => {
                write_claude_live(&self.claude_paths, settings)?;
            }
            ProviderSettings::Grok(settings) => {
                write_grok_live(&self.grok_paths, settings)?;
            }
            ProviderSettings::Unsupported { app } => {
                return Err(SessionError::Message(format!(
                    "暂不支持应用 {} 的切换操作",
                    app.display_name()
                )));
            }
        }
        Ok(())
    }

    fn require(&self, id: &str) -> Result<Provider, SessionError> {
        self.store
            .get_provider(id)?
            .ok_or_else(|| SessionError::Message("供应商不存在".into()))
    }
}

fn optional_url(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn next_sort(store: &Store, app: AppKind) -> Result<i64, SessionError> {
    let max = store
        .list_providers(app)?
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
    use domain::{
        extract_codex_base_url,
        ClaudeKind, CodexKind, GrokKind,
    };

    fn temp_workspace() -> (tempfile::TempDir, Workspace) {
        let dir = tempfile::tempdir().unwrap();
        let ws = Workspace::open(
            dir.path().join("app.db"),
            Some(&dir.path().join("codex")),
        )
        .unwrap();
        (dir, ws)
    }

    fn codex_tp_form(name: &str, key: &str, url: &str, model: &str) -> CodexForm {
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
    fn enable_codex_third_party_writes_live_files() {
        let (_dir, ws) = temp_workspace();
        let provider = ws
            .save_form(
                AppKind::Codex,
                None,
                ProviderForm::Codex(codex_tp_form(
                    "PackyCode",
                    "sk-live",
                    "https://www.packyapi.ai/v1",
                    "gpt-5.6-sol",
                )),
            )
            .unwrap();
        ws.enable(&provider.id).unwrap();

        let live = read_codex_live(&ws.codex_paths).unwrap();
        assert_eq!(live.auth["OPENAI_API_KEY"], "sk-live");
        assert!(live.config_toml.contains("wire_api = \"responses\""));
        assert_eq!(
            extract_codex_base_url(&live.config_toml).as_deref(),
            Some("https://www.packyapi.ai/v1")
        );
        assert_eq!(
            ws.snapshot_for(AppKind::Codex).unwrap().current_id.as_deref(),
            Some(provider.id.as_str())
        );
    }

    #[test]
    fn claude_provider_flow() {
        let (dir, mut ws) = temp_workspace();
        let claude_home = dir.path().join("claude");
        ws.apply_claude_home(Some(claude_home.clone())).unwrap();

        let claude_form = ClaudeForm {
            name: "Packy Claude".into(),
            website_url: "".into(),
            kind: ClaudeKind::ThirdParty,
            api_key: "sk-ant-test".into(),
            base_url: "https://api.packy.ai".into(),
            model: "claude-3-7-sonnet".into(),
            model_mappings: Vec::new(),
        };

        let p = ws
            .save_claude_form(None, claude_form)
            .unwrap();
        ws.enable(&p.id).unwrap();

        let live = read_claude_live(&ws.claude_paths).unwrap();
        assert_eq!(
            live.settings["env"]["ANTHROPIC_BASE_URL"],
            "https://api.packy.ai"
        );
        assert_eq!(
            live.settings["env"]["ANTHROPIC_AUTH_TOKEN"],
            "sk-ant-test"
        );

        // Switch back to official claude
        ws.enable(OFFICIAL_CLAUDE_ID).unwrap();
        let live_off = read_claude_live(&ws.claude_paths).unwrap();
        assert!(live_off.settings.get("env").is_none());
    }

    #[test]
    fn grok_provider_flow() {
        let (dir, mut ws) = temp_workspace();
        let grok_home = dir.path().join("grok");
        ws.apply_grok_home(Some(grok_home.clone())).unwrap();

        let grok_form = GrokForm {
            name: "Packy Grok".into(),
            website_url: "".into(),
            kind: GrokKind::ThirdParty,
            api_key: "xai-test-key".into(),
            base_url: "https://api.packy.ai/v1".into(),
            model: "grok-4.5".into(),
            model_mappings: Vec::new(),
        };

        let p = ws.save_grok_form(None, grok_form).unwrap();
        ws.enable(&p.id).unwrap();

        let live = read_grok_live(&ws.grok_paths).unwrap();
        assert!(live.config_toml.contains("https://api.packy.ai/v1"));
        assert!(live.config_toml.contains("xai-test-key"));

        // Switch back to official grok
        ws.enable(OFFICIAL_GROK_ID).unwrap();
        let live_off = read_grok_live(&ws.grok_paths).unwrap();
        assert!(!live_off.config_toml.contains("api_backend"));
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
