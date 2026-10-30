//! The minimal graphical converter for Pageformer documents.

#![forbid(unsafe_code)]

mod app;
mod open;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([720.0, 380.0])
            .with_min_inner_size([520.0, 260.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Pageformer Publisher",
        options,
        Box::new(|context| Ok(Box::new(app::Publisher::new(context)))),
    )
}
