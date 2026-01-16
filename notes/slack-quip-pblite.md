# Slack/Quip pblite Format

The `application/vnd.quip.sections+pblite` format is a protobuf-lite (pblite) serialization used by Slack Canvas for full-fidelity clipboard operations.

## Overview

- Format: JSON array with positional fields (not named keys)
- Encoding: UTF-16LE within `org.chromium.web-custom-data`
- Structure: `[null, [section1, section2, ...]]`


## Protobuf Schema

```
// Reverse-engineered Quip/Slack Canvas pblite schema
// Based on analysis of clipboard data from Slack Canvas
// and correlation with slack-shared-boot.js DocBlockStyle enum
//
// NOTE: Field numbers = array positions in pblite format
// pblite uses 0-indexed arrays, but protobuf fields are 1-indexed
// So pblite position N = proto field N+1
//
// DocBlockStyle (from Slack source) -> pblite mapping:
//   PLAIN      -> "paragraph"     -> BlockType 5/6
//   H1         -> "h1"            -> BlockType 7 + height=1
//   H2         -> "h2"            -> BlockType 7 + height=2
//   H3         -> "h3"            -> BlockType 7 + height=3
//   LIST_BULLET    -> "bullet-list"   -> BlockType 5/6 + ListType 5
//   LIST_NUMBERED  -> "ordered-list"  -> BlockType 5/6 + ListType 6
//   LIST_CHECKLIST -> "check-list"    -> BlockType 5/6 + ListType 7
//   BLOCKQUOTE -> "blockquote"    -> BlockType 8
//   CODE       -> "code-block"    -> BlockType 4/6 + metadata[26]=1
//
// EmbeddedObjectType (from Slack source):
//   TEXT, TITLE, TABLE, DIVIDER, CALLOUT, HYPERLINK, HEADER,
//   SLACK_OBJECT, LINK, DRAG_OVERLAY, COMMENT_METADATA, DATE,
//   BACKDROP, AI_ASSISTANT

syntax = "proto3";

package quip;

// Root message - the entire clipboard payload
// pblite: [null, [section1, section2, ...]]
message ClipboardPayload {
  // Field 1 is null in pblite (position 0)
  reserved 1;

  // Field 2 contains array of sections (position 1)
  repeated Section sections = 2;
}

// A document section/block
// pblite positions 0-29 (fields 1-30)
message Section {
  // Position 0: always null
  reserved 1;

  // Position 1: Temporary ID like "temp:C:YWXcf7ba18c0ec2458b9664496e2"
  string temp_id = 2;

  // Position 2: Size/order value (e.g., 934000, 267000)
  int64 sort_key = 3;

  // Position 3: always 0
  int32 field_4 = 4;

  // Positions 4-7: always null
  reserved 5, 6, 7, 8;

  // Position 8: Section ID like "aaa", "aaaaaaU", etc.
  string section_id = 9;

  // Position 9: Width
  // For images: display width
  // For callouts: style indicator (e.g., 69)
  int32 width = 10;

  // Position 10: Height - IMPORTANT for SPECIAL blocks (type 7):
  // - 0 = DIVIDER (when content is empty)
  // - 1 = H1 heading
  // - 2 = H2 heading
  // - 3 = H3 heading
  // For images: display height
  // For callouts: style indicator (e.g., 49)
  int32 height = 11;

  // Position 11: always 0
  int32 field_12 = 12;

  // Position 12: Content (complex, varies by block type)
  Content content = 13;

  // Position 13: Formatting styles array
  repeated FormattingStyle formatting = 14;

  // Positions 14-15: always null
  reserved 15, 16;

  // Position 16: Metadata
  SectionMetadata metadata = 17;

  // Positions 17-19: always null
  reserved 18, 19, 20;

  // Position 20: Block type enum
  BlockType block_type = 21;

  // Position 21: Section ID (duplicate of position 8)
  string section_id_dup = 22;

  // Position 22: always null
  reserved 23;

  // Position 23: Document ID like "YWX9AAJc40h"
  string doc_id = 24;

  // Position 24: Empty string
  string field_25 = 25;

  // Position 25: Flag (0 or 1)
  int32 is_title = 26;

  // Positions 26-28: always null
  reserved 27, 28, 29;

  // Position 29: Counter/index
  int32 index = 30;
}

// Block type enumeration (position 20)
// Maps to DocBlockStyle in Slack client code
enum BlockType {
  BLOCK_TYPE_UNKNOWN = 0;
  BLOCK_TYPE_CONTINUATION = 4;    // Child/continuation block (nested lists, code lines, table cells)
  BLOCK_TYPE_PARAGRAPH = 5;       // Normal paragraph (DocBlockStyle.PLAIN)
  BLOCK_TYPE_PARAGRAPH_ALT = 6;   // Paragraph variant (headers in cells, first list item)
  BLOCK_TYPE_SPECIAL = 7;         // Embedded objects - check height field:
                                  //   height=0 + empty content = DIVIDER
                                  //   height=1 = H1 (DocBlockStyle.H1)
                                  //   height=2 = H2 (DocBlockStyle.H2)
                                  //   height=3 = H3 (DocBlockStyle.H3)
                                  //   content[2] present = IMAGE
                                  //   content[63] present = CALLOUT container
  BLOCK_TYPE_BLOCKQUOTE = 8;      // Block quote (DocBlockStyle.BLOCKQUOTE)
  BLOCK_TYPE_FORMATTED = 9;       // Text with inline HTML (<b>, <i>, <code>, etc.)
  BLOCK_TYPE_TITLE = 27;          // Document title (EmbeddedObjectType.TITLE)
}

// Content field (position 12)
// Structure varies significantly by block type
message Content {
  // For simple text: [null, [null, "text content"]]
  // Position 0: null
  reserved 1;

  // Position 1: Text wrapper - contains actual text with inline HTML
  TextWrapper text = 2;

  // Position 2: Image data (for SPECIAL blocks with images)
  ImageData image = 3;

  // Positions 3-29: reserved/unused
  reserved 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30;

  // Position 30: Table data (for TABLE embedded objects)
  TableData table = 31;

  // Positions 31-62: reserved/unused
  reserved 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63;

  // Position 63: Callout data (for CALLOUT embedded objects)
  CalloutData callout = 64;
}

message TextWrapper {
  reserved 1;
  string text = 2;
}

// Image data structure (found at content position 2)
message ImageData {
  reserved 1;
  int32 x = 2;
  int32 y = 3;
  ImageInfo main_image = 4;
  repeated ImageInfo thumbnails = 5;
  reserved 6, 7;
  string filename = 8;
  reserved 9, 10, 11, 12;
  int32 field_13 = 13;
  reserved 14, 15, 16, 17, 18, 19;
  EmbeddedImage embedded = 20;
}

message ImageInfo {
  reserved 1, 2;
  int32 width = 3;
  int32 height = 4;
  string mime_type = 5;
  string file_id = 6;
  string thumb_size = 7;  // e.g., "thumb_64", "thumb_80"
}

message EmbeddedImage {
  reserved 1;
  string data_url = 2;  // "data:image/png;base64,..."
}

// Callout container data (found at content position 63)
// Callouts are container blocks with child sections for content
message CalloutData {
  reserved 1;

  // Position 1: Array containing callout metadata
  // Structure: [null, callout_id, [child_temp_ids...]]
  CalloutMetadata metadata = 2;
}

message CalloutMetadata {
  reserved 1;

  // Callout identifier
  string callout_id = 2;

  // References to child section temp_ids that contain callout content
  repeated string child_refs = 3;
}

// Table structure (found at content position 30)
message TableData {
  reserved 1;
  repeated TableRow rows = 2;
  repeated TableColumn columns = 3;
  repeated TableCell cells = 4;
  reserved 5;
  int32 field_6 = 6;
  int32 field_7 = 7;
}

message TableRow {
  reserved 1;
  string row_id = 2;      // "row:uuid"
  string temp_ref = 3;    // "aaa:temp"
}

message TableColumn {
  reserved 1;
  string col_id = 2;      // "col:uuid"
  string temp_ref = 3;    // "aaa:temp"
  double width = 4;       // e.g., 388.5
}

message TableCell {
  reserved 1;
  string cell_id = 2;     // "cell:uuid"
  string row_id = 3;      // Reference to row
  string col_id = 4;      // Reference to column
  repeated string content_refs = 5;  // ["temp:C:..."]
}

// Section metadata (position 16, 31-element array)
message SectionMetadata {
  // Positions 0-3: null
  reserved 1, 2, 3, 4;

  // Position 4: usually 0
  int32 field_5 = 5;

  // Positions 5-25: null
  reserved 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26;

  // Position 26: Code block marker (1 = inside code block)
  int32 is_code_block = 27;

  // Positions 27-29: null
  reserved 28, 29, 30;

  // Position 30: block subtype (usually 5)
  int32 subtype = 31;
}

// Formatting style (position 13)
// Used for list items to group them and specify list type
message FormattingStyle {
  // Position 0: null
  reserved 1;

  // Position 1: Array of temp IDs - items in same list share ID
  repeated string list_group_ids = 2;

  // Positions 2-3: null
  reserved 3, 4;

  // Position 4: List type
  ListType list_type = 5;
}

// List type enumeration (formatting position 4)
enum ListType {
  LIST_TYPE_UNKNOWN = 0;
  LIST_TYPE_BULLETED = 5;
  LIST_TYPE_NUMBERED = 6;
  LIST_TYPE_CHECKLIST = 7;
}
```

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
