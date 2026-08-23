//! Pure domain crate for AI provider management across Codex, Claude Code, and Grok Build.
//! No filesystem or SQLite dependencies here.

mod app_kind;
mod claude;
mod codex;
mod error;
mod grok;
mod provider;

pub use app_kind::AppKind;
pub use claude::{
    backfill_claude_settings, extract_claude_api_key, extract_claude_base_url,
    extract_claude_model, extract_claude_provider_name, generate_claude_env,
    official_claude_provider, official_claude_settings, parse_claude_form,
    ClaudeForm, ClaudeKind, ClaudeModelMapping, ClaudePreset, ClaudeSettings,
    CLAUDE_PRESETS, DEFAULT_CLAUDE_MODEL, OFFICIAL_CLAUDE_ID,
};
pub use codex::{
    backfill_codex_settings, extract_codex_api_key, extract_codex_base_url, extract_codex_model,
    extract_codex_provider_name, fetch_models_from_api, generate_catalog_json,
    generate_third_party_auth, generate_third_party_config,
    generate_third_party_config_with_catalog, has_login_material, official_codex_provider,
    official_codex_settings, parse_codex_form, CodexForm, CodexKind, CodexModelMapping,
    CodexPreset, CodexSettings, DEFAULT_CODEX_MODEL, OFFICIAL_CODEX_ID, RESPONSES_PRESETS,
};
pub use error::DomainError;
pub use grok::{
    backfill_grok_settings, extract_grok_api_key, extract_grok_base_url, extract_grok_model,
    extract_grok_provider_name, generate_grok_config_toml, official_grok_provider,
    official_grok_settings, parse_grok_form, GrokForm, GrokKind, GrokModelMapping,
    GrokPreset, GrokSettings, DEFAULT_GROK_MODEL, GROK_PRESETS, OFFICIAL_GROK_ID,
};
pub use provider::{new_provider_id, Provider, ProviderSettings};

#[derive(Debug, Clone, PartialEq)]
pub enum ProviderForm {
    Codex(CodexForm),
    Claude(ClaudeForm),
    Grok(GrokForm),
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn third_party_config_is_responses_custom() {
        let toml = generate_third_party_config("PackyCode", "https://www.packyapi.ai/v1", "gpt-5.6-sol");
        assert!(toml.contains("model_provider = \"custom\""));
        assert!(toml.contains("wire_api = \"responses\""));
        assert!(toml.contains("requires_openai_auth = true"));
        assert!(toml.contains("base_url = \"https://www.packyapi.ai/v1\""));
        assert!(toml.contains("model = \"gpt-5.6-sol\""));
        assert_eq!(
            extract_codex_base_url(&toml).as_deref(),
            Some("https://www.packyapi.ai/v1")
        );
        assert_eq!(extract_codex_model(&toml).as_deref(), Some("gpt-5.6-sol"));
        assert_eq!(extract_codex_provider_name(&toml).as_deref(), Some("PackyCode"));
    }

    #[test]
    fn quotes_are_escaped_in_toml_strings() {
        let toml = generate_third_party_config(r#"Acme "Labs""#, "https://example.com/v1", "m");
        assert!(toml.contains(r#"name = "Acme \"Labs\"""#));
    }

    #[test]
    fn login_material_detects_oauth_and_api_key() {
        assert!(!has_login_material(&json!({})));
        assert!(!has_login_material(&json!({"OPENAI_API_KEY": ""})));
        assert!(has_login_material(&json!({"OPENAI_API_KEY": "sk-test"})));
        assert!(has_login_material(
            &json!({"tokens": {"access_token": "chatgpt-oauth"}})
        ));
    }

    #[test]
    fn parse_form_requires_third_party_fields() {
        let err = parse_codex_form(CodexForm {
            name: "x".into(),
            website_url: String::new(),
            kind: CodexKind::ResponsesThirdParty,
            api_key: String::new(),
            base_url: String::new(),
            model: String::new(),
            model_mappings: Vec::new(),
        })
        .unwrap_err();
        assert!(matches!(err, DomainError::Validation(_)));
    }

    #[test]
    fn official_form_does_not_need_key() {
        let settings = parse_codex_form(CodexForm {
            name: "OpenAI Official".into(),
            website_url: "https://chatgpt.com/codex".into(),
            kind: CodexKind::Official,
            api_key: String::new(),
            base_url: String::new(),
            model: String::new(),
            model_mappings: Vec::new(),
        })
        .unwrap();
        assert_eq!(settings.kind, CodexKind::Official);
        assert!(!has_login_material(&settings.auth));
        assert!(settings.config_toml.trim().is_empty());
    }

    #[test]
    fn claude_env_generation_works() {
        let form = ClaudeForm {
            name: "Anthropic Third Party".into(),
            website_url: "https://example.com".into(),
            kind: ClaudeKind::ThirdParty,
            api_key: "sk-ant-test".into(),
            base_url: "https://api.example.com".into(),
            model: "claude-3-7-sonnet-20250219".into(),
            model_mappings: Vec::new(),
        };
        let settings = parse_claude_form(form).unwrap();
        assert_eq!(settings.kind, ClaudeKind::ThirdParty);
        let env_obj = settings
            .env
            .get("env")
            .and_then(|v| v.as_object())
            .or_else(|| settings.env.as_object())
            .unwrap();
        assert_eq!(env_obj.get("ANTHROPIC_BASE_URL").unwrap(), "https://api.example.com");
        assert_eq!(env_obj.get("ANTHROPIC_AUTH_TOKEN").unwrap(), "sk-ant-test");
        assert_eq!(env_obj.get("ANTHROPIC_MODEL").unwrap(), "claude-3-7-sonnet-20250219");
    }

    #[test]
    fn grok_config_generation_works() {
        let form = GrokForm {
            name: "Packy Grok".into(),
            website_url: "https://packy.ai".into(),
            kind: GrokKind::ThirdParty,
            api_key: "xai-test-key".into(),
            base_url: "https://api.packy.ai/v1".into(),
            model: "grok-4.5".into(),
            model_mappings: Vec::new(),
        };
        let settings = parse_grok_form(form).unwrap();
        assert_eq!(settings.kind, GrokKind::ThirdParty);
        assert!(settings.config_toml.contains("base_url = \"https://api.packy.ai/v1\""));
        assert!(settings.config_toml.contains("api_key = \"xai-test-key\""));
        assert!(settings.config_toml.contains("model = \"grok-4.5\""));
    }
}
