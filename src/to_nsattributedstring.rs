//! macOS-specific NSAttributedString conversion for perfect paste compatibility
//!
//! This module converts markdown AST to NSAttributedString, which provides the best
//! clipboard compatibility with native macOS apps (TextEdit, Notes, Mail, Pages).
//!
//! ## Implementation Status
//!
//! This is a **work in progress**. The basic skeleton is in place, but key features need implementation:
//!
//! ### TODO:
//! - [ ] Font attributes (bold, italic, monospace) using NSFont and NSFontDescriptor
//! - [ ] NSTextAttachment for images (https://developer.apple.com/documentation/appkit/nstextattachment)
//! - [ ] NSTextTable for markdown tables (https://developer.apple.com/documentation/appkit/nstexttable)
//! - [ ] Paragraph styles for headings, blockquotes, lists
//! - [ ] Code blocks with background color
//! - [ ] Links as NSURL attributes
//! - [ ] Strikethrough formatting
//!
//! ### References:
//! - NSAttributedString: https://developer.apple.com/documentation/foundation/nsattributedstring
//! - NSTextAttachment: https://developer.apple.com/documentation/appkit/nstextattachment
//! - NSTextTable: https://developer.apple.com/documentation/appkit/nstexttable
//! - Attributed String Programming Guide: https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/AttributedStrings/

#![cfg(target_os = "macos")]

use crate::EmbedMode;
use crate::image::{ImageError, load_image_with_fallback};
use log::{debug, warn};
use markdown::mdast::Node;
use std::path::Path;

use objc2::rc::{Retained, autoreleasepool};
use objc2::ClassType;
use objc2_foundation::{
    NSAttributedString, NSMutableAttributedString, NSString, NSData,
    NSRange, NSDictionary, NSNumber,
};
use objc2_app_kit::{
    NSPasteboard, NSTextAttachment, NSImage, NSTextTable, NSFont, NSParagraphStyle,
};

/// Convert markdown AST to NSMutableAttributedString
///
/// This is the main entry point for macOS clipboard writing. The resulting
/// attributed string can be written directly to NSPasteboard.
pub fn mdast_to_nsattributed_string(
    node: &Node,
    base_dir: &Path,
    embed_mode: EmbedMode,
    strict: bool,
) -> Result<Retained<NSMutableAttributedString>, ImageError> {
    autoreleasepool(|_| {
        let attr_string = NSMutableAttributedString::new();
        let mut ctx = AttributedStringContext::new(base_dir, embed_mode, strict);

        node_to_attributed_string(node, &attr_string, &mut ctx)?;

        Ok(attr_string)
    })
}

/// Write NSAttributedString to the macOS pasteboard
///
/// This writes the attributed string directly to NSPasteboard, allowing macOS apps
/// to get rich text with embedded images when pasting.
pub fn write_to_pasteboard(
    attr_string: &NSAttributedString,
) -> Result<(), String> {
    autoreleasepool(|_| {
        let pasteboard = NSPasteboard::generalPasteboard();
        pasteboard.clearContents();

        // Write the attributed string directly - macOS will automatically provide
        // multiple representations (RTFD, RTF, plain text, etc.)
        let objects = objc2_foundation::NSArray::from_slice(&[attr_string]);

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
    embed_mode: EmbedMode,
    strict: bool,
}

impl<'a> AttributedStringContext<'a> {
    fn new(base_dir: &'a Path, embed_mode: EmbedMode, strict: bool) -> Self {
        Self {
            base_dir,
            embed_mode,
            strict,
        }
    }
}

/// Recursively convert markdown AST node to attributed string
fn node_to_attributed_string(
    node: &Node,
    attr_string: &NSMutableAttributedString,
    ctx: &mut AttributedStringContext,
) -> Result<(), ImageError> {
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
            let start_len = attr_string.length();
            for child in &strong.children {
                node_to_attributed_string(child, attr_string, ctx)?;
            }
            let range = NSRange::new(start_len, attr_string.length() - start_len);
            apply_bold(attr_string, range);
        }
        Node::Emphasis(em) => {
            let start_len = attr_string.length();
            for child in &em.children {
                node_to_attributed_string(child, attr_string, ctx)?;
            }
            let range = NSRange::new(start_len, attr_string.length() - start_len);
            apply_italic(attr_string, range);
        }
        Node::Heading(heading) => {
            let start_len = attr_string.length();
            for child in &heading.children {
                node_to_attributed_string(child, attr_string, ctx)?;
            }
            let range = NSRange::new(start_len, attr_string.length() - start_len);
            apply_heading(attr_string, range, heading.depth);
            append_text(attr_string, "\n\n");
        }
        Node::Image(image) => {
            embed_image(attr_string, &image.url, &image.alt, ctx)?;
        }
        // TODO: Implement remaining node types
        _ => {
            warn!("Unhandled node type in NSAttributedString conversion");
        }
    }
    Ok(())
}

/// Append plain text to attributed string
fn append_text(attr_string: &NSMutableAttributedString, text: &str) {
    let ns_string = NSString::from_str(text);
    let append_string = NSAttributedString::initWithString(
        NSAttributedString::alloc(),
        &ns_string
    );
    attr_string.appendAttributedString(&append_string);
}

/// Apply bold formatting to a range
fn apply_bold(attr_string: &NSMutableAttributedString, range: NSRange) {
    // TODO: Implement font traits for bold
    // This requires NSFontDescriptor and NSFontManager
    debug!("apply_bold not yet implemented");
}

/// Apply italic formatting to a range
fn apply_italic(attr_string: &NSMutableAttributedString, range: NSRange) {
    // TODO: Implement font traits for italic
    debug!("apply_italic not yet implemented");
}

/// Apply heading formatting to a range
fn apply_heading(attr_string: &NSMutableAttributedString, range: NSRange, depth: u8) {
    // TODO: Implement font size changes for headings
    debug!("apply_heading not yet implemented for depth {}", depth);
}

/// Embed an image as NSTextAttachment
fn embed_image(
    attr_string: &NSMutableAttributedString,
    url: &str,
    alt: &str,
    ctx: &mut AttributedStringContext,
) -> Result<(), ImageError> {
    let img = load_image_with_fallback(url, ctx.base_dir, ctx.embed_mode, ctx.strict)?;

    if let Some(embedded_img) = img {
        // Create NSImage from image data
        let ns_data = unsafe {
            NSData::dataWithBytes_length(
                embedded_img.data.as_ptr() as *const std::ffi::c_void,
                embedded_img.data.len(),
            )
        };

        if let Some(ns_image) = NSImage::initWithData(NSImage::alloc(), &ns_data) {
            // Create NSTextAttachment with the image
            let attachment = NSTextAttachment::new();
            // TODO: Set the image on the attachment
            // attachment.setImage(&ns_image);

            // Create attributed string from attachment
            // TODO: Use NSAttributedString::attributedStringWithAttachment
            debug!("Image embedding not yet fully implemented");
        } else {
            warn!("Failed to create NSImage from data for: {}", url);
            // Fallback: insert alt text or URL
            append_text(attr_string, &format!("[{}]({})", alt, url));
        }
    } else {
        // No image data, insert alt text or URL
        append_text(attr_string, &format!("[{}]({})", alt, url));
    }

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
        let result = mdast_to_nsattributed_string(&ast, Path::new("."), EmbedMode::None, false);
        assert!(result.is_ok());
        let attr_string = result.unwrap();
        assert!(attr_string.length() > 0);
    }

    #[test]
    fn test_bold_text() {
        let ast = parse_markdown("**bold**");
        let result = mdast_to_nsattributed_string(&ast, Path::new("."), EmbedMode::None, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_heading() {
        let ast = parse_markdown("# Heading 1");
        let result = mdast_to_nsattributed_string(&ast, Path::new("."), EmbedMode::None, false);
        assert!(result.is_ok());
    }
}
