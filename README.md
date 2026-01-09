# mdcopy

A CLI tool that converts Markdown to clipboard with multiple formats (plain text, HTML, and RTF), enabling rich-text pasting into applications like email clients, word processors, and note-taking apps.

## Why mdcopy?

When you copy Markdown text and paste it into applications like Gmail, Notion, or Word, you typically get the raw Markdown syntax rather than formatted text. mdcopy solves this by:

- Converting Markdown to rich text and copying it to your clipboard in three formats simultaneously (plain text, HTML, RTF)
- Allowing you to paste formatted content into virtually any application
- Embedding images directly in the clipboard so they paste inline
- Syntax highlighting code blocks with customizable themes

## Installation

### Homebrew (macOS)

```bash
brew install tarqd/tap/mdcopy
```

### Cargo (from source)

```bash
cargo install --path .
```

## Usage

```bash
# Read from stdin
echo "# Hello World" | mdcopy

# Read from file
mdcopy -i document.md

# Output to file instead of clipboard
mdcopy -i document.md -o output.html

# Output to stdout
mdcopy -i document.md -o -
```

## CLI Options

| Option | Description |
|--------|-------------|
| `-i, --input <FILE>` | Input file (use `-` for stdin, default: stdin) |
| `-o, --output <FILE>` | Output to file instead of clipboard (use `-` for stdout) |
| `-r, --root <DIR>` | Base directory for resolving relative image paths |
| `-e, --embed <MODE>` | Image embedding mode: `all`, `local` (default), `none` |
| `-c, --config <FILE>` | Path to configuration file |
| `--strict` | Fail on errors instead of graceful fallback |
| `-v, --verbose` | Increase logging verbosity (`-v`, `-vv`, `-vvv`) |
| `-q, --quiet` | Suppress all output except errors |

### Syntax Highlighting Options

| Option | Description |
|--------|-------------|
| `--highlight` | Enable/disable syntax highlighting (default: enabled) |
| `--highlight-theme <NAME>` | Theme to use (default: `base16-ocean.dark`) |
| `--highlight-themes-dir <DIR>` | Custom themes directory |
| `--highlight-syntaxes-dir <DIR>` | Custom syntaxes directory |
| `--list-themes` | List available themes and exit |

## Features

### Markdown Support

Supports GitHub Flavored Markdown (GFM) including:
- Headings, paragraphs, and text formatting (bold, italic, strikethrough)
- Code blocks with syntax highlighting and inline code
- Ordered and unordered lists
- Blockquotes and horizontal rules
- Links and images
- Tables with column alignment

### Syntax Highlighting

Code blocks are syntax highlighted using the [syntect](https://github.com/trishume/syntect) library with `base16-ocean.dark` as the default theme.

- Supports 50+ programming languages out of the box
- Use `--list-themes` to see available themes
- Add custom themes (`.tmTheme` files) to `~/.config/mdcopy/themes/`
- Add custom syntax definitions to `~/.config/mdcopy/syntaxes/`
- Configure language aliases (e.g., map `jsx` to `JavaScript`)

### Image Embedding

Images can be embedded as base64 data URLs in HTML output and hex-encoded data in RTF output.

**Embedding modes (`--embed`):**
- `local` (default): Embed only local/relative images
- `all`: Embed both local and remote images (fetches remote images)
- `none`: Don't embed any images, keep original URLs

**RTF limitations:** Only PNG and JPEG images can be embedded in RTF. Other formats fall back to hyperlinks.

### Multi-Format Clipboard

When outputting to clipboard (default), mdcopy sets three formats simultaneously:
- **Plain text**: Original Markdown source
- **HTML**: Rendered HTML with embedded images and syntax highlighting
- **RTF**: Rich Text Format for applications that don't support HTML paste

This allows pasting into virtually any application with appropriate formatting.

## Configuration

mdcopy supports a TOML configuration file at `~/.config/mdcopy/config.toml` (or `~/Library/Application Support/mdcopy/config.toml` on macOS).

Configuration precedence: CLI arguments > environment variables > config file > defaults

### Example Configuration

```toml
# Default settings
embed = "local"
strict = false

[highlight]
enable = true
theme = "base16-ocean.dark"

# Custom language mappings
[highlight.languages]
jsx = "JavaScript"
tsx = "TypeScript"
```

### Environment Variables

All settings can be configured via environment variables with the `MDCOPY_` prefix:

- `MDCOPY_INPUT` - Input file path
- `MDCOPY_OUTPUT` - Output file path
- `MDCOPY_ROOT` - Base directory for images
- `MDCOPY_EMBED` - Embedding mode (all, local, none)
- `MDCOPY_STRICT` - Strict mode (true/false)
- `MDCOPY_HIGHLIGHT` - Enable highlighting (true/false)
- `MDCOPY_HIGHLIGHT_THEME` - Theme name
- `MDCOPY_HIGHLIGHT_THEMES_DIR` - Custom themes directory
- `MDCOPY_HIGHLIGHT_SYNTAXES_DIR` - Custom syntaxes directory

## Examples

```bash
# Convert README and copy to clipboard
mdcopy -i README.md

# Embed all images including remote ones
mdcopy -i doc.md --embed all

# Skip image embedding entirely
mdcopy -i doc.md --embed none

# Set custom base directory for relative image paths
mdcopy -i doc.md --root /path/to/images

# Use a specific syntax highlighting theme
mdcopy -i doc.md --highlight-theme "Solarized (dark)"

# List all available themes
mdcopy --list-themes

# Disable syntax highlighting
mdcopy -i doc.md --highlight=false

# Fail on missing images instead of warning
mdcopy -i doc.md --strict

# Debug output
mdcopy -i doc.md -vv
```

## License

MIT
