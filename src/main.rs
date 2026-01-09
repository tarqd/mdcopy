mod image;
mod to_html;
mod to_rtf;

use clap::{Parser, ValueEnum};
use clipboard_rs::{Clipboard, ClipboardContent, ClipboardContext};
use log::{debug, info, LevelFilter};
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
#[command(name = "rich-markdown-pbcopy")]
#[command(about = "Convert markdown to clipboard with text, HTML, and RTF formats")]
struct Args {
    /// Input file (use - for stdin, default: stdin)
    #[arg(short, long, default_value = "-")]
    input: PathBuf,

    /// Output to file instead of clipboard (use - for stdout)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Root directory for resolving relative image paths (default: input file's directory or cwd)
    #[arg(short, long)]
    root: Option<PathBuf>,

    /// Image embedding mode
    #[arg(short, long, value_enum, default_value = "local")]
    embed: EmbedMode,

    /// Fail on errors instead of falling back gracefully
    #[arg(long, num_args = 0..=1, default_missing_value = "true", default_value = "false")]
    strict: bool,

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

fn resolve_base_dir(input: &PathBuf, root: Option<PathBuf>) -> PathBuf {
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

    debug!("Input: {:?}", args.input);
    debug!("Embed mode: {:?}", args.embed);
    debug!("Strict mode: {}", args.strict);

    let markdown_text = read_input(&args.input)?;
    info!("Read {} bytes of markdown", markdown_text.len());

    let base_dir = resolve_base_dir(&args.input, args.root);
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

    let html_output = to_html::mdast_to_html(&ast, &base_dir, args.embed, args.strict)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    let rtf_output = to_rtf::mdast_to_rtf(&ast, &base_dir, args.embed, args.strict)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    info!("Generated HTML ({} bytes) and RTF ({} bytes)", html_output.len(), rtf_output.len());

    match args.output {
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
