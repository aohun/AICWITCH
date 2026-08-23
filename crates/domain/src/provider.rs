use serde::{Deserialize, Serialize};

use crate::{AppKind, CodexSettings};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,
    pub app: AppKind,
    pub name: String,
    pub website_url: Option<String>,
    pub settings: ProviderSettings,
    pub created_at: i64,
    pub sort_index: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ProviderSettings {
    Codex(CodexSettings),
    Unsupported { app: AppKind },
}

impl Provider {
    pub fn codex_settings(&self) -> Option<&CodexSettings> {
        match &self.settings {
            ProviderSettings::Codex(settings) => Some(settings),
            ProviderSettings::Unsupported { .. } => None,
        }
    }

    pub fn is_official_codex(&self) -> bool {
        self.codex_settings()
            .is_some_and(|settings| settings.kind.is_official())
    }
}

pub fn new_provider_id(name: &str) -> String {
    let slug = slugify(name);
    let millis = unix_millis();
    if slug.is_empty() {
        format!("provider-{millis}")
    } else {
        format!("{slug}-{millis}")
    }
}

fn slugify(name: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

fn unix_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}