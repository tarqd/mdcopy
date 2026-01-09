use crate::EmbedMode;
use base64::{engine::general_purpose::STANDARD, Engine};
use std::fs;
use std::path::Path;

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

pub fn load_image(url: &str, base_dir: &Path, embed_mode: EmbedMode) -> Option<EmbeddedImage> {
    if embed_mode == EmbedMode::None {
        return None;
    }

    if is_data_url(url) {
        return None; // Already embedded
    }

    if is_remote_url(url) {
        if embed_mode == EmbedMode::All {
            return fetch_remote_image(url);
        }
        return None;
    }

    // Local/relative URL
    let path = base_dir.join(url);
    let data = fs::read(&path).ok()?;
    let mime_type = guess_mime_type_from_path(&path, &data);

    Some(EmbeddedImage { data, mime_type })
}

fn fetch_remote_image(url: &str) -> Option<EmbeddedImage> {
    let url = if url.starts_with("//") {
        format!("https:{}", url)
    } else {
        url.to_string()
    };

    let response = ureq::get(&url).call().ok()?;

    let mime_type = response
        .headers()
        .get("Content-Type")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.split(';').next().unwrap_or(s).trim().to_string())
        .unwrap_or_else(|| "application/octet-stream".to_string());

    let data = response.into_body().read_to_vec().ok()?;

    // Verify it's actually an image based on magic bytes
    let verified_mime = guess_mime_type_from_data(&data);
    if !verified_mime.starts_with("image/") && verified_mime != "application/octet-stream" {
        return None;
    }

    Some(EmbeddedImage {
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
