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
    state: State,
}

impl Session {
    fn new() -> Self {
        Self {
            input: None,
            output_directory: String::new(),
            confidential_file: String::new(),
            state: State::Idle,
        }
    }

    fn can_export(&self) -> bool {
        self.input.is_some() && !matches!(self.state, State::Working { .. })
    }

    /// Accepts a Markdown file or a confidential INI file.
    fn accept_path(&mut self, path: PathBuf) -> bool {
        match path
            .extension()
            .and_then(OsStr::to_str)
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("md") => {
                self.input = Some(path);
                true
            }
            Some("ini") => {
                self.confidential_file = path.display().to_string();
                true
            }
            _ => false,
        }
    }

    fn source_root(&self) -> Option<PathBuf> {
        self.input.as_ref().map(|input| {
            input
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf()
        })
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
        let confidential_file = if self.confidential_file.trim().is_empty() {
            None
        } else {
            Some(PathBuf::from(self.confidential_file.trim()))
        };
        self.state = State::Working { format };
        Some(ExportRequest {
            input,
            source_root,
            output,
            format,
            confidential_file,
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
    input_field: String,
    template_label: String,
    template_path: Option<PathBuf>,
    message: Option<String>,
    receiver: Option<Receiver<Result<Completed, String>>>,
}

impl Publisher {
    pub fn new() -> Self {
        Self {
            session: Session::new(),
            input_field: String::new(),
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
        let is_markdown = path
            .extension()
            .and_then(OsStr::to_str)
            .is_some_and(|extension| extension.eq_ignore_ascii_case("md"));
        if is_markdown {
            self.input_field = path.display().to_string();
            self.refresh_template();
        }
        self.message = None;
    }

    fn accept_input_field(&mut self) {
        let path = PathBuf::from(self.input_field.trim());
        if path.as_os_str().is_empty() {
            self.message = Some("Enter a Markdown path or drop a file".to_owned());
            return;
        }
        self.accept_input(path);
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

    fn render(&mut self, ui: &mut egui::Ui) {
        ui.heading("Pageformer Publisher");
        ui.label("Convert a Markdown file with its Pageformer template.");
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label("Markdown:");
            ui.text_edit_singleline(&mut self.input_field);
            if ui.button("Use path").clicked() {
                self.accept_input_field();
            }
        });
        if let Some(input) = self.session.input.clone() {
            ui.label(format!("Selected: {}", input.display()));
        }

        ui.horizontal(|ui| {
            ui.label("Template:");
            ui.label(&self.template_label);
            if let Some(path) = self.template_path.clone() {
                if ui.button("Open").clicked() {
                    self.open(&path);
                }
                if let Some(folder) = path.parent() {
                    let folder = folder.to_path_buf();
                    if ui.button("Open folder").clicked() {
                        self.open(&folder);
                    }
                }
            }
        });

        ui.horizontal(|ui| {
            ui.label("Output directory:");
            ui.text_edit_singleline(&mut self.session.output_directory);
        });
        ui.label("Leave it empty to write beside the source; a document with {NAME} fields needs a directory outside its source tree.");

        ui.horizontal(|ui| {
            ui.label("Confidential file:");
            ui.text_edit_singleline(&mut self.session.confidential_file);
        });

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label("Export:");
            let enabled = self.session.can_export();
            for format in Format::ALL {
                if ui
                    .add_enabled(enabled, egui::Button::new(format.name().to_uppercase()))
                    .clicked()
                {
                    self.start(format);
                }
            }
        });

        ui.add_space(8.0);
        match self.session.state.clone() {
            State::Idle => {
                ui.label("Drop a Markdown file here or paste a path.");
            }
            State::Working { format } => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(format!("Rendering {}...", format.name()));
                });
            }
            State::Succeeded { output, warnings } => {
                ui.colored_label(
                    egui::Color32::from_rgb(0x2e, 0x7d, 0x32),
                    format!("Wrote {}", output.display()),
                );
                ui.horizontal(|ui| {
                    if ui.button("Open output").clicked() {
                        self.open(&output);
                    }
                    if let Some(folder) = output.parent() {
                        let folder = folder.to_path_buf();
                        if ui.button("Open folder").clicked() {
                            self.open(&folder);
                        }
                    }
                });
                for warning in warnings {
                    ui.label(warning);
                }
            }
            State::Failed { message } => {
                ui.colored_label(egui::Color32::from_rgb(0xb0, 0x2a, 0x2a), message);
            }
        }

        if let Some(message) = &self.message {
            ui.add_space(4.0);
            ui.colored_label(egui::Color32::from_rgb(0xb0, 0x2a, 0x2a), message);
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
        egui::CentralPanel::default().show(ui, |ui| {
            self.render(ui);
        });
    }
}

fn run_export(request: ExportRequest) -> Result<Completed, String> {
    let ExportRequest {
        input,
        source_root,
        output,
        format,
        confidential_file,
    } = request;
    let store = match confidential_file {
        Some(path) => {
            let roots = [source_root.as_path()];
            confidential::loader::load(Some(&path), &roots).map_err(|error| error.to_string())?
        }
        None => ConfidentialStore::default(),
    };
    let job = DocumentJob::new(Source::File(input), source_root)
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
        let session = session_with_input("docs/A.md");

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
    }

    #[test]
    fn the_source_root_is_the_input_parent() {
        let session = session_with_input("docs/A.md");

        assert_eq!(session.source_root(), Some(PathBuf::from("docs")));
    }
}
