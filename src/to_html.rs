use crate::image::load_image;
use crate::EmbedMode;
use markdown::mdast::{AlignKind, Node};
use std::path::Path;

pub fn mdast_to_html(node: &Node, base_dir: &Path, embed_mode: EmbedMode) -> String {
    let mut html = String::new();
    node_to_html(node, &mut html, base_dir, embed_mode);
    html
}

fn node_to_html(node: &Node, html: &mut String, base_dir: &Path, embed_mode: EmbedMode) {
    match node {
        Node::Root(root) => {
            for child in &root.children {
                node_to_html(child, html, base_dir, embed_mode);
            }
        }
        Node::Heading(heading) => {
            html.push_str(&format!("<h{}>", heading.depth));
            for child in &heading.children {
                node_to_html(child, html, base_dir, embed_mode);
            }
            html.push_str(&format!("</h{}>\n", heading.depth));
        }
        Node::Paragraph(para) => {
            html.push_str("<p>");
            for child in &para.children {
                node_to_html(child, html, base_dir, embed_mode);
            }
            html.push_str("</p>\n");
        }
        Node::Text(text) => {
            html.push_str(&html_escape(&text.value));
        }
        Node::Strong(strong) => {
            html.push_str("<strong>");
            for child in &strong.children {
                node_to_html(child, html, base_dir, embed_mode);
            }
            html.push_str("</strong>");
        }
        Node::Emphasis(em) => {
            html.push_str("<em>");
            for child in &em.children {
                node_to_html(child, html, base_dir, embed_mode);
            }
            html.push_str("</em>");
        }
        Node::InlineCode(code) => {
            html.push_str("<code>");
            html.push_str(&html_escape(&code.value));
            html.push_str("</code>");
        }
        Node::Code(code) => {
            html.push_str("<pre><code");
            if let Some(lang) = &code.lang {
                html.push_str(&format!(" class=\"language-{}\"", html_escape(lang)));
            }
            html.push('>');
            html.push_str(&html_escape(&code.value));
            html.push_str("</code></pre>\n");
        }
        Node::Link(link) => {
            html.push_str(&format!("<a href=\"{}\">", html_escape(&link.url)));
            for child in &link.children {
                node_to_html(child, html, base_dir, embed_mode);
            }
            html.push_str("</a>");
        }
        Node::Image(image) => {
            let src = if let Some(img) = load_image(&image.url, base_dir, embed_mode) {
                img.to_data_url()
            } else {
                image.url.clone()
            };
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
                node_to_html(child, html, base_dir, embed_mode);
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
                node_to_html(child, html, base_dir, embed_mode);
            }
            html.push_str("</li>\n");
        }
        Node::Blockquote(bq) => {
            html.push_str("<blockquote>\n");
            for child in &bq.children {
                node_to_html(child, html, base_dir, embed_mode);
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
                node_to_html(child, html, base_dir, embed_mode);
            }
            html.push_str("</del>");
        }
        Node::Table(table) => {
            html.push_str("<table>\n<thead>\n");
            if let Some(first_row) = table.children.first() {
                render_table_row(first_row, html, &table.align, true, base_dir, embed_mode);
            }
            html.push_str("</thead>\n<tbody>\n");
            for row in table.children.iter().skip(1) {
                render_table_row(row, html, &table.align, false, base_dir, embed_mode);
            }
            html.push_str("</tbody>\n</table>\n");
        }
        Node::Html(raw) => {
            html.push_str(&raw.value);
        }
        _ => {}
    }
}

fn render_table_row(
    node: &Node,
    html: &mut String,
    align: &[AlignKind],
    is_header: bool,
    base_dir: &Path,
    embed_mode: EmbedMode,
) {
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
                    node_to_html(child, html, base_dir, embed_mode);
                }
            }
            html.push_str(&format!("</{}>\n", tag));
        }
        html.push_str("</tr>\n");
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
