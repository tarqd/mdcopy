//! macOS-specific NSAttributedString conversion for perfect paste compatibility
//!
//! This module converts markdown AST to NSAttributedString, which provides the best
//! clipboard compatibility with native macOS apps (TextEdit, Notes, Mail, Pages).
//!
//! ## Design Philosophy
//!
//! Unlike HTML/RTF output, NSAttributedString **always embeds images** for optimal
//! clipboard behavior. There's no "embed mode" - images are always embedded as
//! NSTextAttachment objects, which is how native macOS apps expect them.
//!
//! ## Implementation Status
//!
//! This is a **work in progress**. The basic skeleton is in place, but key features need implementation:
//!
//! ### TODO:
//! - [x] ~~Bold formatting~~ **DONE!**
//! - [x] ~~Italic formatting~~ **DONE!**
//! - [x] ~~Headings~~ **DONE!**
//! - [x] ~~NSTextAttachment for images~~ **DONE!**
//! - [x] ~~Monospace font for inline code~~ **DONE!**
//! - [x] ~~Links~~ **DONE!**
//! - [x] ~~Strikethrough formatting~~ **DONE!**
//! - [x] ~~Code blocks with background color~~ **DONE!**
//! - [x] ~~Lists (basic bullet points)~~ **DONE!**
//! - [x] ~~Blockquotes (gray text)~~ **DONE!**
//! - [x] ~~NSTextTable for markdown tables~~ **DONE!**
//! - [ ] Advanced paragraph styles (indentation, spacing)
//! - [ ] Table column alignment (left/center/right)
//!
//! ### Implemented Features:
//! - **Image embedding** ✅: Both local and remote images via NSTextAttachment
//! - **Bold text** ✅: Using `NSFont::boldSystemFontOfSize()`
//! - **Italic text** ✅: Using `NSFontDescriptor` with `NSFontItalicTrait`
//! - **Headings** ✅: Scaled fonts (H1=2x, H2=1.5x, H3=1.17x) with bold weight
//! - **Inline code** ✅: Monospaced font via `NSFont::monospacedSystemFontOfSize_weight()`
//! - **Code blocks** ✅: Monospace font + light gray background (RGB: 0.95, 0.95, 0.95)
//! - **Links** ✅: Clickable links using `NSLinkAttributeName`
//! - **Strikethrough** ✅: Using `NSStrikethroughStyleAttributeName`
//! - **Lists** ✅: Bullet points (• character) for unordered lists
//! - **Blockquotes** ✅: Gray text color (RGB: 0.5, 0.5, 0.5)
//! - **Tables** ✅: Using `NSTextTable` and `NSTextTableBlock` with borders and padding
//!
//! ### References:
//! - NSAttributedString: https://developer.apple.com/documentation/foundation/nsattributedstring
//! - NSTextAttachment: https://developer.apple.com/documentation/appkit/nstextattachment
//! - NSTextTable: https://developer.apple.com/documentation/appkit/nstexttable
//! - Attributed String Programming Guide: https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/AttributedStrings/

#![cfg(target_os = "macos")]

use log::{debug, warn};
use markdown::mdast::Node;
use std::path::Path;

use objc2::AnyThread;
use objc2::rc::{Retained, autoreleasepool};
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2_app_kit::{
    NSAttributedStringAttachmentConveniences, NSBackgroundColorAttributeName, NSColor, NSFont,
    NSFontAttributeName, NSFontDescriptorSymbolicTraits, NSFontItalicTrait,
    NSForegroundColorAttributeName, NSImage, NSLinkAttributeName, NSMutableParagraphStyle,
    NSParagraphStyleAttributeName, NSPasteboard, NSPasteboardWriting,
    NSStrikethroughStyleAttributeName, NSTextAttachment, NSTextBlock, NSTextTable,
    NSTextTableBlock,
};
use objc2_foundation::{NSAttributedString, NSMutableAttributedString, NSRange, NSString, NSURL};

/// Convert markdown AST to NSMutableAttributedString
///
/// This is the main entry point for macOS clipboard writing. The resulting
/// attributed string can be written directly to NSPasteboard.
///
/// Images are always embedded as NSTextAttachment, regardless of whether they're
/// local or remote. This provides the best paste experience in macOS apps.
pub fn mdast_to_nsattributed_string(
    node: &Node,
    base_dir: &Path,
) -> Result<Retained<NSMutableAttributedString>, String> {
    autoreleasepool(|_| {
        let attr_string = NSMutableAttributedString::new();
        let mut ctx = AttributedStringContext::new(base_dir);

        node_to_attributed_string(node, &attr_string, &mut ctx)?;

        Ok(attr_string)
    })
}

/// Write NSAttributedString to the macOS pasteboard
///
/// This writes the attributed string directly to NSPasteboard, allowing macOS apps
/// to get rich text with embedded images when pasting.
pub fn write_to_pasteboard(attr_string: &NSAttributedString) -> Result<(), String> {
    autoreleasepool(|_| {
        let pasteboard = NSPasteboard::generalPasteboard();
        pasteboard.clearContents();

        // Write the attributed string directly - macOS will automatically provide
        // multiple representations (RTFD, RTF, plain text, etc.)
        // NSAttributedString implements NSPasteboardWriting, so we need to convert it
        let protocol_obj: &ProtocolObject<dyn NSPasteboardWriting> =
            ProtocolObject::from_ref(attr_string);
        let objects = objc2_foundation::NSArray::from_slice(&[protocol_obj]);

        if !pasteboard.writeObjects(&objects) {
            return Err("Failed to write attributed string to pasteboard".into());
        }

        debug!("Wrote NSAttributedString to pasteboard");
        Ok(())
    })
}

/// Context for building attributed string
struct AttributedStringContext<'a> {
    base_dir: &'a Path,
}

impl<'a> AttributedStringContext<'a> {
    fn new(base_dir: &'a Path) -> Self {
        Self { base_dir }
    }
}

/// Recursively convert markdown AST node to attributed string
fn node_to_attributed_string(
    node: &Node,
    attr_string: &NSMutableAttributedString,
    ctx: &mut AttributedStringContext,
) -> Result<(), String> {
    match node {
        Node::Root(root) => {
            for child in &root.children {
                node_to_attributed_string(child, attr_string, ctx)?;
            }
        }
        Node::Paragraph(para) => {
            for child in &para.children {
                node_to_attributed_string(child, attr_string, ctx)?;
            }
            // Add paragraph break
            append_text(attr_string, "\n");
        }
        Node::Text(text) => {
            append_text(attr_string, &text.value);
        }
        Node::Strong(strong) => {
            let temp_string = NSMutableAttributedString::new();
            for child in &strong.children {
                node_to_attributed_string(child, &temp_string, ctx)?;
            }
            let range = NSRange::new(0, temp_string.length());
            apply_bold(&temp_string, range);
            attr_string.appendAttributedString(&temp_string);
        }
        Node::Emphasis(em) => {
            let temp_string = NSMutableAttributedString::new();
            for child in &em.children {
                node_to_attributed_string(child, &temp_string, ctx)?;
            }
            let range = NSRange::new(0, temp_string.length());
            apply_italic(&temp_string, range);
            attr_string.appendAttributedString(&temp_string);
        }
        Node::Heading(heading) => {
            let temp_string = NSMutableAttributedString::new();
            for child in &heading.children {
                node_to_attributed_string(child, &temp_string, ctx)?;
            }
            let range = NSRange::new(0, temp_string.length());
            apply_heading(&temp_string, range, heading.depth);
            attr_string.appendAttributedString(&temp_string);
            append_text(attr_string, "\n\n");
        }
        Node::Image(image) => {
            embed_image(attr_string, &image.url, &image.alt, ctx)?;
        }
        Node::InlineCode(code) => {
            let temp_string = NSMutableAttributedString::new();
            append_text(&temp_string, &code.value);
            let range = NSRange::new(0, temp_string.length());
            apply_monospace(&temp_string, range);
            attr_string.appendAttributedString(&temp_string);
        }
        Node::Link(link) => {
            let temp_string = NSMutableAttributedString::new();
            for child in &link.children {
                node_to_attributed_string(child, &temp_string, ctx)?;
            }
            let range = NSRange::new(0, temp_string.length());
            apply_link(&temp_string, range, &link.url);
            attr_string.appendAttributedString(&temp_string);
        }
        Node::Delete(del) => {
            let temp_string = NSMutableAttributedString::new();
            for child in &del.children {
                node_to_attributed_string(child, &temp_string, ctx)?;
            }
            let range = NSRange::new(0, temp_string.length());
            apply_strikethrough(&temp_string, range);
            attr_string.appendAttributedString(&temp_string);
        }
        Node::Code(code) => {
            let temp_string = NSMutableAttributedString::new();
            append_text(&temp_string, &code.value);
            append_text(&temp_string, "\n");
            let range = NSRange::new(0, temp_string.length());
            apply_code_block(&temp_string, range);
            attr_string.appendAttributedString(&temp_string);
        }
        Node::List(list) => {
            for child in &list.children {
                node_to_attributed_string(child, attr_string, ctx)?;
            }
            append_text(attr_string, "\n");
        }
        Node::ListItem(item) => {
            // Add bullet point or number
            append_text(attr_string, "• ");
            for child in &item.children {
                node_to_attributed_string(child, attr_string, ctx)?;
            }
        }
        Node::Blockquote(quote) => {
            let temp_string = NSMutableAttributedString::new();
            for child in &quote.children {
                node_to_attributed_string(child, &temp_string, ctx)?;
            }
            let range = NSRange::new(0, temp_string.length());
            apply_blockquote(&temp_string, range);
            attr_string.appendAttributedString(&temp_string);
        }
        Node::Table(table) => {
            render_table(attr_string, table, ctx)?;
        }
        Node::TableRow(_) | Node::TableCell(_) => {
            // These are handled by render_table, should not be encountered directly
        }
        _ => {
            warn!(
                "Unhandled node type in NSAttributedString conversion: {:?}",
                std::any::type_name_of_val(node)
            );
        }
    }
    Ok(())
}

/// Append plain text to attributed string
fn append_text(attr_string: &NSMutableAttributedString, text: &str) {
    let ns_string = NSString::from_str(text);
    let append_string = NSAttributedString::initWithString(NSAttributedString::alloc(), &ns_string);
    attr_string.appendAttributedString(&append_string);
}

/// Apply bold formatting to a range
fn apply_bold(attr_string: &NSMutableAttributedString, range: NSRange) {
    unsafe {
        // Get the current font or use system font
        let current_font = attr_string.attribute_atIndex_effectiveRange(
            NSFontAttributeName,
            range.location,
            std::ptr::null_mut(),
        );

        let font_size = if let Some(font_obj) = current_font {
            // Try to get the current font and its size
            if let Some(current_font) = font_obj.downcast_ref::<NSFont>() {
                current_font.pointSize()
            } else {
                NSFont::systemFontSize()
            }
        } else {
            NSFont::systemFontSize()
        };

        // Create bold font
        let bold_font = NSFont::boldSystemFontOfSize(font_size);

        // Apply the bold font to the range
        attr_string.addAttribute_value_range(NSFontAttributeName, &bold_font as &AnyObject, range);
    }
}

/// Apply italic formatting to a range
///
/// Uses NSFontDescriptor with symbolic traits to create an italic font.
fn apply_italic(attr_string: &NSMutableAttributedString, range: NSRange) {
    unsafe {
        // Get the current font or use system font
        let current_font = attr_string.attribute_atIndex_effectiveRange(
            NSFontAttributeName,
            range.location,
            std::ptr::null_mut(),
        );

        // Extract font size first to avoid lifetime issues
        let font_size = if let Some(ref font_obj) = current_font {
            if let Some(font) = font_obj.downcast_ref::<NSFont>() {
                font.pointSize()
            } else {
                NSFont::systemFontSize()
            }
        } else {
            NSFont::systemFontSize()
        };

        // Get the font descriptor from current font or create a new one
        let descriptor = if let Some(ref font_obj) = current_font {
            if let Some(font) = font_obj.downcast_ref::<NSFont>() {
                font.fontDescriptor()
            } else {
                let system_font = NSFont::systemFontOfSize(font_size);
                system_font.fontDescriptor()
            }
        } else {
            let system_font = NSFont::systemFontOfSize(font_size);
            system_font.fontDescriptor()
        };

        // Get current symbolic traits and add italic trait
        let current_traits = descriptor.symbolicTraits();
        let italic_traits = current_traits | NSFontDescriptorSymbolicTraits(NSFontItalicTrait);

        // Create new descriptor with italic trait
        let italic_descriptor = descriptor.fontDescriptorWithSymbolicTraits(italic_traits);

        // Create font from the italic descriptor
        let italic_font = NSFont::fontWithDescriptor_size(&italic_descriptor, font_size)
            .unwrap_or_else(|| NSFont::systemFontOfSize(font_size));

        // Apply the italic font to the range
        attr_string.addAttribute_value_range(
            NSFontAttributeName,
            &italic_font as &AnyObject,
            range,
        );
    }
}

/// Apply heading formatting to a range
///
/// Headings use larger font sizes:
/// - H1: 2x base size
/// - H2: 1.5x base size
/// - H3: 1.17x base size
/// - H4-H6: base size (bold via markdown strong)
fn apply_heading(attr_string: &NSMutableAttributedString, range: NSRange, depth: u8) {
    unsafe {
        let base_size = NSFont::systemFontSize();

        let heading_size = match depth {
            1 => base_size * 2.0,  // H1: 2em
            2 => base_size * 1.5,  // H2: 1.5em
            3 => base_size * 1.17, // H3: 1.17em
            _ => base_size,        // H4-H6: 1em
        };

        // Use bold font for headings
        let heading_font = NSFont::boldSystemFontOfSize(heading_size);

        // Apply the heading font to the range
        attr_string.addAttribute_value_range(
            NSFontAttributeName,
            &heading_font as &AnyObject,
            range,
        );
    }
}

/// Apply monospace font to a range (for inline code)
///
/// Uses the system's monospaced font at the current or default size.
fn apply_monospace(attr_string: &NSMutableAttributedString, range: NSRange) {
    unsafe {
        // Get current font size or use system default
        let current_font = attr_string.attribute_atIndex_effectiveRange(
            NSFontAttributeName,
            range.location,
            std::ptr::null_mut(),
        );

        let font_size = if let Some(font_obj) = current_font {
            if let Some(current_font) = font_obj.downcast_ref::<NSFont>() {
                current_font.pointSize()
            } else {
                NSFont::systemFontSize()
            }
        } else {
            NSFont::systemFontSize()
        };

        // Create monospaced font using userFixedPitchFontOfSize for compatibility
        // This is more widely supported than monospacedSystemFontOfSize_weight
        let mono_font = NSFont::userFixedPitchFontOfSize(font_size)
            .unwrap_or_else(|| NSFont::systemFontOfSize(font_size));

        // Apply the monospaced font to the range
        attr_string.addAttribute_value_range(NSFontAttributeName, &mono_font as &AnyObject, range);
    }
}

/// Apply link formatting to a range
///
/// Sets the NSLinkAttributeName to make the text clickable when pasted.
/// macOS apps will render this as a blue underlined link.
fn apply_link(attr_string: &NSMutableAttributedString, range: NSRange, url: &str) {
    unsafe {
        let ns_url_string = NSString::from_str(url);

        // Apply the link attribute
        attr_string.addAttribute_value_range(
            NSLinkAttributeName,
            &ns_url_string as &AnyObject,
            range,
        );
    }
}

/// Apply strikethrough formatting to a range
///
/// Uses NSStrikethroughStyleAttributeName with single line style.
fn apply_strikethrough(attr_string: &NSMutableAttributedString, range: NSRange) {
    unsafe {
        // NSUnderlineStyleSingle = 1
        // We need to box the value to pass it as an object
        use objc2::rc::Retained;
        use objc2_foundation::NSNumber;
        let style: Retained<NSNumber> = NSNumber::new_i32(1);

        // Apply strikethrough style
        attr_string.addAttribute_value_range(
            NSStrikethroughStyleAttributeName,
            &style as &AnyObject,
            range,
        );
    }
}

/// Apply code block formatting to a range
///
/// Uses monospace font and light gray background color for code blocks.
fn apply_code_block(attr_string: &NSMutableAttributedString, range: NSRange) {
    unsafe {
        // Get current or default font size
        let current_font = attr_string.attribute_atIndex_effectiveRange(
            NSFontAttributeName,
            range.location,
            std::ptr::null_mut(),
        );

        let font_size = if let Some(font_obj) = current_font {
            if let Some(current_font) = font_obj.downcast_ref::<NSFont>() {
                current_font.pointSize()
            } else {
                NSFont::systemFontSize()
            }
        } else {
            NSFont::systemFontSize()
        };

        // Apply monospace font using userFixedPitchFontOfSize for compatibility
        // This is more widely supported than monospacedSystemFontOfSize_weight
        let mono_font = NSFont::userFixedPitchFontOfSize(font_size)
            .unwrap_or_else(|| NSFont::systemFontOfSize(font_size));
        attr_string.addAttribute_value_range(NSFontAttributeName, &mono_font as &AnyObject, range);

        // Apply light gray background color (RGB: 0.95, 0.95, 0.95)
        let bg_color = NSColor::colorWithRed_green_blue_alpha(0.95, 0.95, 0.95, 1.0);
        attr_string.addAttribute_value_range(
            NSBackgroundColorAttributeName,
            &bg_color as &AnyObject,
            range,
        );
    }
}

/// Apply blockquote formatting to a range
///
/// This is a simplified implementation. Ideally would use NSParagraphStyle
/// with leftIndent, but for now we just add a visual indicator.
fn apply_blockquote(attr_string: &NSMutableAttributedString, range: NSRange) {
    // For now, just a placeholder. Proper implementation would use:
    // - NSParagraphStyle with leftIndent
    // - Possibly a border/bar on the left (harder in attributed strings)
    // - Different text color (gray)
    unsafe {
        // Apply gray color to blockquotes
        let gray_color = NSColor::colorWithRed_green_blue_alpha(0.5, 0.5, 0.5, 1.0);
        attr_string.addAttribute_value_range(
            NSForegroundColorAttributeName,
            &gray_color as &AnyObject,
            range,
        );
    }
}

/// Embed an image as NSTextAttachment
///
/// Attempts to load the image using NSImage (which handles all formats natively),
/// then embeds it as an NSTextAttachment for inline rendering when pasted.
///
/// Supports both local file paths and remote URLs. NSImage loads the data lazily
/// when the attributed string is written to the pasteboard.
fn embed_image(
    attr_string: &NSMutableAttributedString,
    url: &str,
    alt: &str,
    ctx: &mut AttributedStringContext,
) -> Result<(), String> {
    // Determine if this is a local file or remote URL
    let ns_image = if url.starts_with("http://") || url.starts_with("https://") {
        // Remote URL - let NSImage fetch it
        let nsurl = NSURL::URLWithString(&NSString::from_str(url));
        if let Some(nsurl) = nsurl {
            NSImage::initWithContentsOfURL(NSImage::alloc(), &nsurl)
        } else {
            warn!("Failed to create NSURL for: {}", url);
            None
        }
    } else {
        // Local file path - resolve relative to base_dir
        let path = if std::path::Path::new(url).is_absolute() {
            url.to_string()
        } else {
            ctx.base_dir.join(url).to_string_lossy().to_string()
        };

        let ns_path = NSString::from_str(&path);
        NSImage::initWithContentsOfFile(NSImage::alloc(), &ns_path)
    };

    if let Some(ns_image) = ns_image {
        // Create NSTextAttachment with the image
        let attachment = NSTextAttachment::new();

        // Set the image on the attachment (macOS 10.11+)
        attachment.setImage(Some(&ns_image));

        // Create attributed string from attachment
        let attachment_string = NSAttributedString::attributedStringWithAttachment(&attachment);

        // Append to the main attributed string
        attr_string.appendAttributedString(&attachment_string);

        debug!("Image embedded successfully: {}", url);
    } else {
        warn!("Failed to load image: {}", url);
        // Fallback: insert alt text with URL
        append_text(attr_string, &format!("[{}]({})", alt, url));
    }

    Ok(())
}

/// Render a table using NSTextTable and NSTextTableBlock
///
/// This uses the NSTextTable API to create proper table layouts. Each cell gets
/// its own NSTextTableBlock which is attached to the text via NSParagraphStyle.
/// Cell content is recursively rendered, so all formatting (bold, italic, code, etc.)
/// works inside table cells.
fn render_table(
    attr_string: &NSMutableAttributedString,
    table: &markdown::mdast::Table,
    ctx: &mut AttributedStringContext,
) -> Result<(), String> {
    use markdown::mdast::Node;

    // Determine number of columns from first row
    let num_columns = if let Some(Node::TableRow(first_row)) = table.children.first() {
        first_row.children.len()
    } else {
        return Ok(()); // Empty table
    };

    // Create NSTextTable
    let ns_table = NSTextTable::new();
    ns_table.setNumberOfColumns(num_columns);

    // Add a newline before table if there's existing content
    if attr_string.length() > 0 {
        append_text(attr_string, "\n");
    }

    // Render each row
    for (row_idx, row_node) in table.children.iter().enumerate() {
        if let Node::TableRow(row) = row_node {
            let is_header = row_idx == 0;

            // Render each cell in the row
            for (col_idx, cell_node) in row.children.iter().enumerate() {
                if let Node::TableCell(cell) = cell_node {
                    // Create a temporary attributed string for cell content
                    let cell_string = NSMutableAttributedString::new();

                    // Recursively render cell contents (handles bold, italic, code, etc.)
                    for child in &cell.children {
                        node_to_attributed_string(child, &cell_string, ctx)?;
                    }

                    // Add newline at end of cell content (required by NSTextTable)
                    append_text(&cell_string, "\n");

                    // Apply bold to header cells
                    if is_header && cell_string.length() > 0 {
                        let range = NSRange::new(0, cell_string.length() - 1); // Exclude the newline
                        apply_bold(&cell_string, range);
                    }

                    // Create NSTextTableBlock for this cell
                    let text_block = NSTextTableBlock::initWithTable_startingRow_rowSpan_startingColumn_columnSpan(
                        NSTextTableBlock::alloc(),
                        &ns_table,
                        row_idx as isize,
                        1,
                        col_idx as isize,
                        1,
                    );

                    // Configure cell borders and padding
                    unsafe {
                        // Set border width (1.0 point)
                        text_block.setWidth_type_forLayer(
                            1.0,
                            objc2_app_kit::NSTextBlockValueType::AbsoluteValueType,
                            objc2_app_kit::NSTextBlockLayer::Border,
                        );

                        // Set border color (light gray)
                        let border_color =
                            NSColor::colorWithRed_green_blue_alpha(0.8, 0.8, 0.8, 1.0);
                        text_block.setBorderColor(Some(&border_color));

                        // Set padding (4.0 points)
                        text_block.setWidth_type_forLayer(
                            4.0,
                            objc2_app_kit::NSTextBlockValueType::AbsoluteValueType,
                            objc2_app_kit::NSTextBlockLayer::Padding,
                        );
                    }

                    // Create paragraph style with the table block
                    let paragraph_style = NSMutableParagraphStyle::new();
                    let blocks_array =
                        objc2_foundation::NSArray::from_slice(&[&text_block as &NSTextBlock]);
                    paragraph_style.setTextBlocks(&blocks_array);

                    // Apply paragraph style to the entire cell content
                    let full_range = NSRange::new(0, cell_string.length());
                    cell_string.addAttribute_value_range(
                        NSParagraphStyleAttributeName,
                        &paragraph_style as &AnyObject,
                        full_range,
                    );

                    // Append cell to main attributed string
                    attr_string.appendAttributedString(&cell_string);
                }
            }
        }
    }

    // Add newline after table
    append_text(attr_string, "\n");

    Ok(())
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

    #[test]
    fn test_basic_text() {
        let ast = parse_markdown("Hello world");
        let result = mdast_to_nsattributed_string(&ast, Path::new("."));
        assert!(result.is_ok());
        let attr_string = result.unwrap();
        assert!(attr_string.length() > 0);
    }

    #[test]
    fn test_bold_text() {
        let ast = parse_markdown("**bold**");
        let result = mdast_to_nsattributed_string(&ast, Path::new("."));
        assert!(result.is_ok());
    }

    #[test]
    fn test_italic_text() {
        let ast = parse_markdown("*italic*");
        let result = mdast_to_nsattributed_string(&ast, Path::new("."));
        assert!(result.is_ok());
    }

    #[test]
    fn test_bold_italic() {
        let ast = parse_markdown("***bold and italic***");
        let result = mdast_to_nsattributed_string(&ast, Path::new("."));
        assert!(result.is_ok());
    }

    #[test]
    fn test_heading() {
        let ast = parse_markdown("# Heading 1");
        let result = mdast_to_nsattributed_string(&ast, Path::new("."));
        assert!(result.is_ok());
    }

    #[test]
    fn test_inline_code() {
        let ast = parse_markdown("`code`");
        let result = mdast_to_nsattributed_string(&ast, Path::new("."));
        assert!(result.is_ok());
    }

    #[test]
    fn test_link() {
        let ast = parse_markdown("[example](https://example.com)");
        let result = mdast_to_nsattributed_string(&ast, Path::new("."));
        assert!(result.is_ok());
    }

    #[test]
    fn test_strikethrough() {
        let ast = parse_markdown("~~deleted~~");
        let result = mdast_to_nsattributed_string(&ast, Path::new("."));
        assert!(result.is_ok());
    }

    #[test]
    fn test_mixed_formatting() {
        let ast = parse_markdown("**bold** and `code` and [link](url) and ~~strike~~");
        let result = mdast_to_nsattributed_string(&ast, Path::new("."));
        assert!(result.is_ok());
    }

    #[test]
    fn test_code_block() {
        let ast = parse_markdown("```rust\nfn main() {}\n```");
        let result = mdast_to_nsattributed_string(&ast, Path::new("."));
        assert!(result.is_ok());
    }

    #[test]
    fn test_list() {
        let ast = parse_markdown("- Item 1\n- Item 2\n- Item 3");
        let result = mdast_to_nsattributed_string(&ast, Path::new("."));
        assert!(result.is_ok());
    }

    #[test]
    fn test_blockquote() {
        let ast = parse_markdown("> This is a quote\n> with multiple lines");
        let result = mdast_to_nsattributed_string(&ast, Path::new("."));
        assert!(result.is_ok());
    }

    #[test]
    fn test_table() {
        let ast = parse_markdown(
            "| Header 1 | Header 2 |\n\
             |----------|----------|\n\
             | Cell 1   | Cell 2   |\n\
             | Cell 3   | Cell 4   |",
        );
        let result = mdast_to_nsattributed_string(&ast, Path::new("."));
        assert!(result.is_ok());
    }

    #[test]
    fn test_table_with_formatting() {
        let ast = parse_markdown(
            "| **Bold** | *Italic* |\n\
             |----------|----------|\n\
             | `code`   | [link](url) |",
        );
        let result = mdast_to_nsattributed_string(&ast, Path::new("."));
        assert!(result.is_ok());
    }
}
