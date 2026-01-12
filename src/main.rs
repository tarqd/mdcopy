mod config;
mod highlight;
mod image;
mod to_html;
mod to_markdown;
#[cfg(target_os = "macos")]
mod to_nsattributedstring;
mod to_rtf;

use clap::Parser;
use clipboard_rs::{Clipboard, ClipboardContent, ClipboardContext};
use config::{CliArgs, CliHighlightArgs, CliImageArgs, Config, default_config_dir};
use log::{LevelFilter, debug, info};
use markdown::{Constructs, Options, ParseOptions};
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardFormat {
    Html,
    Rtf,
    Markdown,
    #[cfg(target_os = "macos")]
    Native,
}

fn parse_formats(s: &str) -> Result<Vec<ClipboardFormat>, String> {
    let mut formats = Vec::new();
    for part in s.split(',') {
        match part.trim().to_lowercase().as_str() {
            "html" => formats.push(ClipboardFormat::Html),
            "rtf" => formats.push(ClipboardFormat::Rtf),
            "markdown" | "md" => formats.push(ClipboardFormat::Markdown),
            #[cfg(target_os = "macos")]
            "native" | "nsattributedstring" => formats.push(ClipboardFormat::Native),
            #[cfg(not(target_os = "macos"))]
            "native" | "nsattributedstring" => {
                return Err("Native format is only available on macOS".to_string());
            }
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
#[command(disable_help_flag = true)]
struct Args {
    /// Print help (includes current settings with sources)
    #[arg(long, action = clap::ArgAction::SetTrue)]
    help: bool,
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
    #[arg(short = 's', long, overrides_with = "no_strict")]
    strict: bool,

    #[arg(short = 'S', long, overrides_with = "strict", hide = true)]
    no_strict: bool,

    /// Enable syntax highlighting
    #[arg(short = 'h', long, overrides_with = "no_highlight")]
    highlight: bool,

    #[arg(short = 'H', long, overrides_with = "highlight", hide = true)]
    no_highlight: bool,

    /// Syntax highlighting theme
    #[arg(short = 't', long = "highlight-theme")]
    highlight_theme: Option<String>,

    /// Custom themes directory
    #[arg(long = "highlight-themes-dir")]
    highlight_themes_dir: Option<PathBuf>,

    /// Custom syntaxes directory
    #[arg(short = 'x', long = "highlight-syntaxes-dir")]
    highlight_syntaxes_dir: Option<PathBuf>,

    /// Embed all images (sets both local and remote)
    #[arg(short = 'e', long, overrides_with_all = ["no_embed", "embed_local", "no_embed_local", "embed_remote", "no_embed_remote"])]
    embed: bool,

    #[arg(short = 'E', long, overrides_with_all = ["embed", "embed_local", "no_embed_local", "embed_remote", "no_embed_remote"], hide = true)]
    no_embed: bool,

    /// Embed local images
    #[arg(long, overrides_with_all = ["no_embed_local", "embed", "no_embed"])]
    embed_local: bool,

    #[arg(long, overrides_with_all = ["embed_local", "embed", "no_embed"], hide = true)]
    no_embed_local: bool,

    /// Embed remote images
    #[arg(long, overrides_with_all = ["no_embed_remote", "embed", "no_embed"])]
    embed_remote: bool,

    #[arg(long, overrides_with_all = ["embed_remote", "embed", "no_embed"], hide = true)]
    no_embed_remote: bool,

    /// Optimize all images (sets both local and remote)
    #[arg(short = 'z', long, overrides_with_all = ["no_optimize", "optimize_local", "no_optimize_local", "optimize_remote", "no_optimize_remote"])]
    optimize: bool,

    #[arg(short = 'Z', long, overrides_with_all = ["optimize", "optimize_local", "no_optimize_local", "optimize_remote", "no_optimize_remote"], hide = true)]
    no_optimize: bool,

    /// Optimize local images
    #[arg(long, overrides_with_all = ["no_optimize_local", "optimize", "no_optimize"])]
    optimize_local: bool,

    #[arg(long, overrides_with_all = ["optimize_local", "optimize", "no_optimize"], hide = true)]
    no_optimize_local: bool,

    /// Optimize remote images
    #[arg(long, overrides_with_all = ["no_optimize_remote", "optimize", "no_optimize"])]
    optimize_remote: bool,

    #[arg(long, overrides_with_all = ["optimize_remote", "optimize", "no_optimize"], hide = true)]
    no_optimize_remote: bool,

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

    /// Show current configuration as TOML and exit
    #[arg(long)]
    show_config: bool,

    /// Increase logging verbosity (-v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Suppress all output except errors
    #[arg(short, long)]
    quiet: bool,

    /// Output format(s): html, rtf, markdown, native (comma-separated for clipboard, single for file output)
    ///
    /// Native format (macOS only) uses NSAttributedString for best clipboard compatibility
    /// with native apps like TextEdit, Notes, Mail. Native is clipboard-only.
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
    // --embed / --no-embed are shorthands that set both embed_local and embed_remote
    let (embed_local_base, embed_remote_base) = match (args.embed, args.no_embed) {
        (true, false) => (Some(true), Some(true)),
        (false, true) => (Some(false), Some(false)),
        _ => (None, None),
    };

    // Explicit flags override --embed / --no-embed shorthands
    let embed_local = match (args.embed_local, args.no_embed_local) {
        (true, false) => Some(true),
        (false, true) => Some(false),
        _ => embed_local_base,
    };
    let embed_remote = match (args.embed_remote, args.no_embed_remote) {
        (true, false) => Some(true),
        (false, true) => Some(false),
        _ => embed_remote_base,
    };

    // --optimize / --no-optimize are shorthands that set both optimize_local and optimize_remote
    let (optimize_local_base, optimize_remote_base) = match (args.optimize, args.no_optimize) {
        (true, false) => (Some(true), Some(true)),
        (false, true) => (Some(false), Some(false)),
        _ => (None, None),
    };

    // Explicit flags override --optimize / --no-optimize shorthands
    let optimize_local = match (args.optimize_local, args.no_optimize_local) {
        (true, false) => Some(true),
        (false, true) => Some(false),
        _ => optimize_local_base,
    };
    let optimize_remote = match (args.optimize_remote, args.no_optimize_remote) {
        (true, false) => Some(true),
        (false, true) => Some(false),
        _ => optimize_remote_base,
    };

    // --strict / --no-strict
    let strict = match (args.strict, args.no_strict) {
        (true, false) => Some(true),
        (false, true) => Some(false),
        _ => None,
    };

    // --highlight / --no-highlight
    let highlight = match (args.highlight, args.no_highlight) {
        (true, false) => Some(true),
        (false, true) => Some(false),
        _ => None,
    };

    let cli_args = CliArgs {
        input: args.input,
        output: args.output.clone(),
        root: args.root,
        strict,
        highlight: CliHighlightArgs {
            enable: highlight,
            theme: args.highlight_theme,
            themes_dir: args.highlight_themes_dir,
            syntaxes_dir: args.highlight_syntaxes_dir,
        },
        image: CliImageArgs {
            embed_local,
            embed_remote,
            optimize_local,
            optimize_remote,
            max_dimension: args.max_dimension,
            quality: args.quality,
        },
    };

    let (cfg, sources) = Config::build(cli_args, args.config);

    // Handle --help (after config loading so we can show current settings)
    if args.help {
        use clap::CommandFactory;
        let mut cmd = Args::command();
        let help = cmd.render_help().to_string();
        // Reformat flags to show --[no-]flag style with both short codes
        let help = help
            .replace("--embed-local", "--[no-]embed-local")
            .replace("--embed-remote", "--[no-]embed-remote")
            .replace("--optimize-local", "--[no-]optimize-local")
            .replace("--optimize-remote", "--[no-]optimize-remote")
            .replace("-e, --embed", "-e, -E, --[no-]embed")
            .replace("-z, --optimize", "-z, -Z, --[no-]optimize")
            .replace("-s, --strict", "-s, -S, --[no-]strict")
            .replace("-h, --highlight", "-h, -H, --[no-]highlight");
        println!("{help}");
        println!("\nCurrent settings:");
        println!("{}", sources.format_settings(&cfg));
        return Ok(());
    }

    // Handle --show-config
    if args.show_config {
        println!("{}", cfg.to_toml());
        return Ok(());
    }

    let effective_theme = cfg.highlight.effective_theme();
    debug!("Input: {:?}", cfg.input);
    debug!("Strict mode: {}", cfg.strict);
    debug!("Syntax highlighting: {}", cfg.highlight.enable);
    debug!("Theme: {}", effective_theme);
    debug!(
        "Image: embed_local={}, embed_remote={}, optimize_local={}, optimize_remote={} (max_dim={}, quality={})",
        cfg.image.embed_local,
        cfg.image.embed_remote,
        cfg.image.optimize_local,
        cfg.image.optimize_remote,
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
            #[cfg(target_os = "macos")]
            if parsed.contains(&ClipboardFormat::Native) {
                eprintln!("Error: Native format is only supported for clipboard output");
                std::process::exit(1);
            }
            parsed
        }
        (Some(fmt), false) => parse_formats(fmt).expect("Invalid format specification"),
        // No format specified - use context-aware defaults
        (None, true) => vec![ClipboardFormat::Html],
        (None, false) => vec![ClipboardFormat::Html, ClipboardFormat::Rtf],
    };

    // Warn if optimize is enabled but embedding is disabled (optimization requires embedding)
    if cfg.image.optimize_local && !cfg.image.embed_local {
        log::warn!(
            "Local image optimization is disabled. Reason: optimization requires embedding. \
             Use --embed-local to enable"
        );
    }
    if cfg.image.optimize_remote && !cfg.image.embed_remote {
        log::warn!(
            "Remote image optimization is disabled. Reason: optimization requires embedding. \
             Use --embed-remote to enable"
        );
    }

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

    #[cfg(target_os = "macos")]
    let native_output = if formats.contains(&ClipboardFormat::Native) {
        Some(
            to_nsattributedstring::mdast_to_nsattributed_string(
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

    #[cfg(target_os = "macos")]
    debug!(
        "Generated: HTML={}, RTF={}, Markdown={}, Native={}",
        html_output.as_ref().map(|s| s.len()).unwrap_or(0),
        rtf_output.as_ref().map(|s| s.len()).unwrap_or(0),
        markdown_output.as_ref().map(|s| s.len()).unwrap_or(0),
        native_output.is_some(),
    );

    #[cfg(not(target_os = "macos"))]
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
                #[cfg(target_os = "macos")]
                ClipboardFormat::Native => {
                    unreachable!("Native format is clipboard-only")
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
                #[cfg(target_os = "macos")]
                ClipboardFormat::Native => {
                    unreachable!("Native format is clipboard-only")
                }
            };
            fs::write(path, output)?;
            info!("Wrote {:?} output to {:?}", formats[0], path);
        }
        None => {
            debug!("Writing to clipboard");

            #[cfg(target_os = "macos")]
            let use_native = formats.contains(&ClipboardFormat::Native);

            #[cfg(target_os = "macos")]
            if use_native {
                // Use native NSAttributedString clipboard on macOS
                let native_result = native_output.as_ref().expect("Native output missing");

                // If -f native,html was specified, use our HTML generator
                let use_our_html = formats.contains(&ClipboardFormat::Html);

                // Pass markdown if -f native,markdown was specified
                let text_for_pasteboard = if formats.contains(&ClipboardFormat::Markdown) {
                    markdown_output.as_deref().or(Some(&markdown_text))
                } else {
                    None
                };

                to_nsattributedstring::write_to_pasteboard(
                    native_result,
                    use_our_html,
                    html_output.as_deref(),
                    text_for_pasteboard,
                )
                .expect("Failed to write NSAttributedString to pasteboard");

                let format_names: Vec<&str> = formats
                    .iter()
                    .map(|f| match f {
                        ClipboardFormat::Html => "HTML",
                        ClipboardFormat::Rtf => "RTF",
                        ClipboardFormat::Markdown => "Markdown",
                        ClipboardFormat::Native => "Native",
                    })
                    .collect();
                info!("Copied to clipboard ({})", format_names.join(", "));
            } else {
                // Use clipboard-rs for non-native formats
                let ctx = ClipboardContext::new().expect("Failed to create clipboard context");

                let mut contents = Vec::new();

                // Always include plain text (original markdown) as fallback
                contents.push(ClipboardContent::Text(markdown_text.clone()));

                if let Some(ref html) = html_output {
                    contents.push(ClipboardContent::Html(html.clone()));
                }
                if let Some(ref rtf) = rtf_output {
                    contents.push(ClipboardContent::Rtf(rtf.clone()));
                }
                if let Some(ref md) = markdown_output {
                    // Markdown with embedded images replaces plain text
                    contents[0] = ClipboardContent::Text(md.clone());
                }

                let format_names: Vec<&str> = formats
                    .iter()
                    .map(|f| match f {
                        ClipboardFormat::Html => "HTML",
                        ClipboardFormat::Rtf => "RTF",
                        ClipboardFormat::Markdown => "Markdown",
                        ClipboardFormat::Native => "Native",
                    })
                    .collect();

                ctx.set(contents).expect("Failed to set clipboard content");
                info!("Copied to clipboard ({})", format_names.join(", "));
            }

            #[cfg(not(target_os = "macos"))]
            {
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
    }

    Ok(())
}
