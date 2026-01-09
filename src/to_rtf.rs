use crate::highlight::HighlightContext;
use crate::image::{load_image_with_fallback, ImageError};
use crate::EmbedMode;
use log::warn;
use markdown::mdast::{AlignKind, Node};
use std::collections::HashMap;
use std::path::Path;
use syntect::easy::HighlightLines;

pub fn mdast_to_rtf(
    node: &Node,
    base_dir: &Path,
    embed_mode: EmbedMode,
    strict: bool,
    highlight: Option<&HighlightContext>,
) -> Result<String, ImageError> {
    let mut ctx = RtfContext::new(base_dir, embed_mode, strict, highlight);
    let mut body = String::new();
    node_to_rtf(node, &mut body, &mut ctx)?;

    // Build the final RTF with color table
    let mut rtf = String::from("{\\rtf1\\ansi\\deff0{\\fonttbl{\\f0 Helvetica;}{\\f1 Courier;}}");

    // Add color table if we have any colors
    if !ctx.colors.is_empty() {
        rtf.push_str("{\\colortbl;");
        let mut colors: Vec<_> = ctx.colors.iter().collect();
        colors.sort_by_key(|(_, idx)| *idx);
        for ((r, g, b), _) in colors {
            rtf.push_str(&format!("\\red{}\\green{}\\blue{};", r, g, b));
        }
        rtf.push('}');
    }

    rtf.push_str("\\f0\\fs24 ");
    rtf.push_str(&body);
    rtf.push('}');
    Ok(rtf)
}

struct RtfContext<'a> {
    base_dir: &'a Path,
    embed_mode: EmbedMode,
    strict: bool,
    highlight: Option<&'a HighlightContext>,
    colors: HashMap<(u8, u8, u8), usize>,
    table_align: Vec<AlignKind>,
    table_cell_index: usize,
    in_table_header: bool,
}

impl<'a> RtfContext<'a> {
    fn new(
        base_dir: &'a Path,
        embed_mode: EmbedMode,
        strict: bool,
        highlight: Option<&'a HighlightContext>,
    ) -> Self {
        Self {
            base_dir,
            embed_mode,
            strict,
            highlight,
            colors: HashMap::new(),
            table_align: Vec::new(),
            table_cell_index: 0,
            in_table_header: false,
        }
    }

    fn get_color_index(&mut self, r: u8, g: u8, b: u8) -> usize {
        let key = (r, g, b);
        let next_idx = self.colors.len() + 1; // RTF color indices start at 1
        *self.colors.entry(key).or_insert(next_idx)
    }
}

fn node_to_rtf(node: &Node, rtf: &mut String, ctx: &mut RtfContext) -> Result<(), ImageError> {
    match node {
        Node::Root(root) => {
            for child in &root.children {
                node_to_rtf(child, rtf, ctx)?;
            }
        }
        Node::Heading(heading) => {
            let size = match heading.depth {
                1 => 48,
                2 => 36,
                3 => 28,
                4 => 24,
                5 => 22,
                _ => 20,
            };
            rtf.push_str(&format!("{{\\b\\fs{} ", size));
            for child in &heading.children {
                node_to_rtf(child, rtf, ctx)?;
            }
            rtf.push_str("}\\par\\par ");
        }
        Node::Paragraph(para) => {
            for child in &para.children {
                node_to_rtf(child, rtf, ctx)?;
            }
            rtf.push_str("\\par ");
        }
        Node::Text(text) => {
            push_rtf_escaped(rtf, &text.value);
        }
        Node::Strong(strong) => {
            rtf.push_str("{\\b ");
            for child in &strong.children {
                node_to_rtf(child, rtf, ctx)?;
            }
            rtf.push('}');
        }
        Node::Emphasis(em) => {
            rtf.push_str("{\\i ");
            for child in &em.children {
                node_to_rtf(child, rtf, ctx)?;
            }
            rtf.push('}');
        }
        Node::InlineCode(code) => {
            rtf.push_str("{\\f1 ");
            push_rtf_escaped(rtf, &code.value);
            rtf.push('}');
        }
        Node::Code(code) => {
            if let Some(highlight_ctx) = ctx.highlight {
                let syntax = code
                    .lang
                    .as_ref()
                    .map(|lang| highlight_ctx.find_syntax(lang))
                    .unwrap_or_else(|| highlight_ctx.syntax_set.find_syntax_plain_text());

                let mut highlighter = HighlightLines::new(syntax, &highlight_ctx.theme);
                rtf.push_str("{\\f1\\fs20 ");

                for line in code.value.lines() {
                    if let Ok(ranges) = highlighter.highlight_line(line, &highlight_ctx.syntax_set) {
                        for (style, text) in ranges {
                            let color_idx =
                                ctx.get_color_index(style.foreground.r, style.foreground.g, style.foreground.b);
                            rtf.push_str(&format!("\\cf{} ", color_idx));
                            push_rtf_escaped(rtf, text);
                        }
                    } else {
                        push_rtf_escaped(rtf, line);
                    }
                    rtf.push_str("\\line ");
                }

                rtf.push_str("}\\par ");
            } else {
                rtf.push_str("{\\f1\\fs20 ");
                push_rtf_escaped(rtf, &code.value);
                rtf.push_str("}\\par ");
            }
        }
        Node::Link(link) => {
            for child in &link.children {
                node_to_rtf(child, rtf, ctx)?;
            }
        }
        Node::List(list) => {
            for child in &list.children {
                node_to_rtf(child, rtf, ctx)?;
            }
        }
        Node::ListItem(item) => {
            rtf.push_str("\\bullet  ");
            for child in &item.children {
                node_to_rtf(child, rtf, ctx)?;
            }
        }
        Node::Blockquote(bq) => {
            rtf.push_str("{\\li400 ");
            for child in &bq.children {
                node_to_rtf(child, rtf, ctx)?;
            }
            rtf.push('}');
        }
        Node::ThematicBreak(_) => {
            rtf.push_str("\\par\\brdrb\\brdrs\\brdrw10\\brsp20 \\par ");
        }
        Node::Break(_) => {
            rtf.push_str("\\line ");
        }
        Node::Delete(del) => {
            rtf.push_str("{\\strike ");
            for child in &del.children {
                node_to_rtf(child, rtf, ctx)?;
            }
            rtf.push('}');
        }
        Node::Table(table) => {
            ctx.table_align = table.align.clone();
            for (i, child) in table.children.iter().enumerate() {
                ctx.in_table_header = i == 0;
                node_to_rtf(child, rtf, ctx)?;
            }
            ctx.table_align.clear();
            rtf.push_str("\\par ");
        }
        Node::TableRow(row) => {
            let col_count = ctx.table_align.len().max(1);
            let col_width = 9000 / col_count;

            rtf.push_str("\\trowd ");
            for i in 0..col_count {
                let align = ctx.table_align.get(i).unwrap_or(&AlignKind::None);
                match align {
                    AlignKind::Left => rtf.push_str("\\ql"),
                    AlignKind::Center => rtf.push_str("\\qc"),
                    AlignKind::Right => rtf.push_str("\\qr"),
                    AlignKind::None => rtf.push_str("\\ql"),
                }
                rtf.push_str(&format!("\\cellx{} ", (i + 1) * col_width));
            }

            ctx.table_cell_index = 0;
            for child in &row.children {
                node_to_rtf(child, rtf, ctx)?;
            }
            rtf.push_str("\\row ");
        }
        Node::TableCell(cell) => {
            if ctx.in_table_header {
                rtf.push_str("{\\b ");
            }
            rtf.push_str("\\intbl ");
            for child in &cell.children {
                node_to_rtf(child, rtf, ctx)?;
            }
            if ctx.in_table_header {
                rtf.push('}');
            }
            rtf.push_str("\\cell ");
            ctx.table_cell_index += 1;
        }
        Node::Image(image) => {
            let img = load_image_with_fallback(
                &image.url,
                ctx.base_dir,
                ctx.embed_mode,
                ctx.strict,
            )?;

            if let Some(img) = img {
                if let Some(format) = img.rtf_format() {
                    // RTF embedded image: {\pict\pngblip <hex data>}
                    rtf.push_str(&format!("{{\\pict{} ", format));
                    rtf.push_str(&img.to_rtf_hex());
                    rtf.push('}');
                    return Ok(());
                } else {
                    warn!(
                        "RTF does not support {} images, using hyperlink fallback: {}",
                        img.mime_type, image.url
                    );
                }
            }
            // Fallback: link to the image with alt text or URL as display text
            let text = if !image.alt.is_empty() {
                &image.alt
            } else {
                &image.url
            };
            rtf.push_str("{\\field{\\*\\fldinst{HYPERLINK \"");
            push_rtf_escaped(rtf, &image.url);
            rtf.push_str("\"}}{\\fldrslt ");
            push_rtf_escaped(rtf, text);
            rtf.push_str("}}");
        }
        Node::Html(_) => {}
        Node::Definition(_) => {}
        Node::FootnoteDefinition(_) => {}
        Node::FootnoteReference(fnref) => {
            rtf.push_str(&format!("[^{}]", fnref.identifier));
        }
        _ => {}
    }
    Ok(())
}

fn push_rtf_escaped(rtf: &mut String, text: &str) {
    for c in text.chars() {
        match c {
            '\\' => rtf.push_str("\\\\"),
            '{' => rtf.push_str("\\{"),
            '}' => rtf.push_str("\\}"),
            '\n' => rtf.push_str("\\line "),
            c if c.is_ascii() => rtf.push(c),
            c => rtf.push_str(&format!("\\u{}?", c as i16)),
        }
    }
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

    fn render_rtf(md: &str) -> String {
        let ast = parse_markdown(md);
        mdast_to_rtf(&ast, Path::new("."), crate::EmbedMode::None, false, None).unwrap()
    }

    #[test]
    fn test_rtf_header() {
        let rtf = render_rtf("Hello");
        assert!(rtf.starts_with("{\\rtf1\\ansi\\deff0"));
        assert!(rtf.contains("{\\fonttbl"));
        assert!(rtf.ends_with("}"));
    }

    #[test]
    fn test_rtf_escape_backslash() {
        let mut s = String::new();
        push_rtf_escaped(&mut s, "\\");
        assert_eq!(s, "\\\\");
    }

    #[test]
    fn test_rtf_escape_braces() {
        let mut s = String::new();
        push_rtf_escaped(&mut s, "{text}");
        assert_eq!(s, "\\{text\\}");
    }

    #[test]
    fn test_rtf_escape_newline() {
        let mut s = String::new();
        push_rtf_escaped(&mut s, "line1\nline2");
        assert_eq!(s, "line1\\line line2");
    }

    #[test]
    fn test_rtf_escape_unicode() {
        let mut s = String::new();
        push_rtf_escaped(&mut s, "é");
        // é is U+00E9 (233 in decimal)
        assert_eq!(s, "\\u233?");
    }

    #[test]
    fn test_heading() {
        let rtf = render_rtf("# Heading 1");
        // h1 should be bold and size 48
        assert!(rtf.contains("{\\b\\fs48"));
        assert!(rtf.contains("Heading 1"));
    }

    #[test]
    fn test_heading_sizes() {
        // Test different heading levels have different sizes
        let h1 = render_rtf("# H1");
        let h2 = render_rtf("## H2");
        let h3 = render_rtf("### H3");

        assert!(h1.contains("\\fs48"));
        assert!(h2.contains("\\fs36"));
        assert!(h3.contains("\\fs28"));
    }

    #[test]
    fn test_paragraph() {
        let rtf = render_rtf("Hello world");
        assert!(rtf.contains("Hello world"));
        assert!(rtf.contains("\\par"));
    }

    #[test]
    fn test_strong() {
        let rtf = render_rtf("**bold**");
        assert!(rtf.contains("{\\b bold}"));
    }

    #[test]
    fn test_emphasis() {
        let rtf = render_rtf("*italic*");
        assert!(rtf.contains("{\\i italic}"));
    }

    #[test]
    fn test_inline_code() {
        let rtf = render_rtf("`code`");
        // Inline code should use monospace font (f1)
        assert!(rtf.contains("{\\f1 code}"));
    }

    #[test]
    fn test_code_block() {
        let rtf = render_rtf("```\ncode\n```");
        // Code blocks should use monospace font and smaller size
        assert!(rtf.contains("{\\f1\\fs20"));
        assert!(rtf.contains("code"));
    }

    #[test]
    fn test_list_item() {
        let rtf = render_rtf("- item");
        assert!(rtf.contains("\\bullet"));
        assert!(rtf.contains("item"));
    }

    #[test]
    fn test_blockquote() {
        let rtf = render_rtf("> quoted");
        // Blockquotes should have left indent
        assert!(rtf.contains("{\\li400"));
        assert!(rtf.contains("quoted"));
    }

    #[test]
    fn test_thematic_break() {
        let rtf = render_rtf("---");
        assert!(rtf.contains("\\brdrb"));
    }

    #[test]
    fn test_line_break() {
        let rtf = render_rtf("line one  \nline two");
        assert!(rtf.contains("\\line"));
    }

    #[test]
    fn test_strikethrough() {
        let rtf = render_rtf("~~deleted~~");
        assert!(rtf.contains("{\\strike deleted}"));
    }

    #[test]
    fn test_table() {
        let md = "| A | B |\n|---|---|\n| 1 | 2 |";
        let rtf = render_rtf(md);
        assert!(rtf.contains("\\trowd"));
        assert!(rtf.contains("\\cell"));
        assert!(rtf.contains("\\row"));
    }

    #[test]
    fn test_table_header_bold() {
        let md = "| Header |\n|---|\n| Cell |";
        let rtf = render_rtf(md);
        // Header cells should be bold
        assert!(rtf.contains("{\\b"));
    }

    #[test]
    fn test_table_alignment() {
        let md = "| Left | Center | Right |\n|:-----|:------:|------:|\n| L | C | R |";
        let rtf = render_rtf(md);
        assert!(rtf.contains("\\ql")); // left
        assert!(rtf.contains("\\qc")); // center
        assert!(rtf.contains("\\qr")); // right
    }

    #[test]
    fn test_footnote_reference() {
        // Note: We just test that it doesn't crash and produces something
        let md = "Text[^1]\n\n[^1]: Footnote";
        let rtf = render_rtf(md);
        assert!(rtf.contains("[^1]"));
    }

    #[test]
    fn test_link_text_only() {
        // Links in RTF just show the text (no hyperlink in basic RTF)
        let rtf = render_rtf("[link text](https://example.com)");
        assert!(rtf.contains("link text"));
    }

    #[test]
    fn test_image_fallback() {
        // When image can't be loaded, should fallback to hyperlink
        let rtf = render_rtf("![alt](image.png)");
        assert!(rtf.contains("HYPERLINK"));
        assert!(rtf.contains("image.png"));
    }

    #[test]
    fn test_nested_formatting() {
        let rtf = render_rtf("**bold *and italic* text**");
        assert!(rtf.contains("{\\b"));
        assert!(rtf.contains("{\\i"));
    }

    #[test]
    fn test_color_table_empty_by_default() {
        // Without syntax highlighting, there should be no colortbl
        let rtf = render_rtf("Hello");
        assert!(!rtf.contains("\\colortbl"));
    }

    #[test]
    fn test_rtf_context_get_color_index() {
        let mut ctx = RtfContext::new(Path::new("."), crate::EmbedMode::None, false, None);

        // First color should get index 1 (RTF color indices are 1-based)
        let idx1 = ctx.get_color_index(255, 0, 0);
        assert_eq!(idx1, 1);

        // Same color should return same index
        let idx1_again = ctx.get_color_index(255, 0, 0);
        assert_eq!(idx1_again, 1);

        // Different color should get next index
        let idx2 = ctx.get_color_index(0, 255, 0);
        assert_eq!(idx2, 2);
    }

    #[test]
    fn test_complex_document() {
        let md = r#"# Title

This is a paragraph with **bold** and *italic*.

- List item 1
- List item 2

```
code block
```

> A quote
"#;
        let rtf = render_rtf(md);
        assert!(rtf.starts_with("{\\rtf1"));
        assert!(rtf.contains("Title"));
        assert!(rtf.contains("{\\b bold}"));
        assert!(rtf.contains("{\\i italic}"));
        assert!(rtf.contains("\\bullet"));
        assert!(rtf.contains("{\\f1\\fs20"));
        assert!(rtf.contains("{\\li400"));
        assert!(rtf.ends_with("}"));
    }
}
