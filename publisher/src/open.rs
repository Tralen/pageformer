//! Desktop integration: open paths and choose files.

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Opens a file or folder with the default application.
pub fn open_path(path: &Path) -> io::Result<()> {
    let mut command = command();
    command.arg(path);
    command.spawn().map(|_| ())
}

fn command() -> Command {
    if cfg!(target_os = "windows") {
        Command::new("explorer")
    } else if cfg!(target_os = "macos") {
        Command::new("open")
    } else {
        Command::new("xdg-open")
    }
}

/// Chooses a file with the platform dialog.
///
/// On Linux the dialog is `zenity`, which works on minimal window managers
/// without a desktop portal; on Windows and macOS it is the native `rfd`
/// dialog.
#[cfg(target_os = "linux")]
pub fn choose_file(title: &str, filter_name: &str, extension: &str) -> Option<PathBuf> {
    let filter = format!("{filter_name} | *.{extension}");
    let output = Command::new("zenity")
        .arg("--file-selection")
        .arg(format!("--title={title}"))
        .arg(format!("--file-filter={filter}"))
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    (!path.is_empty()).then(|| PathBuf::from(path))
}

/// Chooses a file with the native platform dialog.
#[cfg(not(target_os = "linux"))]
pub fn choose_file(title: &str, filter_name: &str, extension: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title(title)
        .add_filter(filter_name, &[extension])
        .pick_file()
}

/// Chooses a folder with the platform dialog.
#[cfg(target_os = "linux")]
pub fn choose_folder(title: &str) -> Option<PathBuf> {
    let output = Command::new("zenity")
        .arg("--file-selection")
        .arg("--directory")
        .arg(format!("--title={title}"))
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    (!path.is_empty()).then(|| PathBuf::from(path))
}

/// Chooses a folder with the native platform dialog.
#[cfg(not(target_os = "linux"))]
pub fn choose_folder(title: &str) -> Option<PathBuf> {
    rfd::FileDialog::new().set_title(title).pick_folder()
}
