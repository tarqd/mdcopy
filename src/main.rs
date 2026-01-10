mod config;
mod highlight;
mod image;
mod to_html;
mod to_rtf;

use clap::{Parser, ValueEnum};
use clipboard_rs::{Clipboard, ClipboardContent, ClipboardContext};
use config::{CliArgs, CliHighlightArgs, Config, default_config_dir};
use log::{LevelFilter, debug, info};
use markdown::{Constructs, Options, ParseOptions};
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Default, ValueEnum, PartialEq, Eq)]
pub enum EmbedMode {
    /// Embed both local and remote images
    All,
    /// Only embed local/relative images (default)
    #[default]
    Local,
    /// Don't embed any images
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardFormat {
    Html,
    Rtf,
    Markdown,
}

fn parse_formats(s: &str) -> Result<Vec<ClipboardFormat>, String> {
    let mut formats = Vec::new();
    for part in s.split(',') {
        match part.trim().to_lowercase().as_str() {
            "html" => formats.push(ClipboardFormat::Html),
            "rtf" => formats.push(ClipboardFormat::Rtf),
            "markdown" | "md" => formats.push(ClipboardFormat::Markdown),
            other => return Err(format!("Unknown format: {}", other)),
        }
    }
    if formats.is_empty() {
        return Err("At least one format must be specified".to_string());
    }
    Ok(formats)
}

/// Embed images in markdown text by replacing image URLs with data URLs
fn embed_images_in_markdown(
    markdown: &str,
    base_dir: &std::path::Path,
    embed_mode: EmbedMode,
    strict: bool,
) -> Result<String, image::ImageError> {
    use image::{is_data_url, load_image_with_fallback};

    let mut result = String::with_capacity(markdown.len());
    let mut remaining = markdown;

    // Simple regex-like pattern matching for ![alt](url)
    while let Some(start) = remaining.find("![") {
        result.push_str(&remaining[..start]);
        remaining = &remaining[start..];

        // Find the ]( part
        if let Some(bracket_end) = remaining.find("](") {
            let alt_text = &remaining[2..bracket_end];
            let after_paren = &remaining[bracket_end + 2..];

            // Find closing )
            if let Some(url_end) = after_paren.find(')') {
                let url = &after_paren[..url_end];

                // Try to embed the image
                let new_url = if is_data_url(url) {
                    url.to_string()
                } else {
                    match load_image_with_fallback(url, base_dir, embed_mode, strict)? {
                        Some(img) => img.to_data_url(),
                        None => url.to_string(),
                    }
                };

                result.push_str("![");
                result.push_str(alt_text);
                result.push_str("](");
                result.push_str(&new_url);
                result.push(')');

                remaining = &after_paren[url_end + 1..];
                continue;
            }
        }

        // Couldn't parse as image, just copy the ![ and continue
        result.push_str("![");
        remaining = &remaining[2..];
    }

    result.push_str(remaining);
    Ok(result)
}

#[derive(Parser)]
#[command(name = "mdcopy")]
#[command(about = "Convert markdown to clipboard with text, HTML, and RTF formats")]
struct Args {
    /// Input file (use - for stdin, default: stdin)
    #[arg(short, long)]
    input: Option<PathBuf>,

    /// Output to file instead of clipboard (use - for stdout)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Root directory for resolving relative image paths (default: input file's directory or cwd)
    #[arg(short, long)]
    root: Option<PathBuf>,

    /// Image embedding mode [possible values: all, local, none]
    #[arg(short, long, value_enum)]
    embed: Option<EmbedMode>,

    /// Fail on errors instead of falling back gracefully
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    strict: Option<bool>,

    /// Enable/disable syntax highlighting
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    highlight: Option<bool>,

    /// Syntax highlighting theme
    #[arg(long = "highlight-theme")]
    highlight_theme: Option<String>,

    /// Custom themes directory
    #[arg(long = "highlight-themes-dir")]
    highlight_themes_dir: Option<PathBuf>,

    /// Custom syntaxes directory
    #[arg(long = "highlight-syntaxes-dir")]
    highlight_syntaxes_dir: Option<PathBuf>,

    /// Path to configuration file
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// List available syntax highlighting themes and exit
    #[arg(long)]
    list_themes: bool,

    /// Increase logging verbosity (-v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Suppress all output except errors
    #[arg(short, long)]
    quiet: bool,

    /// Clipboard formats to include (comma-separated: html,rtf,markdown)
    #[arg(short, long, default_value = "html,rtf")]
    format: String,
}

fn init_logger(verbose: u8, quiet: bool) {
    let level = if quiet {
        LevelFilter::Error
    } else {
        match verbose {
            0 => LevelFilter::Warn,
            1 => LevelFilter::Info,
            2 => LevelFilter::Debug,
            _ => LevelFilter::Trace,
        }
    };

    env_logger::Builder::new()
        .filter_level(level)
        .format_target(false)
        .format_timestamp(None)
        .init();
}

fn read_input(path: &PathBuf) -> io::Result<String> {
    if path.as_os_str() == "-" {
        let mut content = String::new();
        io::stdin().read_to_string(&mut content)?;
        Ok(content)
    } else {
        fs::read_to_string(path)
    }
}

fn resolve_base_dir(input: &std::path::Path, root: Option<PathBuf>) -> PathBuf {
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

fn main() -> io::Result<()> {
    let args = Args::parse();
    init_logger(args.verbose, args.quiet);

    // Handle --list-themes early (before config loading)
    if args.list_themes {
        // Use provided themes dir, or fall back to default config dir
        let themes_dir = args
            .highlight_themes_dir
            .clone()
            .or_else(|| default_config_dir().map(|p| p.join("themes")));
        let themes = highlight::HighlightContext::list_themes(themes_dir.as_ref());
        println!("Available themes:");
        for theme in themes {
            println!("  {}", theme);
        }
        return Ok(());
    }

    // Build configuration from CLI args, env vars, and config file
    let cli_args = CliArgs {
        input: args.input,
        output: args.output.clone(),
        root: args.root,
        embed: args.embed,
        strict: args.strict,
        highlight: CliHighlightArgs {
            enable: args.highlight,
            theme: args.highlight_theme,
            themes_dir: args.highlight_themes_dir,
            syntaxes_dir: args.highlight_syntaxes_dir,
        },
    };

    let cfg = Config::build(cli_args, args.config);

    let effective_theme = cfg.highlight.effective_theme();
    debug!("Input: {:?}", cfg.input);
    debug!("Embed mode: {:?}", cfg.embed);
    debug!("Strict mode: {}", cfg.strict);
    debug!("Syntax highlighting: {}", cfg.highlight.enable);
    debug!("Theme: {}", effective_theme);

    let highlight_ctx = if !cfg.highlight.enable {
        None
    } else {
        highlight::HighlightContext::new(
            effective_theme,
            &cfg.highlight.languages,
            cfg.highlight.get_themes_dir().as_ref(),
            cfg.highlight.get_syntaxes_dir().as_ref(),
        )
    };

    let markdown_text = read_input(&cfg.input)?;
    info!("Read {} bytes of markdown", markdown_text.len());

    let base_dir = resolve_base_dir(&cfg.input, cfg.root);
    debug!("Base directory for images: {:?}", base_dir);

    let options = Options {
        parse: ParseOptions {
            constructs: Constructs::gfm(),
            ..Default::default()
        },
        ..Default::default()
    };

    let ast = markdown::to_mdast(&markdown_text, &options.parse).expect("Failed to parse markdown");
    debug!("Parsed markdown AST");

    let formats = parse_formats(&args.format).expect("Invalid format specification");

    // Only generate outputs for requested formats
    let html_output = if formats.contains(&ClipboardFormat::Html) || cfg.output.is_some() {
        Some(
            to_html::mdast_to_html(
                &ast,
                &base_dir,
                cfg.embed,
                cfg.strict,
                highlight_ctx.as_ref(),
            )
            .map_err(io::Error::other)?,
        )
    } else {
        None
    };

    let rtf_output = if formats.contains(&ClipboardFormat::Rtf) {
        Some(
            to_rtf::mdast_to_rtf(
                &ast,
                &base_dir,
                cfg.embed,
                cfg.strict,
                highlight_ctx.as_ref(),
            )
            .map_err(io::Error::other)?,
        )
    } else {
        None
    };

    let markdown_output = if formats.contains(&ClipboardFormat::Markdown) {
        Some(
            embed_images_in_markdown(&markdown_text, &base_dir, cfg.embed, cfg.strict)
                .map_err(io::Error::other)?,
        )
    } else {
        None
    };

    debug!(
        "Generated: HTML={}, RTF={}, Markdown={}",
        html_output.as_ref().map(|s| s.len()).unwrap_or(0),
        rtf_output.as_ref().map(|s| s.len()).unwrap_or(0),
        markdown_output.as_ref().map(|s| s.len()).unwrap_or(0),
    );

    match cfg.output {
        Some(path) if path.as_os_str() == "-" => {
            debug!("Writing HTML to stdout");
            let output = html_output
                .as_ref()
                .expect("HTML output required for stdout");
            io::stdout().write_all(output.as_bytes())?;
        }
        Some(path) => {
            debug!("Writing HTML to {:?}", path);
            let output = html_output.as_ref().expect("HTML output required for file");
            fs::write(&path, output)?;
            info!("Wrote output to {:?}", path);
        }
        None => {
            debug!("Writing to clipboard");
            let ctx = ClipboardContext::new().expect("Failed to create clipboard context");

            let mut contents = Vec::new();

            // Always include plain text (original markdown) as fallback
            contents.push(ClipboardContent::Text(markdown_text));

            if let Some(html) = html_output {
                contents.push(ClipboardContent::Html(html));
            }
            if let Some(rtf) = rtf_output {
                contents.push(ClipboardContent::Rtf(rtf));
            }
            if let Some(md) = markdown_output {
                // Markdown with embedded images goes as text if no plain text yet
                // But we already have plain text, so this replaces it
                contents[0] = ClipboardContent::Text(md);
            }

            let format_names: Vec<&str> = formats
                .iter()
                .map(|f| match f {
                    ClipboardFormat::Html => "HTML",
                    ClipboardFormat::Rtf => "RTF",
                    ClipboardFormat::Markdown => "Markdown",
                })
                .collect();

            ctx.set(contents).expect("Failed to set clipboard content");
            info!("Copied to clipboard ({})", format_names.join(", "));
        }
    }

    Ok(())
}
