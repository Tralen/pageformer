//! The tooltip font: a regular-weight system sans, so black text on a light
//! background keeps crisp stems at small sizes.

/// Candidate regular sans fonts on Linux, in preference order. Liberation Sans
/// is metrically close to Arial, so the tooltip keeps its line breaks.
const LINUX_FONTS: &[&str] = &[
    "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
    "/usr/share/fonts/TTF/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/freefont/FreeSans.ttf",
];

/// Candidate regular sans fonts on Windows.
const WINDOWS_FONTS: &[&str] = &[
    "C:\\Windows\\Fonts\\segoeui.ttf",
    "C:\\Windows\\Fonts\\arial.ttf",
];

/// Candidate regular sans fonts on macOS.
const MACOS_FONTS: &[&str] = &[
    "/System/Library/Fonts/Supplemental/Arial.ttf",
    "/Library/Fonts/Arial.ttf",
];

/// Reads the first available system sans font, or `None` when none of the
/// known paths exists.
pub fn tooltip_font() -> Option<Vec<u8>> {
    candidates()
        .iter()
        .find_map(|path| std::fs::read(path).ok())
}

fn candidates() -> &'static [&'static str] {
    if cfg!(target_os = "windows") {
        WINDOWS_FONTS
    } else if cfg!(target_os = "macos") {
        MACOS_FONTS
    } else {
        LINUX_FONTS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_candidates_are_not_empty() {
        assert!(!candidates().is_empty());
    }
}
