#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectedImageFormat {
    Png,
    Jpeg,
    WebP,
}

pub fn detect_image_format(bytes: &[u8]) -> Option<DetectedImageFormat> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        return Some(DetectedImageFormat::Png);
    }

    if bytes.len() >= 3 && bytes[0..3] == [0xff, 0xd8, 0xff] {
        return Some(DetectedImageFormat::Jpeg);
    }

    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some(DetectedImageFormat::WebP);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_supported_magic_bytes() {
        assert_eq!(
            detect_image_format(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]),
            Some(DetectedImageFormat::Png)
        );
        assert_eq!(
            detect_image_format(&[0xff, 0xd8, 0xff, 0xdb]),
            Some(DetectedImageFormat::Jpeg)
        );
        assert_eq!(
            detect_image_format(b"RIFF\x20\x00\x00\x00WEBP"),
            Some(DetectedImageFormat::WebP)
        );
    }

    #[test]
    fn rejects_text_with_image_extension() {
        assert_eq!(detect_image_format(b"not actually an image"), None);
    }
}
