//! Tests for the command-line pipeline.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_pageformer")
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("examples")
        .join("escritorio")
        .join(name)
}

fn default_template() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("default.pageformer.yaml")
}

fn run(arguments: &[&OsStr]) -> Output {
    Command::new(binary())
        .args(arguments)
        .output()
        .expect("the binary must run")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("UTF-8 stdout")
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("UTF-8 stderr")
}

fn write_markdown(path: &Path, title: &str) {
    fs::write(path, format!("# {title}\n\nText.\n")).expect("the Markdown must be written");
}

fn write_private(path: &Path, contents: &str) {
    fs::write(path, contents).expect("the confidential file must be written");
    make_private(path);
}

#[cfg(unix)]
fn make_private(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .expect("the permissions must change");
}

#[cfg(not(unix))]
fn make_private(_path: &Path) {}

fn run_with_environment(arguments: &[&OsStr], variables: &[(&str, &str)]) -> Output {
    let mut command = Command::new(binary());
    command.args(arguments);
    for (key, value) in variables {
        command.env(key, value);
    }
    command.output().expect("the binary must run")
}

fn confidential_project() -> (TempDir, TempDir, PathBuf) {
    let project = TempDir::new().expect("a temporary project");
    let source = TempDir::new().expect("a temporary source");
    let values = project.path().join("values.ini");
    write_private(&values, "NOME = Fulano de Tal\nCPF = 000.000.000-00\n");
    fs::write(
        source.path().join("Doc.md"),
        "---\ntitle: Documento\nlanguage: pt-BR\n---\n\n# Documento\n\nNome: {NOME}; CPF: {CPF}.\n",
    )
    .expect("the document must be written");
    (project, source, values)
}

fn example_confidential_file() -> (TempDir, PathBuf) {
    let directory = TempDir::new().expect("a temporary directory");
    let path = directory.path().join("embargos.ini");
    write_private(
        &path,
        include_str!("../assets/examples/confidencial/embargos.ini"),
    );
    (directory, path)
}

#[test]
fn a_confidential_check_lists_names_only() {
    let (project, source, values) = confidential_project();
    let _project = project;
    let output = run(&[
        OsStr::new("--check"),
        OsStr::new("--color=never"),
        OsStr::new("--confidential-file"),
        values.as_os_str(),
        source.path().join("Doc.md").as_os_str(),
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    let report = stdout(&output);
    assert!(
        report.contains("confidential fields: CPF, NOME"),
        "{report}"
    );
    assert!(!report.contains("Fulano"), "{report}");
    assert!(!stderr(&output).contains("Fulano"), "{}", stderr(&output));
}

#[test]
fn the_example_embargos_renders_with_its_confidential_file() {
    let (_values_directory, values) = example_confidential_file();
    let output = TempDir::new().expect("a temporary output");
    let result = run(&[
        OsStr::new("--color=never"),
        OsStr::new("--confidential-file"),
        values.as_os_str(),
        OsStr::new("--output"),
        output.path().as_os_str(),
        fixture("Embargos.md").as_os_str(),
    ]);
    assert!(result.status.success(), "{}", stderr(&result));
    let pdf = output.path().join("Embargos.pdf");
    let bytes = fs::read(&pdf).expect("the PDF must exist");
    assert!(bytes.starts_with(b"%PDF-"));
    assert!(
        !stdout(&result).contains("55.444.333"),
        "{}",
        stdout(&result)
    );
    assert!(
        !stderr(&result).contains("55.444.333"),
        "{}",
        stderr(&result)
    );
}

#[test]
fn an_unknown_confidential_field_names_the_field_and_location() {
    let (project, source, values) = confidential_project();
    let _project = project;
    fs::write(
        source.path().join("Doc.md"),
        "# Documento\n\nNome: {DESCONHECIDO}.\n",
    )
    .expect("the document must be written");
    let output = run(&[
        OsStr::new("--check"),
        OsStr::new("--color=never"),
        OsStr::new("--confidential-file"),
        values.as_os_str(),
        source.path().join("Doc.md").as_os_str(),
    ]);
    assert!(!output.status.success());
    let errors = stderr(&output);
    assert!(errors.contains("{DESCONHECIDO}"), "{errors}");
    assert!(errors.contains("block 2"), "{errors}");
    assert!(!errors.contains("Fulano"), "{errors}");
}

#[test]
fn a_confidential_document_requires_an_output_outside_the_source_tree() {
    let (project, source, values) = confidential_project();
    let document = source.path().join("Doc.md");

    let default_output = run(&[
        OsStr::new("--color=never"),
        OsStr::new("--confidential-file"),
        values.as_os_str(),
        document.as_os_str(),
    ]);
    assert!(!default_output.status.success());
    assert!(
        stderr(&default_output).contains("must be written outside its source tree"),
        "{}",
        stderr(&default_output)
    );

    let inside = run(&[
        OsStr::new("--color=never"),
        OsStr::new("--confidential-file"),
        values.as_os_str(),
        OsStr::new("--output"),
        source.path().join("out").as_os_str(),
        document.as_os_str(),
    ]);
    assert!(!inside.status.success());
    assert!(
        stderr(&inside).contains("must be written outside its source tree"),
        "{}",
        stderr(&inside)
    );

    fs::create_dir_all(source.path().join("nested")).expect("the nested directory must exist");
    let escape = run(&[
        OsStr::new("--color=never"),
        OsStr::new("--confidential-file"),
        values.as_os_str(),
        OsStr::new("--output"),
        source
            .path()
            .join("nested")
            .join("..")
            .join("out")
            .as_os_str(),
        document.as_os_str(),
    ]);
    assert!(!escape.status.success());

    let output_directory = project.path().join("output");
    let success = run(&[
        OsStr::new("--color=never"),
        OsStr::new("--confidential-file"),
        values.as_os_str(),
        OsStr::new("--output"),
        output_directory.as_os_str(),
        document.as_os_str(),
    ]);
    assert!(success.status.success(), "{}", stderr(&success));
    let pdf = output_directory.join("Doc.pdf");
    let bytes = fs::read(&pdf).expect("the PDF must exist");
    assert!(bytes.starts_with(b"%PDF-"));
    let text = String::from_utf8_lossy(&bytes);
    let metadata = text
        .split_once("<x:xmpmeta")
        .and_then(|(_, rest)| rest.split_once("</x:xmpmeta>"))
        .map(|(packet, _)| packet)
        .unwrap_or_default();
    assert!(
        !metadata.contains("Fulano"),
        "the value reached the XMP metadata"
    );
    assert!(!stdout(&success).contains("Fulano"), "{}", stdout(&success));
    assert!(!stderr(&success).contains("Fulano"), "{}", stderr(&success));
}

#[test]
fn a_confidential_directory_input_needs_an_output_outside_the_tree() {
    let project = TempDir::new().expect("a temporary project");
    let source = TempDir::new().expect("a temporary source");
    fs::write(source.path().join("Doc.md"), "# Doc\n\n{NOME}.\n")
        .expect("the document must be written");
    let values = project.path().join("values.ini");
    write_private(&values, "NOME=Fulano\n");

    let output = run(&[
        OsStr::new("--color=never"),
        OsStr::new("--confidential-file"),
        values.as_os_str(),
        OsStr::new("--output"),
        source.path().join("out").as_os_str(),
        source.path().as_os_str(),
    ]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("must be written outside its source tree"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn a_confidential_file_inside_the_source_tree_is_rejected() {
    let source = TempDir::new().expect("a source");
    let values = source.path().join("values.ini");
    write_private(&values, "NOME=Fulano\n");
    write_markdown(&source.path().join("Doc.md"), "Doc");
    let output = run(&[
        OsStr::new("--check"),
        OsStr::new("--color=never"),
        OsStr::new("--confidential-file"),
        values.as_os_str(),
        source.path().join("Doc.md").as_os_str(),
    ]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("is inside the source tree"),
        "{}",
        stderr(&output)
    );
}

#[cfg(unix)]
#[test]
fn a_readable_confidential_file_is_rejected() {
    use std::os::unix::fs::PermissionsExt;

    let project = TempDir::new().expect("a project");
    let values = project.path().join("values.ini");
    fs::write(&values, "NOME=Fulano\n").expect("the file must be written");
    fs::set_permissions(&values, fs::Permissions::from_mode(0o644))
        .expect("the permissions must change");
    let output = run(&[
        OsStr::new("--check"),
        OsStr::new("--color=never"),
        OsStr::new("--confidential-file"),
        values.as_os_str(),
        fixture("Contrato.md").as_os_str(),
    ]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("chmod 600"), "{}", stderr(&output));
}

#[test]
fn confidential_values_can_come_from_the_environment() {
    let source = TempDir::new().expect("a source");
    fs::write(
        source.path().join("Doc.md"),
        "# Documento\n\nValor: {TEST_FIELD}.\n",
    )
    .expect("the document must be written");
    let output = run_with_environment(
        &[
            OsStr::new("--check"),
            OsStr::new("--color=never"),
            source.path().join("Doc.md").as_os_str(),
        ],
        &[("PAGEFORMER_CONFIDENTIAL_TEST_FIELD", "Do ambiente")],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    let report = stdout(&output);
    assert!(
        report.contains("confidential fields: TEST_FIELD"),
        "{report}"
    );
    assert!(!report.contains("Do ambiente"), "{report}");
    assert!(
        !stderr(&output).contains("Do ambiente"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn the_contract_fixture_passes_the_pipeline_check() {
    let output = run(&[OsStr::new("--check"), fixture("Contrato.md").as_os_str()]);
    assert!(
        output.status.success(),
        "the check failed: {}",
        stderr(&output)
    );
    let report = stdout(&output);
    assert!(report.contains("8 headings"), "{report}");
    assert!(report.contains(".pageformer.yaml"), "{report}");
    assert!(!report.contains("template embedded"), "{report}");
}

#[test]
fn every_input_is_reported() {
    let (_values_directory, values) = example_confidential_file();
    let output = run(&[
        OsStr::new("--check"),
        OsStr::new("--confidential-file"),
        values.as_os_str(),
        fixture("Contrato.md").as_os_str(),
        fixture("Embargos.md").as_os_str(),
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    let report = stdout(&output);
    assert!(report.contains("Contrato.md"), "{report}");
    assert!(report.contains("Embargos.md"), "{report}");
    assert!(report.contains("7 headings"), "{report}");
    assert!(report.contains("2 files checked"), "{report}");
    assert!(
        report.contains("confidential fields:"),
        "the embargos check lists fields: {report}"
    );
    assert!(!report.contains("55.444.333"), "{report}");
}

#[test]
fn the_contract_fixture_renders_a_pdf() {
    let project = TempDir::new().expect("a temporary project");
    let output = run(&[
        OsStr::new("--output"),
        project.path().as_os_str(),
        fixture("Contrato.md").as_os_str(),
    ]);
    assert!(
        output.status.success(),
        "the render failed: {}",
        stderr(&output)
    );
    let pdf = project.path().join("Contrato.pdf");
    let bytes = fs::read(&pdf).expect("the PDF must exist");
    assert!(bytes.starts_with(b"%PDF-"), "the output is not a PDF");
    assert!(bytes.len() > 10_000, "the PDF is suspiciously small");
    assert!(
        stdout(&output).contains("Contrato.md -> "),
        "{}",
        stdout(&output)
    );
}

#[test]
fn a_directory_is_walked_in_path_order_and_skips_ignored_directories() {
    let project = TempDir::new().expect("a temporary project");
    let root = project.path();
    fs::create_dir_all(root.join("sub")).expect("the subdirectory must be created");
    fs::create_dir_all(root.join(".hidden")).expect("the hidden directory must be created");
    fs::create_dir_all(root.join("target")).expect("the target directory must be created");
    write_markdown(&root.join("B.md"), "B");
    write_markdown(&root.join("A.md"), "A");
    write_markdown(&root.join("sub/C.md"), "C");
    write_markdown(&root.join(".hidden/D.md"), "D");
    write_markdown(&root.join("target/E.md"), "E");
    fs::write(root.join("notes.txt"), "not Markdown").expect("the text file must be written");

    let template = default_template();
    let output = run(&[
        OsStr::new("--check"),
        OsStr::new("--color=never"),
        OsStr::new("--template"),
        template.as_os_str(),
        root.as_os_str(),
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    let report = stdout(&output);
    let a = report.find("A.md").expect("A.md is reported");
    let b = report.find("B.md").expect("B.md is reported");
    let c = report.find("C.md").expect("C.md is reported");
    assert!(a < b && b < c, "the files are out of order: {report}");
    assert!(!report.contains("D.md"), "{report}");
    assert!(!report.contains("E.md"), "{report}");
    assert!(!report.contains("notes.txt"), "{report}");
    assert!(report.contains("3 files checked"), "{report}");
}

#[test]
fn an_empty_directory_is_a_warning_not_a_failure() {
    let project = TempDir::new().expect("a temporary project");
    let template = default_template();
    let output = run(&[
        OsStr::new("--check"),
        OsStr::new("--color=never"),
        OsStr::new("--template"),
        template.as_os_str(),
        project.path().as_os_str(),
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("no Markdown files found"),
        "{}",
        stderr(&output)
    );
    assert!(
        stdout(&output).contains("0 files checked"),
        "{}",
        stdout(&output)
    );
}

#[test]
fn one_bad_file_does_not_stop_the_batch() {
    let project = TempDir::new().expect("a temporary project");
    write_markdown(&project.path().join("good.md"), "Good");
    fs::write(project.path().join("bad.md"), "<div>raw</div>\n")
        .expect("the bad fixture must be written");
    let template = default_template();
    let output = run(&[
        OsStr::new("--check"),
        OsStr::new("--color=never"),
        OsStr::new("--template"),
        template.as_os_str(),
        project.path().as_os_str(),
    ]);
    assert!(!output.status.success());
    assert!(stdout(&output).contains("good.md"), "{}", stdout(&output));
    assert!(stderr(&output).contains("bad.md"), "{}", stderr(&output));
    assert!(
        stdout(&output).contains("1 file checked, 1 failed"),
        "{}",
        stdout(&output)
    );
}

#[test]
fn a_directory_mirrors_its_tree_under_the_output() {
    let source = TempDir::new().expect("a temporary source");
    let output = TempDir::new().expect("a temporary output");
    fs::create_dir_all(source.path().join("sub")).expect("the subdirectory must be created");
    write_markdown(&source.path().join("A.md"), "A");
    write_markdown(&source.path().join("sub/B.md"), "B");
    let template = default_template();

    let result = run(&[
        OsStr::new("--color=never"),
        OsStr::new("--template"),
        template.as_os_str(),
        OsStr::new("--output"),
        output.path().as_os_str(),
        source.path().as_os_str(),
    ]);
    assert!(result.status.success(), "{}", stderr(&result));

    let a = output.path().join("A.pdf");
    let b = output.path().join("sub/B.pdf");
    assert!(
        fs::read(&a)
            .expect("the first PDF must exist")
            .starts_with(b"%PDF-")
    );
    assert!(
        fs::read(&b)
            .expect("the nested PDF must exist")
            .starts_with(b"%PDF-")
    );
    assert!(
        stdout(&result).contains("2 files rendered"),
        "{}",
        stdout(&result)
    );
}

#[test]
fn colors_can_be_forced_and_disabled() {
    let colored = run(&[
        OsStr::new("--check"),
        OsStr::new("--color=always"),
        fixture("Contrato.md").as_os_str(),
    ]);
    assert!(colored.status.success(), "{}", stderr(&colored));
    assert!(stdout(&colored).contains('\u{1b}'), "{}", stdout(&colored));

    let colored_failure = run(&[
        OsStr::new("--check"),
        OsStr::new("--color=always"),
        OsStr::new("does-not-exist.md"),
    ]);
    assert!(!colored_failure.status.success());
    assert!(
        stderr(&colored_failure).contains('\u{1b}'),
        "{}",
        stderr(&colored_failure)
    );

    let plain = run(&[
        OsStr::new("--check"),
        OsStr::new("--color=never"),
        fixture("Contrato.md").as_os_str(),
    ]);
    assert!(plain.status.success(), "{}", stderr(&plain));
    assert!(!stdout(&plain).contains('\u{1b}'), "{}", stdout(&plain));
    assert!(!stderr(&plain).contains('\u{1b}'), "{}", stderr(&plain));

    let plain_failure = run(&[
        OsStr::new("--check"),
        OsStr::new("--color=never"),
        OsStr::new("does-not-exist.md"),
    ]);
    assert!(!plain_failure.status.success());
    assert!(
        !stderr(&plain_failure).contains('\u{1b}'),
        "{}",
        stderr(&plain_failure)
    );
}

#[test]
fn an_invalid_explicit_template_fails_with_a_diagnostic() {
    let project = TempDir::new().expect("a temporary project");
    let template = project.path().join("broken.yaml");
    fs::write(&template, "version: \"0.5\"\n").expect("the broken template must be written");

    let output = run(&[
        OsStr::new("--check"),
        OsStr::new("--template"),
        template.as_os_str(),
        fixture("Contrato.md").as_os_str(),
    ]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("unsupported template schema version 0.5"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn the_text_format_writes_a_txt_file_under_the_output() {
    let source = TempDir::new().expect("a temporary source");
    let output = TempDir::new().expect("a temporary output");
    write_markdown(&source.path().join("Doc.md"), "Doc");

    let result = run(&[
        OsStr::new("--format"),
        OsStr::new("txt"),
        OsStr::new("--color=never"),
        OsStr::new("--output"),
        output.path().as_os_str(),
        source.path().join("Doc.md").as_os_str(),
    ]);
    assert!(result.status.success(), "{}", stderr(&result));

    let text = fs::read_to_string(output.path().join("Doc.txt")).expect("the text must be written");
    assert_eq!(text, "Doc\n\nText.\n");
    assert!(!output.path().join("Doc.pdf").exists());
}

#[test]
fn an_unknown_format_fails_before_rendering() {
    let output = run(&[
        OsStr::new("--format"),
        OsStr::new("epub"),
        fixture("Contrato.md").as_os_str(),
    ]);

    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("pdf, txt, odt or docx"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn the_odt_format_writes_an_opendocument_package() {
    let source = TempDir::new().expect("a temporary source");
    let output = TempDir::new().expect("a temporary output");
    write_markdown(&source.path().join("Doc.md"), "Doc");

    let result = run(&[
        OsStr::new("--format"),
        OsStr::new("odt"),
        OsStr::new("--color=never"),
        OsStr::new("--output"),
        output.path().as_os_str(),
        source.path().join("Doc.md").as_os_str(),
    ]);
    assert!(result.status.success(), "{}", stderr(&result));

    let bytes = fs::read(output.path().join("Doc.odt")).expect("the package must be written");
    assert_eq!(&bytes[..4], b"PK\x03\x04");
    assert!(!output.path().join("Doc.pdf").exists());
}

#[test]
fn the_docx_format_writes_an_ooxml_package() {
    let source = TempDir::new().expect("a temporary source");
    let output = TempDir::new().expect("a temporary output");
    write_markdown(&source.path().join("Doc.md"), "Doc");

    let result = run(&[
        OsStr::new("--format"),
        OsStr::new("docx"),
        OsStr::new("--color=never"),
        OsStr::new("--output"),
        output.path().as_os_str(),
        source.path().join("Doc.md").as_os_str(),
    ]);
    assert!(result.status.success(), "{}", stderr(&result));

    let bytes = fs::read(output.path().join("Doc.docx")).expect("the package must be written");
    assert_eq!(&bytes[..4], b"PK\x03\x04");
    assert!(!output.path().join("Doc.pdf").exists());
}

#[test]
fn a_missing_input_file_fails() {
    let output = run(&[OsStr::new("does-not-exist.md")]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("cannot read does-not-exist.md"),
        "{}",
        stderr(&output)
    );
}
