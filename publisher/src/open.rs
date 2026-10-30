//! Opens a file or folder with the platform handler.

use std::io;
use std::path::Path;
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
