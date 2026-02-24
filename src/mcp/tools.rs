use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::PathBuf;

/// Parameters for rendering markdown to clipboard.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RenderToClipboardParams {
    /// Absolute path to a markdown file. Preferred for local files —
    /// avoids reading file content into the conversation.
    #[serde(default)]
    pub file_path: Option<String>,

    /// Raw markdown text. Use for generated content or short snippets.
    #[serde(default)]
    pub text: Option<String>,

    /// Embed local images as base64. Recommended for email/docs paste.
    #[serde(default = "default_true")]
    #[schemars(default = "default_true")]
    pub embed_local: bool,

    /// Fetch and embed remote images. Enable for offline/self-contained output.
    #[serde(default)]
    pub embed_remote: bool,

    /// Resize and compress embedded images. Reduces output size.
    #[serde(default = "default_true")]
    #[schemars(default = "default_true")]
    pub optimize: bool,

    /// Root directory for resolving relative image paths.
    /// Defaults to the input file's parent directory.
    #[serde(default)]
    pub root: Option<String>,
}

fn default_true() -> bool {
    true
}

/// Resolve the markdown input from either a file_path or text parameter.
/// Returns (markdown_text, base_dir).
pub fn resolve_input(
    file_path: Option<&str>,
    text: Option<&str>,
    root: Option<&str>,
) -> Result<(String, PathBuf), io::Error> {
    let (markdown_text, inferred_base) = match (file_path, text) {
        (Some(path), _) => {
            let p = PathBuf::from(path);
            let content = fs::read_to_string(&p)?;
            let parent = p
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("."));
            (content, parent)
        }
        (None, Some(t)) => {
            let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            (t.to_string(), cwd)
        }
        (None, None) => {
            return Err(io::Error::other(
                "Either file_path or text must be provided",
            ));
        }
    };

    let base_dir = root.map(PathBuf::from).unwrap_or(inferred_base);
    Ok((markdown_text, base_dir))
}

/// Parameters for rendering a mermaid diagram to an image file.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RenderMermaidParams {
    /// Absolute path to a mermaid diagram file. Mutually exclusive with `source`.
    #[serde(default)]
    pub source_file: Option<String>,

    /// Raw mermaid diagram text. Mutually exclusive with `source_file`.
    #[serde(default)]
    pub source: Option<String>,

    /// Output file path. If not provided, a temporary file is created.
    #[serde(default)]
    pub output_file: Option<String>,

    /// Overwrite existing output file. If false and file exists, will ask user
    /// via elicitation (if supported) or return an error.
    #[serde(default)]
    pub overwrite: bool,

    /// Output format: "svg", "png", or "jpeg". Defaults to config file setting.
    #[serde(default)]
    pub format: Option<String>,

    /// Optimize rasterized output (resize/compress). Defaults to config file setting.
    #[serde(default)]
    pub optimize: Option<bool>,

    /// Max width in pixels for rasterization. Defaults to config file setting.
    #[serde(default)]
    pub max_width: Option<u32>,

    /// Max height in pixels for rasterization. Defaults to config file setting.
    #[serde(default)]
    pub max_height: Option<u32>,

    /// Quality for image compression (0-100). Defaults to config file setting.
    #[serde(default)]
    pub quality: Option<u8>,
}

/// Elicitation form for confirming file overwrite.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[schemars(description = "Confirm overwriting an existing file")]
pub struct OverwriteConfirm {
    /// Set to true to overwrite the existing file.
    pub overwrite: bool,
}

rmcp::elicit_safe!(OverwriteConfirm);
