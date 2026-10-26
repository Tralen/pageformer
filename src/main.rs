//! Command-line entry point.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{Arg, ArgAction, Command};

use pageformer::markdown;
use pageformer::pdf;
use pageformer::template::loader::{self, ResolvedTemplate, TemplateOrigin};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Render,
    Check,
}

fn main() -> ExitCode {
    let matches = command().get_matches();
    let Some(inputs) = matches.get_many::<String>("input") else {
        eprintln!("error: no input was given");
        return ExitCode::FAILURE;
    };
    let inputs = inputs.map(PathBuf::from).collect::<Vec<_>>();
    let explicit = matches.get_one::<String>("template").map(PathBuf::from);
    let output = matches.get_one::<String>("output").map(PathBuf::from);
    let mode = if matches.get_flag("check") {
        Mode::Check
    } else {
        Mode::Render
    };
    let global = loader::global_template_path();

    let results = inputs
        .iter()
        .map(|input| {
            (
                input.as_path(),
                process(
                    input,
                    mode,
                    explicit.as_deref(),
                    output.as_deref(),
                    global.as_deref(),
                ),
            )
        })
        .collect::<Vec<_>>();
    let failures = results.iter().filter(|(_, result)| result.is_err()).count();
    for (path, result) in results {
        match result {
            Ok(outcome) => {
                for warning in outcome.warnings {
                    eprintln!("warning: {}: {warning}", path.display());
                }
                println!("{}", outcome.message);
            }
            Err(error) => eprintln!("error: {}: {error:#}", path.display()),
        }
    }
    if failures == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn command() -> Command {
    Command::new("pageformer")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Generates PDF/A documents from Markdown and a YAML template.")
        .arg(
            Arg::new("input")
                .value_name("INPUT")
                .help("Markdown file to process")
                .num_args(1..)
                .required(true)
                .value_parser(clap::value_parser!(String)),
        )
        .arg(
            Arg::new("template")
                .short('t')
                .long("template")
                .value_name("FILE")
                .help("Use this template instead of resolving one")
                .value_parser(clap::value_parser!(String)),
        )
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .value_name("DIR")
                .help("Write the PDFs to this directory")
                .conflicts_with("check")
                .value_parser(clap::value_parser!(String)),
        )
        .arg(
            Arg::new("check")
                .long("check")
                .action(ArgAction::SetTrue)
                .help("Parse the Markdown and validate the template without writing a PDF"),
        )
}

fn process(
    input: &Path,
    mode: Mode,
    explicit: Option<&Path>,
    output_directory: Option<&Path>,
    global: Option<&Path>,
) -> Result<Outcome> {
    let source =
        fs::read_to_string(input).with_context(|| format!("cannot read {}", input.display()))?;
    let document = markdown::parse(&source).with_context(|| "cannot parse the Markdown")?;
    let resolved = resolve_template(input, explicit, global)?;
    match mode {
        Mode::Check => Ok(Outcome {
            message: format!(
                "{}: {} blocks, {} headings, template {}",
                input.display(),
                document.block_count(),
                document.heading_count(),
                resolved.origin()
            ),
            warnings: Vec::new(),
        }),
        Mode::Render => {
            let source_directory = input
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            let assets = pdf::load_assets(
                &document,
                resolved.template(),
                source_directory,
                resolved.base_directory(),
            )?;
            let rendered = pdf::render(&document, resolved.template(), &assets)
                .with_context(|| "cannot render the PDF")?;
            let path = output_path(input, output_directory)?;
            write_atomic(&path, rendered.bytes())
                .with_context(|| format!("cannot write {}", path.display()))?;
            Ok(Outcome {
                message: format!("{} -> {}", input.display(), path.display()),
                warnings: rendered.warnings().to_vec(),
            })
        }
    }
}

fn resolve_template(
    input: &Path,
    explicit: Option<&Path>,
    global: Option<&Path>,
) -> Result<ResolvedTemplate> {
    match explicit {
        Some(path) => {
            let template = loader::load_file(path)?;
            Ok(ResolvedTemplate::new(
                TemplateOrigin::Explicit(path.to_owned()),
                template,
            ))
        }
        None => {
            let directory = input
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            Ok(loader::resolve(directory, global)?)
        }
    }
}

fn output_path(input: &Path, directory: Option<&Path>) -> Result<PathBuf> {
    let stem = input
        .file_stem()
        .with_context(|| format!("{} has no file name", input.display()))?;
    let mut name = OsString::from(stem);
    name.push(".pdf");
    match directory {
        Some(directory) => Ok(directory.join(name)),
        None => Ok(input.with_file_name(name)),
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("cannot create {}", parent.display()))?;
    }
    let temporary = path.with_extension("pdf.tmp");
    fs::write(&temporary, bytes)
        .with_context(|| format!("cannot write {}", temporary.display()))?;
    fs::rename(&temporary, path).with_context(|| format!("cannot replace {}", path.display()))
}

struct Outcome {
    message: String,
    warnings: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_command_requires_an_input() {
        let error = command()
            .try_get_matches_from(["pageformer"])
            .expect_err("the parser must reject a missing input");
        assert_eq!(
            error.kind(),
            clap::error::ErrorKind::MissingRequiredArgument
        );
    }

    #[test]
    fn the_explicit_template_can_be_given_before_the_input() {
        let matches = command()
            .try_get_matches_from(["pageformer", "--template", "t.yaml", "a.md"])
            .expect("valid arguments");
        assert_eq!(
            matches.get_one::<String>("template").map(String::as_str),
            Some("t.yaml")
        );
    }

    #[test]
    fn check_and_output_cannot_be_combined() {
        let error = command()
            .try_get_matches_from(["pageformer", "--check", "--output", "out", "a.md"])
            .expect_err("the options conflict");
        assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
    }

    #[test]
    fn the_output_name_replaces_the_extension() {
        let path = output_path(Path::new("docs/Contrato.md"), None).expect("an output path");
        assert_eq!(path, PathBuf::from("docs/Contrato.pdf"));
    }

    #[test]
    fn the_output_directory_is_used_when_given() {
        let path = output_path(Path::new("docs/Contrato.md"), Some(Path::new("out")))
            .expect("an output path");
        assert_eq!(path, PathBuf::from("out/Contrato.pdf"));
    }
}
