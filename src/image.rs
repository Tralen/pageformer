//! Intrinsic image sizes shared by the backends.

/// The display size of an image in points, capped to the given width.
pub(crate) fn dimensions(bytes: &[u8], max_width: f64) -> (f64, f64) {
    let (pixels_width, pixels_height) = pixels(bytes).unwrap_or((400.0, 200.0));
    let mut width = pixels_width * 0.75;
    let mut height = pixels_height * 0.75;
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return (max_width * 0.5, max_width * 0.25);
    }
    if width > max_width {
        let scale = max_width / width;
        width = max_width;
        height *= scale;
    }
    (width, height)
}

/// The pixel dimensions of a PNG, GIF or JPEG image.
pub(crate) fn pixels(bytes: &[u8]) -> Option<(f64, f64)> {
    png_size(bytes)
        .or_else(|| gif_size(bytes))
        .or_else(|| jpeg_size(bytes))
}

fn png_size(bytes: &[u8]) -> Option<(f64, f64)> {
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return None;
    }
    let width = be_u32(bytes, 16)?;
    let height = be_u32(bytes, 20)?;
    Some((f64::from(width), f64::from(height)))
}

fn gif_size(bytes: &[u8]) -> Option<(f64, f64)> {
    if !bytes.starts_with(b"GIF87a") && !bytes.starts_with(b"GIF89a") {
        return None;
    }
    let width = le_u16(bytes, 6)?;
    let height = le_u16(bytes, 8)?;
    Some((f64::from(width), f64::from(height)))
}

fn jpeg_size(bytes: &[u8]) -> Option<(f64, f64)> {
    if !bytes.starts_with(&[0xff, 0xd8]) {
        return None;
    }
    let mut index = 2_usize;
    while index + 4 <= bytes.len() {
        if *bytes.get(index)? != 0xff {
            index = index.checked_add(1)?;
            continue;
        }
        let marker = *bytes.get(index.checked_add(1)?)?;
        if marker == 0xd8 || marker == 0xff {
            index = index.checked_add(2)?;
            continue;
        }
        let length = usize::from(be_u16(bytes, index.checked_add(2)?)?);
        if length < 2 {
            return None;
        }
        let is_start_of_frame =
            (0xc0..=0xcf).contains(&marker) && marker != 0xc4 && marker != 0xc8 && marker != 0xcc;
        if is_start_of_frame {
            let height = be_u16(bytes, index.checked_add(5)?)?;
            let width = be_u16(bytes, index.checked_add(7)?)?;
            return Some((f64::from(width), f64::from(height)));
        }
        index = index.checked_add(2)?.checked_add(length)?;
    }
    None
}

fn be_u16(bytes: &[u8], start: usize) -> Option<u16> {
    let end = start.checked_add(2)?;
    let slice = bytes.get(start..end)?;
    let array: [u8; 2] = slice.try_into().ok()?;
    Some(u16::from_be_bytes(array))
}

fn be_u32(bytes: &[u8], start: usize) -> Option<u32> {
    let end = start.checked_add(4)?;
    let slice = bytes.get(start..end)?;
    let array: [u8; 4] = slice.try_into().ok()?;
    Some(u32::from_be_bytes(array))
}

fn le_u16(bytes: &[u8], start: usize) -> Option<u16> {
    let end = start.checked_add(2)?;
    let slice = bytes.get(start..end)?;
    let array: [u8; 2] = slice.try_into().ok()?;
    Some(u16::from_le_bytes(array))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] =
        b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x10\x00\x00\x00\x08\x08\x06\x00\x00\x00";

    #[test]
    fn a_png_size_is_read_from_the_header() {
        assert_eq!(png_size(PNG), Some((16.0, 8.0)));
    }

    #[test]
    fn an_unknown_image_falls_back_to_a_ratio() {
        let sizes = dimensions(b"not an image", 400.0);

        assert!(sizes.0 > 0.0 && sizes.1 > 0.0);
        assert!(sizes.0 <= 400.0);
    }

    #[test]
    fn a_wide_image_is_capped_to_the_text_width() {
        let (width, height) = dimensions(PNG, 6.0);

        assert_eq!(width, 6.0);
        assert_eq!(height, 3.0);
    }
}
