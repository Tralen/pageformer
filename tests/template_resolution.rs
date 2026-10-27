//! Tests for template resolution across directories.

use std::fs;
use std::path::{Path, PathBuf};

use pageformer::template::PageSize;
use pageformer::template::loader::{self, TemplateOrigin};
use tempfile::TempDir;

const A4_TEMPLATE: &str = "version: 3\npage:\n  size: a4\n";
const LETTER_TEMPLATE: &str = "version: 3\npage:\n  size: letter\n";
const LEGAL_TEMPLATE: &str = "version: 3\npage:\n  size: legal\n";

fn write(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("the parent directory must be created");
    }
    fs::write(path, contents).expect("the file must be written");
}

fn local_template(directory: &Path) -> PathBuf {
    directory.join(loader::LOCAL_FILE_NAME)
}

#[test]
fn the_nearest_local_template_wins_entirely() {
    let project = TempDir::new().expect("a temporary project");
    write(&local_template(project.path()), A4_TEMPLATE);
    let docs = project.path().join("docs");
    let nested = local_template(&docs);
    write(&nested, LETTER_TEMPLATE);

    let resolved = loader::resolve(&docs, None).expect("a resolved template");
    assert_eq!(resolved.template().page().size(), PageSize::Letter);
    assert_eq!(resolved.origin(), &TemplateOrigin::Local(nested.clone()));
    assert_eq!(resolved.base_directory(), Some(docs.as_path()));
}

#[test]
fn an_ancestor_template_is_used_when_the_nearest_directory_has_none() {
    let project = TempDir::new().expect("a temporary project");
    let project_template = local_template(project.path());
    write(&project_template, LEGAL_TEMPLATE);
    let docs = project.path().join("docs");
    fs::create_dir_all(&docs).expect("the nested directory must be created");

    let resolved = loader::resolve(&docs, None).expect("a resolved template");
    assert_eq!(resolved.template().page().size(), PageSize::Legal);
    assert_eq!(resolved.origin(), &TemplateOrigin::Local(project_template));
}

#[test]
fn an_invalid_nearest_template_never_falls_back() {
    let project = TempDir::new().expect("a temporary project");
    write(&local_template(project.path()), LEGAL_TEMPLATE);
    let docs = project.path().join("docs");
    let broken = local_template(&docs);
    write(&broken, "version: 4\n");

    let error = loader::resolve(&docs, None).expect_err("the broken template must fail");
    let message = error.to_string();
    assert!(
        message.contains("unsupported template schema version 4"),
        "{message}"
    );
    assert!(
        message.contains(&broken.display().to_string()),
        "the error must name the broken template: {message}"
    );
}

#[test]
fn the_global_template_is_used_without_a_local_one() {
    let project = TempDir::new().expect("a temporary project");
    let sources = project.path().join("sources");
    fs::create_dir_all(&sources).expect("the source directory must be created");
    let global = project.path().join("config").join("pageformer.yaml");
    write(&global, LETTER_TEMPLATE);

    let resolved = loader::resolve(&sources, Some(&global)).expect("the global template must load");
    assert_eq!(resolved.template().page().size(), PageSize::Letter);
    assert_eq!(resolved.origin(), &TemplateOrigin::Global(global.clone()));
    assert_eq!(
        resolved.base_directory(),
        Some(global.parent().expect("a parent"))
    );
}

#[test]
fn the_embedded_template_is_the_last_resort() {
    let project = TempDir::new().expect("a temporary project");
    let resolved = loader::resolve(project.path(), None).expect("the embedded template");
    assert_eq!(resolved.template().page().size(), PageSize::A4);
    assert_eq!(resolved.origin(), &TemplateOrigin::Embedded);
    assert_eq!(resolved.base_directory(), None);
}

#[test]
fn a_missing_global_template_falls_back_to_the_embedded_one() {
    let project = TempDir::new().expect("a temporary project");
    let missing = project.path().join("config").join("pageformer.yaml");
    let resolved = loader::resolve(project.path(), Some(&missing)).expect("the embedded template");
    assert_eq!(resolved.origin(), &TemplateOrigin::Embedded);
}

#[test]
fn an_unreadable_global_template_is_an_error() {
    let project = TempDir::new().expect("a temporary project");
    let directory = project.path().join("config").join("pageformer.yaml");
    fs::create_dir_all(&directory).expect("a directory that is not a template");

    let error = loader::resolve(project.path(), Some(&directory))
        .expect_err("a directory is not a template");
    assert!(
        error.to_string().contains("cannot read template"),
        "{error}"
    );
}
