# mdcopy

A CLI tool that converts Markdown to clipboard with multiple formats (plain text, HTML, and RTF), enabling rich-text pasting into applications like email clients, word processors, and note-taking apps.

## Installation

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

## Options

| Option | Description |
|--------|-------------|
| `-i, --input <FILE>` | Input file (use `-` for stdin, default: stdin) |
| `-o, --output <FILE>` | Output to file instead of clipboard (use `-` for stdout) |
| `-r, --root <DIR>` | Base directory for resolving relative image paths |
| `-e, --embed <MODE>` | Image embedding mode: `all`, `local` (default), `none` |
| `--strict` | Fail on errors instead of graceful fallback |
| `-v, --verbose` | Increase logging verbosity (`-v`, `-vv`, `-vvv`) |
| `-q, --quiet` | Suppress all output except errors |

## Features

### Markdown Support

Supports GitHub Flavored Markdown (GFM) including:
- Headings, paragraphs, and text formatting (bold, italic, strikethrough)
- Code blocks with language hints and inline code
- Ordered and unordered lists
- Blockquotes and horizontal rules
- Links and images
- Tables with column alignment

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
- **HTML**: Rendered HTML with embedded images
- **RTF**: Rich Text Format for applications that don't support HTML paste

This allows pasting into virtually any application with appropriate formatting.

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

# Fail on missing images instead of warning
mdcopy -i doc.md --strict

# Debug output
mdcopy -i doc.md -vv
```

## License

MIT
