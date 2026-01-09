use crate::EmbedMode;
use base64::{engine::general_purpose::STANDARD, Engine};
use log::{debug, trace, warn};
use std::fs;
use std::path::Path;

#[derive(Debug)]
pub enum ImageError {
    NotFound(String),
    FetchFailed(String, String),
    ReadFailed(String, String),
    InvalidImage(String),
}

impl std::fmt::Display for ImageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImageError::NotFound(path) => write!(f, "Image not found: {}", path),
            ImageError::FetchFailed(url, reason) => {
                write!(f, "Failed to fetch image '{}': {}", url, reason)
            }
            ImageError::ReadFailed(path, reason) => {
                write!(f, "Failed to read image '{}': {}", path, reason)
            }
            ImageError::InvalidImage(url) => write!(f, "Invalid image data: {}", url),
        }
    }
}

impl std::error::Error for ImageError {}

#[derive(Clone)]
pub struct EmbeddedImage {
    pub data: Vec<u8>,
    pub mime_type: String,
}

pub fn is_remote_url(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://") || url.starts_with("//")
}

pub fn is_data_url(url: &str) -> bool {
    url.starts_with("data:")
}

/// Load an image, returning Ok(Some(image)) on success, Ok(None) if skipped, Err on failure
pub fn load_image(
    url: &str,
    base_dir: &Path,
    embed_mode: EmbedMode,
) -> Result<Option<EmbeddedImage>, ImageError> {
    if embed_mode == EmbedMode::None {
        trace!("Skipping image (embed mode: none): {}", url);
        return Ok(None);
    }

    if is_data_url(url) {
        trace!("Skipping data URL (already embedded)");
        return Ok(None);
    }

    if is_remote_url(url) {
        if embed_mode == EmbedMode::All {
            debug!("Fetching remote image: {}", url);
            return fetch_remote_image(url).map(Some);
        }
        trace!("Skipping remote image (embed mode: local): {}", url);
        return Ok(None);
    }

    // Local/relative URL
    let path = base_dir.join(url);
    debug!("Loading local image: {:?}", path);

    let data = fs::read(&path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            ImageError::NotFound(path.display().to_string())
        } else {
            ImageError::ReadFailed(path.display().to_string(), e.to_string())
        }
    })?;

    let mime_type = guess_mime_type_from_path(&path, &data);
    trace!("Loaded {} bytes, mime type: {}", data.len(), mime_type);

    Ok(Some(EmbeddedImage { data, mime_type }))
}

fn fetch_remote_image(url: &str) -> Result<EmbeddedImage, ImageError> {
    let url = if url.starts_with("//") {
        format!("https:{}", url)
    } else {
        url.to_string()
    };

    let response = ureq::get(&url)
        .call()
        .map_err(|e| ImageError::FetchFailed(url.clone(), e.to_string()))?;

    let status = response.status();
    trace!("HTTP {} for {}", status, url);

    let mime_type = response
        .headers()
        .get("Content-Type")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.split(';').next().unwrap_or(s).trim().to_string())
        .unwrap_or_else(|| "application/octet-stream".to_string());

    let data = response
        .into_body()
        .read_to_vec()
        .map_err(|e| ImageError::FetchFailed(url.clone(), e.to_string()))?;

    trace!("Fetched {} bytes, content-type: {}", data.len(), mime_type);

    // Verify it's actually an image based on magic bytes
    let verified_mime = guess_mime_type_from_data(&data);
    if !verified_mime.starts_with("image/") && verified_mime != "application/octet-stream" {
        return Err(ImageError::InvalidImage(url));
    }

    Ok(EmbeddedImage {
        data,
        mime_type: if mime_type.starts_with("image/") {
            mime_type
        } else {
            verified_mime
        },
    })
}

fn guess_mime_type_from_data(data: &[u8]) -> String {
    if data.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        return "image/png".to_string();
    }
    if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return "image/jpeg".to_string();
    }
    if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        return "image/gif".to_string();
    }
    if data.starts_with(b"RIFF") && data.len() > 12 && &data[8..12] == b"WEBP" {
        return "image/webp".to_string();
    }
    if data.starts_with(&[0x00, 0x00, 0x01, 0x00]) {
        return "image/x-icon".to_string();
    }
    if data.starts_with(b"BM") {
        return "image/bmp".to_string();
    }
    "application/octet-stream".to_string()
}

fn guess_mime_type_from_path(path: &Path, data: &[u8]) -> String {
    let from_data = guess_mime_type_from_data(data);
    if from_data != "application/octet-stream" {
        return from_data;
    }

    // Fall back to extension
    match path.extension().and_then(|e| e.to_str()) {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        Some("bmp") => "image/bmp",
        _ => "application/octet-stream",
    }
    .to_string()
}

impl EmbeddedImage {
    pub fn to_data_url(&self) -> String {
        let b64 = STANDARD.encode(&self.data);
        format!("data:{};base64,{}", self.mime_type, b64)
    }

    pub fn to_rtf_hex(&self) -> String {
        self.data.iter().map(|b| format!("{:02x}", b)).collect()
    }

    pub fn rtf_format(&self) -> Option<&'static str> {
        match self.mime_type.as_str() {
            "image/png" => Some("\\pngblip"),
            "image/jpeg" => Some("\\jpegblip"),
            _ => None, // RTF only supports PNG and JPEG natively
        }
    }
}

/// Helper to handle image loading with fallback/fail behavior
pub fn load_image_with_fallback(
    url: &str,
    base_dir: &Path,
    embed_mode: EmbedMode,
    fail_on_error: bool,
) -> Result<Option<EmbeddedImage>, ImageError> {
    match load_image(url, base_dir, embed_mode) {
        Ok(img) => Ok(img),
        Err(e) => {
            if fail_on_error {
                Err(e)
            } else {
                warn!("{}", e);
                Ok(None)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_is_remote_url() {
        assert!(is_remote_url("http://example.com/image.png"));
        assert!(is_remote_url("https://example.com/image.png"));
        assert!(is_remote_url("//example.com/image.png"));
        assert!(!is_remote_url("image.png"));
        assert!(!is_remote_url("./images/photo.jpg"));
        assert!(!is_remote_url("/absolute/path/image.png"));
    }

    #[test]
    fn test_is_data_url() {
        assert!(is_data_url("data:image/png;base64,iVBORw0KGgo="));
        assert!(is_data_url("data:text/plain,hello"));
        assert!(!is_data_url("http://example.com"));
        assert!(!is_data_url("image.png"));
    }

    #[test]
    fn test_guess_mime_type_from_data_png() {
        let png_header = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        assert_eq!(guess_mime_type_from_data(&png_header), "image/png");
    }

    #[test]
    fn test_guess_mime_type_from_data_jpeg() {
        let jpeg_header = [0xFF, 0xD8, 0xFF, 0xE0];
        assert_eq!(guess_mime_type_from_data(&jpeg_header), "image/jpeg");
    }

    #[test]
    fn test_guess_mime_type_from_data_gif() {
        assert_eq!(guess_mime_type_from_data(b"GIF87a..."), "image/gif");
        assert_eq!(guess_mime_type_from_data(b"GIF89a..."), "image/gif");
    }

    #[test]
    fn test_guess_mime_type_from_data_webp() {
        // WEBP format: RIFF....WEBP (12+ bytes with WEBP at offset 8)
        let webp_header = b"RIFF\x00\x00\x00\x00WEBPmore";
        assert_eq!(guess_mime_type_from_data(webp_header), "image/webp");
    }

    #[test]
    fn test_guess_mime_type_from_data_bmp() {
        assert_eq!(guess_mime_type_from_data(b"BM..."), "image/bmp");
    }

    #[test]
    fn test_guess_mime_type_from_data_unknown() {
        assert_eq!(
            guess_mime_type_from_data(b"unknown data"),
            "application/octet-stream"
        );
    }

    #[test]
    fn test_guess_mime_type_from_path_by_extension() {
        let unknown_data = b"unknown";
        assert_eq!(
            guess_mime_type_from_path(Path::new("image.png"), unknown_data),
            "image/png"
        );
        assert_eq!(
            guess_mime_type_from_path(Path::new("photo.jpg"), unknown_data),
            "image/jpeg"
        );
        assert_eq!(
            guess_mime_type_from_path(Path::new("photo.jpeg"), unknown_data),
            "image/jpeg"
        );
        assert_eq!(
            guess_mime_type_from_path(Path::new("anim.gif"), unknown_data),
            "image/gif"
        );
        assert_eq!(
            guess_mime_type_from_path(Path::new("icon.svg"), unknown_data),
            "image/svg+xml"
        );
    }

    #[test]
    fn test_guess_mime_type_prefers_magic_bytes() {
        // PNG magic bytes but .jpg extension - should detect as PNG
        let png_header = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        assert_eq!(
            guess_mime_type_from_path(Path::new("wrong.jpg"), &png_header),
            "image/png"
        );
    }

    #[test]
    fn test_embedded_image_to_data_url() {
        let img = EmbeddedImage {
            data: vec![1, 2, 3, 4],
            mime_type: "image/png".to_string(),
        };
        let data_url = img.to_data_url();
        assert!(data_url.starts_with("data:image/png;base64,"));
        assert_eq!(data_url, "data:image/png;base64,AQIDBA==");
    }

    #[test]
    fn test_embedded_image_to_rtf_hex() {
        let img = EmbeddedImage {
            data: vec![0x00, 0xFF, 0xAB, 0x12],
            mime_type: "image/png".to_string(),
        };
        assert_eq!(img.to_rtf_hex(), "00ffab12");
    }

    #[test]
    fn test_embedded_image_rtf_format() {
        let png = EmbeddedImage {
            data: vec![],
            mime_type: "image/png".to_string(),
        };
        assert_eq!(png.rtf_format(), Some("\\pngblip"));

        let jpeg = EmbeddedImage {
            data: vec![],
            mime_type: "image/jpeg".to_string(),
        };
        assert_eq!(jpeg.rtf_format(), Some("\\jpegblip"));

        let gif = EmbeddedImage {
            data: vec![],
            mime_type: "image/gif".to_string(),
        };
        assert_eq!(gif.rtf_format(), None);

        let webp = EmbeddedImage {
            data: vec![],
            mime_type: "image/webp".to_string(),
        };
        assert_eq!(webp.rtf_format(), None);
    }

    #[test]
    fn test_load_image_embed_none() {
        let result = load_image("image.png", Path::new("."), EmbedMode::None);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_load_image_data_url_skipped() {
        let result = load_image(
            "data:image/png;base64,abc",
            Path::new("."),
            EmbedMode::All,
        );
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_load_image_remote_url_skipped_in_local_mode() {
        let result = load_image(
            "https://example.com/image.png",
            Path::new("."),
            EmbedMode::Local,
        );
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_load_image_local_file() {
        let temp_dir = TempDir::new().unwrap();
        let image_path = temp_dir.path().join("test.png");

        // Write a minimal PNG header
        let mut file = std::fs::File::create(&image_path).unwrap();
        file.write_all(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A])
            .unwrap();

        let result = load_image("test.png", temp_dir.path(), EmbedMode::Local);
        assert!(result.is_ok());
        let img = result.unwrap().unwrap();
        assert_eq!(img.mime_type, "image/png");
        assert_eq!(img.data.len(), 8);
    }

    #[test]
    fn test_load_image_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let result = load_image("nonexistent.png", temp_dir.path(), EmbedMode::Local);
        assert!(matches!(result, Err(ImageError::NotFound(_))));
    }

    #[test]
    fn test_load_image_with_fallback_strict_mode() {
        let temp_dir = TempDir::new().unwrap();
        let result = load_image_with_fallback(
            "nonexistent.png",
            temp_dir.path(),
            EmbedMode::Local,
            true, // strict
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_load_image_with_fallback_graceful_mode() {
        let temp_dir = TempDir::new().unwrap();
        let result = load_image_with_fallback(
            "nonexistent.png",
            temp_dir.path(),
            EmbedMode::Local,
            false, // graceful
        );
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_image_error_display() {
        let err = ImageError::NotFound("/path/to/image.png".to_string());
        assert_eq!(err.to_string(), "Image not found: /path/to/image.png");

        let err = ImageError::FetchFailed("http://example.com".to_string(), "timeout".to_string());
        assert_eq!(
            err.to_string(),
            "Failed to fetch image 'http://example.com': timeout"
        );

        let err = ImageError::ReadFailed("/path".to_string(), "permission denied".to_string());
        assert_eq!(
            err.to_string(),
            "Failed to read image '/path': permission denied"
        );

        let err = ImageError::InvalidImage("http://example.com".to_string());
        assert_eq!(err.to_string(), "Invalid image data: http://example.com");
    }
}
