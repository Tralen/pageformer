//! Shared user configuration paths.
//!
//! The template loader and the confidential resolver agree on one directory
//! under the user configuration root, so both read their files from the same
//! place on every platform.

use std::path::PathBuf;

/// The directory name used under the user configuration directory.
pub(crate) const DIRECTORY_NAME: &str = "pageformer";

/// The user configuration directory, or `None` when the platform provides no
/// home directory.
pub(crate) fn user_config_directory() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        let base = std::env::var_os("APPDATA").map(PathBuf::from)?;
        Some(base.join(DIRECTORY_NAME))
    }
    #[cfg(not(windows))]
    {
        if let Some(base) = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
        {
            return Some(base.join(DIRECTORY_NAME));
        }
        let home = std::env::var_os("HOME").map(PathBuf::from)?;
        Some(home.join(".config").join(DIRECTORY_NAME))
    }
}
