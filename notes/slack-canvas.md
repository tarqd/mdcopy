# Slack Canvas Clipboard Paste Format

## Code Block Recognition

Slack Canvas recognizes code blocks from HTML paste but **does not support syntax highlighting/language selection**.

### Minimum Required HTML

```html
<pre><code>console.log('hello')</code></pre>
```

**Required:**
- `<code>` wrapper inside `<pre>`

### What Doesn't Work
- `<pre>` without `<code>` wrapper - pastes as plain text, not code block
- `<table>` - tables not supported via paste (ignored entirely)
- Images may not paste reliably

### Notes

- Slack Canvas outputs only `<pre>content</pre>` when copying (no `<code>` wrapper)
- But requires `<pre><code>content</code></pre>` when pasting
- No language/syntax highlighting support in Slack Canvas code blocks

## Native Clipboard Format

Slack uses `org.chromium.web-custom-data` with multiple MIME types encoded as UTF-16LE with length-prefixed strings.

### MIME Types

#### `application/vnd.quip.sections+pblite`
Internal Quip-based format (Slack acquired Quip). Contains structured document data in a protobuf-lite style nested array format.

Structure is a nested array with positional fields (not named keys):
```
[null, [[null, "temp:C:...", 45000, 0, null, null, null, null, "sectionId", 0, 4, 0,
  [null, [null, "Canvas Title"]],  // Content
  [...],  // Formatting/styles
  ...
]]]
```

Each section contains:
- Position 0: null marker
- Position 1: temp ID string
- Position 2: size/offset number
- Position 8: section ID string
- Position 12: content as nested `[null, [null, "text"]]`
- Position 13: style/formatting array

This format is complex and undocumented. Reverse-engineering not recommended.

#### `slack/rich-text`
Block-based JSON format similar to Slack's Block Kit API.

**Structure:**
```json
{
  "blocks": [
    {
      "type": "rich_text",
      "elements": [...]
    }
  ]
}
```

**Block Types:**

| Type | Description |
|------|-------------|
| `rich_text` | Top-level container for rich text content |

**Element Types:**

| Type | Description | Example |
|------|-------------|---------|
| `rich_text_section` | Paragraph/text section | `{"type": "rich_text_section", "elements": [...]}` |
| `rich_text_preformatted` | Code block | `{"type": "rich_text_preformatted", "elements": [...]}` |
| `rich_text_list` | List container | `{"type": "rich_text_list", "style": "bullet", "elements": [...]}` |
| `rich_text_quote` | Blockquote | `{"type": "rich_text_quote", "elements": [...]}` |

**Text Element:**
```json
{
  "type": "text",
  "text": "Hello world",
  "style": {
    "bold": true,
    "italic": true,
    "strike": true,
    "code": true
  }
}
```

**Link Element:**
```json
{
  "type": "link",
  "url": "https://example.com",
  "text": "Click here"
}
```

**User/Channel Mentions:**
```json
{"type": "user", "user_id": "U12345"}
{"type": "channel", "channel_id": "C12345"}
```

**List Example:**
```json
{
  "type": "rich_text_list",
  "style": "bullet",  // or "ordered"
  "indent": 0,
  "elements": [
    {
      "type": "rich_text_section",
      "elements": [{"type": "text", "text": "List item"}]
    }
  ]
}
```

**Code Block Example:**
```json
{
  "type": "rich_text_preformatted",
  "elements": [
    {"type": "text", "text": "console.log('hello')"}
  ]
}
```

Note: No language field for code blocks - syntax highlighting not supported.

#### `slack/texty`
Delta/Quill-like operational transform format. Each operation inserts content with optional attributes.

**Structure:**
```json
{
  "ops": [
    {"insert": "text", "attributes": {...}},
    {"insert": "\n", "attributes": {...}}
  ]
}
```

**Inline Attributes:**
| Attribute | Type | Description |
|-----------|------|-------------|
| `bold` | boolean | Bold text |
| `italic` | boolean | Italic text |
| `strike` | boolean | Strikethrough |
| `code` | boolean | Inline code |
| `link` | string | URL for hyperlink |

**Block Attributes (on `\n` inserts):**
| Attribute | Type | Description |
|-----------|------|-------------|
| `code-block` | boolean | Code block line |
| `blockquote` | boolean | Quote block line |
| `list` | string | `"bullet"` or `"ordered"` |
| `indent` | number | Indentation level (0+) |
| `header` | number | Heading level (1-6) |

**Examples:**

Bold text:
```json
{"ops": [{"insert": "bold", "attributes": {"bold": true}}, {"insert": "\n"}]}
```

Code block:
```json
{"ops": [
  {"insert": "console.log('hello')"},
  {"insert": "\n", "attributes": {"code-block": true}}
]}
```

Bullet list:
```json
{"ops": [
  {"insert": "Item 1"},
  {"insert": "\n", "attributes": {"list": "bullet"}},
  {"insert": "Item 2"},
  {"insert": "\n", "attributes": {"list": "bullet"}}
]}
```

Mixed formatting:
```json
{"ops": [
  {"insert": "normal "},
  {"insert": "bold and italic", "attributes": {"bold": true, "italic": true}},
  {"insert": " normal\n"}
]}
```

#### `text/markdown`
Plain markdown text representation. Standard markdown syntax.

### Element Handling Across Formats

| Element | HTML | slack/rich-text | slack/texty |
|---------|------|-----------------|-------------|
| **Image** | `<img src="data:image/png;base64,...">` | Filename only: `{"text": "image.png"}` | Omitted entirely |
| **Table** | Not preserved | Flattened to text in one section | Tab-separated cells, newline rows |
| **Heading** | Bold text only | `bold: true` style | `bold: true` attribute |
| **Callout** | Plain text | Plain `rich_text_section` | Plain text |
| **Divider** | Not preserved | Plain text "Divider" | Plain text "Divider" |
| **Checklist** | Plain text | Plain `rich_text_section` | `"- [-] "` prefix with indent |
| **Strikethrough** | Not preserved | Not preserved | Not preserved |

### Data Loss Notes

- **Images**: Only HTML contains actual image data (base64). JSON formats lose images.
- **Tables**: **Cannot be pasted into Slack Canvas at all** - not supported via paste handler
- **Semantic headings**: All formats lose heading levels - just becomes bold
- **Callouts**: Background color/styling completely lost
- **Dividers**: Become plain text, not a visual separator
- **Strikethrough**: Lost in all clipboard formats

### Priority Order

When pasting, Slack Canvas appears to prefer:
1. `slack/rich-text` from web-custom-data (if present)
2. HTML fallback

If slack/rich-text is present, HTML is ignored for most elements.

### HTML Output

Slack's HTML output is minimal with no classes or data attributes.

#### Inline Formatting
| Format | HTML |
|--------|------|
| Bold | `<b>text</b>` |
| Italic | `<i>text</i>` |
| Bold + Italic | `<i><b>text</b></i>` |
| Strikethrough | Plain text (not preserved in HTML) |
| Inline code | `<code>text</code>` |
| Links | `<a href="url">text</a>` |

#### Block Elements
| Element | HTML |
|---------|------|
| Paragraph | Plain text with `<br>` for line breaks |
| Code block | `<pre>content</pre>` |
| Heading | Plain text (styling not preserved) |
| Callout | Plain text with `<br>` (styling not preserved) |
| Bullet list | Plain text (no `<ul>/<li>` tags) |
| Numbered list | Plain text (no `<ol>/<li>` tags) |
| Checklist | Plain text (no structure preserved) |
| Image | `<img src='data:image/png;base64,...'>` |
| Divider | Not preserved |

#### Tables
Slack Canvas does not appear to support tables.

#### Notes on HTML Output
- No semantic list structure (`<ul>`, `<ol>`, `<li>`)
- No heading tags (`<h1>`, `<h2>`, etc.)
- No table support
- Strikethrough not preserved in HTML
- Images embedded as base64 data URLs
- Very flat structure with `<br>` for most line breaks

### Asymmetry Note

Slack has a copy/paste asymmetry:
- **Copies**: `<pre>content</pre>`
- **Requires for paste**: `<pre><code>content</code></pre>`

## Pasting INTO Slack Canvas

### What Works
| Element | Required HTML |
|---------|---------------|
| Bold | `<b>text</b>` or `<strong>text</strong>` |
| Italic | `<i>text</i>` or `<em>text</em>` |
| Inline code | `<code>text</code>` |
| Code block | `<pre><code>content</code></pre>` |
| Link | `<a href="url">text</a>` |
| Image | `<img src="...">` |

### What Doesn't Work
- `<pre>` without `<code>` wrapper (pastes as plain text)
- Syntax highlighting / language on code blocks (not supported)
