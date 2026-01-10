# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

```bash
cargo build              # Build debug
cargo build --release    # Build release
cargo run -- [args]      # Run with arguments
cargo test               # Run all tests
cargo test config::      # Run tests in config module
cargo test test_name     # Run a specific test
cargo clippy             # Run linter
cargo fmt                # Format code
```

## Architecture

mdcopy is a Rust CLI tool that converts Markdown to clipboard with three formats (plain text, HTML, RTF). It uses the `markdown` crate to parse Markdown into an AST, then renders that AST to HTML and RTF.

### Module Structure

- `main.rs` - CLI argument parsing (clap), config merging, orchestration
- `config.rs` - Configuration system with precedence: CLI > env vars > config file > defaults
- `to_html.rs` - Converts markdown AST to HTML with syntax highlighting
- `to_rtf.rs` - Converts markdown AST to RTF with syntax highlighting and color tables
- `image.rs` - Image loading (local/remote), embedding as base64/hex, MIME type detection
- `highlight.rs` - Syntax highlighting via syntect, custom theme/syntax loading

### Key Dependencies

- `markdown` - Parses GFM markdown to AST (`markdown::to_mdast`)
- `syntect` - Syntax highlighting for code blocks
- `clipboard-rs` - Multi-format clipboard access (text, HTML, RTF)
- `ureq` - HTTP client for fetching remote images

### Data Flow

1. Input: Read markdown from file or stdin
2. Parse: `markdown::to_mdast()` produces AST
3. Render: `to_html::mdast_to_html()` and `to_rtf::mdast_to_rtf()` traverse AST
4. Output: Write to clipboard (3 formats) or file (HTML only)

### Configuration

Config is loaded in `Config::build()` which merges sources in order:
1. Defaults (defined in `impl Default`)
2. Config file (`~/.config/mdcopy/config.toml`)
3. Environment variables (`MDCOPY_*`)
4. CLI arguments

### Image Embedding

`EmbedMode` controls image handling:
- `All`: Embed local + fetch remote images
- `Local`: Embed local only, keep remote URLs
- `None`: Keep all original URLs

Images are converted to base64 data URLs for HTML and hex-encoded for RTF. Only PNG/JPEG work in RTF; others fall back to hyperlinks.

### Syntax Highlighting

`HighlightContext` wraps syntect's theme and syntax sets. Language aliases (e.g., `js` -> `JavaScript`) are configured in `highlight.languages`. Custom themes (`.tmTheme`) and syntaxes go in `~/.config/mdcopy/themes/` and `syntaxes/`.
