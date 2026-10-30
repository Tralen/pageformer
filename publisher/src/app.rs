//! The publisher window: a converter over the shared pipeline.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::Duration;

use eframe::egui;
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
    Succeeded {
        output: PathBuf,
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

/// One finished export, with the warnings already redacted.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Completed {
    output: PathBuf,
    warnings: Vec<String>,
}

/// The pure converter state: the input, the paths and the visible state.
struct Session {
    input: Option<PathBuf>,
    output_directory: String,
    confidential_file: String,
    template_file: String,
    state: State,
}

impl Session {
    fn new() -> Self {
        Self {
            input: None,
            output_directory: String::new(),
            confidential_file: String::new(),
            template_file: String::new(),
            state: State::Idle,
        }
    }

    fn can_export(&self) -> bool {
        self.input.is_some() && !matches!(self.state, State::Working { .. })
    }

    /// Accepts a Markdown file or a confidential INI file.
    fn accept_path(&mut self, path: PathBuf) -> bool {
        if is_markdown_file(&path) {
            self.output_directory = source_directory(&path).display().to_string();
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

    fn output_path(&self, format: Format) -> Option<PathBuf> {
        let input = self.input.as_ref()?;
        let name = input.file_name()?;
        let directory = self.output_directory.trim();
        if directory.is_empty() {
            Some(input.with_extension(format.extension()))
        } else {
            Some(
                Path::new(directory)
                    .join(name)
                    .with_extension(format.extension()),
            )
        }
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
}

/// The eframe application.
pub struct Publisher {
    session: Session,
    template_label: String,
    template_path: Option<PathBuf>,
    message: Option<String>,
    receiver: Option<Receiver<Result<Completed, String>>>,
}

impl Publisher {
    pub fn new(context: &eframe::CreationContext<'_>) -> Self {
        context.egui_ctx.all_styles_mut(|style| {
            style.spacing.tooltip_width = 340.0;
        });
        install_fonts(&context.egui_ctx);
        Self {
            session: Session::new(),
            template_label: "not selected".to_owned(),
            template_path: None,
            message: None,
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
        self.message = None;
    }

    fn refresh_template(&mut self) {
        let Some(input) = self.session.input.clone() else {
            self.template_label = "not selected".to_owned();
            self.template_path = None;
            return;
        };
        let Some(source_root) = self.session.source_root() else {
            self.template_label = "not selected".to_owned();
            self.template_path = None;
            return;
        };
        let job = DocumentJob::new(Source::File(input), source_root)
            .with_template(explicit_path(&self.session.template_file))
            .with_global_template(loader::global_template_path());
        match pipeline::resolve_template(&job) {
            Ok(resolved) => {
                self.template_label = resolved.origin().to_string();
                self.template_path = resolved.origin().path().map(Path::to_path_buf);
            }
            Err(error) => {
                self.template_label = format!("error: {error}");
                self.template_path = None;
            }
        }
    }

    fn start(&mut self, format: Format) {
        let Some(request) = self.session.begin(format) else {
            return;
        };
        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);
        thread::spawn(move || {
            let _ = sender.send(run_export(request));
        });
    }

    fn poll_receiver(&mut self) {
        let Some(receiver) = self.receiver.take() else {
            return;
        };
        match receiver.try_recv() {
            Ok(result) => self.session.finish(result),
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
        }
    }

    fn choose_output(&mut self) {
        if let Some(path) = open::choose_folder("Choose the output folder") {
            self.session.output_directory = path.display().to_string();
        }
    }

    fn render(&mut self, ui: &mut egui::Ui) {
        let template_label = self.template_label.clone();
        let template_path = self.template_path.clone();
        let mut edit_template = false;
        let mut open_template_folder = None;
        egui::Sides::new()
            .shrink_left()
            .truncate()
            .show(
                ui,
                |ui| {
                    with_tooltip(
                        ui.label("Template:"),
                        "The template in use: the explicit file from Advanced, or the nearest local file, then the global file, then the embedded fallback.",
                    );
                    with_tooltip(
                        ui.add(egui::Label::new(
                            egui::RichText::new(&template_label).small(),
                        )),
                        &template_label,
                    );
                },
                |ui| {
                    if template_path.is_some() {
                        edit_template = with_tooltip(
                            ui.small_button("Edit"),
                            "Open the template in the default editor.",
                        )
                        .clicked();
                        if let Some(folder) = template_path.as_deref().and_then(Path::parent) {
                            let folder = folder.to_path_buf();
                            let open_folder = with_tooltip(
                                ui.small_button("Folder"),
                                "Open the folder that contains the template.",
                            );
                            if open_folder.clicked() {
                                open_template_folder = Some(folder);
                            }
                        }
                    }
                },
            );
        if edit_template && let Some(path) = &template_path {
            self.open(path);
        }
        if let Some(folder) = open_template_folder {
            self.open(&folder);
        }

        ui.add_space(8.0);

        let button_width = 120.0;
        let gap = 12.0;
        let spacing = ui.spacing().item_spacing.x;
        let drop_height = 184.0;
        let drop_width = (ui.available_width() - button_width - gap - spacing).max(140.0);
        let working = matches!(self.session.state, State::Working { .. });
        ui.horizontal_top(|ui| {
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(drop_width, drop_height), egui::Sense::click());
            let visuals = ui.visuals().clone();
            let fill = if response.hovered() {
                visuals.widgets.hovered.bg_fill
            } else {
                visuals.extreme_bg_color
            };
            ui.painter().rect_filled(rect, 8.0, fill);
            ui.painter().rect_stroke(
                rect,
                8.0,
                visuals.widgets.inactive.bg_stroke,
                egui::StrokeKind::Inside,
            );
            let font = egui::FontId::proportional(16.0);
            let color = visuals.text_color();
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
                for format in Format::ALL {
                    let button = egui::Button::new(format.name().to_uppercase())
                        .min_size(egui::vec2(button_width, 36.0));
                    let response = with_tooltip(
                        ui.add_enabled(enabled, button),
                        &format!("Export the selected document as {}.", format.name().to_uppercase()),
                    );
                    if response.clicked() {
                        self.start(format);
                    }
                }
            });
        });

        if let Some(input) = self.session.input.clone() {
            ui.add_space(4.0);
            with_tooltip(
                ui.colored_label(
                    egui::Color32::from_rgb(0xa7, 0xc7, 0xe7),
                    format!("Selected: {}", input.display()),
                ),
                "The Markdown file to export.",
            );
        }

        ui.add_space(8.0);
        let advanced = egui::CollapsingHeader::new("Advanced").show(ui, |ui| {
            let template_row = path_row(
                ui,
                "Template:",
                "Explicit YAML template to use instead of resolving one; leave it empty for automatic resolution.",
                &mut self.session.template_file,
                "Explicit YAML template. Leave it empty to use the nearest local file, then the global file, then the embedded fallback.",
                "Choose the template YAML file with the platform dialog.",
            );
            if template_row.chosen {
                self.choose_template();
            } else if template_row.lost_focus {
                self.refresh_template();
            }
            let output_row = path_row(
                ui,
                "Output:",
                "Folder for the generated file. It starts as the Markdown folder; documents with confidential fields need a folder outside the source tree.",
                &mut self.session.output_directory,
                "Folder for the generated file. It starts as the Markdown folder; documents with confidential fields need a folder outside the source tree.",
                "Choose the output folder with the platform dialog.",
            );
            if output_row.chosen {
                self.choose_output();
            }
            let confidential_row = path_row(
                ui,
                "Confidential:",
                "INI file with the confidential {NAME} values. Keep it outside the source tree, with mode 600.",
                &mut self.session.confidential_file,
                "INI file with the confidential {NAME} values. Keep it outside the source tree, with mode 600.",
                "Choose the confidential INI file with the platform dialog.",
            );
            if confidential_row.chosen {
                self.choose_confidential();
            }
        });
        with_tooltip(
            advanced.header_response,
            "Template override, output folder and confidential values.",
        );

        ui.add_space(8.0);
        match self.session.state.clone() {
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
            State::Succeeded { output, warnings } => {
                with_tooltip(
                    ui.colored_label(
                        egui::Color32::from_rgb(0x2e, 0x7d, 0x32),
                        format!("Wrote {}", output.display()),
                    ),
                    "The generated file.",
                );
                ui.horizontal(|ui| {
                    let open = with_tooltip(
                        ui.button("Open"),
                        "Open the generated file in the default application.",
                    );
                    if open.clicked() {
                        self.open(&output);
                    }
                    if let Some(folder) = output.parent() {
                        let folder = folder.to_path_buf();
                        let open_folder = with_tooltip(
                            ui.button("Folder"),
                            "Open the folder that contains the generated file.",
                        );
                        if open_folder.clicked() {
                            self.open(&folder);
                        }
                    }
                });
                for warning in warnings {
                    with_tooltip(ui.label(warning), "A warning from the renderer.");
                }
            }
            State::Failed { message } => {
                with_tooltip(
                    ui.colored_label(egui::Color32::from_rgb(0xb0, 0x2a, 0x2a), message),
                    "The export failed; correct the input, template or output folder and try again.",
                );
            }
        }

        if let Some(message) = &self.message {
            ui.add_space(4.0);
            with_tooltip(
                ui.colored_label(egui::Color32::from_rgb(0xb0, 0x2a, 0x2a), message),
                "A message about the file picker or the desktop action.",
            );
        }
    }
}

impl eframe::App for Publisher {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_drops(ui.ctx());
        self.poll_receiver();
        if matches!(self.session.state, State::Working { .. }) {
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
        .fill(egui::Color32::from_gray(245))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(140)))
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
            .color(egui::Color32::BLACK)
            .size(14.0)
            .family(egui::FontFamily::Name("Tooltip".into())),
    );
}

/// Registers a regular-weight system sans as the `Tooltip` family, so black
/// text on the light tooltip keeps crisp stems; the embedded proportional font
/// remains the fallback when no system font is found.
fn install_fonts(context: &egui::Context) {
    let mut definitions = egui::FontDefinitions::default();
    let fallback = definitions
        .families
        .get(&egui::FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let mut family = fallback;
    if let Some(bytes) = crate::fonts::tooltip_font() {
        definitions.font_data.insert(
            "tooltip".to_owned(),
            std::sync::Arc::new(egui::FontData::from_owned(bytes)),
        );
        family.insert(0, "tooltip".to_owned());
    }
    definitions
        .families
        .insert(egui::FontFamily::Name("Tooltip".into()), family);
    context.set_fonts(definitions);
}

/// What one path row reported in a frame.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct PathRow {
    lost_focus: bool,
    chosen: bool,
}

/// Lays out a label, a text field and a Choose button with a fixed label and
/// button width, so every path row in a section aligns.
fn path_row(
    ui: &mut egui::Ui,
    label: &str,
    label_tooltip: &str,
    value: &mut String,
    field_tooltip: &str,
    choose_tooltip: &str,
) -> PathRow {
    let label_width = 100.0;
    let choose_width = 78.0;
    let spacing = ui.spacing().item_spacing.x;
    let height = ui.spacing().interact_size.y;
    let field_width = (ui.available_width() - label_width - choose_width - spacing).max(80.0);
    let mut report = PathRow::default();
    ui.horizontal(|ui| {
        let label_response = with_tooltip(ui.label(label), label_tooltip);
        let label_padding = (label_width - label_response.rect.width() - spacing).max(0.0);
        ui.add_space(label_padding);
        let field = ui.add(egui::TextEdit::singleline(value).desired_width(field_width));
        report.lost_focus = field.lost_focus();
        with_tooltip(field, field_tooltip);
        let choose = ui.add_sized([choose_width, height], egui::Button::new("Choose"));
        let choose = with_tooltip(choose, choose_tooltip);
        report.chosen = choose.clicked();
    });
    report
}

/// An explicit path from a text field; blank means automatic resolution.
fn explicit_path(value: &str) -> Option<PathBuf> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
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

fn run_export(request: ExportRequest) -> Result<Completed, String> {
    let ExportRequest {
        input,
        source_root,
        output,
        format,
        confidential_file,
        template,
    } = request;
    let store = match confidential_file {
        Some(path) => {
            let roots = [source_root.as_path()];
            confidential::loader::load(Some(&path), &roots).map_err(|error| error.to_string())?
        }
        None => ConfidentialStore::default(),
    };
    let job = DocumentJob::new(Source::File(input), source_root)
        .with_template(template)
        .with_global_template(loader::global_template_path());
    pipeline::export(&job, format, &output, &store)
        .map(|outcome| Completed {
            output: outcome.output().to_path_buf(),
            warnings: outcome
                .warnings()
                .iter()
                .map(|warning| store.redact(warning))
                .collect(),
        })
        .map_err(|error| store.redact(&error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session_with_input(input: &str) -> Session {
        let mut session = Session::new();
        assert!(session.accept_path(PathBuf::from(input)));
        session
    }

    #[test]
    fn a_markdown_file_enables_export() {
        let session = session_with_input("docs/A.md");

        assert!(session.can_export());
        assert_eq!(session.input, Some(PathBuf::from("docs/A.md")));
        assert_eq!(session.output_directory, "docs");
    }

    #[test]
    fn the_output_directory_follows_a_new_markdown_selection() {
        let mut session = session_with_input("docs/A.md");

        assert!(session.accept_path(PathBuf::from("other/B.md")));

        assert_eq!(session.output_directory, "other");
    }

    #[test]
    fn a_bare_file_name_uses_the_current_directory() {
        let session = session_with_input("A.md");

        assert_eq!(session.source_root(), Some(PathBuf::from(".")));
        assert_eq!(session.output_directory, ".");
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
        let mut session = session_with_input("docs/A.md");
        session.output_directory.clear();

        assert_eq!(
            session.output_path(Format::Pdf),
            Some(PathBuf::from("docs/A.pdf"))
        );
    }

    #[test]
    fn the_output_directory_receives_the_file_name() {
        let mut session = session_with_input("docs/A.md");
        session.output_directory = "out".to_owned();

        assert_eq!(
            session.output_path(Format::Odt),
            Some(PathBuf::from("out/A.odt"))
        );
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
}
