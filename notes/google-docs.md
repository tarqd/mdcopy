# Google Docs Clipboard Paste Format

## Code Block Recognition

Google Docs recognizes code blocks from HTML paste using `<pre><code>` structure with the `lang` attribute.

### Minimum Required HTML

```html
<pre><code class="language-javascript">console.log('hello')</code></pre>
```

**Both are required:**
1. `class="language-..."` on `<code>` (sets the syntax highlighting language)
2. `<pre>` wrapper around `<code>`

### What Doesn't Work
- `data-language` on `<pre>` or `<code>` - ignored
- `lang` attribute on `<pre>` - ignored
- `lang` attribute on `<code>` - ignored
- `<pre>` without `<code>` wrapper - no code block created

## Native Clipboard Format

Google Docs uses `org.chromium.web-custom-data` with a custom binary format for full fidelity paste.

### Format Structure

```
[length prefix][MIME type 1][separator][JSON payload][MIME type 2][UUID]
```

Encoded as **UTF-16LE** with length-prefixed strings.

### MIME Types Used

1. `application/x-vnd.google-docs-document-slice-clip+wrapped` - main document slice
2. `application/x-vnd.google-docs-internal-clip-id` - internal clipboard ID

### JSON Payload Structure

```json
{
  "dih": 1349133373,
  "data": "{\"resolved\":{...},...}",
  "edi": "...",
  "edrk": "...",
  "dct": "kix",
  "ds": false,
  "cses": false,
  "sm": "other",
  "si": ""
}
```

The `data` field contains escaped JSON with the document structure.

### Inner Data Structure

```json
{
  "resolved": {
    "dsl_spacers": "\nconsole.log('hello')\n\n",
    "dsl_styleslices": [
      // ... many style slice types ...
      {
        "stsl_type": "code_snippet",
        "stsl_trailing": {"cos_l": "JavaScript"},
        "stsl_trailingType": "code_snippet",
        "stsl_styles": [null, {"cos_l": "JavaScript"}]
      }
    ],
    "dsl_metastyleslices": [...],
    "dsl_entitymap": {},
    // ... other fields ...
  },
  "autotext_content": {}
}
```

### Key Fields for Code Snippets

| Field | Description |
|-------|-------------|
| `stsl_type` | `"code_snippet"` marks the style slice as a code block |
| `cos_l` | Code snippet language (e.g., `"JavaScript"`, `"Python"`) |
| `dsl_spacers` | The actual text content with newlines |

### Style Slice Array

The `dsl_styleslices` array contains many style types:
- `autogen`, `cell`, `code_snippet`, `collapsed_heading`, `column_sector`
- `comment`, `doco_anchor`, `document`, `equation`, `field`
- `footnote`, `headings`, `horizontal_rule`, `language`, `link`
- `list`, `paragraph`, `row`, `tbl`, `text`, etc.

Each slice has `stsl_type`, optional `stsl_leading`/`stsl_trailing` for block-level styles, and `stsl_styles` array for character-level styles.

## Complexity Assessment

Constructing a valid Google Docs clipboard payload is complex because:

1. **Binary encoding**: UTF-16LE with length prefixes
2. **Full document model**: Must include many style slice types, not just code_snippet
3. **Unknown fields**: `dih`, `edi`, `edrk` appear to be hashes/tokens
4. **Validation**: Google Docs may validate the payload structure

### Potential Approaches

1. **Minimal payload**: Try constructing just the essential fields
2. **Template-based**: Capture a real paste and modify just the content/language
3. **Accept limitation**: Use HTML paste (code block works, just no language)
