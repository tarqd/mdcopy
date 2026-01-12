use crate::config::ImageConfig;
use base64::{Engine, engine::general_purpose::STANDARD};
use log::{debug, trace, warn};
use rimage::codecs::mozjpeg::{MozJpegEncoder, MozJpegOptions};
use rimage::codecs::oxipng::OxiPngEncoder;
use rimage::operations::resize::{FilterType, Resize, ResizeAlg};
use std::collections::HashMap;
use std::fs;
use std::io::{BufReader, Cursor};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tempfile::TempDir;
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;
use zune_image::image::Image;
use zune_image::traits::{EncoderTrait, OperationsTrait};

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
    image_config: &ImageConfig,
) -> Result<Option<EmbeddedImage>, ImageError> {
    // Skip if embedding is completely disabled
    if !image_config.embed_local && !image_config.embed_remote {
        trace!("Skipping image (embed disabled): {}", url);
        return Ok(None);
    }

    if is_data_url(url) {
        trace!("Skipping data URL (already embedded)");
        return Ok(None);
    }

    if is_remote_url(url) {
        if image_config.embed_remote {
            debug!("Fetching remote image: {}", url);
            return fetch_remote_image(url).map(Some);
        }
        trace!("Skipping remote image (embed_remote: false): {}", url);
        return Ok(None);
    }

    // Local/relative URL
    if !image_config.embed_local {
        trace!("Skipping local image (embed_local: false): {}", url);
        return Ok(None);
    }

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
    image_config: &ImageConfig,
    fail_on_error: bool,
) -> Result<Option<EmbeddedImage>, ImageError> {
    match load_image(url, base_dir, image_config) {
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

/// Cache for images to avoid duplicate loads/fetches/optimization.
/// Maps source URL/path to cached file path in temp directory.
pub struct ImageCache {
    /// Temp directory for cached images (cleaned up on drop)
    temp_dir: Option<TempDir>,
    /// Maps source URL/path to cached file path
    cache: Mutex<HashMap<String, PathBuf>>,
}

impl ImageCache {
    pub fn new() -> Self {
        let temp_dir = TempDir::new().ok();
        if temp_dir.is_none() {
            warn!("Failed to create temp directory for image cache");
        }
        Self {
            temp_dir,
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// Load an image, using cache to avoid duplicate work.
    /// When optimization is enabled, both local and remote images are
    /// processed and cached to temp directory.
    pub fn get_or_load(
        &self,
        url: &str,
        base_dir: &Path,
        image_config: &ImageConfig,
        strict: bool,
    ) -> Result<Option<EmbeddedImage>, ImageError> {
        // Skip if embedding is completely disabled or it's a data URL
        if (!image_config.embed_local && !image_config.embed_remote) || is_data_url(url) {
            return load_image_with_fallback(url, base_dir, image_config, strict);
        }

        // Remote images when embed_remote is false: skip
        if is_remote_url(url) && !image_config.embed_remote {
            return Ok(None);
        }

        // Local images when embed_local is false: skip
        if !is_remote_url(url) && !image_config.embed_local {
            return Ok(None);
        }

        // Check cache first
        {
            let cache = self.cache.lock().unwrap();
            if let Some(cached_path) = cache.get(url) {
                trace!("Image cache hit: {}", url);
                return load_cached_image(cached_path);
            }
        }

        // Load the original image
        let original = if is_remote_url(url) {
            self.fetch_remote(url, strict)?
        } else {
            load_image_with_fallback(url, base_dir, image_config, strict)?
        };

        // If optimization enabled for this image type, optimize and cache
        let should_optimize = if is_remote_url(url) {
            image_config.optimize_remote
        } else {
            image_config.optimize_local
        };

        if should_optimize && let Some(ref img) = original {
            return self.optimize_and_cache(url, img, image_config, strict);
        }
        Ok(original)
    }

    /// Fetch a remote image, caching the raw download
    fn fetch_remote(&self, url: &str, strict: bool) -> Result<Option<EmbeddedImage>, ImageError> {
        let temp_dir = match &self.temp_dir {
            Some(dir) => dir.path(),
            None => {
                // No temp dir, fetch directly without caching
                return match fetch_remote_image(url) {
                    Ok(img) => Ok(Some(img)),
                    Err(e) if strict => Err(e),
                    Err(e) => {
                        warn!("{}", e);
                        Ok(None)
                    }
                };
            }
        };

        trace!("Fetching remote image: {}", url);
        let filename = url_to_filename(url);
        let cached_path = temp_dir.join(&filename);

        match fetch_and_save_remote_image(url, &cached_path) {
            Ok(()) => {
                self.cache
                    .lock()
                    .unwrap()
                    .insert(url.to_string(), cached_path.clone());
                load_cached_image(&cached_path)
            }
            Err(e) if strict => Err(e),
            Err(e) => {
                warn!("{}", e);
                Ok(None)
            }
        }
    }

    /// Optimize an image and cache the result
    fn optimize_and_cache(
        &self,
        source: &str,
        img: &EmbeddedImage,
        image_config: &ImageConfig,
        strict: bool,
    ) -> Result<Option<EmbeddedImage>, ImageError> {
        match optimize_image(&img.data, image_config) {
            Ok(optimized) => {
                trace!(
                    "Optimized image: {} -> {} bytes",
                    img.data.len(),
                    optimized.data.len()
                );

                // Cache to temp file
                if let Some(temp_dir) = &self.temp_dir {
                    let filename = url_to_filename(source);
                    let cached_path = temp_dir.path().join(filename);

                    if let Err(e) = fs::write(&cached_path, &optimized.data) {
                        trace!("Failed to cache optimized image: {}", e);
                    } else {
                        self.cache
                            .lock()
                            .unwrap()
                            .insert(source.to_string(), cached_path);
                    }
                }

                Ok(Some(optimized))
            }
            Err(e) => {
                if strict {
                    warn!("Image optimization failed: {}", e);
                } else {
                    trace!("Image optimization failed, using original: {}", e);
                }
                Ok(Some(img.clone()))
            }
        }
    }
}

impl Default for ImageCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Generate a filesystem-safe filename from a URL (hash-based)
fn url_to_filename(url: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Fetch a remote image and save it to a file
fn fetch_and_save_remote_image(url: &str, dest: &Path) -> Result<(), ImageError> {
    let url = if url.starts_with("//") {
        format!("https:{}", url)
    } else {
        url.to_string()
    };

    debug!("Fetching remote image: {}", url);

    let response = ureq::get(&url)
        .call()
        .map_err(|e| ImageError::FetchFailed(url.clone(), e.to_string()))?;

    let data = response
        .into_body()
        .read_to_vec()
        .map_err(|e| ImageError::FetchFailed(url.clone(), e.to_string()))?;

    // Verify it's actually an image
    let mime = guess_mime_type_from_data(&data);
    if !mime.starts_with("image/") && mime != "application/octet-stream" {
        return Err(ImageError::InvalidImage(url));
    }

    fs::write(dest, &data)
        .map_err(|e| ImageError::ReadFailed(dest.display().to_string(), e.to_string()))?;

    trace!("Cached remote image to {:?}", dest);
    Ok(())
}

/// Load a cached remote image from temp file
fn load_cached_image(path: &Path) -> Result<Option<EmbeddedImage>, ImageError> {
    let data = fs::read(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            ImageError::NotFound(path.display().to_string())
        } else {
            ImageError::ReadFailed(path.display().to_string(), e.to_string())
        }
    })?;

    // Detect mime type from magic bytes (cached files have hash names, no extension)
    let mime_type = guess_mime_type_from_data(&data);
    Ok(Some(EmbeddedImage { data, mime_type }))
}

/// Optimize an image by resizing and compressing.
/// Returns JPEG for opaque images, PNG for images with transparency.
pub fn optimize_image(
    data: &[u8],
    image_config: &ImageConfig,
) -> Result<EmbeddedImage, ImageError> {
    // Decode image using BufReader<Cursor> which implements BufRead + Seek
    let reader = BufReader::new(Cursor::new(data));
    let mut img = Image::read(reader, DecoderOptions::default())
        .map_err(|e| ImageError::InvalidImage(format!("Failed to decode image: {:?}", e)))?;

    // Get dimensions
    let (width, height) = img.dimensions();
    let max_dim = image_config.max_dimension as usize;

    debug!(
        "Optimizing image: {}x{}, max_dim={}, quality={}",
        width, height, max_dim, image_config.quality
    );

    // Resize if needed (maintain aspect ratio)
    if width > max_dim || height > max_dim {
        let scale = max_dim as f32 / width.max(height) as f32;
        let new_width = (width as f32 * scale) as usize;
        let new_height = (height as f32 * scale) as usize;

        debug!(
            "Resizing from {}x{} to {}x{}",
            width, height, new_width, new_height
        );

        let resize = Resize::new(
            new_width,
            new_height,
            ResizeAlg::Convolution(FilterType::Lanczos3),
        );
        resize
            .execute_impl(&mut img)
            .map_err(|e| ImageError::InvalidImage(format!("Failed to resize image: {:?}", e)))?;
    }

    // Check if image has alpha channel
    let has_alpha = matches!(
        img.colorspace(),
        ColorSpace::RGBA | ColorSpace::BGRA | ColorSpace::ARGB | ColorSpace::LumaA
    );

    // Encode based on transparency
    if has_alpha {
        // PNG for transparency
        debug!("Encoding as PNG (has alpha channel)");
        let mut encoder = OxiPngEncoder::new();
        let mut result = Vec::new();
        encoder
            .encode(&img, &mut result)
            .map_err(|e| ImageError::InvalidImage(format!("Failed to encode PNG: {:?}", e)))?;
        Ok(EmbeddedImage {
            data: result,
            mime_type: "image/png".to_string(),
        })
    } else {
        // JPEG for opaque (better compression)
        debug!(
            "Encoding as JPEG (opaque, quality={})",
            image_config.quality
        );
        let options = MozJpegOptions {
            quality: image_config.quality as f32,
            ..Default::default()
        };
        let mut encoder = MozJpegEncoder::new_with_options(options);
        let mut result = Vec::new();
        encoder
            .encode(&img, &mut result)
            .map_err(|e| ImageError::InvalidImage(format!("Failed to encode JPEG: {:?}", e)))?;
        Ok(EmbeddedImage {
            data: result,
            mime_type: "image/jpeg".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    // Helper to create ImageConfig for tests
    fn config_embed_local() -> ImageConfig {
        ImageConfig {
            embed_local: true,
            embed_remote: false,
            optimize_local: false,
            optimize_remote: false,
            max_dimension: 1200,
            quality: 80,
        }
    }

    fn config_embed_all() -> ImageConfig {
        ImageConfig {
            embed_local: true,
            embed_remote: true,
            optimize_local: false,
            optimize_remote: false,
            max_dimension: 1200,
            quality: 80,
        }
    }

    fn config_embed_none() -> ImageConfig {
        ImageConfig {
            embed_local: false,
            embed_remote: false,
            optimize_local: false,
            optimize_remote: false,
            max_dimension: 1200,
            quality: 80,
        }
    }

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
        let result = load_image("image.png", Path::new("."), &config_embed_none());
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_load_image_data_url_skipped() {
        let result = load_image(
            "data:image/png;base64,abc",
            Path::new("."),
            &config_embed_all(),
        );
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_load_image_remote_url_skipped_in_local_mode() {
        let result = load_image(
            "https://example.com/image.png",
            Path::new("."),
            &config_embed_local(),
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

        let result = load_image("test.png", temp_dir.path(), &config_embed_local());
        assert!(result.is_ok());
        let img = result.unwrap().unwrap();
        assert_eq!(img.mime_type, "image/png");
        assert_eq!(img.data.len(), 8);
    }

    #[test]
    fn test_load_image_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let result = load_image("nonexistent.png", temp_dir.path(), &config_embed_local());
        assert!(matches!(result, Err(ImageError::NotFound(_))));
    }

    #[test]
    fn test_load_image_with_fallback_strict_mode() {
        let temp_dir = TempDir::new().unwrap();
        let result = load_image_with_fallback(
            "nonexistent.png",
            temp_dir.path(),
            &config_embed_local(),
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
            &config_embed_local(),
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

    #[test]
    fn test_image_cache_local_not_cached() {
        // Local images are NOT cached - they're read fresh each time
        let temp_dir = TempDir::new().unwrap();
        let image_path = temp_dir.path().join("test.png");

        // Write a minimal PNG header
        let mut file = std::fs::File::create(&image_path).unwrap();
        file.write_all(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A])
            .unwrap();

        let cache = ImageCache::new();
        let config = config_embed_local();

        // First load
        let result1 = cache.get_or_load("test.png", temp_dir.path(), &config, false);
        assert!(result1.is_ok());
        let img1 = result1.unwrap().unwrap();
        assert_eq!(img1.mime_type, "image/png");

        // Delete the file - second load should FAIL (no caching for local files)
        std::fs::remove_file(&image_path).unwrap();

        let result2 = cache.get_or_load("test.png", temp_dir.path(), &config, false);
        assert!(result2.is_ok());
        // Returns None because file not found (graceful mode)
        assert!(result2.unwrap().is_none());
    }

    #[test]
    fn test_image_cache_embed_none() {
        let cache = ImageCache::new();

        // Load with embed disabled returns None
        let result = cache.get_or_load("test.png", Path::new("."), &config_embed_none(), false);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_url_to_filename() {
        let f1 = url_to_filename("https://example.com/image.png");
        let f2 = url_to_filename("https://example.com/photo.jpg");

        // Same URL produces same filename
        let f3 = url_to_filename("https://example.com/image.png");
        assert_eq!(f1, f3);

        // Different URLs produce different filenames
        assert_ne!(f1, f2);

        // Filename is a 16-char hex hash
        assert_eq!(f1.len(), 16);
        assert!(f1.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
