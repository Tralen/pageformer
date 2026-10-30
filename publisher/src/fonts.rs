//! The embedded UI font: Liberation Sans Regular, so every proportional label,
//! button and tooltip uses the same regular-weight face on every platform.

/// Liberation Sans Regular, licensed under the SIL Open Font License 1.1; the
/// notice and the license text are in `publisher/assets/fonts/OFL.txt`.
pub const UI_FONT: &[u8] = include_bytes!("../assets/fonts/LiberationSans-Regular.ttf");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_font_is_a_true_type_file() {
        assert!(UI_FONT.starts_with(&[0x00, 0x01, 0x00, 0x00]));
    }
}
