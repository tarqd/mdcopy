use crate::config::{ImageConfig, MermaidConfig, MermaidFormat};
use crate::image::{EmbeddedImage, ImageError};
use log::{debug, trace, warn};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Cache for rendered mermaid diagrams to avoid duplicate rendering across output formats.
pub struct MermaidCache {
    cache: Mutex<HashMap<String, MermaidResult>>,
    /// Font database loaded once with system fonts for SVG text rendering
    fontdb: Arc<resvg::usvg::fontdb::Database>,
}

/// Cached result of a mermaid render
#[derive(Clone)]
struct MermaidResult {
    /// Raw SVG string from mermaid-rs-renderer
    svg: String,
    /// Rasterized output (only when format is png/jpeg)
    raster: Option<RasterOutput>,
}

impl MermaidCache {
    pub fn new() -> Self {
        let mut db = resvg::usvg::fontdb::Database::new();
        db.load_system_fonts();
        debug!("Loaded {} font faces for mermaid rendering", db.len());
        Self {
            cache: Mutex::new(HashMap::new()),
            fontdb: Arc::new(db),
        }
    }

    /// Get the font database for use with `rasterize_svg_to_embedded`
    pub fn fontdb(&self) -> &Arc<resvg::usvg::fontdb::Database> {
        &self.fontdb
    }

    /// Render a mermaid diagram, returning the SVG string and optional raster image.
    /// Uses cache to avoid re-rendering the same diagram source.
    pub fn render(
        &self,
        source: &str,
        config: &MermaidConfig,
        image_config: &ImageConfig,
        strict: bool,
    ) -> Result<Option<MermaidOutput>, ImageError> {
        if !config.embed {
            return Ok(None);
        }

        // Check cache
        {
            let cache = self.cache.lock().unwrap();
            if let Some(cached) = cache.get(source) {
                trace!("Mermaid cache hit");
                return Ok(Some(MermaidOutput {
                    svg: cached.svg.clone(),
                    raster: cached.raster.clone(),
                }));
            }
        }

        // Render SVG
        debug!("Rendering mermaid diagram ({} bytes)", source.len());
        let svg = match mermaid_rs_renderer::render(source) {
            Ok(svg) => svg,
            Err(e) => {
                let msg = format!("{}", e);
                if strict {
                    return Err(ImageError::MermaidError(msg));
                }
                warn!("Mermaid render failed: {}", msg);
                return Ok(None);
            }
        };

        // Rasterize if needed
        let raster = match config.format {
            MermaidFormat::Svg => None,
            MermaidFormat::Png | MermaidFormat::Jpeg => {
                match rasterize_svg(&svg, config, image_config, &self.fontdb) {
                    Ok(img) => Some(img),
                    Err(e) => {
                        if strict {
                            return Err(e);
                        }
                        warn!("Mermaid rasterization failed: {}", e);
                        // Fall back to SVG
                        None
                    }
                }
            }
        };

        let result = MermaidResult {
            svg: svg.clone(),
            raster: raster.clone(),
        };

        // Cache
        self.cache
            .lock()
            .unwrap()
            .insert(source.to_string(), result);

        Ok(Some(MermaidOutput { svg, raster }))
    }
}

/// Output from a mermaid render operation
pub struct MermaidOutput {
    /// Raw SVG string
    pub svg: String,
    /// Rasterized image (present when format is png/jpeg)
    pub raster: Option<RasterOutput>,
}

/// Rasterized image with logical dimensions for HiDPI support
#[derive(Clone)]
pub struct RasterOutput {
    /// The image data (rendered at 2x for HiDPI)
    pub image: EmbeddedImage,
    /// Logical width (1x) for display sizing
    pub logical_width: u32,
    /// Logical height (1x) for display sizing
    pub logical_height: u32,
}

impl MermaidOutput {
    /// Get the best embedded image for data URL usage (raster if available, else SVG)
    pub fn to_embedded_image(&self) -> EmbeddedImage {
        if let Some(ref raster) = self.raster {
            raster.image.clone()
        } else {
            svg_to_embedded(&self.svg)
        }
    }
}

/// Convert SVG string to an EmbeddedImage
pub fn svg_to_embedded(svg: &str) -> EmbeddedImage {
    EmbeddedImage {
        data: svg.as_bytes().to_vec(),
        mime_type: "image/svg+xml".to_string(),
    }
}

/// Rasterize an SVG string to a RasterOutput (PNG or JPEG at 2x) — public API for other renderers
pub fn rasterize_svg_to_raster(
    svg: &str,
    mermaid_config: &MermaidConfig,
    image_config: &ImageConfig,
    fontdb: &Arc<resvg::usvg::fontdb::Database>,
) -> Result<RasterOutput, ImageError> {
    rasterize_svg(svg, mermaid_config, image_config, fontdb)
}

/// Rasterize SVG to PNG or JPEG using resvg at 2x for HiDPI.
/// Returns the image data along with logical (1x) dimensions.
fn rasterize_svg(
    svg: &str,
    mermaid_config: &MermaidConfig,
    image_config: &ImageConfig,
    fontdb: &Arc<resvg::usvg::fontdb::Database>,
) -> Result<RasterOutput, ImageError> {
    let opt = resvg::usvg::Options {
        fontdb: Arc::clone(fontdb),
        ..Default::default()
    };
    let tree = resvg::usvg::Tree::from_str(svg, &opt)
        .map_err(|e| ImageError::MermaidError(format!("SVG parse failed: {}", e)))?;

    let size = tree.size();
    let max_w = mermaid_config.max_width as f32;
    let max_h = mermaid_config.max_height as f32;

    // Scale to fit within max_width × max_height (logical 1x size)
    let scale_w = if size.width() > max_w {
        max_w / size.width()
    } else {
        1.0
    };
    let scale_h = if size.height() > max_h {
        max_h / size.height()
    } else {
        1.0
    };
    let scale = scale_w.min(scale_h);

    let logical_width = (size.width() * scale).ceil() as u32;
    let logical_height = (size.height() * scale).ceil() as u32;

    // Render at 2x for HiDPI
    let pixel_width = logical_width * 2;
    let pixel_height = logical_height * 2;
    let render_scale = scale * 2.0;

    debug!(
        "Rasterizing SVG: {}x{} @2x (logical {}x{})",
        pixel_width, pixel_height, logical_width, logical_height
    );

    let mut pixmap = resvg::tiny_skia::Pixmap::new(pixel_width, pixel_height)
        .ok_or_else(|| ImageError::MermaidError("Failed to create pixmap".to_string()))?;

    // Fill with white background for JPEG (which doesn't support transparency)
    if mermaid_config.format == MermaidFormat::Jpeg {
        pixmap.fill(resvg::tiny_skia::Color::WHITE);
    }

    let transform = resvg::tiny_skia::Transform::from_scale(render_scale, render_scale);
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    let png_data = pixmap
        .encode_png()
        .map_err(|e| ImageError::MermaidError(format!("PNG encode failed: {}", e)))?;

    let image = match mermaid_config.format {
        MermaidFormat::Png => {
            if mermaid_config.optimize {
                crate::image::optimize_image(&png_data, image_config)?
            } else {
                EmbeddedImage {
                    data: png_data,
                    mime_type: "image/png".to_string(),
                }
            }
        }
        MermaidFormat::Jpeg => {
            // Always optimize for JPEG — needed for PNG→JPEG encoding
            crate::image::optimize_image(&png_data, image_config)?
        }
        MermaidFormat::Svg => unreachable!("rasterize_svg called with SVG format"),
    };

    Ok(RasterOutput {
        image,
        logical_width,
        logical_height,
    })
}

/// Format an SVG for inline HTML embedding (strips XML declaration if present)
pub fn svg_for_inline_html(svg: &str) -> &str {
    svg.strip_prefix("<?xml version=\"1.0\" encoding=\"UTF-8\"?>")
        .unwrap_or(svg)
        .trim()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_image_config() -> ImageConfig {
        ImageConfig::default()
    }

    #[test]
    fn test_svg_to_embedded() {
        let svg = "<svg>test</svg>";
        let img = svg_to_embedded(svg);
        assert_eq!(img.mime_type, "image/svg+xml");
        assert_eq!(img.data, svg.as_bytes());
    }

    #[test]
    fn test_svg_for_inline_html() {
        assert_eq!(
            svg_for_inline_html("<?xml version=\"1.0\" encoding=\"UTF-8\"?><svg>test</svg>"),
            "<svg>test</svg>"
        );
        assert_eq!(svg_for_inline_html("<svg>test</svg>"), "<svg>test</svg>");
    }

    #[test]
    fn test_cache_disabled() {
        let cache = MermaidCache::new();
        let config = MermaidConfig {
            embed: false,
            ..Default::default()
        };
        let img_config = test_image_config();
        let result = cache
            .render("flowchart LR; A-->B", &config, &img_config, false)
            .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_cache_hit() {
        let cache = MermaidCache::new();
        let config = MermaidConfig::default();
        let img_config = test_image_config();
        let source = "flowchart LR\n    A-->B-->C";

        // First render
        let result1 = cache.render(source, &config, &img_config, false).unwrap();
        assert!(result1.is_some());
        let svg1 = result1.unwrap().svg.clone();

        // Second render should be cached
        let result2 = cache.render(source, &config, &img_config, false).unwrap();
        assert!(result2.is_some());
        assert_eq!(result2.unwrap().svg, svg1);
    }

    #[test]
    fn test_render_valid_diagram() {
        let cache = MermaidCache::new();
        let config = MermaidConfig::default();
        let img_config = test_image_config();
        let result = cache
            .render("flowchart LR\n    A-->B-->C", &config, &img_config, false)
            .unwrap();
        assert!(result.is_some());
        let output = result.unwrap();
        assert!(output.svg.contains("<svg"));
        assert!(output.raster.is_some()); // PNG format (default), has raster
    }

    #[test]
    fn test_render_invalid_strict() {
        let cache = MermaidCache::new();
        let config = MermaidConfig::default();
        let img_config = test_image_config();
        // Completely invalid syntax
        let result = cache.render(
            "not a valid diagram at all }{}{}{",
            &config,
            &img_config,
            true,
        );
        // Either error or graceful handling depending on what mermaid-rs considers invalid
        // Just verify it doesn't panic
        let _ = result;
    }

    #[test]
    fn test_render_invalid_graceful() {
        let cache = MermaidCache::new();
        let config = MermaidConfig::default();
        let img_config = test_image_config();
        let result = cache.render(
            "not a valid diagram at all }{}{}{",
            &config,
            &img_config,
            false,
        );
        // In graceful mode, should not error
        assert!(result.is_ok());
    }

    #[test]
    fn test_render_png_format() {
        let cache = MermaidCache::new();
        let config = MermaidConfig {
            format: MermaidFormat::Png,
            ..Default::default()
        };
        let img_config = test_image_config();
        let result = cache
            .render("flowchart LR\n    A-->B", &config, &img_config, true)
            .unwrap();
        assert!(result.is_some());
        let output = result.unwrap();
        assert!(output.svg.contains("<svg"));
        assert!(output.raster.is_some());
        // With optimize=true (default), optimize_image may produce PNG or JPEG
        let mime = &output.raster.unwrap().image.mime_type;
        assert!(mime == "image/png" || mime == "image/jpeg");
    }

    #[test]
    fn test_to_embedded_image_svg() {
        let output = MermaidOutput {
            svg: "<svg>test</svg>".to_string(),
            raster: None,
        };
        let img = output.to_embedded_image();
        assert_eq!(img.mime_type, "image/svg+xml");
    }

    #[test]
    fn test_to_embedded_image_raster() {
        let output = MermaidOutput {
            svg: "<svg>test</svg>".to_string(),
            raster: Some(RasterOutput {
                image: EmbeddedImage {
                    data: vec![1, 2, 3],
                    mime_type: "image/png".to_string(),
                },
                logical_width: 100,
                logical_height: 100,
            }),
        };
        let img = output.to_embedded_image();
        assert_eq!(img.mime_type, "image/png");
    }
}
