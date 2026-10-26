//! Command-line entry point.
//!
//! This stage of the project parses the Markdown, resolves the template and
//! reports what the pipeline found. Rendering backends plug into the document
//! model later, so the command is a pipeline check rather than a generator.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{Arg, Command};

use pageformer::markdown;
use pageformer::template::loader::{self, ResolvedTemplate, TemplateOrigin};

fn main() -> ExitCode {
    let matches = command().get_matches();
    let Some(inputs) = matches.get_many::<String>("input") else {
        eprintln!("error: no input was given");
        return ExitCode::FAILURE;
    };
    let inputs = inputs.map(PathBuf::from).collect::<Vec<_>>();
    let explicit = matches.get_one::<String>("template").map(PathBuf::from);
    let global = loader::global_template_path();

    let results = inputs
        .iter()
        .map(|input| {
            (
                input.as_path(),
                check(input, explicit.as_deref(), global.as_deref()),
            )
        })
        .collect::<Vec<_>>();
    let failures = results.iter().filter(|(_, result)| result.is_err()).count();
    for (path, result) in results {
        match result {
            Ok(report) => println!(
                "{}: {} blocks, {} headings, template {}",
                path.display(),
                report.blocks,
                report.headings,
                report.origin
            ),
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
        .about("Parses Markdown and validates the template picked for each document.")
        .arg(
            Arg::new("input")
                .value_name("INPUT")
                .help("Markdown file to check")
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
}

fn check(input: &Path, explicit: Option<&Path>, global: Option<&Path>) -> Result<Report> {
    let source = std::fs::read_to_string(input)
        .with_context(|| format!("cannot read {}", input.display()))?;
    let document = markdown::parse(&source).with_context(|| "cannot parse the Markdown")?;
    let resolved = match explicit {
        Some(path) => {
            let template = loader::load_file(path)?;
            ResolvedTemplate::new(TemplateOrigin::Explicit(path.to_owned()), template)
        }
        None => {
            let directory = input
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            loader::resolve(directory, global)?
        }
    };
    Ok(Report {
        blocks: document.block_count(),
        headings: document.heading_count(),
        origin: resolved.origin().to_string(),
    })
}

struct Report {
    blocks: usize,
    headings: usize,
    origin: String,
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
}
