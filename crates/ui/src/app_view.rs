use domain::{
    extract_codex_api_key, extract_codex_base_url, extract_codex_model, has_login_material,
    CodexForm, CodexKind, CodexModelMapping, CodexPreset, Provider, RESPONSES_PRESETS,
};
use gpui::{
    div, prelude::FluentBuilder, px, rgb, App, AppContext, Context, Entity, FontWeight, Hsla,
    InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Subscription, Window, WindowControlArea,
};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputState},
    notification::Notification,
    scroll::ScrollableElement,
    select::{Select, SelectEvent, SelectItem, SelectState},
    tag::Tag,
    v_flex, ActiveTheme, Disableable as _, Icon, IconName, Selectable as _, Sizable as _, WindowExt,
};
use session::Workspace;
use store::{AppLanguage, ThemePreference};

use crate::assets::CustomIcon;
use crate::theme::{self, StatusColors};

pub const CHROME_HEIGHT: f32 = 46.;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DragAppId(pub String);

pub struct DragGhostView {
    pub label: SharedString,
}

impl Render for DragGhostView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(12.))
            .py(px(6.))
            .rounded(px(8.))
            .bg(cx.theme().primary)
            .text_color(cx.theme().primary_foreground)
            .text_size(px(13.))
            .font_weight(FontWeight::SEMIBOLD)
            .shadow_md()
            .opacity(0.9)
            .child(self.label.clone())
    }
}

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
            .w_full()
            .gap(px(8.))
            .child(if is_official {
                IconName::Bot
            } else {
                IconName::SquareTerminal
            })
            .child(
                div()
                    .text_size(px(13.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(cx.theme().foreground)
                    .child(self.preset.name),
            )
    }

    fn matches(&self, query: &str) -> bool {
        let q = query.to_lowercase();
        self.preset.name.to_lowercase().contains(&q)
            || self.preset.base_url.to_lowercase().contains(&q)
            || self.preset.id.to_lowercase().contains(&q)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelSelectItem {
    pub name: String,
}

impl SelectItem for ModelSelectItem {
    type Value = String;

    fn title(&self) -> SharedString {
        self.name.clone().into()
    }

    fn value(&self) -> &Self::Value {
        &self.name
    }

    fn render(&self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .text_size(px(12.))
            .text_color(cx.theme().foreground)
            .child(self.name.clone())
    }

    fn matches(&self, query: &str) -> bool {
        self.name.to_lowercase().contains(&query.to_lowercase())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReasoningOptionItem {
    pub label: String,
    pub value: Option<String>,
}

impl SelectItem for ReasoningOptionItem {
    type Value = Option<String>;

    fn title(&self) -> SharedString {
        self.label.clone().into()
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }

    fn render(&self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .text_size(px(12.))
            .text_color(cx.theme().foreground)
            .child(self.label.clone())
    }
}

pub struct CatalogRowDraft {
    pub display_name: Entity<InputState>,
    pub model: Entity<InputState>,
    pub context_window: Entity<InputState>,
    pub reasoning_effort: Entity<SelectState<Vec<ReasoningOptionItem>>>,
    pub model_select: Option<Entity<SelectState<Vec<ModelSelectItem>>>>,
    pub _model_select_sub: Option<Subscription>,
}

impl CatalogRowDraft {
    pub fn new(
        display_name_val: &str,
        model_val: &str,
        context_window_val: Option<u64>,
        reasoning_effort_val: Option<&str>,
        fetched_models: &[String],
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        let display_name = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("例如: DeepSeek V4 Flash")
                .default_value(display_name_val.to_string())
        });
        let model = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("例如: deepseek-v4-flash")
                .default_value(model_val.to_string())
        });
        let context_str = context_window_val
            .map(|n| n.to_string())
            .unwrap_or_default();
        let context_window = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("例如: 128000")
                .default_value(context_str)
        });

        let options = vec![
            ReasoningOptionItem {
                label: "未设置".into(),
                value: None,
            },
            ReasoningOptionItem {
                label: "low".into(),
                value: Some("low".into()),
            },
            ReasoningOptionItem {
                label: "medium".into(),
                value: Some("medium".into()),
            },
            ReasoningOptionItem {
                label: "high".into(),
                value: Some("high".into()),
            },
        ];
        let selected_idx = match reasoning_effort_val {
            Some("low") => Some(1),
            Some("medium") => Some(2),
            Some("high") => Some(3),
            _ => Some(0),
        };
        let reasoning_effort = cx.new(|cx| {
            let index_path = selected_idx.map(|i| gpui_component::IndexPath::default().row(i));
            SelectState::new(options, index_path, window, cx)
        });

        let (model_select, _model_select_sub) = if !fetched_models.is_empty() {
            let items: Vec<ModelSelectItem> = fetched_models
                .iter()
                .map(|m| ModelSelectItem { name: m.clone() })
                .collect();
            let selected_model_idx = fetched_models
                .iter()
                .position(|m| m == model_val)
                .map(|i| gpui_component::IndexPath::default().row(i));
            let select = cx.new(|cx| {
                SelectState::new(items, selected_model_idx, window, cx).searchable(true)
            });
            let model_state = model.clone();
            let display_name_state = display_name.clone();
            let sub = window.subscribe(
                &select,
                cx,
                move |_, event: &SelectEvent<Vec<ModelSelectItem>>, window, cx| {
                    if let SelectEvent::Confirm(Some(m)) = event {
                        let val = m.clone();
                        model_state.update(cx, |input, cx| input.set_value(val.clone(), window, cx));
                        display_name_state.update(cx, |input, cx| {
                            let curr = input.value().to_string();
                            if curr.trim().is_empty() {
                                input.set_value(val, window, cx);
                            }
                        });
                    }
                },
            );
            (Some(select), Some(sub))
        } else {
            (None, None)
        };

        Self {
            display_name,
            model,
            context_window,
            reasoning_effort,
            model_select,
            _model_select_sub,
        }
    }

    pub fn set_fetched_models(
        &mut self,
        fetched_models: &[String],
        window: &mut Window,
        cx: &mut App,
    ) {
        if fetched_models.is_empty() {
            self.model_select = None;
            self._model_select_sub = None;
            return;
        }

        let current_val = self.model.read(cx).value().to_string();
        let items: Vec<ModelSelectItem> = fetched_models
            .iter()
            .map(|m| ModelSelectItem { name: m.clone() })
            .collect();
        let selected_model_idx = fetched_models
            .iter()
            .position(|m| m == &current_val)
            .map(|i| gpui_component::IndexPath::default().row(i));
        let select = cx.new(|cx| {
            SelectState::new(items, selected_model_idx, window, cx).searchable(true)
        });
        let model_state = self.model.clone();
        let display_name_state = self.display_name.clone();
        let sub = window.subscribe(
            &select,
            cx,
            move |_, event: &SelectEvent<Vec<ModelSelectItem>>, window, cx| {
                if let SelectEvent::Confirm(Some(m)) = event {
                    let val = m.clone();
                    model_state.update(cx, |input, cx| input.set_value(val.clone(), window, cx));
                    display_name_state.update(cx, |input, cx| {
                        let curr = input.value().to_string();
                        if curr.trim().is_empty() {
                            input.set_value(val, window, cx);
                        }
                    });
                }
            },
        );
        self.model_select = Some(select);
        self._model_select_sub = Some(sub);
    }

    pub fn to_mapping(&self, cx: &App) -> Option<CodexModelMapping> {
        let model_val = self.model.read(cx).value().to_string();
        let model_trimmed = model_val.trim();
        if model_trimmed.is_empty() {
            return None;
        }
        let display_name_val = self.display_name.read(cx).value().to_string();
        let context_str = self.context_window.read(cx).value().to_string();
        let context_window = context_str.trim().parse::<u64>().ok();
        let reasoning_effort = self
            .reasoning_effort
            .read(cx)
            .selected_value()
            .cloned()
            .flatten();

        Some(CodexModelMapping {
            display_name: if display_name_val.trim().is_empty() {
                model_trimmed.to_string()
            } else {
                display_name_val.trim().to_string()
            },
            model: model_trimmed.to_string(),
            context_window,
            reasoning_effort,
        })
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsTab {
    #[default]
    General,
    Usage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UsageViewMode {
    #[default]
    Daily,
    Monthly,
    Projects,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UsageWindowChoice {
    Days7,
    #[default]
    Days30,
    Days90,
    Year1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UsageMetric {
    #[default]
    Cost,
    Tokens,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageWindowSelectItem {
    pub choice: UsageWindowChoice,
    pub label: String,
}

impl SelectItem for UsageWindowSelectItem {
    type Value = UsageWindowChoice;
    fn title(&self) -> SharedString {
        self.label.clone().into()
    }
    fn value(&self) -> &Self::Value {
        &self.choice
    }
}

pub struct RouterApp {
    workspace: Workspace,
    providers: Vec<Provider>,
    current_id: Option<String>,
    route: Route,
    previous_route: Route,
    sidebar_open: bool,
    theme: ThemePreference,
    language: AppLanguage,
    main_apps: Vec<String>,
    launch_on_startup: bool,
    minimize_to_tray: bool,
    settings_tab: SettingsTab,
    usage_view_mode: UsageViewMode,
    usage_window: UsageWindowChoice,
    usage_metric: UsageMetric,
    usage_window_select: Entity<SelectState<Vec<UsageWindowSelectItem>>>,
    _usage_window_sub: Option<Subscription>,
    search_input: Entity<InputState>,
    settings_search_input: Entity<InputState>,
    last_error: Option<SharedString>,
    form: Option<FormDraft>,
    logs: Vec<String>,
}

struct FormDraft {
    editing_id: Option<String>,
    kind: CodexKind,
    name: Entity<InputState>,
    api_key: Entity<InputState>,
    base_url: Entity<InputState>,
    model: Entity<InputState>,
    preset_select: Entity<SelectState<Vec<PresetSelectItem>>>,
    catalog_rows: Vec<CatalogRowDraft>,
    fetched_models: Vec<String>,
    has_fetched_models: bool,
    default_model_select: Option<Entity<SelectState<Vec<ModelSelectItem>>>>,
    is_fetching_models: bool,
    _preset_sub: Option<Subscription>,
    _default_model_sub: Option<Subscription>,
}

impl RouterApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let workspace = Workspace::open_default().expect("open workspace");
        let settings = workspace.settings().unwrap_or_default();
        crate::theme::apply_theme(settings.theme, Some(window), cx);

        let search_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("搜索供应商、模型或端点...")
        });

        let settings_search_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("搜索设置...")
        });

        let window_items = vec![
            UsageWindowSelectItem {
                choice: UsageWindowChoice::Days7,
                label: if settings.language == AppLanguage::En { "Last 7 Days".into() } else { "过去 7 天".into() },
            },
            UsageWindowSelectItem {
                choice: UsageWindowChoice::Days30,
                label: if settings.language == AppLanguage::En { "Last 30 Days".into() } else { "过去 30 天".into() },
            },
            UsageWindowSelectItem {
                choice: UsageWindowChoice::Days90,
                label: if settings.language == AppLanguage::En { "Last 90 Days".into() } else { "过去 90 天".into() },
            },
            UsageWindowSelectItem {
                choice: UsageWindowChoice::Year1,
                label: if settings.language == AppLanguage::En { "Last 1 Year".into() } else { "过去 1 年".into() },
            },
        ];
        let usage_window_select = cx.new(|cx| {
            SelectState::new(
                window_items,
                Some(gpui_component::IndexPath::default().row(1)),
                window,
                cx,
            )
        });

        let usage_window_sub = cx.subscribe(
            &usage_window_select,
            |this: &mut RouterApp,
             _emitter: Entity<SelectState<Vec<UsageWindowSelectItem>>>,
             event: &SelectEvent<Vec<UsageWindowSelectItem>>,
             cx: &mut Context<Self>| {
                if let SelectEvent::Confirm(Some(choice)) = event {
                    this.usage_window = *choice;
                    cx.notify();
                }
            },
        );

        let supported = ["codex", "claude", "claude-desktop", "grok"];
        let mut main_apps: Vec<String> = settings
            .main_apps
            .into_iter()
            .filter(|a| supported.contains(&a.as_str()))
            .collect();
        if main_apps.is_empty() {
            main_apps = vec![
                "codex".into(),
                "claude".into(),
                "claude-desktop".into(),
                "grok".into(),
            ];
        }

        let mut app = Self {
            workspace,
            providers: Vec::new(),
            current_id: None,
            route: Route::Dashboard,
            previous_route: Route::Dashboard,
            sidebar_open: true,
            theme: settings.theme,
            language: settings.language,
            main_apps,
            launch_on_startup: settings.launch_on_startup,
            minimize_to_tray: settings.minimize_to_tray,
            settings_tab: SettingsTab::General,
            usage_view_mode: UsageViewMode::Daily,
            usage_window: UsageWindowChoice::Days30,
            usage_metric: UsageMetric::Cost,
            usage_window_select,
            _usage_window_sub: Some(usage_window_sub),
            search_input,
            settings_search_input,
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
        if self.route != route {
            self.previous_route = self.route;
            self.form = None;
        }
        self.route = route;
        cx.notify();
    }

    fn set_language(&mut self, language: AppLanguage, window: &mut Window, cx: &mut Context<Self>) {
        self.language = language;
        if let Err(err) = self.workspace.set_language(language) {
            self.fail(err, window, cx);
            return;
        }
        let msg = match language {
            AppLanguage::ZhCn => "已切换界面语言为简体中文",
            AppLanguage::En => "Interface language set to English",
        };
        self.logs.push(msg.into());
        notify_success(msg, window, cx);
        cx.notify();
    }

    fn toggle_main_app(&mut self, app_id: &str, _window: &mut Window, cx: &mut Context<Self>) {
        match self.workspace.toggle_main_app(app_id) {
            Ok(is_enabled) => {
                if is_enabled {
                    if !self.main_apps.iter().any(|a| a == app_id) {
                        self.main_apps.push(app_id.to_string());
                    }
                } else {
                    self.main_apps.retain(|a| a != app_id);
                }
                cx.notify();
            }
            Err(err) => self.fail(err, _window, cx),
        }
    }

    fn move_main_app(
        &mut self,
        source_id: &str,
        target_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if source_id == target_id {
            return;
        }
        let Some(from_pos) = self.main_apps.iter().position(|id| id == source_id) else {
            return;
        };
        let Some(to_pos) = self.main_apps.iter().position(|id| id == target_id) else {
            return;
        };
        let item = self.main_apps.remove(from_pos);
        self.main_apps.insert(to_pos, item);
        if let Err(err) = self.workspace.reorder_main_apps(self.main_apps.clone()) {
            self.fail(err, window, cx);
            return;
        }
        cx.notify();
    }

    fn toggle_launch_on_startup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let new_val = !self.launch_on_startup;
        self.launch_on_startup = new_val;
        if let Err(err) = self.workspace.set_launch_on_startup(new_val) {
            self.fail(err, window, cx);
            return;
        }
        let msg = if new_val {
            "已开启开机自启"
        } else {
            "已关闭开机自启"
        };
        self.logs.push(msg.into());
        notify_success(msg, window, cx);
        cx.notify();
    }

    fn toggle_minimize_to_tray(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let new_val = !self.minimize_to_tray;
        self.minimize_to_tray = new_val;
        if let Err(err) = self.workspace.set_minimize_to_tray(new_val) {
            self.fail(err, window, cx);
            return;
        }
        let msg = if new_val {
            "已开启关闭时最小化到托盘"
        } else {
            "已关闭最小化到托盘"
        };
        self.logs.push(msg.into());
        notify_success(msg, window, cx);
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
        form.base_url
            .update(cx, |input, cx| input.set_value(preset.base_url, window, cx));
        form.model
            .update(cx, |input, cx| input.set_value(preset.model, window, cx));
        cx.notify();
    }

    fn fetch_models_for_form(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(form) = self.form.as_mut() else {
            return;
        };
        let base_url = form.base_url.read(cx).value().to_string();
        let api_key = form.api_key.read(cx).value().to_string();

        if base_url.trim().is_empty() {
            window.push_notification(Notification::warning("请先填写 API 端点 (Base URL)"), cx);
            return;
        }

        form.is_fetching_models = true;
        cx.notify();

        window.push_notification(Notification::info("正在从供应商获取模型列表..."), cx);

        let view = cx.entity().downgrade();
        window
            .spawn(cx, move |cx: &mut gpui::AsyncWindowContext| {
                let mut cx = cx.clone();
                let base_url = base_url.clone();
                let api_key = api_key.clone();
                async move {
                    let result: Result<Vec<String>, String> = cx
                        .background_executor()
                        .spawn(async move { domain::fetch_models_from_api(&base_url, &api_key) })
                        .await;

                    let _ = cx.update(|window: &mut Window, cx: &mut App| {
                        let _ = view.update(cx, |this, cx| {
                            if let Some(form) = this.form.as_mut() {
                                form.is_fetching_models = false;
                            }
                            match result {
                                Ok(models) => {
                                    let count = models.len();
                                    this.logs.push(format!("获取模型列表成功，共 {} 个模型", count));
                                    window.push_notification(
                                        Notification::success(format!("获取成功！找到 {} 个可用模型", count)),
                                        cx,
                                    );

                                    if let Some(form) = this.form.as_mut() {
                                        form.fetched_models = models.clone();
                                        form.has_fetched_models = true;

                                        // Auto-set default model if empty or generic default
                                        let current_model = form.model.read(cx).value().to_string();
                                        if let Some(first) = models.first() {
                                            if current_model.trim().is_empty()
                                                || current_model == domain::DEFAULT_CODEX_MODEL
                                            {
                                                let first = first.clone();
                                                form.model.update(cx, |input: &mut InputState, cx| {
                                                    input.set_value(first, window, cx);
                                                });
                                            }
                                        }

                                        // Setup default model dropdown selector
                                        let items: Vec<ModelSelectItem> = models
                                            .iter()
                                            .map(|m| ModelSelectItem { name: m.clone() })
                                            .collect();
                                        let updated_model_val = form.model.read(cx).value().to_string();
                                        let selected_idx = models
                                            .iter()
                                            .position(|m| m == &updated_model_val)
                                            .map(|i| gpui_component::IndexPath::default().row(i));
                                        let default_select = cx.new(|cx| {
                                            SelectState::new(items, selected_idx, window, cx).searchable(true)
                                        });
                                        let form_model_state = form.model.clone();
                                        let default_sub = window.subscribe(
                                            &default_select,
                                            cx,
                                            move |_, event: &SelectEvent<Vec<ModelSelectItem>>, window, cx| {
                                                if let SelectEvent::Confirm(Some(m)) = event {
                                                    let val = m.clone();
                                                    form_model_state.update(cx, |input, cx| {
                                                        input.set_value(val, window, cx);
                                                    });
                                                }
                                            },
                                        );
                                        form.default_model_select = Some(default_select);
                                        form._default_model_sub = Some(default_sub);

                                        // Update existing catalog rows
                                        for row in &mut form.catalog_rows {
                                            row.set_fetched_models(&models, window, cx);
                                        }
                                    }
                                }
                                Err(err) => {
                                    this.logs.push(format!("获取模型列表失败: {}", err));
                                    window.push_notification(Notification::error(err), cx);
                                }
                            }
                            cx.notify();
                        });
                    });
                }
            })
            .detach();
    }

    fn add_catalog_row(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(form) = self.form.as_mut() {
            let fetched = form.fetched_models.clone();
            form.catalog_rows.push(CatalogRowDraft::new(
                "",
                "",
                Some(128_000),
                None,
                &fetched,
                window,
                cx,
            ));
            cx.notify();
        }
    }

    fn remove_catalog_row(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(form) = self.form.as_mut() {
            if index < form.catalog_rows.len() {
                form.catalog_rows.remove(index);
                cx.notify();
            }
        }
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

    fn draggable_nav_item(
        &self,
        app_id: &'static str,
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
        let app_id_str = app_id.to_string();
        let target_app_id = app_id.to_string();

        div()
            .id(SharedString::from(format!("nav-app-{}", app_id)))
            .h(px(38.))
            .w_full()
            .px(px(10.))
            .rounded(px(8.))
            .flex()
            .items_center()
            .gap(px(10.))
            .text_size(px(14.))
            .text_color(if disabled { fg.opacity(0.4) } else { fg })
            .cursor_pointer()
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
            .on_drag(DragAppId(app_id_str), {
                let ghost_label = SharedString::from(label);
                move |_, _, _, cx| {
                    let label = ghost_label.clone();
                    cx.new(|_| DragGhostView { label })
                }
            })
            .on_drop(cx.listener(move |this, dragged: &DragAppId, window, cx| {
                this.move_main_app(&dragged.0, &target_app_id, window, cx);
            }))
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let count = self.providers.len();
        let border = cx.theme().sidebar_border;
        let is_en = self.language == AppLanguage::En;

        let mut app_nav_items = Vec::new();
        for app_id in &self.main_apps {
            let item = match app_id.as_str() {
                "amp" => Some(self.draggable_nav_item(
                    "amp",
                    CustomIcon::Amp,
                    Some(rgb(0xEA580C).into()),
                    "Amp",
                    Route::Codex,
                    Some(if is_en { "Soon".to_string() } else { "即将支持".to_string() }),
                    true,
                    cx,
                )),
                "claude" => Some(self.draggable_nav_item(
                    "claude",
                    CustomIcon::Claude,
                    Some(rgb(0xD97757).into()),
                    "Claude Code",
                    Route::Claude,
                    Some(if is_en { "Soon".to_string() } else { "即将支持".to_string() }),
                    true,
                    cx,
                )),
                "claude-desktop" => Some(self.draggable_nav_item(
                    "claude-desktop",
                    CustomIcon::Claude,
                    Some(rgb(0xD97757).into()),
                    "Claude Desktop",
                    Route::Claude,
                    Some(if is_en { "Soon".to_string() } else { "即将支持".to_string() }),
                    true,
                    cx,
                )),
                "codex" => Some(self.draggable_nav_item(
                    "codex",
                    CustomIcon::OpenAI,
                    Some(rgb(0x10A37F).into()),
                    "Codex",
                    Route::Codex,
                    Some(format!("{count}")),
                    false,
                    cx,
                )),
                "cursor" => Some(self.draggable_nav_item(
                    "cursor",
                    CustomIcon::Cursor,
                    Some(rgb(0x6366F1).into()),
                    "Cursor CLI",
                    Route::Codex,
                    Some(if is_en { "Soon".to_string() } else { "即将支持".to_string() }),
                    true,
                    cx,
                )),
                "deepseek" | "gemini" => Some(self.draggable_nav_item(
                    "deepseek",
                    CustomIcon::DeepSeek,
                    Some(rgb(0x3B82F6).into()),
                    "DeepSeek Harness",
                    Route::Codex,
                    Some(if is_en { "Soon".to_string() } else { "即将支持".to_string() }),
                    true,
                    cx,
                )),
                "fx" | "hermes" => Some(self.draggable_nav_item(
                    "fx",
                    CustomIcon::Fx,
                    Some(rgb(0x4B5563).into()),
                    "Fx",
                    Route::Codex,
                    Some(if is_en { "Soon".to_string() } else { "即将支持".to_string() }),
                    true,
                    cx,
                )),
                "opencode" => Some(self.draggable_nav_item(
                    "opencode",
                    CustomIcon::OpenCode,
                    Some(rgb(0x0284C7).into()),
                    "OpenCode",
                    Route::Codex,
                    Some(if is_en { "Soon".to_string() } else { "即将支持".to_string() }),
                    true,
                    cx,
                )),
                "grok" => Some(self.draggable_nav_item(
                    "grok",
                    CustomIcon::Grok,
                    Some(rgb(0x8B5CF6).into()),
                    "Grok Build",
                    Route::Grok,
                    Some(if is_en { "Soon".to_string() } else { "即将支持".to_string() }),
                    true,
                    cx,
                )),
                "kimi" => Some(self.draggable_nav_item(
                    "kimi",
                    CustomIcon::Kimi,
                    Some(rgb(0x2563EB).into()),
                    "Kimi Code",
                    Route::Codex,
                    Some(if is_en { "Soon".to_string() } else { "即将支持".to_string() }),
                    true,
                    cx,
                )),
                "ohmypi" | "openclaw" => Some(self.draggable_nav_item(
                    "ohmypi",
                    CustomIcon::OhMyPi,
                    Some(rgb(0xEC4899).into()),
                    "Oh My Pi",
                    Route::Codex,
                    Some(if is_en { "Soon".to_string() } else { "即将支持".to_string() }),
                    true,
                    cx,
                )),
                "pi" => Some(self.draggable_nav_item(
                    "pi",
                    CustomIcon::Pi,
                    Some(rgb(0x3B82F6).into()),
                    "Pi",
                    Route::Codex,
                    Some(if is_en { "Soon".to_string() } else { "即将支持".to_string() }),
                    true,
                    cx,
                )),
                _ => None,
            };
            if let Some(i) = item {
                app_nav_items.push(i);
            }
        }

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
                if is_en { "Dashboard" } else { "仪表盘" },
                Route::Dashboard,
                None,
                false,
                cx,
            ))
            .children(app_nav_items)
            .child(div().flex_1())
            .child(self.nav_item(
                "nav-notifications",
                IconName::Bell,
                Some(rgb(0xF59E0B).into()), // Amber
                if is_en { "Notifications" } else { "系统通知" },
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
                if is_en { "Settings" } else { "偏好设置" },
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

    fn render_switch(&self, checked: bool, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let bg_color = if checked {
            theme.primary
        } else {
            theme.border
        };

        div()
            .w(px(38.))
            .h(px(22.))
            .rounded(px(11.))
            .bg(bg_color)
            .p(px(2.))
            .flex()
            .items_center()
            .child(
                div()
                    .size(px(18.))
                    .rounded(px(9.))
                    .bg(rgb(0xFFFFFF))
                    .shadow_xs()
                    .when(checked, |this| this.ml(px(16.)))
                    .when(!checked, |this| this.ml(px(0.))),
            )
    }

    fn render_settings_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let is_en = self.language == AppLanguage::En;
        let border = cx.theme().sidebar_border;
        let accent = cx.theme().sidebar_accent;
        let query = self.settings_search_input.read(cx).value().to_string();
        let query = query.trim().to_lowercase();

        let general_matches = query.is_empty()
            || "通用 general 界面 语言 简体中文 english language 外观 主题 浅色 深色 跟随系统 theme light dark system 主页面 显示 claude codex gemini grok opencode openclaw hermes pi amp cursor deepseek fx kimi ohmypi 窗口行为 开机自启 startup 托盘 minimize tray"
                .contains(&query);

        let usage_matches = query.is_empty()
            || "用量 usage 统计 账单 消费 费用 成本 消耗 token tokens 日视图 月账单 每日 每月 项目 日期 范围 筛选 cost daily monthly projects window claude codex openai"
                .contains(&query);

        v_flex()
            .w(px(210.))
            .h_full()
            .flex_shrink_0()
            .p(px(8.))
            .pt(px(48.))
            .gap(px(6.))
            .child(
                // Waku style Back button
                div()
                    .id("settings-back")
                    .h(px(34.))
                    .px(px(9.))
                    .rounded(px(8.))
                    .flex()
                    .items_center()
                    .gap(px(9.))
                    .cursor_pointer()
                    .text_size(px(13.))
                    .text_color(cx.theme().muted_foreground)
                    .hover(move |element| element.bg(accent))
                    .child(Icon::new(CustomIcon::ArrowLeft).size(px(15.)).text_color(cx.theme().muted_foreground))
                    .child(if is_en { "Back" } else { "返回" })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.route = this.previous_route;
                        cx.notify();
                    })),
            )
            .child(
                Input::new(&self.settings_search_input)
                    .small()
                    .cleanable(true),
            )
            .child(
                div()
                    .h(px(1.))
                    .mx(px(4.))
                    .my(px(2.))
                    .bg(border),
            )
            .child(
                v_flex()
                    .w_full()
                    .gap(px(3.))
                    .when(general_matches, |this| {
                        this.child(self.render_settings_sidebar_item(
                            SettingsTab::General,
                            IconName::Settings,
                            if is_en { "General" } else { "通用" },
                            cx,
                        ))
                    })
                    .when(usage_matches, |this| {
                        this.child(self.render_settings_sidebar_item(
                            SettingsTab::Usage,
                            IconName::ChartPie,
                            if is_en { "Usage" } else { "用量" },
                            cx,
                        ))
                    })
                    .when(!general_matches && !usage_matches, |this| {
                        this.child(
                            div()
                                .px(px(10.))
                                .py(px(16.))
                                .text_size(px(12.))
                                .text_color(cx.theme().muted_foreground)
                                .child(if is_en { "No matching settings" } else { "未找到匹配设置" }),
                        )
                    }),
            )
    }

    fn render_settings_sidebar_item(
        &self,
        tab: SettingsTab,
        icon: impl Into<Icon>,
        label: &'static str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let active = self.settings_tab == tab;
        let accent = cx.theme().sidebar_accent;
        let fg = cx.theme().sidebar_foreground;

        div()
            .id(SharedString::from(format!("settings-tab-{:?}", tab)))
            .h(px(36.))
            .w_full()
            .px(px(10.))
            .rounded(px(8.))
            .flex()
            .items_center()
            .gap(px(8.))
            .text_size(px(13.))
            .text_color(fg)
            .cursor_pointer()
            .when(active, |this| this.bg(accent).font_weight(FontWeight::SEMIBOLD))
            .when(!active, |this| this.hover(|this| this.bg(accent.opacity(0.5))))
            .child(
                Icon::new(icon)
                    .size(px(16.))
                    .flex_shrink_0()
                    .text_color(if active { cx.theme().foreground } else { fg.opacity(0.7) }),
            )
            .child(div().flex_1().truncate().child(label))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.settings_tab = tab;
                cx.notify();
            }))
    }

    fn render_app_toggle_chip(
        &self,
        id: &'static str,
        label: &'static str,
        icon: impl Into<Icon>,
        icon_color: Hsla,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_active = self.main_apps.iter().any(|a| a == id);
        let theme = cx.theme();

        div()
            .id(SharedString::from(format!("app-chip-{}", id)))
            .h(px(32.))
            .px(px(12.))
            .rounded(px(16.))
            .flex()
            .items_center()
            .gap(px(7.))
            .cursor_pointer()
            .text_size(px(12.))
            .font_weight(FontWeight::MEDIUM)
            .when(is_active, |this| {
                this.bg(rgb(0x2563EB))
                    .text_color(rgb(0xFFFFFF))
                    .shadow_xs()
            })
            .when(!is_active, |this| {
                this.bg(theme.secondary.opacity(0.5))
                    .text_color(theme.muted_foreground)
                    .border_1()
                    .border_color(theme.border)
                    .hover(|this| this.bg(theme.secondary))
            })
            .child(
                Icon::new(icon)
                    .size(px(14.))
                    .text_color(if is_active { rgb(0xFFFFFF).into() } else { icon_color }),
            )
            .child(label)
            .on_click(cx.listener(move |this, _, window, cx| {
                this.toggle_main_app(id, window, cx);
            }))
    }

    fn render_general_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let is_en = self.language == AppLanguage::En;
        let theme = cx.theme().clone();
        let query = self.settings_search_input.read(cx).value().to_string();
        let query = query.trim().to_lowercase();

        let lang_match = query.is_empty()
            || "界面语言 interface language 简体中文 英文 english 语言".contains(&query);
        let theme_match = query.is_empty()
            || "外观主题 appearance theme 浅色 深色 跟随系统 light dark system 主题".contains(&query);
        let apps_match = query.is_empty()
            || "主页面显示 main page apps claude codex grok 侧边栏 导航".contains(&query);
        let window_match = query.is_empty()
            || "窗口行为 window behavior 开机自启 startup 关闭时最小化到托盘 minimize tray 托盘".contains(&query);

        let none_match = !lang_match && !theme_match && !apps_match && !window_match;

        v_flex()
            .w_full()
            .gap(px(16.))
            .child(
                div()
                    .text_size(px(24.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.foreground)
                    .child(if is_en { "General" } else { "通用" }),
            )
            .when(none_match, |this| {
                this.child(
                    theme::tile(cx).child(
                        v_flex()
                            .w_full()
                            .items_center()
                            .justify_center()
                            .py(px(32.))
                            .gap(px(8.))
                            .child(Icon::new(IconName::Search).size(px(20.)).text_color(theme.muted_foreground))
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .text_color(theme.muted_foreground)
                                    .child(if is_en { "No matching general settings found" } else { "未找到相关通用设置项" }),
                            ),
                    ),
                )
            })
            // 1. 界面语言
            .when(lang_match, |this| {
                this.child(
                    theme::tile(cx).child(
                        v_flex()
                            .w_full()
                            .gap(px(10.))
                            .child(
                                v_flex()
                                    .gap(px(2.))
                                    .child(theme::tile_label(if is_en { "INTERFACE LANGUAGE / 界面语言" } else { "界面语言" }, cx))
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(theme.muted_foreground)
                                            .child(if is_en {
                                                "Switch and preview interface language immediately."
                                            } else {
                                                "切换后立即预览界面语言，保存后永久生效。"
                                            }),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .gap(px(8.))
                                    .child(
                                        Button::new("lang-zh")
                                            .outline()
                                            .small()
                                            .selected(self.language == AppLanguage::ZhCn)
                                            .label("简体中文")
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.set_language(AppLanguage::ZhCn, window, cx);
                                            })),
                                    )
                                    .child(
                                        Button::new("lang-en")
                                            .outline()
                                            .small()
                                            .selected(self.language == AppLanguage::En)
                                            .label("English")
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.set_language(AppLanguage::En, window, cx);
                                            })),
                                    ),
                            ),
                    ),
                )
            })
            // 2. 外观主题
            .when(theme_match, |this| {
                this.child(
                    theme::tile(cx).child(
                        v_flex()
                            .w_full()
                            .gap(px(10.))
                            .child(
                                v_flex()
                                    .gap(px(2.))
                                    .child(theme::tile_label(if is_en { "APPEARANCE THEME / 外观主题" } else { "外观主题" }, cx))
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(theme.muted_foreground)
                                            .child(if is_en {
                                                "Select application appearance theme, takes effect immediately."
                                            } else {
                                                "选择应用的外观主题，立即生效。"
                                            }),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .gap(px(8.))
                                    .child(
                                        Button::new("theme-lt")
                                            .outline()
                                            .small()
                                            .icon(IconName::Sun)
                                            .selected(self.theme == ThemePreference::Light)
                                            .label(if is_en { "Light" } else { "浅色" })
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.set_theme_preference(ThemePreference::Light, window, cx);
                                            })),
                                    )
                                    .child(
                                        Button::new("theme-dk")
                                            .outline()
                                            .small()
                                            .icon(IconName::Moon)
                                            .selected(self.theme == ThemePreference::Dark)
                                            .label(if is_en { "Dark" } else { "深色" })
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.set_theme_preference(ThemePreference::Dark, window, cx);
                                            })),
                                    )
                                    .child(
                                        Button::new("theme-sys")
                                            .outline()
                                            .small()
                                            .icon(IconName::Palette)
                                            .selected(self.theme == ThemePreference::System)
                                            .label(if is_en { "Follow System" } else { "跟随系统" })
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.set_theme_preference(ThemePreference::System, window, cx);
                                            })),
                                    ),
                            ),
                    ),
                )
            })
            // 3. 主页面显示
            .when(apps_match, |this| {
                this.child(
                    theme::tile(cx).child(
                        v_flex()
                            .w_full()
                            .gap(px(10.))
                            .child(
                                v_flex()
                                    .gap(px(2.))
                                    .child(theme::tile_label(if is_en { "MAIN PAGE APPS / 主页面显示" } else { "主页面显示" }, cx))
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(theme.muted_foreground)
                                            .child(if is_en {
                                                "Select applications to show on the main navigation sidebar."
                                            } else {
                                                "选择在主页面侧边栏显示的应用。"
                                            }),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .w_full()
                                    .flex_wrap()
                                    .gap(px(8.))
                                    .child(self.render_app_toggle_chip("codex", "Codex", CustomIcon::OpenAI, rgb(0x10A37F).into(), cx))
                                    .child(self.render_app_toggle_chip("claude", "Claude Code", CustomIcon::Claude, rgb(0xD97757).into(), cx))
                                    .child(self.render_app_toggle_chip("claude-desktop", "Claude Desktop", CustomIcon::Claude, rgb(0xD97757).into(), cx))
                                    .child(self.render_app_toggle_chip("grok", "Grok Build", CustomIcon::Grok, rgb(0x8B5CF6).into(), cx)),
                            ),
                    ),
                )
            })
            // 4. 窗口行为
            .when(window_match, |this| {
                this.child(
                    theme::tile(cx).child(
                        v_flex()
                            .w_full()
                            .gap(px(14.))
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap(px(6.))
                                    .child(Icon::new(IconName::LayoutDashboard).size(px(15.)))
                                    .child(theme::tile_label(if is_en { "WINDOW BEHAVIOR / 窗口行为" } else { "窗口行为" }, cx)),
                            )
                            .child(
                                // 开机自启
                                h_flex()
                                    .w_full()
                                    .items_center()
                                    .justify_between()
                                    .p(px(8.))
                                    .rounded(px(8.))
                                    .bg(theme.secondary.opacity(0.4))
                                    .child(
                                        h_flex()
                                            .items_center()
                                            .gap(px(10.))
                                            .child(
                                                div()
                                                    .size(px(32.))
                                                    .rounded(px(8.))
                                                    .bg(theme.border)
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .child(Icon::new(IconName::Settings2).size(px(16.))),
                                            )
                                            .child(
                                                v_flex()
                                                    .gap(px(2.))
                                                    .child(
                                                        div()
                                                            .text_size(px(13.))
                                                            .font_weight(FontWeight::MEDIUM)
                                                            .text_color(theme.foreground)
                                                            .child(if is_en { "Launch on Startup" } else { "开机自启" }),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_size(px(11.))
                                                            .text_color(theme.muted_foreground)
                                                            .child(if is_en {
                                                                "Run Router Switch automatically on system startup"
                                                            } else {
                                                                "随系统启动自动运行 Router Switch"
                                                            }),
                                                    ),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .id("switch-launch-on-startup")
                                            .cursor_pointer()
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.toggle_launch_on_startup(window, cx);
                                            }))
                                            .child(self.render_switch(self.launch_on_startup, cx)),
                                    ),
                            )
                            .child(
                                // 关闭时最小化到托盘
                                h_flex()
                                    .w_full()
                                    .items_center()
                                    .justify_between()
                                    .p(px(8.))
                                    .rounded(px(8.))
                                    .bg(theme.secondary.opacity(0.4))
                                    .child(
                                        h_flex()
                                            .items_center()
                                            .gap(px(10.))
                                            .child(
                                                div()
                                                    .size(px(32.))
                                                    .rounded(px(8.))
                                                    .bg(theme.border)
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .child(Icon::new(IconName::WindowMinimize).size(px(16.))),
                                            )
                                            .child(
                                                v_flex()
                                                    .gap(px(2.))
                                                    .child(
                                                        div()
                                                            .text_size(px(13.))
                                                            .font_weight(FontWeight::MEDIUM)
                                                            .text_color(theme.foreground)
                                                            .child(if is_en { "Minimize to Tray on Close" } else { "关闭时最小化到托盘" }),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_size(px(11.))
                                                            .text_color(theme.muted_foreground)
                                                            .child(if is_en {
                                                                "Hides to system tray when clicking close button instead of quitting."
                                                            } else {
                                                                "勾选后点击关闭按钮会隐藏到系统托盘，取消则直接退出应用。"
                                                            }),
                                                    ),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .id("switch-minimize-to-tray")
                                            .cursor_pointer()
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.toggle_minimize_to_tray(window, cx);
                                            }))
                                            .child(self.render_switch(self.minimize_to_tray, cx)),
                                    ),
                            ),
                    ),
                )
            })
    }

    fn render_usage_daily_table(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        v_flex()
            .w_full()
            .gap(px(6.))
            .child(
                h_flex()
                    .w_full()
                    .px(px(8.))
                    .py(px(4.))
                    .rounded(px(6.))
                    .bg(theme.secondary.opacity(0.5))
                    .text_size(px(11.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.muted_foreground)
                    .child(div().w(px(200.)).child("模型名称"))
                    .child(div().w(px(140.)).child("服务商"))
                    .child(div().w(px(90.)).child("请求次数"))
                    .child(div().w(px(110.)).child("输入 Token"))
                    .child(div().w(px(110.)).child("输出 Token"))
                    .child(div().flex_1().child("预估费用")),
            )
            .child(
                v_flex()
                    .w_full()
                    .gap(px(2.))
                    .child(
                        h_flex()
                            .w_full()
                            .px(px(8.))
                            .py(px(6.))
                            .items_center()
                            .text_size(px(12.))
                            .child(div().w(px(200.)).font_weight(FontWeight::MEDIUM).child("gpt-4o"))
                            .child(div().w(px(140.)).text_color(rgb(0x10A37F)).child("Codex (OpenAI)"))
                            .child(div().w(px(90.)).child("642"))
                            .child(div().w(px(110.)).child("1.85 M"))
                            .child(div().w(px(110.)).child("0.92 M"))
                            .child(div().flex_1().font_weight(FontWeight::SEMIBOLD).child("$9.25")),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .px(px(8.))
                            .py(px(6.))
                            .items_center()
                            .text_size(px(12.))
                            .child(div().w(px(200.)).font_weight(FontWeight::MEDIUM).child("claude-3-7-sonnet"))
                            .child(div().w(px(140.)).text_color(rgb(0xD97757)).child("Claude Code"))
                            .child(div().w(px(90.)).child("418"))
                            .child(div().w(px(110.)).child("1.10 M"))
                            .child(div().w(px(110.)).child("0.54 M"))
                            .child(div().flex_1().font_weight(FontWeight::SEMIBOLD).child("$6.16")),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .px(px(8.))
                            .py(px(6.))
                            .items_center()
                            .text_size(px(12.))
                            .child(div().w(px(200.)).font_weight(FontWeight::MEDIUM).child("o3-mini"))
                            .child(div().w(px(140.)).text_color(rgb(0x10A37F)).child("Codex (OpenAI)"))
                            .child(div().w(px(90.)).child("224"))
                            .child(div().w(px(110.)).child("0.29 M"))
                            .child(div().w(px(110.)).child("0.12 M"))
                            .child(div().flex_1().font_weight(FontWeight::SEMIBOLD).child("$3.05")),
                    ),
            )
    }

    fn render_usage_monthly_table(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        v_flex()
            .w_full()
            .gap(px(6.))
            .child(
                h_flex()
                    .w_full()
                    .px(px(8.))
                    .py(px(4.))
                    .rounded(px(6.))
                    .bg(theme.secondary.opacity(0.5))
                    .text_size(px(11.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.muted_foreground)
                    .child(div().w(px(140.)).child("月份"))
                    .child(div().w(px(110.)).child("请求次数"))
                    .child(div().w(px(130.)).child("总 Token"))
                    .child(div().w(px(130.)).child("缓存命中率"))
                    .child(div().w(px(120.)).child("月度费用"))
                    .child(div().flex_1().child("消耗占比")),
            )
            .child(
                v_flex()
                    .w_full()
                    .gap(px(2.))
                    .child(
                        h_flex()
                            .w_full()
                            .px(px(8.))
                            .py(px(6.))
                            .items_center()
                            .text_size(px(12.))
                            .child(div().w(px(140.)).font_weight(FontWeight::MEDIUM).child("2026 年 8 月"))
                            .child(div().w(px(110.)).child("720 次"))
                            .child(div().w(px(130.)).child("2.80 M"))
                            .child(div().w(px(130.)).text_color(rgb(0x10B981)).child("36.2%"))
                            .child(div().w(px(120.)).font_weight(FontWeight::SEMIBOLD).child("$10.82"))
                            .child(div().flex_1().child("58.6%")),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .px(px(8.))
                            .py(px(6.))
                            .items_center()
                            .text_size(px(12.))
                            .child(div().w(px(140.)).font_weight(FontWeight::MEDIUM).child("2026 年 7 月"))
                            .child(div().w(px(110.)).child("564 次"))
                            .child(div().w(px(130.)).child("2.02 M"))
                            .child(div().w(px(130.)).text_color(rgb(0x10B981)).child("31.8%"))
                            .child(div().w(px(120.)).font_weight(FontWeight::SEMIBOLD).child("$7.64"))
                            .child(div().flex_1().child("41.4%")),
                    ),
            )
    }

    fn render_usage_projects_table(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        v_flex()
            .w_full()
            .gap(px(6.))
            .child(
                h_flex()
                    .w_full()
                    .px(px(8.))
                    .py(px(4.))
                    .rounded(px(6.))
                    .bg(theme.secondary.opacity(0.5))
                    .text_size(px(11.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.muted_foreground)
                    .child(div().w(px(260.)).child("项目根目录"))
                    .child(div().w(px(120.)).child("主要服务商"))
                    .child(div().w(px(90.)).child("请求次数"))
                    .child(div().w(px(120.)).child("消耗 Token"))
                    .child(div().flex_1().child("预估费用")),
            )
            .child(
                v_flex()
                    .w_full()
                    .gap(px(2.))
                    .child(
                        h_flex()
                            .w_full()
                            .px(px(8.))
                            .py(px(6.))
                            .items_center()
                            .text_size(px(12.))
                            .child(div().w(px(260.)).font_weight(FontWeight::MEDIUM).truncate().child("~/Desktop/git/router-switch"))
                            .child(div().w(px(120.)).text_color(rgb(0x10A37F)).child("Codex"))
                            .child(div().w(px(90.)).child("712"))
                            .child(div().w(px(120.)).child("2.65 M"))
                            .child(div().flex_1().font_weight(FontWeight::SEMIBOLD).child("$10.15")),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .px(px(8.))
                            .py(px(6.))
                            .items_center()
                            .text_size(px(12.))
                            .child(div().w(px(260.)).font_weight(FontWeight::MEDIUM).truncate().child("~/Desktop/git/waku"))
                            .child(div().w(px(120.)).text_color(rgb(0xD97757)).child("Claude Code"))
                            .child(div().w(px(90.)).child("386"))
                            .child(div().w(px(120.)).child("1.48 M"))
                            .child(div().flex_1().font_weight(FontWeight::SEMIBOLD).child("$5.62")),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .px(px(8.))
                            .py(px(6.))
                            .items_center()
                            .text_size(px(12.))
                            .child(div().w(px(260.)).font_weight(FontWeight::MEDIUM).truncate().child("~/Desktop/git/ai-api"))
                            .child(div().w(px(120.)).text_color(rgb(0x10A37F)).child("Codex"))
                            .child(div().w(px(90.)).child("186"))
                            .child(div().w(px(120.)).child("0.69 M"))
                            .child(div().flex_1().font_weight(FontWeight::SEMIBOLD).child("$2.69")),
                    ),
            )
    }

    fn render_usage_page(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let is_en = self.language == AppLanguage::En;
        let theme = cx.theme().clone();

        let total_cost = 18.46;
        let codex_cost = 12.30;
        let claude_cost = 6.16;

        let codex_share = codex_cost / total_cost;
        let claude_share = claude_cost / total_cost;

        let range_label = match self.usage_window {
            UsageWindowChoice::Days7 => if is_en { "2026-08-17 ~ 2026-08-23 (Last 7 Days)" } else { "2026-08-17 ~ 2026-08-23 (近 7 天)" },
            UsageWindowChoice::Days30 => if is_en { "2026-07-24 ~ 2026-08-23 (Last 30 Days)" } else { "2026-07-24 ~ 2026-08-23 (近 30 天)" },
            UsageWindowChoice::Days90 => if is_en { "2026-05-25 ~ 2026-08-23 (Last 90 Days)" } else { "2026-05-25 ~ 2026-08-23 (近 90 天)" },
            UsageWindowChoice::Year1 => if is_en { "2025-08-24 ~ 2026-08-23 (Last 1 Year)" } else { "2025-08-24 ~ 2026-08-23 (近 1 年)" },
        };

        v_flex()
            .w_full()
            .gap(px(16.))
            // Header Bar
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .child(
                        v_flex()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_size(px(24.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme.foreground)
                                    .child(if is_en { "Usage" } else { "用量" }),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(theme.muted_foreground)
                                    .child(range_label),
                            ),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap(px(8.))
                            // View Switcher (Daily / Monthly / Projects)
                            .child(
                                h_flex()
                                    .p(px(2.))
                                    .rounded(px(8.))
                                    .bg(theme.secondary.opacity(0.5))
                                    .border_1()
                                    .border_color(theme.border)
                                    .gap(px(2.))
                                    .child(
                                        Button::new("usage-view-daily")
                                            .ghost()
                                            .xsmall()
                                            .selected(self.usage_view_mode == UsageViewMode::Daily)
                                            .label(if is_en { "Daily" } else { "每日" })
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.usage_view_mode = UsageViewMode::Daily;
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Button::new("usage-view-monthly")
                                            .ghost()
                                            .xsmall()
                                            .selected(self.usage_view_mode == UsageViewMode::Monthly)
                                            .label(if is_en { "Monthly" } else { "每月" })
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.usage_view_mode = UsageViewMode::Monthly;
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Button::new("usage-view-projects")
                                            .ghost()
                                            .xsmall()
                                            .selected(self.usage_view_mode == UsageViewMode::Projects)
                                            .label(if is_en { "Projects" } else { "项目" })
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.usage_view_mode = UsageViewMode::Projects;
                                                cx.notify();
                                            })),
                                    ),
                            )
                            // Window Selector Dropdown (when not Monthly)
                            .when(self.usage_view_mode != UsageViewMode::Monthly, |this| {
                                this.child(
                                    div()
                                        .w(px(130.))
                                        .child(Select::new(&self.usage_window_select).small())
                                )
                            })
                            // Refresh
                            .child(
                                Button::new("usage-refresh-btn")
                                    .outline()
                                    .small()
                                    .icon(CustomIcon::RotateCw)
                                    .tooltip(if is_en { "Refresh usage stats" } else { "刷新用量统计" })
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        let msg = if this.language == AppLanguage::En {
                                            "Usage stats refreshed"
                                        } else {
                                            "用量数据已刷新"
                                        };
                                        notify_success(msg, window, cx);
                                    })),
                            )
                            // Metric Switcher (Cost / Tokens)
                            .child(
                                h_flex()
                                    .p(px(2.))
                                    .rounded(px(8.))
                                    .bg(theme.secondary.opacity(0.5))
                                    .border_1()
                                    .border_color(theme.border)
                                    .gap(px(2.))
                                    .child(
                                        Button::new("metric-cost")
                                            .ghost()
                                            .xsmall()
                                            .selected(self.usage_metric == UsageMetric::Cost)
                                            .label(if is_en { "Cost" } else { "费用" })
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.usage_metric = UsageMetric::Cost;
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Button::new("metric-tokens")
                                            .ghost()
                                            .xsmall()
                                            .selected(self.usage_metric == UsageMetric::Tokens)
                                            .label(if is_en { "Tokens" } else { "令牌" })
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.usage_metric = UsageMetric::Tokens;
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    ),
            )
            // Overview row (Headline + Provider share bars)
            .child(
                h_flex()
                    .w_full()
                    .gap(px(12.))
                    // Headline tile
                    .child(
                        theme::tile(cx)
                            .w(px(280.))
                            .child(
                                v_flex()
                                    .w_full()
                                    .gap(px(6.))
                                    .child(theme::tile_label(if self.usage_metric == UsageMetric::Cost {
                                        "ESTIMATED COST / 预估总费用"
                                    } else {
                                        "PROCESSED TOKENS / 处理总 TOKEN"
                                    }, cx))
                                    .child(
                                        div()
                                            .text_size(px(32.))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(theme.foreground)
                                            .child(if self.usage_metric == UsageMetric::Cost {
                                                format!("${:.2}", total_cost)
                                            } else {
                                                "4.82 M".to_string()
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(theme.muted_foreground)
                                            .child(if is_en {
                                                "Calculated based on official model rates and token usage."
                                            } else {
                                                "基于各服务商官方费率与实际输入/输出/缓存换算"
                                            }),
                                    ),
                            ),
                    )
                    // Provider share distribution tile
                    .child(
                        theme::tile(cx)
                            .flex_1()
                            .child(
                                v_flex()
                                    .w_full()
                                    .gap(px(10.))
                                    .child(theme::tile_label("PROVIDER SHARE / 服务商消耗占比", cx))
                                    .child(
                                        v_flex()
                                            .w_full()
                                            .gap(px(8.))
                                            // Codex bar
                                            .child(
                                                v_flex()
                                                    .w_full()
                                                    .gap(px(4.))
                                                    .child(
                                                        h_flex()
                                                            .w_full()
                                                            .justify_between()
                                                            .text_size(px(12.))
                                                            .child(
                                                                h_flex()
                                                                    .items_center()
                                                                    .gap(px(6.))
                                                                    .child(Icon::new(CustomIcon::OpenAI).size(px(13.)).text_color(rgb(0x10A37F)))
                                                                    .child(div().font_weight(FontWeight::MEDIUM).child("Codex (OpenAI)")),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_color(theme.muted_foreground)
                                                                    .child(format!("${:.2} ({:.1}%) • 3.21M tokens", codex_cost, codex_share * 100.)),
                                                            ),
                                                    )
                                                    .child(
                                                        div()
                                                            .h(px(6.))
                                                            .w_full()
                                                            .rounded_full()
                                                            .bg(theme.secondary)
                                                            .child(
                                                                div()
                                                                    .h_full()
                                                                    .w(gpui::relative(codex_share as f32))
                                                                    .rounded_full()
                                                                    .bg(rgb(0x10A37F)),
                                                            ),
                                                    ),
                                            )
                                            // Claude Code bar
                                            .child(
                                                v_flex()
                                                    .w_full()
                                                    .gap(px(4.))
                                                    .child(
                                                        h_flex()
                                                            .w_full()
                                                            .justify_between()
                                                            .text_size(px(12.))
                                                            .child(
                                                                h_flex()
                                                                    .items_center()
                                                                    .gap(px(6.))
                                                                    .child(Icon::new(CustomIcon::Claude).size(px(13.)).text_color(rgb(0xD97757)))
                                                                    .child(div().font_weight(FontWeight::MEDIUM).child("Claude Code")),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_color(theme.muted_foreground)
                                                                    .child(format!("${:.2} ({:.1}%) • 1.61M tokens", claude_cost, claude_share * 100.)),
                                                            ),
                                                    )
                                                    .child(
                                                        div()
                                                            .h(px(6.))
                                                            .w_full()
                                                            .rounded_full()
                                                            .bg(theme.secondary)
                                                            .child(
                                                                div()
                                                                    .h_full()
                                                                    .w(gpui::relative(claude_share as f32))
                                                                    .rounded_full()
                                                                    .bg(rgb(0xD97757)),
                                                            ),
                                                    ),
                                            ),
                                    ),
                            ),
                    ),
            )
            // 4-tile Metrics Strip
            .child(
                h_flex()
                    .w_full()
                    .gap(px(12.))
                    .child(
                        theme::tile(cx)
                            .flex_1()
                            .child(
                                v_flex()
                                    .gap(px(4.))
                                    .child(theme::tile_label("PROMPT TOKENS / 输入", cx))
                                    .child(div().text_size(px(20.)).font_weight(FontWeight::BOLD).child("3.24 M"))
                                    .child(div().text_size(px(11.)).text_color(theme.muted_foreground).child("占比 67.2%")),
                            ),
                    )
                    .child(
                        theme::tile(cx)
                            .flex_1()
                            .child(
                                v_flex()
                                    .gap(px(4.))
                                    .child(theme::tile_label("COMPLETION / 输出", cx))
                                    .child(div().text_size(px(20.)).font_weight(FontWeight::BOLD).child("1.58 M"))
                                    .child(div().text_size(px(11.)).text_color(theme.muted_foreground).child("占比 32.8%")),
                            ),
                    )
                    .child(
                        theme::tile(cx)
                            .flex_1()
                            .child(
                                v_flex()
                                    .gap(px(4.))
                                    .child(theme::tile_label("CACHE HITS / 缓存命中", cx))
                                    .child(div().text_size(px(20.)).font_weight(FontWeight::BOLD).child("1.12 M"))
                                    .child(div().text_size(px(11.)).text_color(rgb(0x10B981)).child("命中率 34.5% (省 $4.20)")),
                            ),
                    )
                    .child(
                        theme::tile(cx)
                            .flex_1()
                            .child(
                                v_flex()
                                    .gap(px(4.))
                                    .child(theme::tile_label("REQUESTS / 请求次数", cx))
                                    .child(div().text_size(px(20.)).font_weight(FontWeight::BOLD).child("1,284 次"))
                                    .child(div().text_size(px(11.)).text_color(theme.muted_foreground).child("活跃模型 3 个")),
                            ),
                    ),
            )
            // Visual Timeline / Bar Chart Tile
            .child(
                theme::tile(cx).child(
                    v_flex()
                        .w_full()
                        .gap(px(12.))
                        .child(
                            h_flex()
                                .w_full()
                                .justify_between()
                                .items_center()
                                .child(theme::tile_label("USAGE TIMELINE / 用量趋势分布", cx))
                                .child(
                                    h_flex()
                                        .items_center()
                                        .gap(px(12.))
                                        .child(
                                            h_flex()
                                                .items_center()
                                                .gap(px(4.))
                                                .child(div().size(px(8.)).rounded_xs().bg(rgb(0x10A37F)))
                                                .child(div().text_size(px(11.)).text_color(theme.muted_foreground).child("Codex")),
                                        )
                                        .child(
                                            h_flex()
                                                .items_center()
                                                .gap(px(4.))
                                                .child(div().size(px(8.)).rounded_xs().bg(rgb(0xD97757)))
                                                .child(div().text_size(px(11.)).text_color(theme.muted_foreground).child("Claude Code")),
                                        ),
                                ),
                        )
                        .child(
                            h_flex()
                                .h(px(110.))
                                .w_full()
                                .items_end()
                                .justify_between()
                                .gap(px(4.))
                                .px(px(6.))
                                .py(px(8.))
                                .rounded(px(8.))
                                .bg(theme.secondary.opacity(0.3))
                                .children((0..28).map(|i| {
                                    let h1 = ((i * 13 + 7) % 65 + 15) as f32;
                                    let h2 = ((i * 19 + 11) % 30 + 5) as f32;
                                    v_flex()
                                        .flex_1()
                                        .h_full()
                                        .items_center()
                                        .justify_end()
                                        .gap(px(1.))
                                        .child(
                                            div()
                                                .w_full()
                                                .max_w(px(16.))
                                                .h(px(h2))
                                                .rounded_t(px(2.))
                                                .bg(rgb(0xD97757)),
                                        )
                                        .child(
                                            div()
                                                .w_full()
                                                .max_w(px(16.))
                                                .h(px(h1))
                                                .rounded_t(px(2.))
                                                .bg(rgb(0x10A37F)),
                                        )
                                })),
                        ),
                ),
            )
            // Tabular Breakdown section
            .child(
                theme::tile(cx).child(
                    v_flex()
                        .w_full()
                        .gap(px(10.))
                        .child(theme::tile_label(match self.usage_view_mode {
                            UsageViewMode::Daily => "MODEL & PROVIDER BREAKDOWN / 模型与服务商细目",
                            UsageViewMode::Monthly => "MONTHLY STATEMENT / 月度账单明细",
                            UsageViewMode::Projects => "PROJECT WORKSPACE BREAKDOWN / 项目工作区细目",
                        }, cx))
                        .child(match self.usage_view_mode {
                            UsageViewMode::Daily => self.render_usage_daily_table(cx).into_any_element(),
                            UsageViewMode::Monthly => self.render_usage_monthly_table(cx).into_any_element(),
                            UsageViewMode::Projects => self.render_usage_projects_table(cx).into_any_element(),
                        }),
                ),
            )
    }

    fn render_settings_page(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(match self.settings_tab {
                SettingsTab::General => self.render_general_settings(cx).into_any_element(),
                SettingsTab::Usage => self.render_usage_page(cx).into_any_element(),
            })
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

    fn render_form_page(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(form) = self.form.as_ref() else {
            return div().into_any_element();
        };

        let is_editing = form.editing_id.is_some();
        let title = if is_editing {
            "编辑 Codex 供应商"
        } else {
            "新建 Codex 供应商"
        };
        let subtitle = if is_editing {
            "修改供应商的接口端点、模型名称与模型映射配置"
        } else {
            "从预设模版快速创建或手动填写第三方 Responses API 供应商"
        };
        let theme = cx.theme();

        v_flex()
            .w_full()
            .gap(px(16.))
            .child(
                // Header bar with breadcrumbs / back button and actions
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .child(
                        h_flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                Button::new("back-btn")
                                    .ghost()
                                    .small()
                                    .icon(IconName::ChevronLeft)
                                    .label("返回")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.form = None;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .text_color(theme.muted_foreground)
                                    .child("/"),
                            )
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.muted_foreground)
                                    .child("Codex 供应商"),
                            )
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .text_color(theme.muted_foreground)
                                    .child("/"),
                            )
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.foreground)
                                    .child(title),
                            ),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                Button::new("cancel-page-btn")
                                    .outline()
                                    .small()
                                    .label("取消")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.form = None;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("save-page-btn")
                                    .primary()
                                    .small()
                                    .icon(IconName::Check)
                                    .label("保存配置")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.submit_form(window, cx);
                                    })),
                            ),
                    ),
            )
            .child(
                v_flex()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(px(22.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.foreground)
                            .child(title),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(theme.muted_foreground)
                            .child(subtitle),
                    ),
            )
            .child(
                // Preset Template Selection Card
                theme::tile(cx).child(
                    v_flex()
                        .w_full()
                        .gap(px(10.))
                        .child(theme::tile_label("PRESET TEMPLATE / 快速选择预设模版", cx))
                        .child(
                            Select::new(&form.preset_select)
                                .placeholder("选择预设模版 (如 DeepSeek, Kimi, 阿里百炼, 自定义模板...)")
                                .search_placeholder("搜索预设模版...")
                                .cleanable(true),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(theme.muted_foreground)
                                .child("💡 提示：选择预设模版会自动为您填入官方端点及推荐模型。"),
                        ),
                ),
            )
            .child(
                // Basic & API Credentials Card
                theme::tile(cx).child(
                    v_flex()
                        .w_full()
                        .gap(px(12.))
                        .child(theme::tile_label("BASIC & API CREDENTIALS / 基础配置与接口凭证", cx))
                        .child(form_field("供应商名称", Input::new(&form.name)))
                        .child(form_field(
                            "API Key (OPENAI_API_KEY)",
                            Input::new(&form.api_key).mask_toggle(),
                        ))
                        .child(form_field("API 端点 (Base URL)", Input::new(&form.base_url)))
                        .child(
                            v_flex()
                                .gap(px(6.))
                                .child(
                                    div()
                                        .text_size(px(12.))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(theme.foreground)
                                        .child("模型名称 (Model)"),
                                )
                                .child(
                                    h_flex()
                                        .w_full()
                                        .gap(px(8.))
                                        .items_center()
                                        .child(
                                            div().flex_1().child(Input::new(&form.model).cleanable(true))
                                        )
                                        .when_some(form.default_model_select.as_ref(), |this, select| {
                                            this.child(
                                                div()
                                                    .w(px(240.))
                                                    .child(
                                                        Select::new(select)
                                                            .placeholder("选择模型...")
                                                            .search_placeholder("搜索模型..."),
                                                    ),
                                            )
                                        })
                                        .child(
                                            Button::new("fetch-models-btn")
                                                .outline()
                                                .icon(IconName::ArrowDown)
                                                .tooltip("从端点拉取可用模型列表")
                                                .disabled(form.is_fetching_models)
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.fetch_models_for_form(window, cx);
                                                })),
                                        ),
                                ),
                        ),
                ),
            )
            .child(
                // Model Mapping Card
                theme::tile(cx).child(
                    v_flex()
                        .w_full()
                        .gap(px(12.))
                        .child(
                            h_flex()
                                .w_full()
                                .items_center()
                                .justify_between()
                                .child(
                                    v_flex()
                                        .gap(px(2.))
                                        .child(theme::tile_label("MODEL MAPPING / 模型映射 (可选)", cx))
                                        .child(
                                            div()
                                                .text_size(px(12.))
                                                .text_color(theme.muted_foreground)
                                                .child("自定义在 Codex 客户端下拉菜单中展示的模型别名、映射到服务商的真实模型、上下文窗口大小及思考等级。"),
                                        ),
                                )
                                .child(
                                    if form.has_fetched_models || !form.catalog_rows.is_empty() {
                                        Button::new("add-mapping-btn")
                                            .primary()
                                            .small()
                                            .icon(IconName::Plus)
                                            .label("新增映射")
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.add_catalog_row(window, cx);
                                            }))
                                            .into_any_element()
                                    } else {
                                        Button::new("fetch-catalog-btn")
                                            .outline()
                                            .small()
                                            .icon(IconName::ArrowDown)
                                            .label("拉取模型")
                                            .disabled(form.is_fetching_models)
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.fetch_models_for_form(window, cx);
                                            }))
                                            .into_any_element()
                                    },
                                ),
                        )
                        .child(
                            if form.catalog_rows.is_empty() {
                                v_flex()
                                    .w_full()
                                    .items_center()
                                    .justify_center()
                                    .py(px(20.))
                                    .gap(px(6.))
                                    .rounded(px(8.))
                                    .border_1()
                                    .border_color(theme.border)
                                    .bg(theme.secondary.opacity(0.3))
                                    .child(
                                        div()
                                            .text_size(px(13.))
                                            .text_color(theme.muted_foreground)
                                            .child("暂无模型映射配置"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(theme.muted_foreground)
                                            .child(if form.has_fetched_models {
                                                "已成功获取可用模型列表，点击右上角「新增映射」添加映射规则"
                                            } else {
                                                "点击「拉取模型」获取供应商可用模型，或点击上方「新增映射」直接添加"
                                            }),
                                    )
                                    .into_any_element()
                            } else {
                                v_flex()
                                    .w_full()
                                    .gap(px(6.))
                                    .child(
                                        h_flex()
                                            .w_full()
                                            .items_center()
                                            .gap(px(10.))
                                            .px(px(8.))
                                            .py(px(6.))
                                            .rounded(px(6.))
                                            .bg(theme.secondary.opacity(0.6))
                                            .child(
                                                div()
                                                    .w(px(200.))
                                                    .text_size(px(12.))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(theme.muted_foreground)
                                                    .child("菜单显示名"),
                                            )
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .text_size(px(12.))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(theme.muted_foreground)
                                                    .child("实际请求模型"),
                                            )
                                            .child(
                                                div()
                                                    .w(px(120.))
                                                    .text_size(px(12.))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(theme.muted_foreground)
                                                    .child("上下文窗口"),
                                            )
                                            .child(
                                                div()
                                                    .w(px(130.))
                                                    .text_size(px(12.))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(theme.muted_foreground)
                                                    .child("思考等级"),
                                            )
                                            .child(
                                                div()
                                                    .w(px(36.))
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .text_size(px(12.))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(theme.muted_foreground)
                                                    .child("操作"),
                                            ),
                                    )
                                    .children(
                                        form.catalog_rows.iter().enumerate().map(|(idx, row)| {
                                            h_flex()
                                                .w_full()
                                                .items_center()
                                                .gap(px(10.))
                                                .px(px(4.))
                                                .py(px(3.))
                                                .child(
                                                    div()
                                                        .w(px(200.))
                                                        .child(Input::new(&row.display_name).small().cleanable(true)),
                                                )
                                                .child(
                                                    h_flex()
                                                        .flex_1()
                                                        .items_center()
                                                        .gap(px(6.))
                                                        .child(
                                                            div().flex_1().child(Input::new(&row.model).small().cleanable(true))
                                                        )
                                                        .when_some(row.model_select.as_ref(), |this, select| {
                                                            this.child(
                                                                div()
                                                                    .w(px(160.))
                                                                    .child(
                                                                        Select::new(select)
                                                                            .small()
                                                                            .placeholder("选择模型")
                                                                            .search_placeholder("搜索模型..."),
                                                                    ),
                                                            )
                                                        }),
                                                )
                                                .child(
                                                    div()
                                                        .w(px(120.))
                                                        .child(Input::new(&row.context_window).small()),
                                                )
                                                .child(
                                                    div()
                                                        .w(px(130.))
                                                        .child(Select::new(&row.reasoning_effort).small()),
                                                )
                                                .child(
                                                    div()
                                                        .w(px(36.))
                                                        .flex()
                                                        .items_center()
                                                        .justify_center()
                                                        .child(
                                                            Button::new(SharedString::from(format!("del-row-{}", idx)))
                                                                .ghost()
                                                                .small()
                                                                .icon(IconName::Delete)
                                                                .tooltip("删除此模型映射")
                                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                                    this.remove_catalog_row(idx, cx);
                                                                })),
                                                        ),
                                                )
                                        }),
                                    )
                                    .into_any_element()
                            },
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(theme.muted_foreground)
                                .child("💡 提示：配置模型映射后，将生成 ~/.codex/router-switch-model-catalog.json，在 Codex 侧边栏/设置中可直接切换已配置的模型。"),
                        ),
                ),
            )
            .child(
                // Bottom Action Buttons
                h_flex()
                    .w_full()
                    .justify_end()
                    .gap(px(10.))
                    .pt(px(8.))
                    .pb(px(24.))
                    .child(
                        Button::new("bottom-cancel-btn")
                            .outline()
                            .label("取消")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.form = None;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("bottom-save-btn")
                            .primary()
                            .icon(IconName::Check)
                            .label("保存供应商配置")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.submit_form(window, cx);
                            })),
                    ),
            )
            .into_any_element()
    }
}

impl Render for RouterApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dark = cx.theme().is_dark();
        let page = if self.form.is_some() {
            self.render_form_page(cx).into_any_element()
        } else {
            match self.route {
                Route::Dashboard => self.render_dashboard_page(cx).into_any_element(),
                Route::Codex => self.render_codex_page(cx).into_any_element(),
                Route::Claude => self.render_codex_page(cx).into_any_element(),
                Route::Grok => self.render_codex_page(cx).into_any_element(),
                Route::Notifications => self.render_notifications_page(cx).into_any_element(),
                Route::Settings => self.render_settings_page(cx).into_any_element(),
            }
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
                        if self.route == Route::Settings {
                            this.child(self.render_settings_sidebar(cx))
                        } else {
                            this.child(self.render_sidebar(cx))
                        }
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
                base_url: String::new(),
                model: String::new(),
                model_mappings: Vec::new(),
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

        let selected_index = if editing_id.is_none() {
            RESPONSES_PRESETS
                .iter()
                .position(|p| p.id == "custom")
                .map(|idx| gpui_component::IndexPath::default().row(idx))
        } else if form.name.trim().is_empty() {
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

        let catalog_rows = form
            .model_mappings
            .iter()
            .map(|m| {
                CatalogRowDraft::new(
                    &m.display_name,
                    &m.model,
                    m.context_window,
                    m.reasoning_effort.as_deref(),
                    &[],
                    window,
                    cx,
                )
            })
            .collect();

        Self {
            editing_id,
            kind: form.kind,
            name: field(window, cx, &form.name, "输入供应商名称，如 PackyCode"),
            api_key: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("sk-...")
                    .masked(true)
                    .default_value(form.api_key)
            }),
            base_url: field(window, cx, &form.base_url, "https://api.example.com/v1"),
            model: field(window, cx, &form.model, "gpt-5.6-sol"),
            preset_select,
            catalog_rows,
            fetched_models: Vec::new(),
            has_fetched_models: false,
            default_model_select: None,
            is_fetching_models: false,
            _preset_sub: Some(_preset_sub),
            _default_model_sub: None,
        }
    }

    fn to_codex_form(&self, cx: &App) -> CodexForm {
        let model_mappings = self
            .catalog_rows
            .iter()
            .filter_map(|row| row.to_mapping(cx))
            .collect();

        CodexForm {
            name: self.name.read(cx).value().to_string(),
            website_url: String::new(),
            kind: CodexKind::ResponsesThirdParty,
            api_key: self.api_key.read(cx).value().to_string(),
            base_url: self.base_url.read(cx).value().to_string(),
            model: self.model.read(cx).value().to_string(),
            model_mappings,
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
