// Items in this module are used by the MCP feature; suppress warnings in non-MCP builds.
#![allow(dead_code)]

use crate::config::{Config, ImageConfig, MermaidConfig};
use crate::highlight::HighlightContext;
use crate::image::ImageCache;
use crate::mermaid::MermaidCache;
use clipboard_rs::{Clipboard, ClipboardContent, ClipboardContext};
use log::{debug, info};
use markdown::mdast::Node;
use markdown::{Constructs, Options, ParseOptions};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Shared rendering context — holds expensive-to-create resources.
/// Created once and reused across multiple render calls.
pub struct RenderContext {
    pub highlight_ctx: Option<HighlightContext>,
    pub image_cache: ImageCache,
    pub mermaid_cache: MermaidCache,
}

impl RenderContext {
    /// Create a new RenderContext from a resolved Config.
    pub fn new(cfg: &Config) -> Self {
        let effective_theme = cfg.highlight.effective_theme();

        let highlight_ctx = if !cfg.highlight.enable {
            None
        } else {
            HighlightContext::new(
                effective_theme,
                &cfg.highlight.languages,
                cfg.highlight.get_themes_dir().as_ref(),
                cfg.highlight.get_syntaxes_dir().as_ref(),
            )
        };

        Self {
            highlight_ctx,
            image_cache: ImageCache::new(),
            mermaid_cache: MermaidCache::new(),
        }
    }
}

/// Parse markdown text into an AST using GFM constructs.
pub fn parse_markdown(text: &str) -> Node {
    let options = Options {
        parse: ParseOptions {
            constructs: Constructs::gfm(),
            ..Default::default()
        },
        ..Default::default()
    };
    markdown::to_mdast(text, &options.parse).expect("Failed to parse markdown")
}

/// Render markdown text to HTML.
pub fn render_to_html(
    text: &str,
    base_dir: &Path,
    image_config: &ImageConfig,
    strict: bool,
    prosemirror: bool,
    mermaid_config: &MermaidConfig,
    ctx: &RenderContext,
) -> Result<String, io::Error> {
    let ast = parse_markdown(text);
    crate::to_html::mdast_to_html(
        &ast,
        base_dir,
        image_config,
        strict,
        ctx.highlight_ctx.as_ref(),
        &ctx.image_cache,
        prosemirror,
        mermaid_config,
        &ctx.mermaid_cache,
    )
    .map_err(io::Error::other)
}

/// Render markdown text to markdown with embedded images.
pub fn render_to_markdown(
    text: &str,
    base_dir: &Path,
    image_config: &ImageConfig,
    strict: bool,
    mermaid_config: &MermaidConfig,
    ctx: &RenderContext,
) -> Result<String, io::Error> {
    let ast = parse_markdown(text);
    crate::to_markdown::mdast_to_markdown(
        &ast,
        base_dir,
        image_config,
        strict,
        &ctx.image_cache,
        mermaid_config,
        &ctx.mermaid_cache,
    )
    .map_err(io::Error::other)
}

/// Copy HTML + plain text to the system clipboard.
pub fn copy_to_clipboard(html: &str, plain_text: &str) -> Result<(), io::Error> {
    debug!("Writing to clipboard");
    let clipboard = ClipboardContext::new().map_err(|e| {
        io::Error::other(format!("Failed to create clipboard context: {}", e))
    })?;

    let contents = vec![
        ClipboardContent::Text(plain_text.to_string()),
        ClipboardContent::Html(html.to_string()),
    ];

    clipboard.set(contents).map_err(|e| {
        io::Error::other(format!("Failed to set clipboard content: {}", e))
    })?;

    info!("Copied to clipboard (HTML)");
    Ok(())
}

/// Write HTML content to a file.
pub fn write_to_file(html: &str, path: &Path) -> Result<(), io::Error> {
    fs::write(path, html)?;
    info!("Wrote HTML output to {:?}", path);
    Ok(())
}

/// Resolve the base directory for relative image paths.
pub fn resolve_base_dir(input: &Path, root: Option<PathBuf>) -> PathBuf {
    if let Some(root) = root {
        root
    } else if input.as_os_str() == "-" {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    } else {
        input
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    }
}
