use crate::config::ImageConfig;
use crate::image::{ImageCache, ImageError};
use markdown::mdast::{AlignKind, Node};
use std::path::Path;

pub fn mdast_to_markdown(
    node: &Node,
    base_dir: &Path,
    image_config: &ImageConfig,
    strict: bool,
    image_cache: &ImageCache,
) -> Result<String, ImageError> {
    let mut ctx = MarkdownContext::new(base_dir, image_config, strict, image_cache);
    let mut output = String::new();
    node_to_markdown(node, &mut output, &mut ctx)?;
    // Trim trailing whitespace but ensure single trailing newline
    let trimmed = output.trim_end();
    if trimmed.is_empty() {
        Ok(String::new())
    } else {
        Ok(format!("{}\n", trimmed))
    }
}

struct MarkdownContext<'a> {
    base_dir: &'a Path,
    image_config: &'a ImageConfig,
    strict: bool,
    image_cache: &'a ImageCache,
    /// Current list depth for indentation
    list_depth: usize,
    /// Stack of list types (true = ordered, false = unordered)
    list_stack: Vec<bool>,
    /// Current item index within each list level
    list_indices: Vec<usize>,
    /// Whether we're inside a tight list (no blank lines between items)
    tight_list: bool,
}

impl<'a> MarkdownContext<'a> {
    fn new(
        base_dir: &'a Path,
        image_config: &'a ImageConfig,
        strict: bool,
        image_cache: &'a ImageCache,
    ) -> Self {
        Self {
            base_dir,
            image_config,
            strict,
            image_cache,
            list_depth: 0,
            list_stack: Vec::new(),
            list_indices: Vec::new(),
            tight_list: false,
        }
    }

    fn list_indent(&self) -> String {
        "    ".repeat(self.list_depth.saturating_sub(1))
    }
}

fn node_to_markdown(
    node: &Node,
    md: &mut String,
    ctx: &mut MarkdownContext,
) -> Result<(), ImageError> {
    match node {
        Node::Root(root) => {
            for (i, child) in root.children.iter().enumerate() {
                if i > 0 {
                    // Add blank line between block elements
                    if !md.ends_with("\n\n") && !md.ends_with("\n") {
                        md.push('\n');
                    }
                    if !md.ends_with("\n\n") {
                        md.push('\n');
                    }
                }
                node_to_markdown(child, md, ctx)?;
            }
        }
        Node::Heading(heading) => {
            for _ in 0..heading.depth {
                md.push('#');
            }
            md.push(' ');
            for child in &heading.children {
                node_to_markdown(child, md, ctx)?;
            }
            md.push('\n');
        }
        Node::Paragraph(para) => {
            let indent = ctx.list_indent();
            if ctx.list_depth > 0 && !indent.is_empty() {
                // Don't indent the first paragraph in a list item
            }
            for child in &para.children {
                node_to_markdown(child, md, ctx)?;
            }
            md.push('\n');
        }
        Node::Text(text) => {
            md.push_str(&text.value);
        }
        Node::Strong(strong) => {
            md.push_str("**");
            for child in &strong.children {
                node_to_markdown(child, md, ctx)?;
            }
            md.push_str("**");
        }
        Node::Emphasis(em) => {
            md.push('*');
            for child in &em.children {
                node_to_markdown(child, md, ctx)?;
            }
            md.push('*');
        }
        Node::InlineCode(code) => {
            // Handle code that contains backticks
            let backticks = if code.value.contains("``") {
                "```"
            } else if code.value.contains('`') {
                "``"
            } else {
                "`"
            };
            // Add space if code starts/ends with backtick
            let needs_space =
                code.value.starts_with('`') || code.value.ends_with('`') || code.value.is_empty();
            md.push_str(backticks);
            if needs_space {
                md.push(' ');
            }
            md.push_str(&code.value);
            if needs_space {
                md.push(' ');
            }
            md.push_str(backticks);
        }
        Node::Code(code) => {
            // Determine fence character and length
            let fence_char = if code.value.contains("```") { '~' } else { '`' };
            let mut fence_len = 3;
            // Ensure fence is long enough to not conflict with content
            let consecutive = count_max_consecutive(&code.value, fence_char);
            if consecutive >= fence_len {
                fence_len = consecutive + 1;
            }
            let fence: String = std::iter::repeat_n(fence_char, fence_len).collect();

            md.push_str(&fence);
            if let Some(lang) = &code.lang {
                md.push_str(lang);
            }
            if let Some(meta) = &code.meta {
                md.push(' ');
                md.push_str(meta);
            }
            md.push('\n');
            md.push_str(&code.value);
            if !code.value.ends_with('\n') {
                md.push('\n');
            }
            md.push_str(&fence);
            md.push('\n');
        }
        Node::Link(link) => {
            md.push('[');
            for child in &link.children {
                node_to_markdown(child, md, ctx)?;
            }
            md.push_str("](");
            md.push_str(&link.url);
            if let Some(title) = &link.title {
                md.push_str(" \"");
                md.push_str(&escape_title(title));
                md.push('"');
            }
            md.push(')');
        }
        Node::Image(image) => {
            let img = ctx.image_cache.get_or_load(
                &image.url,
                ctx.base_dir,
                ctx.image_config,
                ctx.strict,
            )?;
            let src = img
                .map(|i| i.to_data_url())
                .unwrap_or_else(|| image.url.clone());

            md.push_str("![");
            md.push_str(&image.alt);
            md.push_str("](");
            md.push_str(&src);
            if let Some(title) = &image.title {
                md.push_str(" \"");
                md.push_str(&escape_title(title));
                md.push('"');
            }
            md.push(')');
        }
        Node::List(list) => {
            ctx.list_depth += 1;
            ctx.list_stack.push(list.ordered);
            ctx.list_indices.push(list.start.unwrap_or(1) as usize);
            ctx.tight_list = !list.spread;

            for child in &list.children {
                node_to_markdown(child, md, ctx)?;
            }

            ctx.list_depth -= 1;
            ctx.list_stack.pop();
            ctx.list_indices.pop();
            ctx.tight_list = false;
        }
        Node::ListItem(item) => {
            let indent = ctx.list_indent();
            let is_ordered = ctx.list_stack.last().copied().unwrap_or(false);
            let idx = ctx.list_indices.last_mut();

            md.push_str(&indent);
            if is_ordered {
                if let Some(i) = idx {
                    md.push_str(&format!("{}. ", *i));
                    *i += 1;
                } else {
                    md.push_str("1. ");
                }
            } else {
                md.push_str("- ");
            }

            // Handle task list items
            if let Some(checked) = item.checked {
                if checked {
                    md.push_str("[x] ");
                } else {
                    md.push_str("[ ] ");
                }
            }

            // Render children
            let mut first = true;
            for child in &item.children {
                if !first {
                    // For non-first block elements in a list item, add appropriate spacing
                    if !ctx.tight_list {
                        md.push('\n');
                    }
                }
                // For paragraphs in tight lists, don't add the trailing newline
                if let Node::Paragraph(para) = child {
                    for para_child in &para.children {
                        node_to_markdown(para_child, md, ctx)?;
                    }
                    md.push('\n');
                } else {
                    node_to_markdown(child, md, ctx)?;
                }
                first = false;
            }
        }
        Node::Blockquote(bq) => {
            for child in &bq.children {
                let mut child_md = String::new();
                node_to_markdown(child, &mut child_md, ctx)?;
                // Prefix each line with >
                for line in child_md.lines() {
                    md.push_str("> ");
                    md.push_str(line);
                    md.push('\n');
                }
            }
        }
        Node::ThematicBreak(_) => {
            md.push_str("---\n");
        }
        Node::Break(_) => {
            md.push_str("  \n");
        }
        Node::Delete(del) => {
            md.push_str("~~");
            for child in &del.children {
                node_to_markdown(child, md, ctx)?;
            }
            md.push_str("~~");
        }
        Node::Table(table) => {
            render_table(table, md, ctx)?;
        }
        Node::Html(raw) => {
            md.push_str(&raw.value);
            if !raw.value.ends_with('\n') {
                md.push('\n');
            }
        }
        Node::Definition(def) => {
            md.push('[');
            md.push_str(&def.identifier);
            md.push_str("]: ");
            md.push_str(&def.url);
            if let Some(title) = &def.title {
                md.push_str(" \"");
                md.push_str(&escape_title(title));
                md.push('"');
            }
            md.push('\n');
        }
        Node::FootnoteDefinition(fndef) => {
            md.push_str("[^");
            md.push_str(&fndef.identifier);
            md.push_str("]: ");
            for (i, child) in fndef.children.iter().enumerate() {
                if i > 0 {
                    md.push_str("    "); // Continuation indent
                }
                node_to_markdown(child, md, ctx)?;
            }
        }
        Node::FootnoteReference(fnref) => {
            md.push_str("[^");
            md.push_str(&fnref.identifier);
            md.push(']');
        }
        Node::ImageReference(imgref) => {
            md.push_str("![");
            md.push_str(&imgref.alt);
            md.push_str("][");
            md.push_str(&imgref.identifier);
            md.push(']');
        }
        Node::LinkReference(linkref) => {
            md.push('[');
            for child in &linkref.children {
                node_to_markdown(child, md, ctx)?;
            }
            md.push_str("][");
            md.push_str(&linkref.identifier);
            md.push(']');
        }
        // TableRow and TableCell are handled by render_table
        Node::TableRow(_) | Node::TableCell(_) => {}
        _ => {}
    }
    Ok(())
}

fn render_table(
    table: &markdown::mdast::Table,
    md: &mut String,
    ctx: &mut MarkdownContext,
) -> Result<(), ImageError> {
    // Pre-render all cells in a single pass to avoid duplicate image loading
    let mut rendered_rows: Vec<Vec<String>> = Vec::new();
    for row in &table.children {
        if let Node::TableRow(row) = row {
            let mut row_cells = Vec::new();
            for cell in &row.children {
                if let Node::TableCell(cell) = cell {
                    let mut cell_content = String::new();
                    for child in &cell.children {
                        node_to_markdown(child, &mut cell_content, ctx)?;
                    }
                    row_cells.push(cell_content);
                } else {
                    row_cells.push(String::new());
                }
            }
            rendered_rows.push(row_cells);
        }
    }

    // Calculate column widths from pre-rendered content
    let mut col_widths: Vec<usize> = vec![3; table.align.len()]; // minimum width of 3 for ---
    for row_cells in &rendered_rows {
        for (i, cell_content) in row_cells.iter().enumerate() {
            if i < col_widths.len() {
                col_widths[i] = col_widths[i].max(cell_content.len());
            }
        }
    }

    // Render header row using pre-rendered content
    if let Some(header_cells) = rendered_rows.first() {
        md.push('|');
        for (i, cell_content) in header_cells.iter().enumerate() {
            md.push(' ');
            let width = col_widths.get(i).copied().unwrap_or(3);
            md.push_str(&format!("{:width$}", cell_content, width = width));
            md.push_str(" |");
        }
        md.push('\n');
    }

    // Render separator row
    md.push('|');
    for (i, align) in table.align.iter().enumerate() {
        let width = col_widths.get(i).copied().unwrap_or(3);
        md.push(' ');
        match align {
            AlignKind::Left => {
                md.push(':');
                md.push_str(&"-".repeat(width - 1));
            }
            AlignKind::Right => {
                md.push_str(&"-".repeat(width - 1));
                md.push(':');
            }
            AlignKind::Center => {
                md.push(':');
                md.push_str(&"-".repeat(width - 2));
                md.push(':');
            }
            AlignKind::None => {
                md.push_str(&"-".repeat(width));
            }
        }
        md.push_str(" |");
    }
    md.push('\n');

    // Render body rows using pre-rendered content
    for row_cells in rendered_rows.iter().skip(1) {
        md.push('|');
        for (i, cell_content) in row_cells.iter().enumerate() {
            md.push(' ');
            let width = col_widths.get(i).copied().unwrap_or(3);
            md.push_str(&format!("{:width$}", cell_content, width = width));
            md.push_str(" |");
        }
        md.push('\n');
    }

    Ok(())
}

/// Count the maximum consecutive occurrences of a character in a string
fn count_max_consecutive(s: &str, c: char) -> usize {
    let mut max = 0;
    let mut current = 0;
    for ch in s.chars() {
        if ch == c {
            current += 1;
            max = max.max(current);
        } else {
            current = 0;
        }
    }
    max
}

/// Escape special characters in title strings
fn escape_title(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use markdown::{Constructs, Options, ParseOptions};

    fn parse_markdown(md: &str) -> markdown::mdast::Node {
        let options = Options {
            parse: ParseOptions {
                constructs: Constructs::gfm(),
                ..Default::default()
            },
            ..Default::default()
        };
        markdown::to_mdast(md, &options.parse).unwrap()
    }

    fn roundtrip(md: &str) -> String {
        let ast = parse_markdown(md);
        let cache = crate::image::ImageCache::new();
        let image_config = crate::config::ImageConfig {
            embed_local: false,
            embed_remote: false,
            optimize: false,
            max_dimension: 1200,
            quality: 80,
        };
        mdast_to_markdown(&ast, Path::new("."), &image_config, false, &cache).unwrap()
    }

    #[test]
    fn test_heading() {
        assert_eq!(roundtrip("# Heading 1"), "# Heading 1\n");
        assert_eq!(roundtrip("## Heading 2"), "## Heading 2\n");
        assert_eq!(roundtrip("###### Heading 6"), "###### Heading 6\n");
    }

    #[test]
    fn test_paragraph() {
        assert_eq!(roundtrip("Hello world"), "Hello world\n");
    }

    #[test]
    fn test_strong() {
        assert_eq!(roundtrip("**bold**"), "**bold**\n");
    }

    #[test]
    fn test_emphasis() {
        assert_eq!(roundtrip("*italic*"), "*italic*\n");
    }

    #[test]
    fn test_inline_code() {
        assert_eq!(roundtrip("`code`"), "`code`\n");
    }

    #[test]
    fn test_inline_code_with_backticks() {
        assert_eq!(roundtrip("`` `code` ``"), "`` `code` ``\n");
    }

    #[test]
    fn test_code_block() {
        let input = "```rust\nfn main() {}\n```";
        let output = roundtrip(input);
        assert!(output.contains("```rust"));
        assert!(output.contains("fn main() {}"));
    }

    #[test]
    fn test_code_block_no_lang() {
        let input = "```\ncode\n```";
        let output = roundtrip(input);
        assert!(output.starts_with("```\n"));
        assert!(output.contains("code"));
    }

    #[test]
    fn test_link() {
        assert_eq!(
            roundtrip("[link](https://example.com)"),
            "[link](https://example.com)\n"
        );
    }

    #[test]
    fn test_link_with_title() {
        assert_eq!(
            roundtrip("[link](https://example.com \"title\")"),
            "[link](https://example.com \"title\")\n"
        );
    }

    #[test]
    fn test_image() {
        assert_eq!(roundtrip("![alt](image.png)"), "![alt](image.png)\n");
    }

    #[test]
    fn test_unordered_list() {
        let input = "- item 1\n- item 2";
        let output = roundtrip(input);
        assert!(output.contains("- item 1"));
        assert!(output.contains("- item 2"));
    }

    #[test]
    fn test_ordered_list() {
        let input = "1. first\n2. second";
        let output = roundtrip(input);
        assert!(output.contains("1. first"));
        assert!(output.contains("2. second"));
    }

    #[test]
    fn test_blockquote() {
        let output = roundtrip("> quoted text");
        assert!(output.contains("> quoted text"));
    }

    #[test]
    fn test_thematic_break() {
        let output = roundtrip("---");
        assert!(output.contains("---"));
    }

    #[test]
    fn test_line_break() {
        // Two spaces at end create hard break
        let output = roundtrip("line one  \nline two");
        assert!(output.contains("  \n"));
    }

    #[test]
    fn test_strikethrough() {
        assert_eq!(roundtrip("~~deleted~~"), "~~deleted~~\n");
    }

    #[test]
    fn test_table() {
        let input = "| A | B |\n|---|---|\n| 1 | 2 |";
        let output = roundtrip(input);
        assert!(output.contains("|"));
        assert!(output.contains("---"));
        assert!(output.contains("A"));
        assert!(output.contains("B"));
    }

    #[test]
    fn test_table_alignment() {
        let input = "| Left | Center | Right |\n|:-----|:------:|------:|\n| L | C | R |";
        let output = roundtrip(input);
        assert!(output.contains(":--")); // left align
        assert!(output.contains("-:")); // right align
    }

    #[test]
    fn test_nested_formatting() {
        let output = roundtrip("**bold and *italic* text**");
        assert!(output.contains("**"));
        assert!(output.contains("*italic*"));
    }

    #[test]
    fn test_complex_document() {
        let md = r#"# Title

This is a paragraph with **bold** and *italic*.

- List item 1
- List item 2

```rust
fn main() {}
```

> A quote
"#;
        let output = roundtrip(md);
        assert!(output.contains("# Title"));
        assert!(output.contains("**bold**"));
        assert!(output.contains("*italic*"));
        assert!(output.contains("- List"));
        assert!(output.contains("```rust"));
        assert!(output.contains("> A quote"));
    }

    #[test]
    fn test_task_list() {
        let input = "- [ ] unchecked\n- [x] checked";
        let output = roundtrip(input);
        assert!(output.contains("[ ]"));
        assert!(output.contains("[x]"));
    }

    #[test]
    fn test_footnote_reference() {
        let input = "Text[^1]";
        let output = roundtrip(input);
        assert!(output.contains("[^1]"));
    }

    #[test]
    fn test_count_max_consecutive() {
        assert_eq!(count_max_consecutive("abc", '`'), 0);
        assert_eq!(count_max_consecutive("a`b`c", '`'), 1);
        assert_eq!(count_max_consecutive("a``b", '`'), 2);
        assert_eq!(count_max_consecutive("```code```", '`'), 3);
    }

    #[test]
    fn test_escape_title() {
        assert_eq!(escape_title("hello"), "hello");
        assert_eq!(escape_title("hello \"world\""), "hello \\\"world\\\"");
        assert_eq!(escape_title("back\\slash"), "back\\\\slash");
    }

    #[test]
    fn test_html_passthrough() {
        let output = roundtrip("<div>raw html</div>");
        assert!(output.contains("<div>raw html</div>"));
    }
}
