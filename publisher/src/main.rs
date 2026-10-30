//! The minimal graphical converter for Pageformer documents.

#![forbid(unsafe_code)]

mod app;
mod fonts;
mod open;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([720.0, 300.0])
            .with_min_inner_size([app::MIN_INNER_WIDTH, app::MIN_INNER_HEIGHT]),
        ..Default::default()
    };
    eframe::run_native(
        "Pageformer Publisher",
        options,
        Box::new(|context| Ok(Box::new(app::Publisher::new(context)))),
    )
}
