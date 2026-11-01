//! The publisher window: a converter over the shared pipeline.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::Duration;

use eframe::egui;
use pageformer::confidential::config::{
    ConfidentialConfig, OutputOrigin, ResolvedOutput, UserConfig, resolve_output,
};
use pageformer::confidential::{self, ConfidentialStore};
use pageformer::format::Format;
use pageformer::pipeline::{self, DocumentJob, Source};
use pageformer::template::loader;

use crate::open;

/// The visible state of the converter.
#[derive(Debug, Clone, PartialEq, Eq)]
enum State {
    Idle,
    Working {
        format: Format,
    },
    Copying,
    Succeeded {
        output: PathBuf,
        warnings: Vec<String>,
    },
    Copied {
        warnings: Vec<String>,
    },
    Failed {
        message: String,
    },
}

/// One export handed to the worker thread.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ExportRequest {
    input: PathBuf,
    source_root: PathBuf,
    output: PathBuf,
    format: Format,
    confidential_file: Option<PathBuf>,
    template: Option<PathBuf>,
}

/// One plain-text copy handed to the worker thread.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CopyRequest {
    input: PathBuf,
    source_root: PathBuf,
    confidential_file: Option<PathBuf>,
    template: Option<PathBuf>,
}

/// One finished export, with the warnings already redacted.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Completed {
    output: PathBuf,
    warnings: Vec<String>,
}

/// What the worker thread produced.
#[derive(Debug, Clone, PartialEq, Eq)]
enum WorkerOutcome {
    Exported(Completed),
    Copied { text: String, warnings: Vec<String> },
}

/// What one format button does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExportAction {
    Convert(Format),
    ConvertAndCopy(Format),
}

/// One colored export button in the right column.
struct ExportButton {
    label: &'static str,
    fill: egui::Color32,
    text: egui::Color32,
    action: ExportAction,
}

/// The export buttons, in display order. Every format now has a backend.
const EXPORT_BUTTONS: [ExportButton; 6] = [
    ExportButton {
        label: "PDF",
        fill: egui::Color32::from_rgb(0xc6, 0x28, 0x28),
        text: egui::Color32::WHITE,
        action: ExportAction::Convert(Format::Pdf),
    },
    ExportButton {
        label: "ODT",
        fill: egui::Color32::from_rgb(0x2e, 0x7d, 0x32),
        text: egui::Color32::WHITE,
        action: ExportAction::Convert(Format::Odt),
    },
    ExportButton {
        label: "DOCX",
        fill: egui::Color32::from_rgb(0x15, 0x65, 0xc0),
        text: egui::Color32::WHITE,
        action: ExportAction::Convert(Format::Docx),
    },
    ExportButton {
        label: "HTML",
        fill: egui::Color32::from_rgb(0xef, 0x6c, 0x00),
        text: egui::Color32::WHITE,
        action: ExportAction::Convert(Format::Html),
    },
    ExportButton {
        label: "EPUB",
        fill: egui::Color32::from_rgb(0xf9, 0xa8, 0x25),
        text: egui::Color32::BLACK,
        action: ExportAction::Convert(Format::Epub),
    },
    ExportButton {
        label: "TXT",
        fill: egui::Color32::from_rgb(0x61, 0x61, 0x61),
        text: egui::Color32::WHITE,
        action: ExportAction::ConvertAndCopy(Format::Txt),
    },
];

/// The width of one format button.
const BUTTON_WIDTH: f32 = 120.0;
/// The height of one format button.
const BUTTON_HEIGHT: f32 = 36.0;
/// The width of the copy half of the split TXT button.
const COPY_BUTTON_WIDTH: f32 = 48.0;
/// The fill of the copy half of the split TXT button.
const COPY_FILL: egui::Color32 = egui::Color32::from_rgb(0x45, 0x45, 0x45);
/// The title shown at the top of the window.
const TITLE: &str = "Pageformer Publisher";

/// The title color, derived from the active theme so it never relies on a
/// background being dark or light.
fn title_color(visuals: &egui::Visuals) -> egui::Color32 {
    visuals
        .strong_text_color()
        .lerp_to_gamma(visuals.window_fill, 0.12)
}

/// The version shown at the top right of the window.
fn version_label() -> String {
    format!("v{}", env!("CARGO_PKG_VERSION"))
}

/// The pure converter state: the input, the paths and the visible state.
struct Session {
    input: Option<PathBuf>,
    output_override: String,
    automatic_output: Option<PathBuf>,
    automatic_origin: OutputOrigin,
    confidential_file: String,
    template_file: String,
    state: State,
}

impl Session {
    fn new() -> Self {
        Self {
            input: None,
            output_override: String::new(),
            automatic_output: None,
            automatic_origin: OutputOrigin::Source,
            confidential_file: String::new(),
            template_file: String::new(),
            state: State::Idle,
        }
    }

    fn can_export(&self) -> bool {
        self.input.is_some() && !matches!(self.state, State::Working { .. } | State::Copying)
    }

    /// Accepts a Markdown file or a confidential INI file.
    fn accept_path(&mut self, path: PathBuf) -> bool {
        if is_markdown_file(&path) {
            self.output_override.clear();
            self.automatic_output = None;
            self.automatic_origin = OutputOrigin::Source;
            self.input = Some(path);
            true
        } else if is_confidential_file(&path) {
            self.confidential_file = path.display().to_string();
            true
        } else {
            false
        }
    }

    fn source_root(&self) -> Option<PathBuf> {
        self.input.as_ref().map(|input| source_directory(input))
    }

    /// Stores the automatic output resolved from the user configuration.
    fn set_output_resolution(&mut self, resolved: &ResolvedOutput) {
        self.automatic_output = resolved.directory().map(Path::to_path_buf);
        self.automatic_origin = resolved.origin();
    }

    /// Forgets the automatic output, falling back to the source folder.
    fn clear_output_resolution(&mut self) {
        self.automatic_output = None;
        self.automatic_origin = OutputOrigin::Source;
    }

    /// The explicit override, or the automatic directory, or `None` for beside
    /// the source.
    fn output_directory(&self) -> Option<PathBuf> {
        explicit_path(&self.output_override).or_else(|| self.automatic_output.clone())
    }

    fn output_path(&self, format: Format) -> Option<PathBuf> {
        let input = self.input.as_ref()?;
        let name = input.file_name()?;
        match self.output_directory() {
            Some(directory) => Some(directory.join(name).with_extension(format.extension())),
            None => Some(input.with_extension(format.extension())),
        }
    }

    /// The automatic output and its origin, for the Configuration hint. The
    /// automatic value is what an empty override uses.
    fn automatic_summary(&self) -> Option<String> {
        let directory = self
            .automatic_output
            .clone()
            .or_else(|| self.source_root())?;
        Some(format!(
            "{} ({})",
            directory.display(),
            output_origin_label(self.automatic_origin)
        ))
    }

    fn begin(&mut self, format: Format) -> Option<ExportRequest> {
        if !self.can_export() {
            return None;
        }
        let input = self.input.clone()?;
        let source_root = self.source_root()?;
        let output = self.output_path(format)?;
        let confidential_file = explicit_path(&self.confidential_file);
        let template = explicit_path(&self.template_file);
        self.state = State::Working { format };
        Some(ExportRequest {
            input,
            source_root,
            output,
            format,
            confidential_file,
            template,
        })
    }

    fn finish(&mut self, result: Result<Completed, String>) {
        self.state = match result {
            Ok(completed) => State::Succeeded {
                output: completed.output,
                warnings: completed.warnings,
            },
            Err(message) => State::Failed { message },
        };
    }

    fn begin_copy(&mut self) -> Option<CopyRequest> {
        if !self.can_export() {
            return None;
        }
        let input = self.input.clone()?;
        let source_root = self.source_root()?;
        let confidential_file = explicit_path(&self.confidential_file);
        let template = explicit_path(&self.template_file);
        self.state = State::Copying;
        Some(CopyRequest {
            input,
            source_root,
            confidential_file,
            template,
        })
    }

    fn finish_copy(&mut self, warnings: Vec<String>) {
        self.state = State::Copied { warnings };
    }
}

/// The eframe application.
pub struct Publisher {
    session: Session,
    template_label: Option<String>,
    template_path: Option<PathBuf>,
    template_error: Option<String>,
    message: Option<String>,
    output_error: Option<String>,
    receiver: Option<Receiver<Result<WorkerOutcome, String>>>,
}

impl Publisher {
    pub fn new(context: &eframe::CreationContext<'_>) -> Self {
        context.egui_ctx.all_styles_mut(|style| {
            style.spacing.tooltip_width = 340.0;
        });
        install_fonts(&context.egui_ctx);
        Self {
            session: Session::new(),
            template_label: None,
            template_path: None,
            template_error: None,
            message: None,
            output_error: None,
            receiver: None,
        }
    }

    fn accept_input(&mut self, path: PathBuf) {
        if !self.session.accept_path(path.clone()) {
            self.message = Some(format!("{} is not a Markdown or INI file", path.display()));
            return;
        }
        if is_markdown_file(&path) {
            self.refresh_template();
        }
        self.refresh_output();
        self.message = None;
    }

    /// Resolves the automatic output from the user configuration. The explicit
    /// override in the field wins later, in the session.
    fn refresh_output(&mut self) {
        self.output_error = None;
        let Some(source_root) = self.session.source_root() else {
            self.session.clear_output_resolution();
            return;
        };
        let user = match UserConfig::load_default() {
            Ok(user) => user,
            Err(error) => {
                self.session.clear_output_resolution();
                self.output_error = Some(error.to_string());
                return;
            }
        };
        let confidential = if self.session.confidential_file.trim().is_empty() {
            match user.confidential_config() {
                Ok(config) => Some(config),
                Err(error) => {
                    self.session.clear_output_resolution();
                    self.output_error = Some(error.to_string());
                    return;
                }
            }
        } else {
            None
        };
        let resolved = resolve_output(&source_root, None, &user, confidential.as_ref());
        self.session.set_output_resolution(&resolved);
    }

    fn refresh_template(&mut self) {
        let Some(input) = self.session.input.clone() else {
            self.template_label = None;
            self.template_path = None;
            self.template_error = None;
            return;
        };
        let Some(source_root) = self.session.source_root() else {
            self.template_label = None;
            self.template_path = None;
            self.template_error = None;
            return;
        };
        let job = DocumentJob::new(Source::File(input), source_root)
            .with_template(explicit_path(&self.session.template_file))
            .with_global_template(loader::global_template_path());
        match pipeline::resolve_template(&job) {
            Ok(resolved) => {
                self.template_label = Some(resolved.origin().to_string());
                self.template_path = resolved.origin().path().map(Path::to_path_buf);
                self.template_error = None;
            }
            Err(error) => {
                self.template_label = None;
                self.template_path = None;
                self.template_error = Some(error.to_string());
            }
        }
    }

    fn start(&mut self, format: Format) {
        self.refresh_output();
        let Some(request) = self.session.begin(format) else {
            return;
        };
        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);
        thread::spawn(move || {
            let _ = sender.send(run_export(request));
        });
    }

    fn start_copy(&mut self) {
        let Some(request) = self.session.begin_copy() else {
            return;
        };
        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);
        thread::spawn(move || {
            let _ = sender.send(run_copy(request));
        });
    }

    fn poll_receiver(&mut self, context: &egui::Context) {
        let Some(receiver) = self.receiver.take() else {
            return;
        };
        match receiver.try_recv() {
            Ok(Ok(WorkerOutcome::Exported(completed))) => self.session.finish(Ok(completed)),
            Ok(Ok(WorkerOutcome::Copied { text, warnings })) => {
                self.session.finish_copy(warnings);
                context.copy_text(text);
            }
            Ok(Err(message)) => self.session.finish(Err(message)),
            Err(TryRecvError::Empty) => self.receiver = Some(receiver),
            Err(TryRecvError::Disconnected) => {
                self.session
                    .finish(Err("the export worker stopped unexpectedly".to_owned()));
            }
        }
    }

    fn poll_drops(&mut self, context: &egui::Context) {
        let dropped = context.input(|input| input.raw.dropped_files.clone());
        for file in dropped {
            self.accept_input(file.path().to_path_buf());
        }
    }

    fn open(&mut self, path: &Path) {
        if let Err(error) = open::open_path(path) {
            self.message = Some(format!("cannot open {}: {error}", path.display()));
        }
    }

    fn choose_markdown(&mut self) {
        if let Some(path) = open::choose_file("Choose a Markdown file", "Markdown", &["md"]) {
            self.accept_input(path);
        }
    }

    fn choose_template(&mut self) {
        if let Some(path) = open::choose_file("Choose a template", "YAML", &["yaml", "yml"]) {
            self.session.template_file = path.display().to_string();
            self.refresh_template();
        }
    }

    fn choose_confidential(&mut self) {
        if let Some(path) = open::choose_file("Choose a confidential INI file", "INI", &["ini"]) {
            self.session.confidential_file = path.display().to_string();
            self.refresh_output();
        }
    }

    fn choose_output(&mut self) {
        if let Some(path) = open::choose_folder("Choose the output folder") {
            self.session.output_override = path.display().to_string();
        }
    }

    fn render(&mut self, ui: &mut egui::Ui) {
        egui::Sides::new().shrink_left().truncate().show(
            ui,
            |ui| {
                ui.label(
                    egui::RichText::new(TITLE)
                        .size(20.0)
                        .strong()
                        .color(title_color(ui.visuals())),
                );
            },
            |ui| {
                ui.label(egui::RichText::new(version_label()).small());
            },
        );
        ui.add_space(10.0);

        let button_width = BUTTON_WIDTH;
        let gap = 12.0;
        let spacing = ui.spacing().item_spacing.x;
        let button_spacing = ui.spacing().item_spacing.y;
        let button_count = f32::from(u16::try_from(EXPORT_BUTTONS.len()).unwrap_or(u16::MAX));
        let column_height = button_count * BUTTON_HEIGHT + (button_count - 1.0) * button_spacing;
        let drop_height = column_height.max(184.0);
        let drop_width = (ui.available_width() - button_width - gap - spacing).max(140.0);
        let working = matches!(self.session.state, State::Working { .. } | State::Copying);
        ui.horizontal_top(|ui| {
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(drop_width, drop_height), egui::Sense::click());
            let (fill, stroke, color) = {
                let visuals = ui.visuals();
                (
                    if response.hovered() {
                        visuals.widgets.hovered.bg_fill
                    } else {
                        visuals.extreme_bg_color
                    },
                    visuals.widgets.inactive.bg_stroke,
                    visuals.text_color(),
                )
            };
            ui.painter().rect_filled(rect, 8.0, fill);
            ui.painter()
                .rect_stroke(rect, 8.0, stroke, egui::StrokeKind::Inside);
            let font = egui::FontId::proportional(16.0);
            let first = if self.session.input.is_some() {
                "Drop another .md here"
            } else {
                "Drop your .md here"
            };
            ui.painter().text(
                rect.center() + egui::vec2(0.0, -10.0),
                egui::Align2::CENTER_CENTER,
                first,
                font.clone(),
                color,
            );
            ui.painter().text(
                rect.center() + egui::vec2(0.0, 12.0),
                egui::Align2::CENTER_CENTER,
                "or click to choose",
                font,
                color,
            );
            let response = with_tooltip(
                response,
                "Drop a Markdown file, or click to choose one. Use the picker if dragging does not work.",
            );
            if !working && response.clicked() {
                self.choose_markdown();
            }

            ui.add_space(gap);
            ui.vertical(|ui| {
                ui.set_width(button_width);
                let enabled = self.session.can_export();
                for entry in &EXPORT_BUTTONS {
                    match entry.action {
                        ExportAction::ConvertAndCopy(format) => {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 0.0;
                                let main_width = BUTTON_WIDTH - COPY_BUTTON_WIDTH;
                                let export = with_tooltip(
                                    ui.add_enabled(
                                        enabled,
                                        egui::Button::new(
                                            egui::RichText::new(entry.label).color(entry.text),
                                        )
                                        .fill(entry.fill)
                                        .corner_radius(egui::CornerRadius {
                                            nw: 4,
                                            sw: 4,
                                            ne: 0,
                                            se: 0,
                                        })
                                        .min_size(egui::vec2(main_width, BUTTON_HEIGHT)),
                                    ),
                                    "Export the selected document as TXT.",
                                );
                                if export.clicked() {
                                    self.start(format);
                                }
                                let copy = with_tooltip(
                                    ui.add_enabled(
                                        enabled,
                                        egui::Button::new(
                                            egui::RichText::new("Copy").color(entry.text),
                                        )
                                        .fill(COPY_FILL)
                                        .corner_radius(egui::CornerRadius {
                                            nw: 0,
                                            sw: 0,
                                            ne: 4,
                                            se: 4,
                                        })
                                        .min_size(egui::vec2(
                                            COPY_BUTTON_WIDTH,
                                            BUTTON_HEIGHT,
                                        )),
                                    ),
                                    "Copy the selected document as plain text to the clipboard.",
                                );
                                if copy.clicked() {
                                    self.start_copy();
                                }
                            });
                        }
                        ExportAction::Convert(format) => {
                            let button = egui::Button::new(
                                egui::RichText::new(entry.label).color(entry.text),
                            )
                            .fill(entry.fill)
                            .min_size(egui::vec2(BUTTON_WIDTH, BUTTON_HEIGHT));
                            let response = with_tooltip(
                                ui.add_enabled(enabled, button),
                                &format!(
                                    "Export the selected document as {}.",
                                    entry.label
                                ),
                            );
                            if response.clicked() {
                                self.start(format);
                            }
                        }
                    }
                }
            });
        });

        if let Some(input) = &self.session.input {
            ui.add_space(4.0);
            let text = elide_prefixed(
                ui,
                "Selected: ",
                &input.display().to_string(),
                ui.available_width(),
            );
            with_tooltip(
                ui.colored_label(egui::Color32::from_rgb(0xa7, 0xc7, 0xe7), text),
                "The Markdown file to export.",
            );
        }

        ui.add_space(8.0);
        let configuration = egui::CollapsingHeader::new("Configuration").show(ui, |ui| {
            let template_hint = self.template_label.clone();
            let output_hint = if self.output_error.is_none() {
                self.session.automatic_summary()
            } else {
                None
            };
            let template_buttons = [
                RowButton {
                    label: "Choose",
                    tooltip: "Choose the template YAML file.",
                    width: configuration_button_width(ui, "Choose"),
                    enabled: true,
                },
                RowButton {
                    label: "Edit",
                    tooltip: "Open the resolved template in the default editor.",
                    width: configuration_button_width(ui, "Edit"),
                    enabled: self.template_path.is_some(),
                },
                RowButton {
                    label: "Folder",
                    tooltip: "Open the folder that contains the resolved template.",
                    width: configuration_button_width(ui, "Folder"),
                    enabled: self.template_path.is_some(),
                },
            ];
            let output_buttons = [
                RowButton {
                    label: "Choose",
                    tooltip: "Choose an output folder override.",
                    width: configuration_button_width(ui, "Choose"),
                    enabled: true,
                },
                RowButton {
                    label: "Use default",
                    tooltip: "Clear the override and use the automatic directory shown as the hint.",
                    width: configuration_button_width(ui, "Use default"),
                    enabled: explicit_path(&self.session.output_override).is_some(),
                },
            ];
            let confidential_buttons = [RowButton {
                label: "Choose",
                tooltip: "Choose the confidential INI file.",
                width: configuration_button_width(ui, "Choose"),
                enabled: true,
            }];
            let label_width = configuration_label_width(ui);
            let spacing = ui.spacing().item_spacing.x;
            let action_width = button_group_width(&template_buttons, spacing)
                .max(button_group_width(&output_buttons, spacing))
                .max(button_group_width(&confidential_buttons, spacing));
            let field_width = configuration_field_width(
                ui.available_width(),
                label_width,
                action_width,
                spacing,
            );

            let template_row = path_row(
                ui,
                &mut self.session.template_file,
                &PathRowSpec {
                    label: "Template:",
                    label_tooltip: "The template in use when this field is empty: the nearest local file, then the global file, then the embedded fallback.",
                    hint: template_hint.as_deref(),
                    field_tooltip: "Explicit YAML template. Leave it empty to use the resolved template shown as the hint.",
                    label_width,
                    field_width,
                },
                &template_buttons,
            );
            match template_row.clicked {
                Some(0) => self.choose_template(),
                Some(1) => {
                    if let Some(path) = self.template_path.clone() {
                        self.open(&path);
                    }
                }
                Some(2) => {
                    let folder = self
                        .template_path
                        .as_deref()
                        .and_then(Path::parent)
                        .map(Path::to_path_buf);
                    if let Some(folder) = folder {
                        self.open(&folder);
                    }
                }
                _ if template_row.lost_focus => self.refresh_template(),
                _ => {}
            }
            if let Some(error) = &self.template_error {
                with_tooltip(
                    ui.colored_label(
                        egui::Color32::from_rgb(0xb0, 0x2a, 0x2a),
                        format!("Template: {error}"),
                    ),
                    "The template could not be resolved; choose a valid YAML file.",
                );
            }

            let output_row = path_row(
                ui,
                &mut self.session.output_override,
                &PathRowSpec {
                    label: "Output:",
                    label_tooltip: "Leave it empty to use the confidential mapping, the user default, or the source folder. A value is a manual override. Documents with confidential fields need a folder outside the source tree.",
                    hint: output_hint.as_deref(),
                    field_tooltip: "Leave it empty for the automatic directory; type or choose a folder to override it. Documents with confidential fields need a folder outside the source tree.",
                    label_width,
                    field_width,
                },
                &output_buttons,
            );
            match output_row.clicked {
                Some(0) => self.choose_output(),
                Some(1) => {
                    self.session.output_override.clear();
                    self.refresh_output();
                }
                _ if output_row.lost_focus && self.session.output_override.trim().is_empty() => {
                    self.refresh_output();
                }
                _ => {}
            }
            if let Some(error) = &self.output_error {
                with_tooltip(
                    ui.colored_label(
                        egui::Color32::from_rgb(0xb0, 0x2a, 0x2a),
                        format!("Output configuration: {error}"),
                    ),
                    "The automatic output could not be resolved. Choose a folder to override it.",
                );
            }

            let confidential_row = path_row(
                ui,
                &mut self.session.confidential_file,
                &PathRowSpec {
                    label: "Confidential:",
                    label_tooltip: "INI file with the confidential {NAME} values. Leave it empty to use the user-level folder mapping. Keep it outside the source tree, with mode 600.",
                    hint: None,
                    field_tooltip: "INI file with the confidential {NAME} values. Leave it empty to use the user-level folder mapping. Keep it outside the source tree, with mode 600.",
                    label_width,
                    field_width,
                },
                &confidential_buttons,
            );
            match confidential_row.clicked {
                Some(0) => self.choose_confidential(),
                _ if confidential_row.lost_focus => self.refresh_output(),
                _ => {}
            }
        });
        with_tooltip(
            configuration.header_response,
            "Optional template, output folder and confidential values. Leave them empty to use the automatic choices.",
        );

        ui.add_space(8.0);
        if let Some(path) = self.render_status(ui) {
            self.open(&path);
        }

        if let Some(message) = &self.message {
            ui.add_space(4.0);
            with_tooltip(
                ui.colored_label(egui::Color32::from_rgb(0xb0, 0x2a, 0x2a), message),
                "A message about the file picker or the desktop action.",
            );
        }
    }

    /// Draws the status area and reports a path the user asked to open.
    fn render_status(&self, ui: &mut egui::Ui) -> Option<PathBuf> {
        let mut request = None;
        match &self.session.state {
            State::Idle => {
                with_tooltip(
                    ui.label("Drop a Markdown file or click the box to choose."),
                    "Choose a Markdown file to enable the format buttons.",
                );
            }
            State::Working { format } => {
                ui.horizontal(|ui| {
                    let spinner = ui.spinner();
                    with_tooltip(
                        spinner,
                        "The export runs on a worker thread; the window stays responsive.",
                    );
                    with_tooltip(
                        ui.label(format!("Rendering {}...", format.name())),
                        "The export runs on a worker thread; the window stays responsive.",
                    );
                });
            }
            State::Copying => {
                ui.horizontal(|ui| {
                    let spinner = ui.spinner();
                    with_tooltip(
                        spinner,
                        "The plain-text copy runs on a worker thread; the window stays responsive.",
                    );
                    with_tooltip(
                        ui.label("Copying plain text..."),
                        "The plain-text copy runs on a worker thread; the window stays responsive.",
                    );
                });
            }
            State::Succeeded { output, warnings } => {
                let text = elide_prefixed(
                    ui,
                    "Wrote ",
                    &output.display().to_string(),
                    ui.available_width(),
                );
                with_tooltip(
                    ui.colored_label(egui::Color32::from_rgb(0x2e, 0x7d, 0x32), text),
                    "The generated file.",
                );
                ui.horizontal(|ui| {
                    let open = with_tooltip(
                        ui.button("Open"),
                        "Open the generated file in the default application.",
                    );
                    if open.clicked() {
                        request = Some(output.clone());
                    }
                    if let Some(folder) = output.parent() {
                        let folder = folder.to_path_buf();
                        let open_folder = with_tooltip(
                            ui.button("Folder"),
                            "Open the folder that contains the generated file.",
                        );
                        if open_folder.clicked() {
                            request = Some(folder);
                        }
                    }
                });
                for warning in warnings {
                    with_tooltip(ui.label(warning.as_str()), "A warning from the renderer.");
                }
            }
            State::Copied { warnings } => {
                with_tooltip(
                    ui.colored_label(
                        egui::Color32::from_rgb(0x2e, 0x7d, 0x32),
                        "Copied plain text to the clipboard",
                    ),
                    "The document text is on the clipboard; it is never displayed here.",
                );
                for warning in warnings {
                    with_tooltip(ui.label(warning.as_str()), "A warning from the renderer.");
                }
            }
            State::Failed { message } => {
                with_tooltip(
                    ui.colored_label(egui::Color32::from_rgb(0xb0, 0x2a, 0x2a), message.as_str()),
                    "The export failed; correct the input, template or output folder and try again.",
                );
            }
        }
        request
    }
}

impl eframe::App for Publisher {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_drops(ui.ctx());
        self.poll_receiver(ui.ctx());
        if matches!(self.session.state, State::Working { .. } | State::Copying) {
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }
        let mut content_height = 0.0;
        egui::CentralPanel::default().show(ui, |ui| {
            let response = ui.vertical(|ui| {
                self.render(ui);
            });
            content_height = response.response.rect.height();
        });
        let current = ui
            .ctx()
            .input(|input| input.viewport().inner_rect.map(|rect| rect.size()))
            .unwrap_or_else(|| ui.max_rect().size());
        let target_width = current.x.max(MIN_INNER_WIDTH);
        let target_height = (content_height + CENTRAL_PANEL_MARGIN)
            .ceil()
            .max(MIN_INNER_HEIGHT);
        if (target_width - current.x).abs() >= 1.0 || (target_height - current.y).abs() >= 1.0 {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                    target_width,
                    target_height,
                )));
        }
    }
}

/// The minimum inner width of the window, shared with `main`.
pub const MIN_INNER_WIDTH: f32 = 520.0;

/// The minimum inner height of the window, shared with `main`.
pub const MIN_INNER_HEIGHT: f32 = 260.0;

/// The `CentralPanel` inner margin on both sides of the content.
const CENTRAL_PANEL_MARGIN: f32 = 16.0;

fn with_tooltip(response: egui::Response, text: &str) -> egui::Response {
    let mut tooltip = if response.enabled() {
        egui::Tooltip::for_enabled(&response)
    } else {
        egui::Tooltip::for_disabled(&response)
    };
    tooltip.popup = tooltip.popup.frame(tooltip_frame()).interactable(false);
    tooltip.show(|ui| show_tooltip(ui, text));
    response
}

/// The tooltip frame, painted explicitly so it stays light and readable on
/// every theme and does not inherit the window fill.
fn tooltip_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(egui::Color32::from_gray(224))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(150)))
        .corner_radius(4)
        .inner_margin(egui::Margin::symmetric(10, 8))
        .shadow(egui::Shadow {
            offset: [0, 2],
            blur: 6,
            spread: 0,
            color: egui::Color32::from_black_alpha(48),
        })
}

fn show_tooltip(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .color(egui::Color32::from_gray(48))
            .size(14.0),
    );
}

/// Registers the embedded Liberation Sans Regular as the first proportional
/// font, so the whole interface shares one regular-weight face; the embedded
/// Ubuntu-Light and the emoji fonts stay as fallbacks.
fn install_fonts(context: &egui::Context) {
    let mut definitions = egui::FontDefinitions::default();
    definitions.font_data.insert(
        "ui".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(crate::fonts::UI_FONT)),
    );
    definitions
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "ui".to_owned());
    context.set_fonts(definitions);
}

/// What one path row reported in a frame.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct PathRow {
    lost_focus: bool,
    clicked: Option<usize>,
}

/// A fixed-width action button in a path row.
struct RowButton<'a> {
    label: &'a str,
    tooltip: &'a str,
    width: f32,
    enabled: bool,
}

/// The fixed presentation of one path row.
struct PathRowSpec<'a> {
    label: &'a str,
    label_tooltip: &'a str,
    hint: Option<&'a str>,
    field_tooltip: &'a str,
    label_width: f32,
    field_width: f32,
}

/// The horizontal padding on each side of a Configuration button.
const BUTTON_SIDE_PADDING: f32 = 10.0;
/// The extra space after the widest Configuration label.
const LABEL_GAP: f32 = 4.0;

/// Lays out a label, a text field and fixed-width action buttons. The caller
/// passes a common label and field width, so every row aligns.
fn path_row(
    ui: &mut egui::Ui,
    value: &mut String,
    spec: &PathRowSpec<'_>,
    buttons: &[RowButton<'_>],
) -> PathRow {
    let height = ui.spacing().interact_size.y;
    let mut report = PathRow::default();
    ui.horizontal(|ui| {
        let _ = ui.allocate_ui_with_layout(
            egui::vec2(spec.label_width, height),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.set_width(spec.label_width);
                with_tooltip(ui.label(spec.label), spec.label_tooltip);
            },
        );
        let mut field = egui::TextEdit::singleline(value).desired_width(spec.field_width);
        if let Some(hint) = spec.hint {
            let hint = elide_left(ui, hint, (spec.field_width - 20.0).max(0.0));
            field = field.hint_text(hint.as_str());
        }
        let field = ui.add(field);
        report.lost_focus = field.lost_focus();
        with_tooltip(field, spec.field_tooltip);
        for (index, button) in buttons.iter().enumerate() {
            let response = ui.add_enabled(
                button.enabled,
                egui::Button::new(button.label).min_size(egui::vec2(button.width, height)),
            );
            let response = with_tooltip(response, button.tooltip);
            if response.clicked() {
                report.clicked = Some(index);
            }
        }
    });
    report
}

/// The width of a group of buttons, including the spacing between them.
fn button_group_width(buttons: &[RowButton<'_>], spacing: f32) -> f32 {
    let total: f32 = buttons.iter().map(|button| button.width).sum();
    let gaps = f32::from(u16::try_from(buttons.len().saturating_sub(1)).unwrap_or(u16::MAX));
    total + spacing * gaps
}

/// The common field width for the Configuration rows. The action column is as
/// wide as the widest button group, so every Choose button starts on the same
/// vertical line.
fn configuration_field_width(
    available: f32,
    label_width: f32,
    action_width: f32,
    spacing: f32,
) -> f32 {
    (available - label_width - action_width - spacing * 2.0).max(80.0)
}

/// The width of the widest Configuration label plus a small gap.
fn configuration_label_width(ui: &egui::Ui) -> f32 {
    ["Template:", "Output:", "Confidential:"]
        .into_iter()
        .map(|label| text_width(ui, label))
        .fold(0.0, f32::max)
        + LABEL_GAP
}

/// The width of a Configuration button with equal side padding.
fn configuration_button_width(ui: &egui::Ui, label: &str) -> f32 {
    text_width_for(ui, label, egui::TextStyle::Button) + BUTTON_SIDE_PADDING * 2.0
}

/// Removes characters from the left until the measured text fits, keeping the
/// end. The hidden prefix becomes an ellipsis.
fn elide_left_with(text: &str, max_width: f32, measure: impl Fn(&str) -> f32) -> String {
    if measure(text) <= max_width {
        return text.to_owned();
    }
    for (index, _) in text.char_indices().skip(1) {
        let candidate = format!("…{}", &text[index..]);
        if measure(&candidate) <= max_width {
            return candidate;
        }
    }
    "…".to_owned()
}

/// The width of a text in the body font.
fn text_width(ui: &egui::Ui, text: &str) -> f32 {
    text_width_for(ui, text, egui::TextStyle::Body)
}

/// The width of a text in a given style.
fn text_width_for(ui: &egui::Ui, text: &str, style: egui::TextStyle) -> f32 {
    let font_id = style.resolve(ui.style());
    let color = ui.visuals().text_color();
    ui.painter()
        .layout_no_wrap(text.to_owned(), font_id, color)
        .size()
        .x
}

/// Elides a text from the left so its end stays visible.
fn elide_left(ui: &egui::Ui, text: &str, max_width: f32) -> String {
    elide_left_with(text, max_width.max(0.0), |candidate| {
        text_width(ui, candidate)
    })
}

/// Elides the value after a fixed prefix, keeping the prefix and the end of the
/// value visible.
fn elide_prefixed(ui: &egui::Ui, prefix: &str, value: &str, max_width: f32) -> String {
    let available = (max_width - text_width(ui, prefix)).max(0.0);
    format!("{prefix}{}", elide_left(ui, value, available))
}

/// An explicit path from a text field; blank means automatic resolution.
fn explicit_path(value: &str) -> Option<PathBuf> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
}

fn output_origin_label(origin: OutputOrigin) -> &'static str {
    match origin {
        OutputOrigin::Explicit => "manual",
        OutputOrigin::Ini => "confidential mapping",
        OutputOrigin::Default => "user default",
        OutputOrigin::Source => "source folder",
        _ => "configured",
    }
}

/// A source path is the parent of the input, or the current directory when the
/// input has no parent component.
fn source_directory(path: &Path) -> PathBuf {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}

fn is_markdown_file(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
}

fn is_confidential_file(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| extension.eq_ignore_ascii_case("ini"))
}

fn load_store(
    confidential_file: Option<PathBuf>,
    source_root: &Path,
) -> Result<ConfidentialStore, String> {
    let file = match confidential_file {
        Some(path) => Some(path),
        None => ConfidentialConfig::load_default()
            .map_err(|error| error.to_string())?
            .file_for(source_root)
            .map(Path::to_path_buf),
    };
    let roots = [source_root];
    confidential::loader::load(file.as_deref(), &roots).map_err(|error| error.to_string())
}

fn run_export(request: ExportRequest) -> Result<WorkerOutcome, String> {
    let ExportRequest {
        input,
        source_root,
        output,
        format,
        confidential_file,
        template,
    } = request;
    let store = load_store(confidential_file, &source_root)?;
    let job = DocumentJob::new(Source::File(input), source_root)
        .with_template(template)
        .with_global_template(loader::global_template_path());
    pipeline::export(&job, format, &output, &store)
        .map(|outcome| {
            WorkerOutcome::Exported(Completed {
                output: outcome.output().to_path_buf(),
                warnings: outcome
                    .warnings()
                    .iter()
                    .map(|warning| store.redact(warning))
                    .collect(),
            })
        })
        .map_err(|error| store.redact(&error.to_string()))
}

fn run_copy(request: CopyRequest) -> Result<WorkerOutcome, String> {
    let CopyRequest {
        input,
        source_root,
        confidential_file,
        template,
    } = request;
    let store = load_store(confidential_file, &source_root)?;
    let job = DocumentJob::new(Source::File(input), source_root)
        .with_template(template)
        .with_global_template(loader::global_template_path());
    let rendered = pipeline::render(&job, Format::Txt, &store)
        .map_err(|error| store.redact(&error.to_string()))?;
    let text = String::from_utf8(rendered.bytes().to_vec())
        .map_err(|error| store.redact(&error.to_string()))?;
    let warnings = rendered
        .warnings()
        .iter()
        .map(|warning| store.redact(warning))
        .collect();
    Ok(WorkerOutcome::Copied { text, warnings })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session_with_input(input: &str) -> Session {
        let mut session = Session::new();
        assert!(session.accept_path(PathBuf::from(input)));
        session
    }

    fn character_width(text: &str) -> f32 {
        f32::from(u16::try_from(text.chars().count()).unwrap_or(u16::MAX))
    }

    #[test]
    fn a_markdown_file_enables_export() {
        let session = session_with_input("docs/A.md");

        assert!(session.can_export());
        assert_eq!(session.input, Some(PathBuf::from("docs/A.md")));
        assert!(session.output_override.is_empty());
        assert_eq!(session.automatic_origin, OutputOrigin::Source);
    }

    #[test]
    fn a_new_markdown_selection_clears_the_output_choice() {
        let mut session = session_with_input("docs/A.md");
        session.output_override = "out".to_owned();
        session.automatic_output = Some(PathBuf::from("automatic"));
        session.automatic_origin = OutputOrigin::Default;

        assert!(session.accept_path(PathBuf::from("other/B.md")));

        assert!(session.output_override.is_empty());
        assert_eq!(session.automatic_output, None);
        assert_eq!(session.automatic_origin, OutputOrigin::Source);
    }

    #[test]
    fn a_bare_file_name_uses_the_current_directory() {
        let session = session_with_input("A.md");

        assert_eq!(session.source_root(), Some(PathBuf::from(".")));
        assert!(session.output_override.is_empty());
    }

    #[test]
    fn a_confidential_file_is_accepted_without_an_input() {
        let mut session = Session::new();

        assert!(session.accept_path(PathBuf::from("values.ini")));
        assert_eq!(session.confidential_file, "values.ini");
        assert!(!session.can_export());
    }

    #[test]
    fn another_extension_is_rejected() {
        let mut session = Session::new();

        assert!(!session.accept_path(PathBuf::from("notes.txt")));
        assert!(session.input.is_none());
    }

    #[test]
    fn a_second_request_is_rejected_while_working() {
        let mut session = session_with_input("docs/A.md");

        assert!(session.begin(Format::Pdf).is_some());
        assert!(matches!(session.state, State::Working { .. }));
        assert!(session.begin(Format::Txt).is_none());
    }

    #[test]
    fn the_copy_request_keeps_the_source_root_and_template() {
        let mut session = session_with_input("docs/A.md");
        session.template_file = "t.yaml".to_owned();

        let request = session.begin_copy().expect("a copy request");

        assert_eq!(request.input, PathBuf::from("docs/A.md"));
        assert_eq!(request.source_root, PathBuf::from("docs"));
        assert_eq!(request.template, Some(PathBuf::from("t.yaml")));
    }

    #[test]
    fn a_copied_document_stays_exportable() {
        let mut session = session_with_input("docs/A.md");

        assert!(session.begin_copy().is_some());
        assert!(matches!(session.state, State::Copying));
        assert!(session.begin(Format::Pdf).is_none());

        session.finish_copy(Vec::new());

        assert!(matches!(session.state, State::Copied { .. }));
        assert!(session.can_export());
    }

    #[test]
    fn the_export_buttons_follow_the_requested_order() {
        let labels: Vec<&str> = EXPORT_BUTTONS.iter().map(|button| button.label).collect();

        assert_eq!(labels, ["PDF", "ODT", "DOCX", "HTML", "EPUB", "TXT"]);
        assert_eq!(
            EXPORT_BUTTONS[3].action,
            ExportAction::Convert(Format::Html)
        );
        assert_eq!(
            EXPORT_BUTTONS[4].action,
            ExportAction::Convert(Format::Epub)
        );
        assert_eq!(
            EXPORT_BUTTONS[5].action,
            ExportAction::ConvertAndCopy(Format::Txt)
        );
    }

    #[test]
    fn a_failed_export_stays_exportable() {
        let mut session = session_with_input("docs/A.md");

        session.finish(Err("boom".to_owned()));

        assert_eq!(
            session.state,
            State::Failed {
                message: "boom".to_owned()
            }
        );
        assert!(session.can_export());
    }

    #[test]
    fn a_succeeded_export_stays_exportable() {
        let mut session = session_with_input("docs/A.md");
        session.finish(Ok(Completed {
            output: PathBuf::from("docs/A.pdf"),
            warnings: Vec::new(),
        }));

        assert!(session.can_export());
        assert_eq!(
            session.state,
            State::Succeeded {
                output: PathBuf::from("docs/A.pdf"),
                warnings: Vec::new(),
            }
        );
    }

    #[test]
    fn the_output_stays_beside_the_source_without_a_directory() {
        let session = session_with_input("docs/A.md");

        assert_eq!(
            session.output_path(Format::Pdf),
            Some(PathBuf::from("docs/A.pdf"))
        );
    }

    #[test]
    fn an_automatic_output_is_used_when_the_override_is_empty() {
        let mut session = session_with_input("docs/A.md");
        session.automatic_output = Some(PathBuf::from("out"));
        session.automatic_origin = OutputOrigin::Ini;

        assert_eq!(
            session.output_path(Format::Odt),
            Some(PathBuf::from("out/A.odt"))
        );
        assert_eq!(
            session.automatic_summary(),
            Some("out (confidential mapping)".to_owned())
        );
    }

    #[test]
    fn an_override_wins_over_the_automatic_output() {
        let mut session = session_with_input("docs/A.md");
        session.automatic_output = Some(PathBuf::from("automatic"));
        session.automatic_origin = OutputOrigin::Default;
        session.output_override = "manual".to_owned();

        assert_eq!(
            session.output_path(Format::Odt),
            Some(PathBuf::from("manual/A.odt"))
        );
    }

    #[test]
    fn the_automatic_summary_names_the_origin() {
        let mut session = session_with_input("docs/A.md");
        session.automatic_output = Some(PathBuf::from("out"));
        session.automatic_origin = OutputOrigin::Source;

        assert_eq!(
            session.automatic_summary(),
            Some("out (source folder)".to_owned())
        );
    }

    #[test]
    fn the_configuration_rows_share_the_choose_column() {
        let spacing = 8.0;
        let label_width = 90.0;
        let template = [
            RowButton {
                label: "Choose",
                tooltip: "",
                width: 68.0,
                enabled: true,
            },
            RowButton {
                label: "Edit",
                tooltip: "",
                width: 43.0,
                enabled: true,
            },
            RowButton {
                label: "Folder",
                tooltip: "",
                width: 58.0,
                enabled: true,
            },
        ];
        let output = [
            RowButton {
                label: "Choose",
                tooltip: "",
                width: 68.0,
                enabled: true,
            },
            RowButton {
                label: "Use default",
                tooltip: "",
                width: 90.0,
                enabled: true,
            },
        ];
        let confidential = [RowButton {
            label: "Choose",
            tooltip: "",
            width: 68.0,
            enabled: true,
        }];

        let action_width = button_group_width(&template, spacing)
            .max(button_group_width(&output, spacing))
            .max(button_group_width(&confidential, spacing));
        let field_width = configuration_field_width(748.0, label_width, action_width, spacing);
        let choose_column = label_width + spacing + field_width + spacing;

        assert_eq!(action_width, 185.0);
        assert_eq!(choose_column + action_width, 748.0);
        assert!(button_group_width(&template, spacing) <= action_width);
        assert!(button_group_width(&output, spacing) <= action_width);
        assert!(button_group_width(&confidential, spacing) <= action_width);
    }

    #[test]
    fn a_narrow_configuration_keeps_a_minimum_field_width() {
        assert_eq!(configuration_field_width(200.0, 90.0, 185.0, 8.0), 80.0);
    }

    #[test]
    fn a_short_path_is_not_elided() {
        assert_eq!(
            elide_left_with("short.md", 8.0, character_width),
            "short.md"
        );
    }

    #[test]
    fn a_long_path_keeps_its_end() {
        let elided = elide_left_with("/home/user/project/Report.md", 18.0, character_width);

        assert_eq!(elided, "…project/Report.md");
        assert!(character_width(&elided) <= 18.0);
    }

    #[test]
    fn a_path_with_no_room_becomes_an_ellipsis() {
        assert_eq!(elide_left_with("abcdef", 1.0, character_width), "…");
    }

    #[test]
    fn the_request_keeps_the_source_root_and_the_confidential_file() {
        let mut session = session_with_input("docs/A.md");
        session.confidential_file = "values.ini".to_owned();

        let request = session.begin(Format::Docx).expect("a request");

        assert_eq!(request.input, PathBuf::from("docs/A.md"));
        assert_eq!(request.source_root, PathBuf::from("docs"));
        assert_eq!(request.output, PathBuf::from("docs/A.docx"));
        assert_eq!(request.confidential_file, Some(PathBuf::from("values.ini")));
        assert_eq!(request.template, None);
    }

    #[test]
    fn the_request_keeps_an_explicit_template() {
        let mut session = session_with_input("docs/A.md");
        session.template_file = "  custom.yaml  ".to_owned();

        let request = session.begin(Format::Pdf).expect("a request");

        assert_eq!(request.template, Some(PathBuf::from("custom.yaml")));
    }

    #[test]
    fn a_blank_template_falls_back_to_resolution() {
        let mut session = session_with_input("docs/A.md");
        session.template_file = "   ".to_owned();

        let request = session.begin(Format::Pdf).expect("a request");

        assert_eq!(request.template, None);
    }

    #[test]
    fn the_explicit_path_trims_surrounding_whitespace() {
        assert_eq!(
            explicit_path("  a/b.yaml  "),
            Some(PathBuf::from("a/b.yaml"))
        );
        assert_eq!(explicit_path(" \t "), None);
    }

    #[test]
    fn the_source_root_is_the_input_parent() {
        let session = session_with_input("docs/A.md");

        assert_eq!(session.source_root(), Some(PathBuf::from("docs")));
    }

    #[test]
    fn the_title_color_has_contrast_in_both_themes() {
        let dark = egui::Visuals::dark();
        let light = egui::Visuals::light();

        assert!(contrast_ratio(title_color(&dark), dark.window_fill) >= 3.0);
        assert!(contrast_ratio(title_color(&light), light.window_fill) >= 3.0);
        assert_ne!(title_color(&dark), title_color(&light));
    }

    fn contrast_ratio(left: egui::Color32, right: egui::Color32) -> f32 {
        let left = luminance(left);
        let right = luminance(right);
        (left.max(right) + 0.05) / (left.min(right) + 0.05)
    }

    fn luminance(color: egui::Color32) -> f32 {
        fn channel(value: u8) -> f32 {
            let value = f32::from(value) / 255.0;
            if value <= 0.03928 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * channel(color.r()) + 0.7152 * channel(color.g()) + 0.0722 * channel(color.b())
    }
}
