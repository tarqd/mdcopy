# ProseMirror Clipboard Paste Format

## Confluence Code Block Compatibility

Confluence uses ProseMirror as its editor. To have HTML paste as a native code block (with syntax highlighting), you need minimal markup:

```html
<p data-pm-slice="1 1 []"></p><pre data-language="javascript"><code>console.log('hello')</code></pre>
```

### Required Elements

1. **`<p data-pm-slice="1 1 []"></p>`** - Signals to ProseMirror this is a valid paste slice
2. **`<pre data-language="..."><code>...</code></pre>`** - The code block with language for syntax highlighting

### Not Required

- `<meta charset='utf-8'>` - optional
- `class="code-block"` - not needed
- `data-prosemirror-*` attributes - not needed
- `data-language` on `<code>` - not needed (only on `<pre>`)
- Internal `<div>` structure - not needed
- Trailing `<p></p>` - not needed
- `fabric-editor-breakout-mark` wrapper - not needed
- `data-local-id` attributes - not needed
- `org.chromium.source-url` clipboard type - not needed

## Understanding `data-pm-slice`

ProseMirror parses the attribute with this regex:

```javascript
/^(\d+) (\d+)(?: -(\d+))? (.*)/
```

For `"1 1 []"`:

| Group | Value | Meaning |
|-------|-------|---------|
| 1 | `1` | **openStart** - open node boundaries at start |
| 2 | `1` | **openEnd** - open node boundaries at end |
| 3 | (none) | **skip** - levels to descend into DOM before parsing |
| 4 | `[]` | **context** - JSON array of node types for open boundaries |

### What openStart/openEnd Mean

These values describe how the sliced content fits structurally:

- `0, 0` - Complete, closed blocks; paste as-is
- `1, 1` - Content has one open boundary on each side; it's a complete block-level element that inserts as a sibling

### The Skip Parameter

If present (e.g., `"1 1 -2 []"`), ProseMirror skips N levels deep into the DOM:

```javascript
if (sliceData && sliceData[3]) for (let i = +sliceData[3]; i > 0; i--) {
  let child = dom.firstChild
  while (child && child.nodeType != 1) child = child.nextSibling
  if (!child) break
  dom = child
}
```

### Context Array

The `[]` can specify node type context for open boundaries. Empty means "infer from content."

## Summary

`data-pm-slice="1 1 []"` tells ProseMirror: "this is a well-formed block, insert it as a sibling node."

The element containing `data-pm-slice` itself is not parsed as content—it's just a marker. The actual content follows it.
