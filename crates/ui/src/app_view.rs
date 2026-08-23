use std::path::PathBuf;

use domain::{
    extract_codex_api_key, extract_codex_base_url, extract_codex_model, has_login_material,
    CodexForm, CodexKind, CodexPreset, Provider, RESPONSES_PRESETS,
};
use gpui::{
    div, prelude::FluentBuilder, px, rgb, App, AppContext, Context, Entity, FontWeight, Hsla,
    InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Subscription, Window, WindowControlArea,
};
use gpui_component::{
    alert::Alert,
    button::{Button, ButtonVariants as _},
    divider::Divider,
    h_flex,
    input::{Input, InputState},
    notification::Notification,
    scroll::ScrollableElement,
    select::{Select, SelectEvent, SelectItem, SelectState},
    tag::Tag,
    v_flex, ActiveTheme, Disableable as _, Icon, IconName, Selectable as _, Sizable as _, WindowExt,
};
use session::Workspace;
use store::ThemePreference;

use crate::assets::CustomIcon;
use crate::theme::{self, StatusColors};

pub const CHROME_HEIGHT: f32 = 46.;

#[derive(Debug, Clone)]
pub struct PresetSelectItem {
    pub preset: CodexPreset,
}

impl SelectItem for PresetSelectItem {
    type Value = &'static str;

    fn title(&self) -> SharedString {
        self.preset.name.into()
    }

    fn value(&self) -> &Self::Value {
        &self.preset.id
    }

    fn render(&self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let is_official = self.preset.kind.is_official();
        h_flex()
            .items_center()
            .justify_between()
            .w_full()
            .gap(px(8.))
            .child(
                h_flex()
                    .items_center()
                    .gap(px(6.))
                    .child(if is_official {
                        IconName::Bot
                    } else {
                        IconName::SquareTerminal
                    })
                    .child(div().child(self.preset.name)),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(cx.theme().muted_foreground)
                    .child(if is_official {
                        "OAuth"
                    } else {
                        self.preset.model
                    }),
            )
    }

    fn matches(&self, query: &str) -> bool {
        let q = query.to_lowercase();
        self.preset.name.to_lowercase().contains(&q)
            || self.preset.model.to_lowercase().contains(&q)
            || self.preset.base_url.to_lowercase().contains(&q)
            || self.preset.id.to_lowercase().contains(&q)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Dashboard,
    Codex,
    Claude,
    Grok,
    Notifications,
    Settings,
}

pub struct RouterApp {
    workspace: Workspace,
    providers: Vec<Provider>,
    current_id: Option<String>,
    route: Route,
    sidebar_open: bool,
    theme: ThemePreference,
    search_input: Entity<InputState>,
    home_input: Entity<InputState>,
    last_error: Option<SharedString>,
    form: Option<FormDraft>,
    logs: Vec<String>,
}

struct FormDraft {
    editing_id: Option<String>,
    kind: CodexKind,
    name: Entity<InputState>,
    website_url: Entity<InputState>,
    api_key: Entity<InputState>,
    base_url: Entity<InputState>,
    model: Entity<InputState>,
    preset_select: Entity<SelectState<Vec<PresetSelectItem>>>,
    _preset_sub: Option<Subscription>,
}

impl RouterApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let workspace = Workspace::open_default().expect("open workspace");
        let settings = workspace.settings().unwrap_or_default();
        crate::theme::apply_theme(settings.theme, Some(window), cx);

        let home_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("~/.codex")
                .default_value(workspace.codex_home().display().to_string())
        });

        let search_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("搜索供应商、模型或端点...")
        });

        let mut app = Self {
            workspace,
            providers: Vec::new(),
            current_id: None,
            route: Route::Dashboard,
            sidebar_open: true,
            theme: settings.theme,
            search_input,
            home_input,
            last_error: None,
            form: None,
            logs: vec!["应用已启动并加载工作区".into()],
        };
        app.reload();
        app
    }

    fn reload(&mut self) {
        match self.workspace.snapshot() {
            Ok(snapshot) => {
                self.providers = snapshot.providers;
                self.current_id = snapshot.current_id;
                self.last_error = None;
            }
            Err(error) => self.last_error = Some(error.to_string().into()),
        }
    }

    fn set_route(&mut self, route: Route, cx: &mut Context<Self>) {
        self.route = route;
        cx.notify();
    }

    fn apply_home(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let raw = self.home_input.read(cx).value().to_string();
        let home = {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(PathBuf::from(trimmed))
            }
        };
        match self.workspace.apply_codex_home(home) {
            Ok(()) => {
                let path = self.workspace.codex_home().display().to_string();
                self.home_input
                    .update(cx, |input, cx| input.set_value(path, window, cx));
                self.reload();
                self.logs.push("已更新 Codex 工作区路径".into());
                notify_success("已更新 Codex 工作区目录", window, cx);
            }
            Err(error) => self.fail(error, window, cx),
        }
        cx.notify();
    }

    fn reset_home(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.workspace.apply_codex_home(None) {
            Ok(()) => {
                let path = self.workspace.codex_home().display().to_string();
                self.home_input
                    .update(cx, |input, cx| input.set_value(path, window, cx));
                self.reload();
                self.logs.push("已重置 Codex 工作区路径为默认".into());
                notify_success("已重置 Codex 目录为系统默认路径", window, cx);
            }
            Err(error) => self.fail(error, window, cx),
        }
        cx.notify();
    }

    fn enable(&mut self, provider_id: &str, window: &mut Window, cx: &mut Context<Self>) {
        match self.workspace.enable(provider_id) {
            Ok(()) => {
                let provider_name = self
                    .providers
                    .iter()
                    .find(|p| p.id == provider_id)
                    .map(|p| p.name.clone())
                    .unwrap_or_else(|| provider_id.to_string());
                let official = self
                    .providers
                    .iter()
                    .find(|provider| provider.id == provider_id)
                    .is_some_and(Provider::is_official_codex);
                self.reload();
                self.logs.push(format!("已切换并启用供应商: {}", provider_name));
                if official {
                    notify_success(
                        "已切到官方。未登录时请在终端执行 codex login，然后重启 Codex。",
                        window,
                        cx,
                    );
                } else {
                    notify_success(
                        format!("已启用 {} 并写入 ~/.codex，请重启 Codex / 终端生效。", provider_name),
                        window,
                        cx,
                    );
                }
            }
            Err(error) => self.fail(error, window, cx),
        }
        cx.notify();
    }

    fn duplicate(&mut self, provider_id: &str, window: &mut Window, cx: &mut Context<Self>) {
        match self.workspace.duplicate(provider_id) {
            Ok(_) => {
                self.reload();
                self.logs.push("复制了供应商配置".into());
                notify_success("已复制供应商配置", window, cx);
            }
            Err(error) => self.fail(error, window, cx),
        }
        cx.notify();
    }

    fn confirm_delete(&mut self, provider_id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let id = provider_id.to_string();
        let view = cx.entity();
        window.open_dialog(cx, move |dialog, _, _cx| {
            let target = id.clone();
            let view = view.clone();
            dialog
                .confirm()
                .title("确认删除供应商？")
                .child("当前启用的供应商无法删除。此操作仅移除 Router Switch 中的记录。")
                .on_ok(move |_, window, cx| {
                    let target = target.clone();
                    view.update(cx, |this, cx| {
                        match this.workspace.delete(&target) {
                            Ok(()) => {
                                this.reload();
                                this.logs.push(format!("删除了供应商: {}", target));
                                notify_success("供应商已成功删除", window, cx);
                            }
                            Err(error) => this.fail(error, window, cx),
                        }
                        cx.notify();
                    });
                    true
                })
        });
    }

    fn open_create_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.form = Some(FormDraft::create(window, cx));
        cx.notify();
    }

    fn open_edit_form(&mut self, provider_id: &str, window: &mut Window, cx: &mut Context<Self>) {
        match self.workspace.form_for(provider_id) {
            Ok(form) => {
                self.form = Some(FormDraft::from_codex_form(
                    Some(provider_id.to_string()),
                    form,
                    window,
                    cx,
                ));
                cx.notify();
            }
            Err(error) => self.fail(error, window, cx),
        }
    }

    fn submit_form(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(form) = self.form.as_ref() else {
            return true;
        };
        let editing_id = form.editing_id.clone();
        let payload = form.to_codex_form(cx);
        match self.workspace.save_form(editing_id.as_deref(), payload) {
            Ok(_) => {
                self.form = None;
                self.reload();
                self.logs.push("保存了供应商配置".into());
                notify_success("供应商配置已保存", window, cx);
                cx.notify();
                true
            }
            Err(error) => {
                self.fail(error, window, cx);
                false
            }
        }
    }

    fn apply_preset(
        &mut self,
        preset: CodexPreset,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(form) = self.form.as_mut() else {
            return;
        };
        form.kind = preset.kind;
        form.name
            .update(cx, |input, cx| input.set_value(preset.name, window, cx));
        form.website_url
            .update(cx, |input, cx| input.set_value(preset.website_url, window, cx));
        form.base_url
            .update(cx, |input, cx| input.set_value(preset.base_url, window, cx));
        form.model
            .update(cx, |input, cx| input.set_value(preset.model, window, cx));
        cx.notify();
    }

    fn set_form_kind(&mut self, kind: CodexKind, cx: &mut Context<Self>) {
        if let Some(form) = self.form.as_mut() {
            form.kind = kind;
        }
        cx.notify();
    }

    fn set_theme_preference(
        &mut self,
        theme: ThemePreference,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.theme = theme;
        if let Err(error) = self.workspace.set_theme(self.theme) {
            self.fail(error, window, cx);
            return;
        }
        crate::theme::apply_theme(self.theme, Some(window), cx);
        cx.notify();
    }

    fn fail(&mut self, error: impl ToString, window: &mut Window, cx: &mut Context<Self>) {
        let message = error.to_string();
        self.last_error = Some(message.clone().into());
        self.logs.push(format!("错误: {}", message));
        window.push_notification(Notification::error(message), cx);
        cx.notify();
    }

    fn filtered_providers(&self, cx: &App) -> Vec<Provider> {
        let query = self.search_input.read(cx).value().to_string();
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return self.providers.clone();
        }

        self.providers
            .iter()
            .filter(|provider| {
                if provider.name.to_lowercase().contains(&query) {
                    return true;
                }
                if let Some(site) = provider.website_url.as_ref() {
                    if site.to_lowercase().contains(&query) {
                        return true;
                    }
                }
                if let Some(settings) = provider.codex_settings() {
                    if let Some(model) = extract_codex_model(&settings.config_toml) {
                        if model.to_lowercase().contains(&query) {
                            return true;
                        }
                    }
                    if let Some(base_url) = extract_codex_base_url(&settings.config_toml) {
                        if base_url.to_lowercase().contains(&query) {
                            return true;
                        }
                    }
                }
                false
            })
            .cloned()
            .collect()
    }

    fn nav_item(
        &self,
        id: &'static str,
        icon: impl Into<Icon>,
        icon_color: Option<Hsla>,
        label: &'static str,
        route: Route,
        badge: Option<String>,
        disabled: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let active = self.route == route;
        let accent = cx.theme().sidebar_accent;
        let fg = cx.theme().sidebar_foreground;

        div()
            .id(SharedString::new_static(id))
            .h(px(38.))
            .w_full()
            .px(px(10.))
            .rounded(px(8.))
            .flex()
            .items_center()
            .gap(px(10.))
            .text_size(px(14.))
            .text_color(if disabled { fg.opacity(0.4) } else { fg })
            .when(active, |this| this.bg(accent).font_weight(FontWeight::SEMIBOLD))
            .when(!disabled, |this| this.hover(|this| this.bg(accent)))
            .child(
                Icon::new(icon)
                    .size(px(18.))
                    .flex_shrink_0()
                    .text_color(icon_color.unwrap_or(if active { cx.theme().foreground } else { fg.opacity(0.85) })),
            )
            .child(div().flex_1().truncate().child(label))
            .when_some(badge, |this, b| {
                this.child(
                    div()
                        .px(px(6.))
                        .py(px(1.))
                        .rounded(px(6.))
                        .bg(if active { cx.theme().primary } else { accent })
                        .text_size(px(11.))
                        .text_color(if active { cx.theme().primary_foreground } else { fg.opacity(0.6) })
                        .child(b),
                )
            })
            .when(!disabled, |this| {
                this.on_click(cx.listener(move |this, _, _, cx| this.set_route(route, cx)))
            })
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let count = self.providers.len();
        let border = cx.theme().sidebar_border;

        v_flex()
            .w(px(210.))
            .h_full()
            .flex_shrink_0()
            .p(px(8.))
            // Clear traffic lights and chrome bar
            .pt(px(48.))
            .gap(px(4.))
            .child(self.nav_item(
                "nav-dashboard",
                IconName::LayoutDashboard,
                Some(rgb(0x3B82F6).into()), // Blue
                "仪表盘",
                Route::Dashboard,
                None,
                false,
                cx,
            ))
            .child(self.nav_item(
                "nav-codex",
                CustomIcon::OpenAI,
                Some(rgb(0x10A37F).into()), // OpenAI Emerald Green
                "Codex",
                Route::Codex,
                Some(format!("{count}")),
                false,
                cx,
            ))
            .child(self.nav_item(
                "nav-claude",
                CustomIcon::Claude,
                Some(rgb(0xD97757).into()), // Claude Terracotta Orange
                "Claude Code",
                Route::Claude,
                Some("即将支持".into()),
                true,
                cx,
            ))
            .child(self.nav_item(
                "nav-grok",
                CustomIcon::Grok,
                Some(rgb(0x8B5CF6).into()), // xAI Violet
                "Grok Build",
                Route::Grok,
                Some("即将支持".into()),
                true,
                cx,
            ))
            .child(div().flex_1())
            .child(self.nav_item(
                "nav-notifications",
                IconName::Bell,
                Some(rgb(0xF59E0B).into()), // Amber
                "系统通知",
                Route::Notifications,
                if self.logs.len() > 1 {
                    Some(format!("{}", self.logs.len()))
                } else {
                    None
                },
                false,
                cx,
            ))
            .child(
                div()
                    .h(px(1.))
                    .mx(px(8.))
                    .my(px(4.))
                    .bg(border),
            )
            .child(self.nav_item(
                "nav-settings",
                IconName::Settings,
                Some(rgb(0x64748B).into()), // Slate
                "偏好设置",
                Route::Settings,
                None,
                false,
                cx,
            ))
    }

    fn chrome_button(
        &self,
        id: &'static str,
        icon: IconName,
        tooltip: &'static str,
        cx: &mut Context<Self>,
        on_click: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> impl IntoElement {
        Button::new(SharedString::new_static(id))
            .ghost()
            .xsmall()
            .icon(icon)
            .tooltip(tooltip)
            .on_click(cx.listener(move |this, _, window, cx| on_click(this, window, cx)))
    }

    fn render_chrome(&self, _window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let left_pad = if cfg!(target_os = "macos") { 96. } else { 16. };

        h_flex()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .h(px(CHROME_HEIGHT))
            .items_center()
            .child(
                div()
                    .w(px(left_pad))
                    .h_full()
                    .window_control_area(WindowControlArea::Drag),
            )
            .child(
                h_flex()
                    .items_center()
                    .child(self.chrome_button(
                        "chrome-sidebar",
                        IconName::PanelLeft,
                        "切换侧边栏 ⌘ B",
                        cx,
                        |this, _, cx| {
                            this.sidebar_open = !this.sidebar_open;
                            cx.notify();
                        },
                    )),
            )
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .window_control_area(WindowControlArea::Drag),
            )
    }

    fn render_dashboard_page(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let home = self.workspace.codex_home();
        let has_auth = home.join("auth.json").exists();
        let has_config = home.join("config.toml").exists();
        let current_provider = self
            .current_id
            .as_ref()
            .and_then(|id| self.providers.iter().find(|p| &p.id == id));
        let current_name: SharedString = current_provider
            .map(|p| p.name.clone().into())
            .unwrap_or_else(|| "未启用".into());
        let current_model: SharedString = current_provider
            .and_then(|p| p.codex_settings())
            .and_then(|s| extract_codex_model(&s.config_toml))
            .map(SharedString::from)
            .unwrap_or_else(|| "默认官方模型".into());
        let total_count = self.providers.len();
        let official_count = self.providers.iter().filter(|p| p.is_official_codex()).count();
        let third_party_count = total_count.saturating_sub(official_count);

        v_flex()
            .w_full()
            .gap(px(16.))
            .child(
                div()
                    .text_size(px(28.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(cx.theme().foreground)
                    .child("Dashboard"),
            )
            .child(
                // 4-tile metric grid
                h_flex()
                    .w_full()
                    .gap(px(12.))
                    .child(
                        theme::tile(cx)
                            .flex_1()
                            .child(
                                v_flex()
                                    .gap(px(8.))
                                    .child(theme::tile_label("ENGINE", cx))
                                    .child(
                                        h_flex()
                                            .items_center()
                                            .justify_between()
                                            .child(
                                                div()
                                                    .text_size(px(24.))
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(cx.theme().foreground)
                                                    .child(if current_provider.is_some() { "Active" } else { "Offline" }),
                                            )
                                            .child(
                                                div()
                                                    .size(px(10.))
                                                    .rounded_full()
                                                    .bg(if current_provider.is_some() {
                                                        rgb(StatusColors::GREEN_500)
                                                    } else {
                                                        rgb(StatusColors::RED_500)
                                                    }),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(cx.theme().muted_foreground)
                                            .line_clamp(1)
                                            .child(format!("Codex ({})", home.display())),
                                    ),
                            ),
                    )
                    .child(
                        theme::tile(cx)
                            .flex_1()
                            .child(
                                v_flex()
                                    .gap(px(8.))
                                    .child(theme::tile_label("ACTIVE PROVIDER", cx))
                                    .child(
                                        div()
                                            .text_size(px(24.))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(cx.theme().foreground)
                                            .line_clamp(1)
                                            .child(current_name),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(cx.theme().muted_foreground)
                                            .line_clamp(1)
                                            .child(current_model),
                                    ),
                            ),
                    )
                    .child(
                        theme::tile(cx)
                            .flex_1()
                            .child(
                                v_flex()
                                    .gap(px(8.))
                                    .child(theme::tile_label("AUTH & CONFIG", cx))
                                    .child(
                                        h_flex()
                                            .gap(px(6.))
                                            .child(
                                                div()
                                                    .px(px(8.))
                                                    .py(px(4.))
                                                    .rounded(px(6.))
                                                    .bg(if has_auth {
                                                        rgb(StatusColors::GREEN_100)
                                                    } else {
                                                        rgb(StatusColors::RED_100)
                                                    })
                                                    .text_size(px(12.))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(if has_auth {
                                                        rgb(StatusColors::GREEN_700)
                                                    } else {
                                                        rgb(StatusColors::RED_700)
                                                    })
                                                    .child(if has_auth { "auth.json 有" } else { "auth.json 无" }),
                                            )
                                            .child(
                                                div()
                                                    .px(px(8.))
                                                    .py(px(4.))
                                                    .rounded(px(6.))
                                                    .bg(if has_config {
                                                        rgb(StatusColors::BLUE_100)
                                                    } else {
                                                        rgb(StatusColors::RED_100)
                                                    })
                                                    .text_size(px(12.))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(if has_config {
                                                        rgb(StatusColors::BLUE_800)
                                                    } else {
                                                        rgb(StatusColors::RED_700)
                                                    })
                                                    .child(if has_config { "config.toml 有" } else { "config.toml 无" }),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(cx.theme().muted_foreground)
                                            .child("本地配置就绪状态"),
                                    ),
                            ),
                    )
                    .child(
                        theme::tile(cx)
                            .flex_1()
                            .child(
                                v_flex()
                                    .gap(px(8.))
                                    .child(theme::tile_label("TOTAL PROVIDERS", cx))
                                    .child(
                                        div()
                                            .text_size(px(24.))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(cx.theme().foreground)
                                            .child(format!("{total_count}")),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(cx.theme().muted_foreground)
                                            .child(format!("{} 官方 · {} 第三方", official_count, third_party_count)),
                                    ),
                            ),
                    ),
            )
            .child(
                // Quick switcher card
                theme::tile(cx).child(
                    v_flex()
                        .w_full()
                        .gap(px(12.))
                        .child(
                            h_flex()
                                .w_full()
                                .items_center()
                                .justify_between()
                                .child(theme::tile_label("QUICK SWITCHER / 快速切换", cx))
                                .child(
                                    Button::new("manage-all")
                                        .ghost()
                                        .small()
                                        .label("进入完整管理 →")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.set_route(Route::Codex, cx);
                                        })),
                                ),
                        )
                        .child(
                            v_flex()
                                .w_full()
                                .gap(px(8.))
                                .children(self.providers.iter().map(|provider| {
                                    let is_current = self.current_id.as_deref() == Some(&provider.id);
                                    let id = provider.id.clone();
                                    let settings = provider.codex_settings();
                                    let model = settings
                                        .and_then(|s| extract_codex_model(&s.config_toml))
                                        .unwrap_or_else(|| "默认模型".into());
                                    let endpoint = settings
                                        .and_then(|s| extract_codex_base_url(&s.config_toml))
                                        .unwrap_or_else(|| "官方端点".into());

                                    h_flex()
                                        .w_full()
                                        .items_center()
                                        .justify_between()
                                        .p(px(10.))
                                        .rounded(px(10.))
                                        .bg(if is_current {
                                            cx.theme().primary.opacity(0.06)
                                        } else {
                                            cx.theme().secondary.opacity(0.5)
                                        })
                                        .border_1()
                                        .border_color(if is_current {
                                            cx.theme().primary
                                        } else {
                                            cx.theme().border
                                        })
                                        .child(
                                            h_flex()
                                                .items_center()
                                                .gap(px(10.))
                                                .child(
                                                    div()
                                                        .size(px(24.))
                                                        .rounded(px(6.))
                                                        .flex()
                                                        .items_center()
                                                        .justify_center()
                                                        .bg(if is_current {
                                                            cx.theme().primary
                                                        } else {
                                                            cx.theme().border
                                                        })
                                                        .text_color(if is_current {
                                                            cx.theme().primary_foreground
                                                        } else {
                                                            cx.theme().foreground
                                                        })
                                                        .child(if provider.is_official_codex() {
                                                            IconName::Bot
                                                        } else {
                                                            IconName::SquareTerminal
                                                        }),
                                                )
                                                .child(
                                                    div()
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .text_size(px(14.))
                                                        .child(provider.name.clone()),
                                                )
                                                .when(is_current, |this| {
                                                    this.child(Tag::primary().small().child("使用中"))
                                                })
                                                .child(
                                                    div()
                                                        .text_size(px(12.))
                                                        .text_color(cx.theme().muted_foreground)
                                                        .child(format!("{} · {}", model, endpoint)),
                                                ),
                                        )
                                        .child(
                                            Button::new(SharedString::from(format!("dash-enable-{}", provider.id)))
                                                .primary()
                                                .small()
                                                .label(if is_current { "使用中" } else { "一键切换" })
                                                .disabled(is_current)
                                                .on_click(cx.listener(move |this, _, window, cx| {
                                                    this.enable(&id, window, cx);
                                                })),
                                        )
                                })),
                        ),
                ),
            )
    }

    fn render_codex_page(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let filtered = self.filtered_providers(cx);

        v_flex()
            .w_full()
            .gap(px(14.))
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .gap(px(12.))
                    .child(
                        h_flex()
                            .flex_1()
                            .items_center()
                            .gap(px(8.))
                            .child(Input::new(&self.search_input).cleanable(true)),
                    )
                    .child(
                        Button::new("codex-add-top")
                            .primary()
                            .icon(IconName::Plus)
                            .label("新建供应商")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_create_form(window, cx);
                            })),
                    ),
            )
            .child(
                if self.providers.is_empty() {
                    empty_state(cx).into_any_element()
                } else if filtered.is_empty() {
                    empty_search_state(cx).into_any_element()
                } else {
                    v_flex()
                        .w_full()
                        .gap(px(10.))
                        .children(filtered.iter().map(|provider| {
                            self.provider_card(provider, cx)
                        }))
                        .into_any_element()
                },
            )
    }

    fn provider_card(&self, provider: &Provider, cx: &mut Context<Self>) -> impl IntoElement {
        let id = provider.id.clone();
        let is_current = self.current_id.as_deref() == Some(&provider.id);
        let settings = provider.codex_settings();
        let kind = settings.map(|s| s.kind).unwrap_or(CodexKind::Official);
        let endpoint = settings
            .and_then(|s| extract_codex_base_url(&s.config_toml))
            .unwrap_or_else(|| "官方默认端点 (https://api.openai.com/v1)".into());
        let model = settings
            .and_then(|s| extract_codex_model(&s.config_toml))
            .unwrap_or_else(|| "默认模型".into());
        let login_type = settings
            .map(|s| {
                if extract_codex_api_key(&s.auth).is_some() {
                    "API Key"
                } else if has_login_material(&s.auth) {
                    "ChatGPT OAuth"
                } else {
                    "未存登录材料"
                }
            })
            .unwrap_or("未存登录材料");
        let website_url = provider.website_url.clone();

        theme::tile(cx)
            .map(|this| {
                if is_current {
                    this.border_color(cx.theme().primary).bg(cx.theme().primary.opacity(0.04))
                } else {
                    this
                }
            })
            .child(
                h_flex()
                    .w_full()
                    .items_start()
                    .justify_between()
                    .gap(px(12.))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(6.))
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(
                                        div()
                                            .size(px(28.))
                                            .rounded(px(8.))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .bg(if is_current {
                                                cx.theme().primary
                                            } else {
                                                cx.theme().border
                                            })
                                            .text_color(if is_current {
                                                cx.theme().primary_foreground
                                            } else {
                                                cx.theme().foreground
                                            })
                                            .child(if kind.is_official() {
                                                IconName::Bot
                                            } else {
                                                IconName::SquareTerminal
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(15.))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(cx.theme().foreground)
                                            .child(provider.name.clone()),
                                    )
                                    .when(is_current, |this| {
                                        this.child(Tag::primary().small().child("使用中"))
                                    })
                                    .child(
                                        if kind.is_official() {
                                            Tag::secondary().small().child("官方")
                                        } else {
                                            Tag::info().small().child("Responses 第三方")
                                        },
                                    ),
                            )
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap(px(4.))
                                    .child(Icon::new(IconName::Globe).size(px(14.)).text_color(cx.theme().muted_foreground))
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(cx.theme().foreground)
                                            .child(endpoint),
                                    )
                                    .when_some(website_url.filter(|s| !s.trim().is_empty()), |this, url| {
                                        let target = url.clone();
                                        this.child(
                                            Button::new(SharedString::from(format!("web-{}", provider.id)))
                                                .ghost()
                                                .xsmall()
                                                .icon(IconName::ExternalLink)
                                                .tooltip(format!("官网：{}", target))
                                                .on_click(move |_, _, cx| {
                                                    cx.open_url(&target);
                                                }),
                                        )
                                    }),
                            )
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(
                                        h_flex()
                                            .items_center()
                                            .gap(px(2.))
                                            .child(
                                                div()
                                                    .text_size(px(11.))
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child("模型:"),
                                            )
                                            .child(Tag::secondary().small().child(model)),
                                    )
                                    .child(
                                        h_flex()
                                            .items_center()
                                            .gap(px(2.))
                                            .child(
                                                div()
                                                    .text_size(px(11.))
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child("凭证:"),
                                            )
                                            .child(Tag::secondary().small().outline().child(login_type)),
                                    ),
                            ),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap(px(6.))
                            .child(
                                Button::new(SharedString::from(format!("enable-{}", provider.id)))
                                    .primary()
                                    .small()
                                    .icon(if is_current {
                                        IconName::Check
                                    } else {
                                        IconName::SquareTerminal
                                    })
                                    .label(if is_current { "使用中" } else { "启用" })
                                    .disabled(is_current)
                                    .on_click(cx.listener({
                                        let id = id.clone();
                                        move |this, _, window, cx| this.enable(&id, window, cx)
                                    })),
                            )
                            .child(
                                Button::new(SharedString::from(format!("edit-{}", provider.id)))
                                    .outline()
                                    .small()
                                    .icon(IconName::Settings2)
                                    .label("编辑")
                                    .on_click(cx.listener({
                                        let id = id.clone();
                                        move |this, _, window, cx| {
                                            this.open_edit_form(&id, window, cx)
                                        }
                                    })),
                            )
                            .child(
                                Button::new(SharedString::from(format!("copy-{}", provider.id)))
                                    .ghost()
                                    .small()
                                    .icon(IconName::Copy)
                                    .tooltip("复制供应商")
                                    .on_click(cx.listener({
                                        let id = id.clone();
                                        move |this, _, window, cx| this.duplicate(&id, window, cx)
                                    })),
                            )
                            .child(
                                Button::new(SharedString::from(format!("delete-{}", provider.id)))
                                    .ghost()
                                    .small()
                                    .icon(IconName::Delete)
                                    .tooltip("删除供应商")
                                    .disabled(is_current)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.confirm_delete(&id, window, cx)
                                    })),
                            ),
                    ),
            )
    }

    fn render_settings_page(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let home = self.workspace.codex_home();
        let has_auth = home.join("auth.json").exists();
        let has_config = home.join("config.toml").exists();

        v_flex()
            .w_full()
            .gap(px(16.))
            .child(
                div()
                    .text_size(px(28.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(cx.theme().foreground)
                    .child("Settings"),
            )
            .child(
                // Workspace Path Card
                theme::tile(cx).child(
                    v_flex()
                        .w_full()
                        .gap(px(10.))
                        .child(theme::tile_label("CODEX WORKSPACE / 工作区路径", cx))
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(cx.theme().muted_foreground)
                                .child("启用供应商时，将原子写入此目录下的 auth.json 与 config.toml。"),
                        )
                        .child(
                            h_flex()
                                .w_full()
                                .gap(px(8.))
                                .items_center()
                                .child(div().flex_1().child(Input::new(&self.home_input).cleanable(true)))
                                .child(
                                    Button::new("set-apply-home")
                                        .primary()
                                        .small()
                                        .label("应用路径")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.apply_home(window, cx);
                                        })),
                                )
                                .child(
                                    Button::new("set-reset-home")
                                        .outline()
                                        .small()
                                        .label("恢复默认")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.reset_home(window, cx);
                                        })),
                                ),
                        ),
                ),
            )
            .child(
                // Appearance Card
                theme::tile(cx).child(
                    v_flex()
                        .w_full()
                        .gap(px(10.))
                        .child(theme::tile_label("APPEARANCE / 外观主题", cx))
                        .child(
                            h_flex()
                                .gap(px(8.))
                                .child(
                                    Button::new("theme-sys")
                                        .outline()
                                        .small()
                                        .selected(self.theme == ThemePreference::System)
                                        .label("跟随系统")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.set_theme_preference(ThemePreference::System, window, cx);
                                        })),
                                )
                                .child(
                                    Button::new("theme-lt")
                                        .outline()
                                        .small()
                                        .selected(self.theme == ThemePreference::Light)
                                        .label("浅色主题")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.set_theme_preference(ThemePreference::Light, window, cx);
                                        })),
                                )
                                .child(
                                    Button::new("theme-dk")
                                        .outline()
                                        .small()
                                        .selected(self.theme == ThemePreference::Dark)
                                        .label("深色主题")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.set_theme_preference(ThemePreference::Dark, window, cx);
                                        })),
                                ),
                        ),
                ),
            )
            .child(
                // Diagnostics Card
                theme::tile(cx).child(
                    v_flex()
                        .w_full()
                        .gap(px(10.))
                        .child(theme::tile_label("DIAGNOSTICS / 运行与文件状态", cx))
                        .child(
                            h_flex()
                                .gap(px(12.))
                                .child(
                                    h_flex()
                                        .items_center()
                                        .gap(px(6.))
                                        .child(
                                            div()
                                                .text_size(px(13.))
                                                .child("auth.json:"),
                                        )
                                        .child(if has_auth {
                                            Tag::success().small().child("存在")
                                        } else {
                                            Tag::danger().small().child("缺失")
                                        }),
                                )
                                .child(
                                    h_flex()
                                        .items_center()
                                        .gap(px(6.))
                                        .child(
                                            div()
                                                .text_size(px(13.))
                                                .child("config.toml:"),
                                        )
                                        .child(if has_config {
                                            Tag::success().small().child("存在")
                                        } else {
                                            Tag::danger().small().child("缺失")
                                        }),
                                ),
                        )
                        .child(
                            Alert::info(
                                "settings-help",
                                "切换配置后，请完全重启终端或正在运行的 Codex 进程以便生效。",
                            ),
                        ),
                ),
            )
            .child(
                // About Card
                theme::tile(cx).child(
                    v_flex()
                        .w_full()
                        .gap(px(6.))
                        .child(theme::tile_label("ABOUT / 关于", cx))
                        .child(
                            div()
                                .text_size(px(14.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(cx.theme().foreground)
                                .child("Router Switch v0.1.0"),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(cx.theme().muted_foreground)
                                .child("基于 GPUI 构建的现代化 AI 编程配置中枢，完美对齐 Motrix 桌面体验。"),
                        ),
                ),
            )
    }

    fn render_notifications_page(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .w_full()
            .gap(px(16.))
            .child(
                div()
                    .text_size(px(28.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(cx.theme().foreground)
                    .child("Notifications"),
            )
            .child(
                theme::tile(cx).child(
                    v_flex()
                        .w_full()
                        .gap(px(8.))
                        .child(theme::tile_label("ACTIVITY LOG / 操作日志", cx))
                        .children(self.logs.iter().rev().map(|log| {
                            h_flex()
                                .w_full()
                                .items_center()
                                .gap(px(8.))
                                .py(px(4.))
                                .child(
                                    div()
                                        .size(px(6.))
                                        .rounded_full()
                                        .bg(cx.theme().primary),
                                )
                                .child(
                                    div()
                                        .text_size(px(13.))
                                        .text_color(cx.theme().foreground)
                                        .child(log.clone()),
                                )
                        })),
                ),
            )
    }

    fn form_body(&self, view: Entity<Self>, cx: &App) -> impl IntoElement {
        let Some(form) = self.form.as_ref() else {
            return div().child("表单已关闭").into_any_element();
        };
        let theme = cx.theme();
        let official = form.kind.is_official();

        v_flex()
            .w_full()
            .gap(px(12.))
            .child(
                v_flex()
                    .gap(px(4.))
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.foreground)
                            .child("选择预设供应商模版（支持搜索）："),
                    )
                    .child(
                        Select::new(&form.preset_select)
                            .placeholder("从预设模版快速填充 (如 DeepSeek, Kimi, 阿里百炼...)")
                            .search_placeholder("搜索预设模版...")
                            .cleanable(true),
                    ),
            )
            .child(Divider::horizontal())
            .child(
                v_flex()
                    .gap(px(4.))
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.foreground)
                            .child("协议类型："),
                    )
                    .child(
                        h_flex()
                            .gap(px(6.))
                            .child({
                                let view = view.clone();
                                Button::new("kind-official")
                                    .outline()
                                    .small()
                                    .label("OpenAI 官方 (ChatGPT OAuth)")
                                    .selected(official)
                                    .on_click(move |_, _, cx| {
                                        view.update(cx, |this, cx| {
                                            this.set_form_kind(CodexKind::Official, cx)
                                        });
                                    })
                            })
                            .child({
                                let view = view.clone();
                                Button::new("kind-third-party")
                                    .outline()
                                    .small()
                                    .label("Responses 第三方 (API Key)")
                                    .selected(!official)
                                    .on_click(move |_, _, cx| {
                                        view.update(cx, |this, cx| {
                                            this.set_form_kind(CodexKind::ResponsesThirdParty, cx)
                                        });
                                    })
                            }),
                    ),
            )
            .child(form_field("供应商名称", Input::new(&form.name)))
            .child(form_field("供应商官网 (可选)", Input::new(&form.website_url).cleanable(true)))
            .when(!official, |this| {
                this.child(form_field(
                    "API Key (OPENAI_API_KEY)",
                    Input::new(&form.api_key).mask_toggle(),
                ))
                .child(form_field("API 端点 (Base URL)", Input::new(&form.base_url)))
                .child(form_field("模型名称 (Model)", Input::new(&form.model)))
            })
            .child(
                if official {
                    Alert::info(
                        "form-official-notice",
                        "官方供应商启用时仅整理 config.toml，不修改 ChatGPT OAuth 凭证。未登录时请在终端运行 codex login。",
                    )
                } else {
                    Alert::info(
                        "form-custom-notice",
                        "第三方供应商将以 responses 协议写入 ~/.codex/config.toml 并写入 OPENAI_API_KEY。",
                    )
                },
            )
            .into_any_element()
    }

    fn render_form_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(form) = self.form.as_ref() else {
            return div().into_any_element();
        };

        let is_editing = form.editing_id.is_some();
        let title = if is_editing {
            "编辑 Codex 供应商"
        } else {
            "新建 Codex 供应商"
        };
        let view = cx.entity();
        let view_for_body = view.clone();
        let theme = cx.theme();

        div()
            .id("form-modal-backdrop")
            .absolute()
            .inset_0()
            .bg(gpui::hsla(0., 0., 0., 0.5))
            .flex()
            .items_center()
            .justify_center()
            .p(px(20.))
            .on_click(cx.listener(|this, _, _, cx| {
                this.form = None;
                cx.notify();
            }))
            .child(
                div()
                    .id("form-modal-container")
                    .w(px(520.))
                    .max_h(px(580.))
                    .bg(theme.background)
                    .border_1()
                    .border_color(theme.border)
                    .rounded(px(14.))
                    .shadow_lg()
                    .p(px(20.))
                    .flex()
                    .flex_col()
                    .gap(px(16.))
                    .on_click(|_, _, cx| {
                        cx.stop_propagation();
                    })
                    .child(
                        h_flex()
                            .w_full()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(16.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme.foreground)
                                    .child(title),
                            )
                            .child(
                                Button::new("close-form")
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::Close)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.form = None;
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scrollbar()
                            .pr(px(4.))
                            .child(self.form_body(view_for_body, cx)),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .items_center()
                            .justify_end()
                            .gap(px(10.))
                            .pt(px(10.))
                            .border_t_1()
                            .border_color(theme.border)
                            .child(
                                Button::new("cancel-form")
                                    .outline()
                                    .small()
                                    .label("取消")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.form = None;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("submit-form")
                                    .primary()
                                    .small()
                                    .label("保存配置")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.submit_form(window, cx);
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }
}

impl Render for RouterApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dark = cx.theme().is_dark();
        let page = match self.route {
            Route::Dashboard => self.render_dashboard_page(cx).into_any_element(),
            Route::Codex => self.render_codex_page(cx).into_any_element(),
            Route::Claude => self.render_codex_page(cx).into_any_element(),
            Route::Grok => self.render_codex_page(cx).into_any_element(),
            Route::Notifications => self.render_notifications_page(cx).into_any_element(),
            Route::Settings => self.render_settings_page(cx).into_any_element(),
        };

        div()
            .size_full()
            .relative()
            .child(
                h_flex()
                    .size_full()
                    .relative()
                    .bg(cx.theme().sidebar)
                    .text_color(cx.theme().foreground)
                    .font_family(".SystemUIFont")
                    .key_context("RouterApp")
                    .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _window, cx| {
                        if (event.keystroke.modifiers.platform || event.keystroke.modifiers.control) && event.keystroke.key == "b" {
                            this.sidebar_open = !this.sidebar_open;
                            cx.notify();
                        }
                    }))
                    .when(self.sidebar_open, |this| {
                        this.child(self.render_sidebar(cx))
                    })
                    .child(
                        div()
                            .flex_1()
                            .h_full()
                            .min_w_0()
                            .p(px(8.))
                            .when(self.sidebar_open, |this| this.pl(px(0.)))
                            .child(
                                div()
                                    .size_full()
                                    .rounded(px(14.))
                                    .overflow_hidden()
                                    .flex()
                                    .flex_col()
                                    .bg(theme::inset_bg(dark))
                                    .shadow_sm()
                                    .when(!self.sidebar_open, |this| this.pt(px(28.)))
                                    .child(
                                        div()
                                            .id("main-scroll")
                                            .flex_1()
                                            .min_h_0()
                                            .p(px(20.))
                                            .overflow_y_scrollbar()
                                            .child(page),
                                    ),
                            ),
                    )
                    .child(self.render_chrome(window, cx)),
            )
            .when(self.form.is_some(), |this| {
                this.child(self.render_form_modal(cx))
            })
            .children(gpui_component::Root::render_dialog_layer(window, cx))
            .children(gpui_component::Root::render_sheet_layer(window, cx))
            .children(gpui_component::Root::render_notification_layer(window, cx))
    }
}

impl FormDraft {
    fn create(window: &mut Window, cx: &mut Context<RouterApp>) -> Self {
        Self::from_codex_form(
            None,
            CodexForm {
                name: String::new(),
                website_url: String::new(),
                kind: CodexKind::ResponsesThirdParty,
                api_key: String::new(),
                base_url: "https://api.example.com/v1".into(),
                model: domain::DEFAULT_CODEX_MODEL.into(),
            },
            window,
            cx,
        )
    }

    fn from_codex_form(
        editing_id: Option<String>,
        form: CodexForm,
        window: &mut Window,
        cx: &mut Context<RouterApp>,
    ) -> Self {
        let presets: Vec<PresetSelectItem> = RESPONSES_PRESETS
            .iter()
            .copied()
            .map(|p| PresetSelectItem { preset: p })
            .collect();

        let selected_index = if form.name.trim().is_empty() {
            None
        } else {
            RESPONSES_PRESETS
                .iter()
                .position(|p| p.name == form.name)
                .map(|idx| gpui_component::IndexPath::default().row(idx))
        };

        let preset_select = cx.new(|cx| {
            SelectState::new(presets, selected_index, window, cx).searchable(true)
        });

        let view = cx.entity();
        let _preset_sub = window.subscribe(
            &preset_select,
            cx,
            move |_, event: &SelectEvent<Vec<PresetSelectItem>>, window, cx| {
                if let SelectEvent::Confirm(Some(preset_id)) = event {
                    if let Some(preset) = RESPONSES_PRESETS.iter().find(|p| p.id == *preset_id) {
                        view.update(cx, |this, cx| {
                            this.apply_preset(*preset, window, cx);
                        });
                    }
                }
            },
        );

        Self {
            editing_id,
            kind: form.kind,
            name: field(window, cx, &form.name, "输入供应商名称，如 PackyCode"),
            website_url: field(window, cx, &form.website_url, "https://..."),
            api_key: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("sk-...")
                    .masked(true)
                    .default_value(form.api_key)
            }),
            base_url: field(window, cx, &form.base_url, "https://api.example.com/v1"),
            model: field(window, cx, &form.model, "gpt-5.6-sol"),
            preset_select,
            _preset_sub: Some(_preset_sub),
        }
    }

    fn to_codex_form(&self, cx: &App) -> CodexForm {
        CodexForm {
            name: self.name.read(cx).value().to_string(),
            website_url: self.website_url.read(cx).value().to_string(),
            kind: self.kind,
            api_key: self.api_key.read(cx).value().to_string(),
            base_url: self.base_url.read(cx).value().to_string(),
            model: self.model.read(cx).value().to_string(),
        }
    }
}

fn field(
    window: &mut Window,
    cx: &mut Context<RouterApp>,
    value: &str,
    placeholder: &str,
) -> Entity<InputState> {
    let value = value.to_string();
    let placeholder = placeholder.to_string();
    cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder(placeholder)
            .default_value(value)
    })
}

fn notify_success(message: impl Into<SharedString>, window: &mut Window, cx: &mut App) {
    window.push_notification(Notification::success(message), cx);
}

fn empty_state(cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    v_flex()
        .w_full()
        .items_center()
        .justify_center()
        .gap(px(12.))
        .py(px(48.))
        .rounded(px(16.))
        .border_1()
        .border_color(theme.border)
        .bg(theme.secondary.opacity(0.4))
        .child(
            div()
                .size(px(48.))
                .rounded(px(12.))
                .bg(theme.border)
                .flex()
                .items_center()
                .justify_center()
                .text_color(theme.muted_foreground)
                .child(Icon::new(IconName::Bot).size(px(24.))),
        )
        .child(
            div()
                .text_size(px(16.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.foreground)
                .child("还没有配置 Codex 供应商"),
        )
        .child(
            div()
                .text_size(px(12.))
                .text_color(theme.muted_foreground)
                .child("你可以点击右上角的「新建供应商」，或点击「导入 Live」从现有的 ~/.codex 快速导入。"),
        )
}

fn empty_search_state(cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    v_flex()
        .w_full()
        .items_center()
        .justify_center()
        .gap(px(12.))
        .py(px(48.))
        .rounded(px(16.))
        .border_1()
        .border_color(theme.border)
        .bg(theme.secondary.opacity(0.4))
        .child(
            div()
                .size(px(40.))
                .rounded(px(10.))
                .bg(theme.border)
                .flex()
                .items_center()
                .justify_center()
                .text_color(theme.muted_foreground)
                .child(Icon::new(IconName::Search).size(px(20.))),
        )
        .child(
            div()
                .text_size(px(14.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.foreground)
                .child("未找到匹配的供应商"),
        )
        .child(
            div()
                .text_size(px(12.))
                .text_color(theme.muted_foreground)
                .child("请尝试修改搜索词，或清空搜索栏查看所有供应商。"),
        )
}

fn form_field(label: &str, field: impl IntoElement) -> impl IntoElement {
    v_flex()
        .w_full()
        .gap(px(4.))
        .child(
            div()
                .text_size(px(12.))
                .font_weight(FontWeight::MEDIUM)
                .child(label.to_string()),
        )
        .child(field)
}
