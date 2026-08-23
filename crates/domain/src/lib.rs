//! Pure Codex / provider domain. No filesystem or SQLite here.

mod app_kind;
mod codex;
mod error;
mod provider;

pub use app_kind::AppKind;
pub use codex::{
    backfill_codex_settings, extract_codex_api_key, extract_codex_base_url, extract_codex_model,
    extract_codex_provider_name, fetch_models_from_api, generate_catalog_json,
    generate_third_party_auth, generate_third_party_config,
    generate_third_party_config_with_catalog, has_login_material, official_codex_provider,
    official_codex_settings, parse_codex_form, CodexForm, CodexKind, CodexModelMapping,
    CodexPreset, CodexSettings, DEFAULT_CODEX_MODEL, OFFICIAL_CODEX_ID, RESPONSES_PRESETS,
};
pub use error::DomainError;
pub use provider::{new_provider_id, Provider, ProviderSettings};

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
    fn backfill_prefers_live_key_and_toml() {
        let stored = parse_codex_form(CodexForm {
            name: "Packy".into(),
            website_url: String::new(),
            kind: CodexKind::ResponsesThirdParty,
            api_key: "old".into(),
            base_url: "https://old.example/v1".into(),
            model: "old-model".into(),
            model_mappings: Vec::new(),
        })
        .unwrap();
        let live_auth = json!({"OPENAI_API_KEY": "live-key"});
        let live_toml = generate_third_party_config("Packy", "https://live.example/v1", "live-model");
        let merged = backfill_codex_settings(&stored, &live_auth, &live_toml);
        assert_eq!(extract_codex_api_key(&merged.auth).as_deref(), Some("live-key"));
        assert_eq!(
            extract_codex_base_url(&merged.config_toml).as_deref(),
            Some("https://live.example/v1")
        );
        assert_eq!(extract_codex_model(&merged.config_toml).as_deref(), Some("live-model"));
    }

    #[test]
    fn model_mapping_generates_catalog_json_and_config_link() {
        let mappings = vec![
            CodexModelMapping {
                display_name: "DeepSeek V4".into(),
                model: "deepseek-v4-flash".into(),
                context_window: Some(64_000),
                reasoning_effort: Some("high".into()),
            },
            CodexModelMapping {
                display_name: "".into(),
                model: "kimi-k2.7".into(),
                context_window: None,
                reasoning_effort: None,
            },
        ];

        let catalog_json = generate_catalog_json(&mappings).expect("catalog json");
        assert!(catalog_json.contains("deepseek-v4-flash"));
        assert!(catalog_json.contains("DeepSeek V4"));
        assert!(catalog_json.contains("64000"));
        assert!(catalog_json.contains("\"reasoning_effort\": \"high\""));
        assert!(catalog_json.contains("kimi-k2.7"));

        let settings = parse_codex_form(CodexForm {
            name: "MultiModel".into(),
            website_url: String::new(),
            kind: CodexKind::ResponsesThirdParty,
            api_key: "sk-123".into(),
            base_url: "https://api.example.com/v1".into(),
            model: "deepseek-v4-flash".into(),
            model_mappings: mappings,
        })
        .unwrap();

        assert!(settings.config_toml.contains("model_catalog_json = \"router-switch-model-catalog.json\""));
    }
}