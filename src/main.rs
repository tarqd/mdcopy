mod config;
mod highlight;
mod image;
mod to_html;
mod to_markdown;
mod to_rtf;

use clap::{Parser, ValueEnum};
use clipboard_rs::{Clipboard, ClipboardContent, ClipboardContext};
use config::{CliArgs, CliHighlightArgs, CliImageArgs, Config, default_config_dir};
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

#[derive(Parser)]
#[command(name = "mdcopy")]
#[command(version)]
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

    /// Image embedding mode shorthand [possible values: all, local, none]
    #[arg(short, long, value_enum)]
    embed: Option<EmbedMode>,

    /// Embed local images (default: true)
    #[arg(long, overrides_with = "no_embed_local")]
    embed_local: bool,

    /// Don't embed local images
    #[arg(long, overrides_with = "embed_local")]
    no_embed_local: bool,

    /// Embed remote images (default: false)
    #[arg(long, overrides_with = "no_embed_remote")]
    embed_remote: bool,

    /// Don't embed remote images
    #[arg(long, overrides_with = "embed_remote")]
    no_embed_remote: bool,

    /// Enable image optimization (default: true)
    #[arg(long, overrides_with = "no_optimize")]
    optimize: bool,

    /// Disable image optimization
    #[arg(long, overrides_with = "optimize")]
    no_optimize: bool,

    /// Max image dimension in pixels (default: 1200)
    #[arg(long)]
    max_dimension: Option<u32>,

    /// Image quality 1-100 (default: 80)
    #[arg(long)]
    quality: Option<u8>,

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

    /// Output format(s): html, rtf, markdown (comma-separated for clipboard, single for file output)
    #[arg(short, long)]
    format: Option<String>,
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
    // -e/--embed is a shorthand that sets both embed_local and embed_remote
    let (embed_local_from_shorthand, embed_remote_from_shorthand) = match args.embed {
        Some(EmbedMode::All) => (Some(true), Some(true)),
        Some(EmbedMode::Local) => (Some(true), Some(false)),
        Some(EmbedMode::None) => (Some(false), Some(false)),
        None => (None, None),
    };

    // Explicit flags override the shorthand
    let embed_local = match (args.embed_local, args.no_embed_local) {
        (true, false) => Some(true),
        (false, true) => Some(false),
        _ => embed_local_from_shorthand,
    };
    let embed_remote = match (args.embed_remote, args.no_embed_remote) {
        (true, false) => Some(true),
        (false, true) => Some(false),
        _ => embed_remote_from_shorthand,
    };

    let cli_args = CliArgs {
        input: args.input,
        output: args.output.clone(),
        root: args.root,
        strict: args.strict,
        highlight: CliHighlightArgs {
            enable: args.highlight,
            theme: args.highlight_theme,
            themes_dir: args.highlight_themes_dir,
            syntaxes_dir: args.highlight_syntaxes_dir,
        },
        image: CliImageArgs {
            embed_local,
            embed_remote,
            optimize: match (args.optimize, args.no_optimize) {
                (true, false) => Some(true),
                (false, true) => Some(false),
                _ => None,
            },
            max_dimension: args.max_dimension,
            quality: args.quality,
        },
    };

    let cfg = Config::build(cli_args, args.config);

    let effective_theme = cfg.highlight.effective_theme();
    debug!("Input: {:?}", cfg.input);
    debug!("Strict mode: {}", cfg.strict);
    debug!("Syntax highlighting: {}", cfg.highlight.enable);
    debug!("Theme: {}", effective_theme);
    debug!(
        "Image: embed_local={}, embed_remote={}, optimize={} (max_dim={}, quality={})",
        cfg.image.embed_local,
        cfg.image.embed_remote,
        cfg.image.optimize,
        cfg.image.max_dimension,
        cfg.image.quality
    );

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

    // Determine formats based on output mode and explicit --format flag
    let is_file_output = cfg.output.is_some();
    let formats = match (&args.format, is_file_output) {
        // Explicit format specified
        (Some(fmt), true) => {
            let parsed = parse_formats(fmt).expect("Invalid format specification");
            if parsed.len() > 1 {
                eprintln!("Error: File output only supports a single format");
                std::process::exit(1);
            }
            parsed
        }
        (Some(fmt), false) => parse_formats(fmt).expect("Invalid format specification"),
        // No format specified - use context-aware defaults
        (None, true) => vec![ClipboardFormat::Html],
        (None, false) => vec![ClipboardFormat::Html, ClipboardFormat::Rtf],
    };

    // Create shared image cache to avoid duplicate loads across formats
    let image_cache = image::ImageCache::new();

    // Generate requested outputs
    let html_output = if formats.contains(&ClipboardFormat::Html) {
        Some(
            to_html::mdast_to_html(
                &ast,
                &base_dir,
                &cfg.image,
                cfg.strict,
                highlight_ctx.as_ref(),
                &image_cache,
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
                &cfg.image,
                cfg.strict,
                highlight_ctx.as_ref(),
                &image_cache,
            )
            .map_err(io::Error::other)?,
        )
    } else {
        None
    };

    let markdown_output = if formats.contains(&ClipboardFormat::Markdown) {
        Some(
            to_markdown::mdast_to_markdown(&ast, &base_dir, &cfg.image, cfg.strict, &image_cache)
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
        Some(ref path) if path.as_os_str() == "-" => {
            let output = match formats[0] {
                ClipboardFormat::Html => html_output.as_ref().expect("HTML output missing"),
                ClipboardFormat::Rtf => rtf_output.as_ref().expect("RTF output missing"),
                ClipboardFormat::Markdown => {
                    markdown_output.as_ref().expect("Markdown output missing")
                }
            };
            io::stdout().write_all(output.as_bytes())?;
        }
        Some(ref path) => {
            let output = match formats[0] {
                ClipboardFormat::Html => html_output.as_ref().expect("HTML output missing"),
                ClipboardFormat::Rtf => rtf_output.as_ref().expect("RTF output missing"),
                ClipboardFormat::Markdown => {
                    markdown_output.as_ref().expect("Markdown output missing")
                }
            };
            fs::write(path, output)?;
            info!("Wrote {:?} output to {:?}", formats[0], path);
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
                // Markdown with embedded images replaces plain text
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
