mod config;
mod highlight;
mod image;
mod to_html;
mod to_rtf;

use clap::{Parser, ValueEnum};
use clipboard_rs::{Clipboard, ClipboardContent, ClipboardContext};
use config::{CliArgs, CliHighlightArgs, Config, ThemeMode, default_config_dir};
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

    /// Syntax highlighting theme (overrides --highlight-dark/--highlight-light)
    #[arg(long = "highlight-theme")]
    highlight_theme: Option<String>,

    /// Theme to use in dark mode
    #[arg(long = "highlight-theme-dark")]
    highlight_theme_dark: Option<String>,

    /// Theme to use in light mode
    #[arg(long = "highlight-theme-light")]
    highlight_theme_light: Option<String>,

    /// Use dark theme for syntax highlighting
    #[arg(long = "highlight-dark", conflicts_with = "highlight_light")]
    highlight_dark: bool,

    /// Use light theme for syntax highlighting
    #[arg(long = "highlight-light", conflicts_with = "highlight_dark")]
    highlight_light: bool,

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
    let theme_mode = if args.highlight_dark {
        Some(ThemeMode::Dark)
    } else if args.highlight_light {
        Some(ThemeMode::Light)
    } else {
        None
    };

    let cli_args = CliArgs {
        input: args.input,
        output: args.output.clone(),
        root: args.root,
        embed: args.embed,
        strict: args.strict,
        highlight: CliHighlightArgs {
            enable: args.highlight,
            theme: args.highlight_theme,
            theme_dark: args.highlight_theme_dark,
            theme_light: args.highlight_theme_light,
            theme_mode,
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
    debug!("Theme mode: {:?}", cfg.highlight.theme_mode);
    debug!("Effective theme: {}", effective_theme);

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

    let html_output = to_html::mdast_to_html(
        &ast,
        &base_dir,
        cfg.embed,
        cfg.strict,
        highlight_ctx.as_ref(),
    )
    .map_err(io::Error::other)?;
    let rtf_output = to_rtf::mdast_to_rtf(
        &ast,
        &base_dir,
        cfg.embed,
        cfg.strict,
        highlight_ctx.as_ref(),
    )
    .map_err(io::Error::other)?;
    info!(
        "Generated HTML ({} bytes) and RTF ({} bytes)",
        html_output.len(),
        rtf_output.len()
    );

    match cfg.output {
        Some(path) if path.as_os_str() == "-" => {
            debug!("Writing HTML to stdout");
            io::stdout().write_all(html_output.as_bytes())?;
        }
        Some(path) => {
            debug!("Writing HTML to {:?}", path);
            fs::write(&path, &html_output)?;
            info!("Wrote output to {:?}", path);
        }
        None => {
            debug!("Writing to clipboard");
            let ctx = ClipboardContext::new().expect("Failed to create clipboard context");
            ctx.set(vec![
                ClipboardContent::Text(markdown_text),
                ClipboardContent::Html(html_output),
                ClipboardContent::Rtf(rtf_output),
            ])
            .expect("Failed to set clipboard content");
            info!("Copied to clipboard (text, HTML, RTF)");
        }
    }

    Ok(())
}
