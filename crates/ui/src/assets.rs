use std::borrow::Cow;
use gpui::{AssetSource, SharedString};
use gpui_component::IconNamed;

pub struct AppAssets;

const OPENAI_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor">
  <path d="M22.282 9.821a5.985 5.985 0 0 0-.516-4.91 6.046 6.046 0 0 0-6.51-2.9A6.065 6.065 0 0 0 4.981 4.18a5.985 5.985 0 0 0-3.998 2.9 6.046 6.046 0 0 0 .743 7.097 5.98 5.98 0 0 0 .51 4.911 6.051 6.051 0 0 0 6.515 2.9A5.985 5.985 0 0 0 13.26 24a6.056 6.056 0 0 0 5.772-4.206 5.99 5.99 0 0 0 3.997-2.9 6.056 6.056 0 0 0-.747-7.073zM13.26 22.43a4.476 4.476 0 0 1-2.876-1.04l.141-.081 4.779-2.758a.795.795 0 0 0 .392-.681v-6.737l2.02 1.168a.071.071 0 0 1 .038.052v5.583a4.504 4.504 0 0 1-4.494 4.494zM3.6 18.304a4.47 4.47 0 0 1-.535-3.014l.142.085 4.783 2.759a.771.771 0 0 0 .78 0l5.843-3.369v2.332a.08.08 0 0 1-.033.062L9.74 19.95a4.5 4.5 0 0 1-6.14-1.646zM2.34 8.784a4.482 4.482 0 0 1 2.366-1.973V12.7a.766.766 0 0 0 .388.676l5.815 3.355-2.02 1.168a.076.076 0 0 1-.071 0l-4.83-2.786A4.504 4.504 0 0 1 2.34 8.784zm16.597 3.855l-5.833-3.387L15.119 8.1a.076.076 0 0 1 .071 0l4.83 2.791a4.494 4.494 0 0 1-.676 8.105v-5.678a.79.79 0 0 0-.407-.667zm2.01-3.023l-.141-.085-4.774-2.782a.776.776 0 0 0-.785 0L9.409 10.12V7.784a.08.08 0 0 1 .033-.061L14.282 4.9a4.5 4.5 0 0 1 6.668 4.716zm-9.932 3.86l2.977-1.719 2.977 1.719v3.438l-2.977 1.719-2.977-1.719z"/>
</svg>"#;

const CLAUDE_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor">
  <path d="M4.5 10.5C3.67 10.5 3 11.17 3 12s.67 1.5 1.5 1.5h2.88l-2.04 2.04c-.58.58-.58 1.54 0 2.12.58.58 1.54.58 2.12 0l2.04-2.04V18.5c0 .83.67 1.5 1.5 1.5s1.5-.67 1.5-1.5v-2.88l2.04 2.04c.58.58 1.54.58 2.12 0 .58-.58.58-1.54 0-2.12l-2.04-2.04H19.5c.83 0 1.5-.67 1.5-1.5s-.67-1.5-1.5-1.5h-2.88l2.04-2.04c.58-.58.58-1.54 0-2.12-.58-.58-1.54-.58-2.12 0l-2.04 2.04V4.5C14.5 3.67 13.83 3 13 3s-1.5.67-1.5 1.5v2.88l-2.04-2.04c-.58-.58-1.54-.58-2.12 0-.58.58-.58 1.54 0 2.12l2.04 2.04H4.5z"/>
</svg>"#;

const GROK_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor">
  <path d="M18.244 2.25h3.308l-7.227 8.26 8.502 11.24H16.17l-5.214-6.817L4.99 21.75H1.68l7.73-8.835L1.254 2.25H8.08l4.713 6.231zm-1.161 17.52h1.833L7.084 4.126H5.117z"/>
</svg>"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomIcon {
    OpenAI,
    Claude,
    Grok,
}

impl IconNamed for CustomIcon {
    fn path(self) -> SharedString {
        match self {
            Self::OpenAI => "icons/custom/openai.svg",
            Self::Claude => "icons/custom/claude.svg",
            Self::Grok => "icons/custom/grok.svg",
        }
        .into()
    }
}

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        match path {
            "icons/custom/openai.svg" => Ok(Some(Cow::Borrowed(OPENAI_SVG.as_bytes()))),
            "icons/custom/claude.svg" => Ok(Some(Cow::Borrowed(CLAUDE_SVG.as_bytes()))),
            "icons/custom/grok.svg" => Ok(Some(Cow::Borrowed(GROK_SVG.as_bytes()))),
            _ => gpui_component_assets::Assets.load(path),
        }
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        let mut list = gpui_component_assets::Assets.list(path)?;
        if path.is_empty() || path == "icons" || path == "icons/custom" {
            list.push("icons/custom/openai.svg".into());
            list.push("icons/custom/claude.svg".into());
            list.push("icons/custom/grok.svg".into());
        }
        Ok(list)
    }
}
