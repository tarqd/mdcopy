# Slack/Quip pblite Format

The `application/vnd.quip.sections+pblite` format is a protobuf-lite (pblite) serialization used by Slack Canvas for full-fidelity clipboard operations.

## Overview

- Format: JSON array with positional fields (not named keys)
- Encoding: UTF-16LE within `org.chromium.web-custom-data`
- Structure: `[null, [section1, section2, ...]]`

## Section Structure

Each section is an array with ~30+ positional fields:

| Position | Name | Description |
|----------|------|-------------|
| 1 | `temp_id` | Temporary ID like `"temp:C:YWXcf7ba18c..."` |
| 2 | `sort_key` | Sort order value (e.g., 100000, 101000) |
| 8 | `section_id` | Section identifier (e.g., `"aaa"`, `"aaaaaZq"`) |
| 9 | `width` | Width for some block types |
| 10 | `height` | Height - used for block subtypes (see below) |
| 12 | `content` | Content data (varies by block type) |
| 13 | `formatting` | Formatting/style array (used for lists) |
| 16 | `metadata` | Metadata array (31 elements) |
| 20 | `block_type` | Block type indicator (see below) |
| 21 | `section_id_dup` | Usually same as section_id, different for list items |
| 23 | `doc_id` | Document ID |
| 25 | `is_title` | 1 for title sections, 0 otherwise |
| 29 | `index` | Section index |

## Block Types (Position 20)

**Current format** (as of 2024, verified working):

| Type | Description | Notes |
|------|-------------|-------|
| 1 | Paragraph / List container / List item | General-purpose block |
| 2 | Continuation / Table container | Also used for list items |
| 4 | Cell content (first cell) | In tables |
| 45 | Document title | Alternative title type |
| 48 | Document title | Primary title type |

**Legacy format** (older clipboard data):

| Type | Description |
|------|-------------|
| 4 | Continuation/child block |
| 5 | Normal paragraph |
| 6 | Paragraph variant |
| 7 | Special (headings, dividers) |
| 8 | Blockquote |
| 9 | Formatted text |
| 27 | Document title |

## Lists (Bullet, Numbered, Checklist)

Lists require a **container section** followed by **item sections**.

### Container Section
- `block_type`: 1
- `height`: Indicates list type:
  - `5` = Bullet list
  - `6` = Numbered list
  - `7` = Checklist
- `content`: Empty `[null, [null, ""]]`

### Item Sections
- `block_type`: 1 (all items can use type 1)
- `height`: 0
- `section_id_dup`: `{container_sid}-{item_sid}` format
- `formatting`: `[null, [container_temp_id], null, null, list_type]`
  - Position 1: Array containing container's temp_id
  - Position 4: List type (5=bullet, 6=numbered, 7=checklist)

### Example Structure
```
Section 0: Container (t=1, h=5, empty)     <- Bullet list container
Section 1: Item (t=1, h=0, list=5)         <- "Bullet one"
Section 2: Item (t=1, h=0, list=5)         <- "Bullet two"
Section 3: Container (t=1, h=6, empty)     <- Numbered list container
Section 4: Item (t=1, h=0, list=6)         <- "Number one"
```

No separator sections needed between different lists.

## Tables

Tables use a **container section** with structure at `content[30]`, followed by **cell content sections**.

### Table Container
- `block_type`: 2
- `height`: 28
- `content[30]`: Table definition array

### Table Definition (content[30])
```json
[
  null,
  [  // Rows
    [null, "row:UUID", "aaa:temp"],
    [null, "row:UUID", "aab:temp"]
  ],
  [  // Columns (with widths)
    [null, "col:UUID", "aaa:temp", 388.5],
    [null, "col:UUID", "aab:temp", 388.5]
  ],
  [  // Cells (linking row+col to content)
    [null, "cell:UUID", "row:UUID", "col:UUID", ["temp:C:content_id"]],
    ...
  ]
]
```

### Cell Content Sections
- Follow the table container
- Each cell's content is a separate section
- Referenced by `temp_id` from the cell definition
- `block_type`: varies (1, 2, 4)

### Example Structure
```
Section 0: Title (t=48)
Section 1: Table container (t=2, h=28, has content[30])
Section 2: Cell [0,0] content (t=4)
Section 3: Cell [0,1] content (t=1)
Section 4: Cell [1,0] content (t=2)
Section 5: Cell [1,1] content (t=2)
```

## Content Field (Position 12)

### Text Content
```json
[null, [null, "Text content here"]]
```

Inline formatting uses HTML:
```json
[null, [null, "Text with <b>bold</b> and <i>italic</i>"]]
```

### Empty Content
```json
[null, [null, ""]]
```

### Table Content
Table definition at position 30 (see Tables section above).

## Formatting Array (Position 13)

Used for list items:
```json
[null, ["container_temp_id"], null, null, LIST_TYPE]
```

- **Position 1**: Array containing the container section's temp_id
- **Position 4**: List type (5=bullet, 6=numbered, 7=checklist)

Empty for non-list sections: `[]`

## Metadata Array (Position 16)

A 31-element array:
- **Position 4**: Usually 0
- **Position 26**: Code block marker (1 = inside code block)
- **Position 30**: Block subtype (usually 5)

## ID Formats

### temp_id
Format: `temp:C:YWX` + 25 hex characters
Example: `temp:C:YWXcf7ba18c0ec2458b9664496e2`

### section_id
Likely base62-encoded uint64 values (a-z, A-Z, 0-9). We've only observed letters so far, but digits probably appear in larger IDs.

Server allocates these sequentially. For generating pblite, any unique incrementing IDs work fine - e.g., `aaaaaZqaaaa`, `aaaaaZqaaab`, `aaaaaZqaaac`.

### Row/Column/Cell IDs
- Row: `row:` + 25 hex characters
- Column: `col:` + 25 hex characters
- Cell: `cell:` + 25 hex characters

## Working Test Generators

| File | Purpose |
|------|---------|
| `generate-bullets-test.py` | Generates all list types (bullet, numbered, checklist) |
| `generate-table-test.py` | Generates 2x2 table with cell content |
| `encode-web-custom-data.py` | Encodes pblite JSON to clipboard binary format |
| `copy-test-pblite.swift` | Copies encoded data to macOS clipboard |

### Usage
```bash
python3 generate-bullets-test.py && python3 encode-web-custom-data.py && swift copy-test-pblite.swift
```

## web-custom-data Binary Format

Based on Chromium's `custom_data_helper.cc`:

```
[4 bytes] UInt32LE: total data length (excluding these 4 bytes)
[4 bytes] UInt32LE: count of pairs
For each pair:
  [U16String] key (MIME type)
  [U16String] value (data)
```

### U16String Format
```
[4 bytes] UInt32LE: length in UTF-16 chars
[length*2 bytes] UTF-16LE string
[2 bytes padding if length is odd]
```

## Verified Working Elements

All tested and working with Slack Canvas paste:

- ✅ Titles (t=45 or t=48)
- ✅ Paragraphs (t=1)
- ✅ Bullet lists (container h=5, list_type=5)
- ✅ Numbered lists (container h=6, list_type=6)
- ✅ Checklists (container h=7, list_type=7)
- ✅ Tables (t=2, h=28, with row/col/cell structure)

## Legacy Documentation

The following was discovered from older clipboard data and may still work:

### SPECIAL Block Subtypes (Type 7)
The `height` field distinguishes subtypes:
- height=0 + empty: DIVIDER
- height=1 + text: H1 heading
- height=2 + text: H2 heading
- height=3 + text: H3 heading

### Code Blocks
Sections with `metadata[26] = 1` indicate code block content.

### Images
Images have complex structure at `content[2]` with base64 data at `content[2][19][1]`.

### Blockquotes
Block type 8 with non-zero height.
