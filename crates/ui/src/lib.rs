mod app_view;
pub mod assets;
mod theme;

use gpui::{
    px, size, App, AppContext, Application, Bounds, TitlebarOptions, WindowBackgroundAppearance,
    WindowBounds, WindowKind, WindowOptions,
};
use gpui_component::Root;

use crate::app_view::RouterApp;
use crate::assets::AppAssets;

pub fn run() {
    Application::new()
        .with_assets(AppAssets)
        .run(|cx: &mut App| {
            gpui_component::init(cx);
            theme::apply_palette(cx);
            cx.activate(true);

        let window_size = size(px(960.), px(680.));
        let bounds = Bounds::centered(None, window_size, cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Router Switch".into()),
                    appears_transparent: true,
                    traffic_light_position: Some(gpui::point(px(16.), px(16.))),
                }),
            window_background: WindowBackgroundAppearance::Blurred,
            window_min_size: Some(size(px(760.), px(520.))),
            kind: WindowKind::Normal,
            ..Default::default()
        };

        cx.open_window(options, |window, cx| {
            let app = cx.new(|cx| RouterApp::new(window, cx));
            cx.new(|cx| Root::new(app, window, cx))
        })
        .expect("failed to open window");
    });
}
