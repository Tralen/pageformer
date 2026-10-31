//! Intrinsic image sizes and the shared image layout model.

use serde::Deserialize;

/// A width or height requested for an image.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ImageDimension {
    /// An absolute length in typographic points.
    Points(f64),
    /// A percentage of the available text width, from 0 to 100.
    Percent(f64),
}

impl ImageDimension {
    /// Builds a positive, finite absolute length.
    pub fn from_points(points: f64) -> Option<Self> {
        if points.is_finite() && points > 0.0 {
            Some(Self::Points(points))
        } else {
            None
        }
    }

    /// Builds a percentage in the inclusive range 0 to 100.
    pub fn from_percent(percent: f64) -> Option<Self> {
        if percent.is_finite() && percent > 0.0 && percent <= 100.0 {
            Some(Self::Percent(percent))
        } else {
            None
        }
    }

    pub fn points(self) -> Option<f64> {
        match self {
            Self::Points(points) => Some(points),
            Self::Percent(_) => None,
        }
    }

    pub fn percent(self) -> Option<f64> {
        match self {
            Self::Percent(percent) => Some(percent),
            Self::Points(_) => None,
        }
    }
}

/// The requested size of a block image.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ImageSize {
    /// The intrinsic size, capped to the text width.
    Auto,
    /// A width, with the height derived from the intrinsic ratio.
    Width(ImageDimension),
    /// A height, with the width derived from the intrinsic ratio.
    Height(ImageDimension),
}

/// The horizontal alignment of a block image.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageAlignment {
    Left,
    #[default]
    Center,
    Right,
}

/// The resolved size of an image in points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImageLayout {
    width: f64,
    height: f64,
    capped: bool,
}

impl ImageLayout {
    pub fn width(self) -> f64 {
        self.width
    }

    pub fn height(self) -> f64 {
        self.height
    }

    /// Whether the requested size had to be reduced to the available width.
    pub fn is_capped(self) -> bool {
        self.capped
    }
}

/// Resolves a requested image size against its intrinsic ratio, the width
/// available for the image and the frame padding. An oversized request is
/// reduced to the width left inside the frame.
pub fn layout(
    intrinsic: (f64, f64),
    available_width: f64,
    size: ImageSize,
    max_width: ImageDimension,
    padding: f64,
) -> ImageLayout {
    let (intrinsic_width, intrinsic_height) = valid_ratio(intrinsic);
    let ratio = intrinsic_width / intrinsic_height;
    let max_width = match max_width {
        ImageDimension::Points(points) => points.min(available_width),
        ImageDimension::Percent(percent) => available_width * percent / 100.0,
    };
    let content_limit = (available_width - 2.0 * padding.max(0.0)).max(1.0);
    let limit = max_width.min(content_limit).max(f64::MIN_POSITIVE);

    let resolve = |dimension: ImageDimension| match dimension {
        ImageDimension::Points(points) => points,
        ImageDimension::Percent(percent) => available_width * percent / 100.0,
    };
    let (width, capped) = match size {
        ImageSize::Auto => (intrinsic_width, intrinsic_width > limit),
        ImageSize::Width(dimension) => {
            let requested = resolve(dimension);
            (requested, requested > limit)
        }
        ImageSize::Height(dimension) => {
            let height = resolve(dimension);
            let requested = height * ratio;
            (requested, requested > limit)
        }
    };
    let width = width.min(limit);
    let height = width / ratio;
    ImageLayout {
        width,
        height,
        capped,
    }
}

fn valid_ratio(intrinsic: (f64, f64)) -> (f64, f64) {
    let (width, height) = intrinsic;
    if width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0 {
        (width, height)
    } else {
        (300.0, 150.0)
    }
}

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

/// The intrinsic size of an image in points, without a cap.
pub fn intrinsic(bytes: &[u8]) -> (f64, f64) {
    let (pixels_width, pixels_height) = pixels(bytes).unwrap_or((400.0, 200.0));
    valid_ratio((pixels_width * 0.75, pixels_height * 0.75))
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

    #[test]
    fn an_automatic_size_keeps_the_ratio_and_caps_the_width() {
        let layout = layout(
            (400.0, 200.0),
            100.0,
            ImageSize::Auto,
            ImageDimension::Percent(100.0),
            0.0,
        );

        assert_eq!(layout.width(), 100.0);
        assert_eq!(layout.height(), 50.0);
        assert!(layout.is_capped());
    }

    #[test]
    fn an_explicit_width_wins_and_an_explicit_height_derives_the_width() {
        let width = layout(
            (400.0, 200.0),
            200.0,
            ImageSize::Width(ImageDimension::Percent(50.0)),
            ImageDimension::Percent(100.0),
            0.0,
        );
        assert_eq!(width.width(), 100.0);
        assert_eq!(width.height(), 50.0);
        assert!(!width.is_capped());

        let height = layout(
            (400.0, 200.0),
            200.0,
            ImageSize::Height(ImageDimension::Points(40.0)),
            ImageDimension::Percent(100.0),
            0.0,
        );
        assert_eq!(height.width(), 80.0);
        assert_eq!(height.height(), 40.0);
        assert!(!height.is_capped());
    }

    #[test]
    fn an_explicit_size_larger_than_the_limit_is_capped() {
        let layout = layout(
            (400.0, 200.0),
            100.0,
            ImageSize::Width(ImageDimension::Points(200.0)),
            ImageDimension::Percent(100.0),
            0.0,
        );

        assert_eq!(layout.width(), 100.0);
        assert_eq!(layout.height(), 50.0);
        assert!(layout.is_capped());
    }

    #[test]
    fn a_point_max_width_is_applied_to_the_intrinsic_size() {
        let layout = layout(
            (400.0, 200.0),
            300.0,
            ImageSize::Auto,
            ImageDimension::Points(120.0),
            0.0,
        );

        assert_eq!(layout.width(), 120.0);
        assert_eq!(layout.height(), 60.0);
        assert!(layout.is_capped());
    }

    #[test]
    fn an_image_without_a_ratio_falls_back_to_a_landscape_box() {
        let layout = layout(
            (0.0, 0.0),
            300.0,
            ImageSize::Auto,
            ImageDimension::Percent(100.0),
            0.0,
        );

        assert_eq!(layout.width(), 300.0);
        assert_eq!(layout.height(), 150.0);
    }

    #[test]
    fn the_frame_padding_reduces_the_available_content_width() {
        let layout = layout(
            (400.0, 200.0),
            100.0,
            ImageSize::Width(ImageDimension::Percent(100.0)),
            ImageDimension::Percent(100.0),
            10.0,
        );

        assert_eq!(layout.width(), 80.0);
        assert_eq!(layout.height(), 40.0);
        assert!(layout.is_capped());
    }
}
