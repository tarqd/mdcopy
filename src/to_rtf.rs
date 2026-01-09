use crate::image::{load_image_with_fallback, ImageError};
use crate::EmbedMode;
use log::warn;
use markdown::mdast::{AlignKind, Node};
use std::path::Path;

pub fn mdast_to_rtf(
    node: &Node,
    base_dir: &Path,
    embed_mode: EmbedMode,
    strict: bool,
) -> Result<String, ImageError> {
    let mut rtf =
        String::from("{\\rtf1\\ansi\\deff0{\\fonttbl{\\f0 Helvetica;}{\\f1 Courier;}}\\f0\\fs24 ");
    node_to_rtf(
        node,
        &mut rtf,
        &mut RtfContext::new(base_dir, embed_mode, strict),
    )?;
    rtf.push('}');
    Ok(rtf)
}

struct RtfContext<'a> {
    base_dir: &'a Path,
    embed_mode: EmbedMode,
    strict: bool,
    table_align: Vec<AlignKind>,
    table_cell_index: usize,
    in_table_header: bool,
}

impl<'a> RtfContext<'a> {
    fn new(base_dir: &'a Path, embed_mode: EmbedMode, strict: bool) -> Self {
        Self {
            base_dir,
            embed_mode,
            strict,
            table_align: Vec::new(),
            table_cell_index: 0,
            in_table_header: false,
        }
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
            rtf.push_str("{\\f1\\fs20 ");
            push_rtf_escaped(rtf, &code.value);
            rtf.push_str("}\\par ");
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
