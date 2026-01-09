use crate::highlight::HighlightContext;
use crate::image::{load_image_with_fallback, ImageError};
use crate::EmbedMode;
use markdown::mdast::{AlignKind, Node};
use std::path::Path;
use syntect::html::highlighted_html_for_string;

pub fn mdast_to_html(
    node: &Node,
    base_dir: &Path,
    embed_mode: EmbedMode,
    strict: bool,
    highlight: Option<&HighlightContext>,
) -> Result<String, ImageError> {
    let mut html = String::new();
    node_to_html(node, &mut html, base_dir, embed_mode, strict, highlight)?;
    Ok(html)
}

fn node_to_html(
    node: &Node,
    html: &mut String,
    base_dir: &Path,
    embed_mode: EmbedMode,
    strict: bool,
    highlight: Option<&HighlightContext>,
) -> Result<(), ImageError> {
    match node {
        Node::Root(root) => {
            for child in &root.children {
                node_to_html(child, html, base_dir, embed_mode, strict, highlight)?;
            }
        }
        Node::Heading(heading) => {
            html.push_str(&format!("<h{}>", heading.depth));
            for child in &heading.children {
                node_to_html(child, html, base_dir, embed_mode, strict, highlight)?;
            }
            html.push_str(&format!("</h{}>\n", heading.depth));
        }
        Node::Paragraph(para) => {
            html.push_str("<p>");
            for child in &para.children {
                node_to_html(child, html, base_dir, embed_mode, strict, highlight)?;
            }
            html.push_str("</p>\n");
        }
        Node::Text(text) => {
            html.push_str(&html_escape(&text.value));
        }
        Node::Strong(strong) => {
            html.push_str("<strong>");
            for child in &strong.children {
                node_to_html(child, html, base_dir, embed_mode, strict, highlight)?;
            }
            html.push_str("</strong>");
        }
        Node::Emphasis(em) => {
            html.push_str("<em>");
            for child in &em.children {
                node_to_html(child, html, base_dir, embed_mode, strict, highlight)?;
            }
            html.push_str("</em>");
        }
        Node::InlineCode(code) => {
            html.push_str("<code>");
            html.push_str(&html_escape(&code.value));
            html.push_str("</code>");
        }
        Node::Code(code) => {
            if let Some(ctx) = highlight {
                let syntax = code
                    .lang
                    .as_ref()
                    .map(|lang| ctx.find_syntax(lang))
                    .unwrap_or_else(|| ctx.syntax_set.find_syntax_plain_text());

                match highlighted_html_for_string(&code.value, &ctx.syntax_set, syntax, &ctx.theme)
                {
                    Ok(highlighted) => {
                        html.push_str(&highlighted);
                    }
                    Err(_) => {
                        // Fall back to plain code block
                        html.push_str("<pre><code>");
                        html.push_str(&html_escape(&code.value));
                        html.push_str("</code></pre>\n");
                    }
                }
            } else {
                html.push_str("<pre><code");
                if let Some(lang) = &code.lang {
                    html.push_str(&format!(" class=\"language-{}\"", html_escape(lang)));
                }
                html.push('>');
                html.push_str(&html_escape(&code.value));
                html.push_str("</code></pre>\n");
            }
        }
        Node::Link(link) => {
            html.push_str(&format!("<a href=\"{}\">", html_escape(&link.url)));
            for child in &link.children {
                node_to_html(child, html, base_dir, embed_mode, strict, highlight)?;
            }
            html.push_str("</a>");
        }
        Node::Image(image) => {
            let img = load_image_with_fallback(&image.url, base_dir, embed_mode, strict)?;
            let src = img
                .map(|i| i.to_data_url())
                .unwrap_or_else(|| image.url.clone());
            let alt = if !image.alt.is_empty() {
                &image.alt
            } else {
                &image.url
            };
            html.push_str(&format!(
                "<img src=\"{}\" alt=\"{}\" />",
                html_escape(&src),
                html_escape(alt)
            ));
        }
        Node::List(list) => {
            if list.ordered {
                html.push_str("<ol>\n");
            } else {
                html.push_str("<ul>\n");
            }
            for child in &list.children {
                node_to_html(child, html, base_dir, embed_mode, strict, highlight)?;
            }
            if list.ordered {
                html.push_str("</ol>\n");
            } else {
                html.push_str("</ul>\n");
            }
        }
        Node::ListItem(item) => {
            html.push_str("<li>");
            for child in &item.children {
                node_to_html(child, html, base_dir, embed_mode, strict, highlight)?;
            }
            html.push_str("</li>\n");
        }
        Node::Blockquote(bq) => {
            html.push_str("<blockquote>\n");
            for child in &bq.children {
                node_to_html(child, html, base_dir, embed_mode, strict, highlight)?;
            }
            html.push_str("</blockquote>\n");
        }
        Node::ThematicBreak(_) => {
            html.push_str("<hr />\n");
        }
        Node::Break(_) => {
            html.push_str("<br />\n");
        }
        Node::Delete(del) => {
            html.push_str("<del>");
            for child in &del.children {
                node_to_html(child, html, base_dir, embed_mode, strict, highlight)?;
            }
            html.push_str("</del>");
        }
        Node::Table(table) => {
            html.push_str("<table>\n<thead>\n");
            if let Some(first_row) = table.children.first() {
                render_table_row(first_row, html, &table.align, true, base_dir, embed_mode, strict, highlight)?;
            }
            html.push_str("</thead>\n<tbody>\n");
            for row in table.children.iter().skip(1) {
                render_table_row(row, html, &table.align, false, base_dir, embed_mode, strict, highlight)?;
            }
            html.push_str("</tbody>\n</table>\n");
        }
        Node::Html(raw) => {
            html.push_str(&raw.value);
        }
        _ => {}
    }
    Ok(())
}

fn render_table_row(
    node: &Node,
    html: &mut String,
    align: &[AlignKind],
    is_header: bool,
    base_dir: &Path,
    embed_mode: EmbedMode,
    strict: bool,
    highlight: Option<&HighlightContext>,
) -> Result<(), ImageError> {
    if let Node::TableRow(row) = node {
        html.push_str("<tr>\n");
        for (i, cell) in row.children.iter().enumerate() {
            let tag = if is_header { "th" } else { "td" };
            let align_attr = match align.get(i) {
                Some(AlignKind::Left) => " align=\"left\"",
                Some(AlignKind::Center) => " align=\"center\"",
                Some(AlignKind::Right) => " align=\"right\"",
                _ => "",
            };
            html.push_str(&format!("<{}{}>", tag, align_attr));
            if let Node::TableCell(cell) = cell {
                for child in &cell.children {
                    node_to_html(child, html, base_dir, embed_mode, strict, highlight)?;
                }
            }
            html.push_str(&format!("</{}>\n", tag));
        }
        html.push_str("</tr>\n");
    }
    Ok(())
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
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

    fn render_html(md: &str) -> String {
        let ast = parse_markdown(md);
        mdast_to_html(&ast, Path::new("."), crate::EmbedMode::None, false, None).unwrap()
    }

    #[test]
    fn test_html_escape() {
        assert_eq!(html_escape("hello"), "hello");
        assert_eq!(html_escape("<script>"), "&lt;script&gt;");
        assert_eq!(html_escape("a & b"), "a &amp; b");
        assert_eq!(html_escape("\"quoted\""), "&quot;quoted&quot;");
        assert_eq!(
            html_escape("<a href=\"test\">"),
            "&lt;a href=&quot;test&quot;&gt;"
        );
    }

    #[test]
    fn test_heading() {
        assert_eq!(render_html("# Heading 1"), "<h1>Heading 1</h1>\n");
        assert_eq!(render_html("## Heading 2"), "<h2>Heading 2</h2>\n");
        assert_eq!(render_html("### Heading 3"), "<h3>Heading 3</h3>\n");
        assert_eq!(render_html("###### Heading 6"), "<h6>Heading 6</h6>\n");
    }

    #[test]
    fn test_paragraph() {
        assert_eq!(render_html("Hello world"), "<p>Hello world</p>\n");
    }

    #[test]
    fn test_strong() {
        assert_eq!(
            render_html("**bold text**"),
            "<p><strong>bold text</strong></p>\n"
        );
    }

    #[test]
    fn test_emphasis() {
        assert_eq!(render_html("*italic text*"), "<p><em>italic text</em></p>\n");
    }

    #[test]
    fn test_inline_code() {
        assert_eq!(render_html("`code`"), "<p><code>code</code></p>\n");
    }

    #[test]
    fn test_inline_code_escapes_html() {
        assert_eq!(
            render_html("`<script>`"),
            "<p><code>&lt;script&gt;</code></p>\n"
        );
    }

    #[test]
    fn test_code_block_no_highlight() {
        let html = render_html("```\ncode\n```");
        assert!(html.contains("<pre><code>"));
        assert!(html.contains("code"));
        assert!(html.contains("</code></pre>"));
    }

    #[test]
    fn test_code_block_with_language() {
        let html = render_html("```rust\nfn main() {}\n```");
        assert!(html.contains("class=\"language-rust\""));
    }

    #[test]
    fn test_link() {
        assert_eq!(
            render_html("[link](https://example.com)"),
            "<p><a href=\"https://example.com\">link</a></p>\n"
        );
    }

    #[test]
    fn test_link_escapes_url() {
        let html = render_html("[link](https://example.com?a=1&b=2)");
        assert!(html.contains("href=\"https://example.com?a=1&amp;b=2\""));
    }

    #[test]
    fn test_unordered_list() {
        let html = render_html("- item 1\n- item 2");
        assert!(html.contains("<ul>"));
        assert!(html.contains("<li>"));
        assert!(html.contains("item 1"));
        assert!(html.contains("item 2"));
        assert!(html.contains("</li>"));
        assert!(html.contains("</ul>"));
    }

    #[test]
    fn test_ordered_list() {
        let html = render_html("1. first\n2. second");
        assert!(html.contains("<ol>"));
        assert!(html.contains("<li>"));
        assert!(html.contains("first"));
        assert!(html.contains("second"));
        assert!(html.contains("</li>"));
        assert!(html.contains("</ol>"));
    }

    #[test]
    fn test_blockquote() {
        let html = render_html("> quoted text");
        assert!(html.contains("<blockquote>"));
        assert!(html.contains("quoted text"));
        assert!(html.contains("</blockquote>"));
    }

    #[test]
    fn test_thematic_break() {
        assert!(render_html("---").contains("<hr />"));
    }

    #[test]
    fn test_line_break() {
        // Two spaces at end of line create a hard break
        let html = render_html("line one  \nline two");
        assert!(html.contains("<br />"));
    }

    #[test]
    fn test_strikethrough() {
        let html = render_html("~~deleted~~");
        assert!(html.contains("<del>"));
        assert!(html.contains("deleted"));
        assert!(html.contains("</del>"));
    }

    #[test]
    fn test_table() {
        let md = "| A | B |\n|---|---|\n| 1 | 2 |";
        let html = render_html(md);
        assert!(html.contains("<table>"));
        assert!(html.contains("<thead>"));
        assert!(html.contains("<tbody>"));
        assert!(html.contains("<th>"));
        assert!(html.contains("<td>"));
        assert!(html.contains("</table>"));
    }

    #[test]
    fn test_table_alignment() {
        let md = "| Left | Center | Right |\n|:-----|:------:|------:|\n| L | C | R |";
        let html = render_html(md);
        assert!(html.contains("align=\"left\""));
        assert!(html.contains("align=\"center\""));
        assert!(html.contains("align=\"right\""));
    }

    #[test]
    fn test_image_embed_none() {
        let html = render_html("![alt text](image.png)");
        assert!(html.contains("<img"));
        assert!(html.contains("src=\"image.png\""));
        assert!(html.contains("alt=\"alt text\""));
    }

    #[test]
    fn test_image_uses_url_as_alt_when_empty() {
        let html = render_html("![](image.png)");
        assert!(html.contains("alt=\"image.png\""));
    }

    #[test]
    fn test_raw_html_passthrough() {
        let html = render_html("<div>raw html</div>");
        assert!(html.contains("<div>raw html</div>"));
    }

    #[test]
    fn test_nested_formatting() {
        let html = render_html("**bold and *italic* text**");
        assert!(html.contains("<strong>"));
        assert!(html.contains("<em>"));
        assert!(html.contains("italic"));
        assert!(html.contains("</em>"));
        assert!(html.contains("</strong>"));
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
        let html = render_html(md);
        assert!(html.contains("<h1>Title</h1>"));
        assert!(html.contains("<strong>bold</strong>"));
        assert!(html.contains("<em>italic</em>"));
        assert!(html.contains("<ul>"));
        assert!(html.contains("<pre><code"));
        assert!(html.contains("<blockquote>"));
    }
}
