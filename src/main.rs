#![allow(dead_code)]

mod app;
mod monitor;
mod settings;
mod theme;
mod ui;

use app::App;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1320.0, 820.0])
            .with_min_inner_size([480.0, 400.0])
            .with_title("rproc"),
        ..Default::default()
    };
    eframe::run_native(
        "rproc",
        native_options,
        Box::new(|cc| {
            theme::apply(&cc.egui_ctx);
            Ok(Box::new(App::new()))
        }),
    )
}
