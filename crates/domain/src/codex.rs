use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::provider::ProviderSettings;
use crate::{AppKind, DomainError, Provider};

pub const OFFICIAL_CODEX_ID: &str = "codex-official";
pub const DEFAULT_CODEX_MODEL: &str = "gpt-5.6-sol";
const THIRD_PARTY_PROVIDER_ID: &str = "custom";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CodexKind {
    Official,
    ResponsesThirdParty,
}

impl CodexKind {
    pub fn is_official(self) -> bool {
        matches!(self, Self::Official)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CodexSettings {
    pub kind: CodexKind,
    pub auth: Value,
    pub config_toml: String,
}

impl CodexSettings {
    pub fn form_snapshot(&self, name: &str, website_url: Option<&str>) -> CodexForm {
        CodexForm {
            name: name.to_string(),
            website_url: website_url.unwrap_or("").to_string(),
            kind: self.kind,
            api_key: extract_codex_api_key(&self.auth).unwrap_or_default(),
            base_url: extract_codex_base_url(&self.config_toml).unwrap_or_default(),
            model: extract_codex_model(&self.config_toml).unwrap_or_else(|| DEFAULT_CODEX_MODEL.to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexForm {
    pub name: String,
    pub website_url: String,
    pub kind: CodexKind,
    pub api_key: String,
    pub base_url: String,
    pub model: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodexPreset {
    pub id: &'static str,
    pub name: &'static str,
    pub website_url: &'static str,
    pub kind: CodexKind,
    pub base_url: &'static str,
    pub model: &'static str,
    pub provider_label: &'static str,
}

pub const RESPONSES_PRESETS: &[CodexPreset] = &[
    CodexPreset {
        id: "official",
        name: "OpenAI Official",
        website_url: "https://chatgpt.com/codex",
        kind: CodexKind::Official,
        base_url: "",
        model: "",
        provider_label: "openai",
    },
    CodexPreset {
        id: "packycode",
        name: "PackyCode",
        website_url: "https://www.packyapi.ai",
        kind: CodexKind::ResponsesThirdParty,
        base_url: "https://www.packyapi.ai/v1",
        model: DEFAULT_CODEX_MODEL,
        provider_label: "packycode",
    },
    CodexPreset {
        id: "deepseek",
        name: "DeepSeek",
        website_url: "https://platform.deepseek.com",
        kind: CodexKind::ResponsesThirdParty,
        base_url: "https://api.deepseek.com",
        model: "deepseek-v4-flash",
        provider_label: "deepseek",
    },
    CodexPreset {
        id: "custom",
        name: "Custom Responses",
        website_url: "",
        kind: CodexKind::ResponsesThirdParty,
        base_url: "https://api.example.com/v1",
        model: DEFAULT_CODEX_MODEL,
        provider_label: "custom",
    },
];

pub fn official_codex_settings() -> CodexSettings {
    CodexSettings {
        kind: CodexKind::Official,
        auth: json!({}),
        config_toml: String::new(),
    }
}

pub fn official_codex_provider() -> Provider {
    Provider {
        id: OFFICIAL_CODEX_ID.to_string(),
        app: AppKind::Codex,
        name: "OpenAI Official".to_string(),
        website_url: Some("https://chatgpt.com/codex".to_string()),
        settings: ProviderSettings::Codex(official_codex_settings()),
        created_at: 0,
        sort_index: 0,
    }
}

pub fn generate_third_party_auth(api_key: &str) -> Value {
    json!({ "OPENAI_API_KEY": api_key })
}

pub fn generate_third_party_config(provider_name: &str, base_url: &str, model: &str) -> String {
    format!(
        "model_provider = {provider_id}\n\
         model = {model}\n\
         model_reasoning_effort = \"high\"\n\
         disable_response_storage = true\n\
         \n\
         [model_providers.{table}]\n\
         name = {name}\n\
         base_url = {base_url}\n\
         wire_api = \"responses\"\n\
         requires_openai_auth = true\n",
        provider_id = toml_string(THIRD_PARTY_PROVIDER_ID),
        model = toml_string(model),
        table = THIRD_PARTY_PROVIDER_ID,
        name = toml_string(provider_name),
        base_url = toml_string(base_url),
    )
}

pub fn parse_codex_form(form: CodexForm) -> Result<CodexSettings, DomainError> {
    let name = form.name.trim();
    if name.is_empty() {
        return Err(DomainError::validation("名称不能为空"));
    }

    match form.kind {
        CodexKind::Official => Ok(official_codex_settings()),
        CodexKind::ResponsesThirdParty => {
            let api_key = form.api_key.trim();
            let base_url = form.base_url.trim();
            let model = form.model.trim();
            if api_key.is_empty() {
                return Err(DomainError::validation("第三方供应商需要 API Key"));
            }
            if base_url.is_empty() {
                return Err(DomainError::validation("第三方供应商需要 API 端点"));
            }
            if model.is_empty() {
                return Err(DomainError::validation("第三方供应商需要模型名"));
            }
            Ok(CodexSettings {
                kind: CodexKind::ResponsesThirdParty,
                auth: generate_third_party_auth(api_key),
                config_toml: generate_third_party_config(name, base_url, model),
            })
        }
    }
}

pub fn has_login_material(auth: &Value) -> bool {
    if extract_codex_api_key(auth).is_some() {
        return true;
    }
    let tokens = auth.get("tokens");
    non_empty_str(tokens.and_then(|t| t.get("access_token"))).is_some()
        || non_empty_str(tokens.and_then(|t| t.get("refresh_token"))).is_some()
}

pub fn extract_codex_api_key(auth: &Value) -> Option<String> {
    non_empty_str(auth.get("OPENAI_API_KEY")).map(str::to_string)
}

pub fn extract_codex_base_url(config_toml: &str) -> Option<String> {
    let value = parse_toml(config_toml)?;
    let providers = value.get("model_providers")?.as_table()?;
    if let Some(url) = providers
        .get(THIRD_PARTY_PROVIDER_ID)
        .and_then(|entry| entry.get("base_url"))
        .and_then(toml::Value::as_str)
    {
        return Some(url.to_string());
    }
    providers.values().find_map(|entry| {
        entry
            .get("base_url")
            .and_then(toml::Value::as_str)
            .map(str::to_string)
    })
}

pub fn extract_codex_model(config_toml: &str) -> Option<String> {
    parse_toml(config_toml)?
        .get("model")
        .and_then(toml::Value::as_str)
        .map(str::to_string)
}

pub fn extract_codex_provider_name(config_toml: &str) -> Option<String> {
    let value = parse_toml(config_toml)?;
    let providers = value.get("model_providers")?.as_table()?;
    if let Some(name) = providers
        .get(THIRD_PARTY_PROVIDER_ID)
        .and_then(|entry| entry.get("name"))
        .and_then(toml::Value::as_str)
    {
        return Some(name.to_string());
    }
    providers.values().find_map(|entry| {
        entry
            .get("name")
            .and_then(toml::Value::as_str)
            .map(str::to_string)
    })
}

pub fn backfill_codex_settings(
    stored: &CodexSettings,
    live_auth: &Value,
    live_config_toml: &str,
) -> CodexSettings {
    let mut next = stored.clone();
    if has_login_material(live_auth) {
        next.auth = live_auth.clone();
    }
    if !live_config_toml.trim().is_empty() {
        next.config_toml = live_config_toml.to_string();
    }
    next
}

fn parse_toml(config_toml: &str) -> Option<toml::Value> {
    toml::from_str(config_toml).ok()
}

fn toml_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| format!("\"{value}\""))
}

fn non_empty_str(value: Option<&Value>) -> Option<&str> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}