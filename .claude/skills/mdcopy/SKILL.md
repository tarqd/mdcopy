---
name: mdcopy
description: Copy markdown as rich text to clipboard with HTML and RTF formats. Use when the user wants to copy markdown content to clipboard, convert markdown to rich text, or paste formatted markdown into applications like Word, Google Docs, email, or Slack.
allowed-tools:
  - Bash
  - Read
  - Write
---

# mdcopy - Markdown to Rich Clipboard

Copy markdown content to the clipboard as rich text (HTML, RTF) for pasting into any application.

## Quick Start

Copy a markdown file to clipboard:
```bash
mdcopy -i README.md
```

Copy from stdin:
```bash
echo "# Hello **World**" | mdcopy
```

## Common Use Cases

### Copy markdown with syntax highlighting
```bash
mdcopy -i document.md -h
```

### Copy with embedded images
```bash
mdcopy -i document.md -e
```

### Output HTML to stdout instead of clipboard
```bash
mdcopy -i document.md -o - -f html
```

### Save to file
```bash
mdcopy -i document.md -o output.html -f html
```

## Key Options

| Option | Description |
|--------|-------------|
| `-i, --input <FILE>` | Input file (use `-` for stdin, default: stdin) |
| `-o, --output <FILE>` | Output to file instead of clipboard (use `-` for stdout) |
| `-f, --format <FMT>` | Output format: `html`, `rtf`, `markdown`, `native` (macOS) |
| `-h, --highlight` | Enable syntax highlighting for code blocks |
| `-t, --highlight-theme <THEME>` | Set syntax highlighting theme |
| `-e, --embed` | Embed all images (local and remote) as base64 |
| `--embed-local` | Embed only local images |
| `--embed-remote` | Embed only remote images |
| `-z, --optimize` | Optimize embedded images (reduces size) |
| `--max-dimension <PX>` | Max image dimension when optimizing (default: 1200) |
| `--quality <1-100>` | Image quality when optimizing (default: 80) |
| `-s, --strict` | Fail on errors instead of graceful fallback |
| `-r, --root <DIR>` | Root directory for resolving relative image paths |
| `--list-themes` | List available syntax highlighting themes |
| `--show-config` | Show current configuration as TOML |

## Negation Flags

Most boolean flags have negation variants:
- `-H, --no-highlight` - Disable syntax highlighting
- `-E, --no-embed` - Disable all image embedding
- `-S, --no-strict` - Disable strict mode

## Output Formats

By default, mdcopy writes both HTML and RTF to the clipboard, with plain text as fallback. Applications automatically choose the best format.

- **HTML**: Best for web-based apps (Google Docs, Gmail, Slack)
- **RTF**: Best for desktop apps (Microsoft Word, Apple Mail, TextEdit)
- **Native** (macOS only): Uses NSAttributedString for best native app compatibility
- **Markdown**: Outputs markdown with embedded images

Multiple formats for clipboard:
```bash
mdcopy -i doc.md -f html,rtf,markdown
```

## Configuration

mdcopy loads settings from `~/.config/mdcopy/config.toml`:

```toml
[highlight]
enable = true
theme = "base16-ocean.dark"

[image]
embed_local = true
embed_remote = false
optimize_local = true
max_dimension = 1200
quality = 80
```

Settings precedence: CLI args > Environment vars (`MDCOPY_*`) > Config file > Defaults

## Examples for Common Workflows

### Prepare documentation for sharing
```bash
# Copy README with embedded images and syntax highlighting
mdcopy -i README.md -h -e
```

### Generate HTML for email
```bash
# Optimize images for smaller email size
mdcopy -i newsletter.md -h -e -z --quality 70 -o - -f html
```

### Convert markdown note to rich text
```bash
# Pipe from another command
cat notes/*.md | mdcopy -h
```

### Use a specific theme for code
```bash
mdcopy -i code-samples.md -h -t "Solarized (dark)"
```
