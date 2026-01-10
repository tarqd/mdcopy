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
//! - [ ] Font attributes (bold, italic, monospace) using NSFont and NSFontDescriptor
//! - [x] ~~NSTextAttachment for images~~ **DONE!** (https://developer.apple.com/documentation/appkit/nstextattachment)
//! - [ ] NSTextTable for markdown tables (https://developer.apple.com/documentation/appkit/nstexttable)
//! - [ ] Paragraph styles for headings, blockquotes, lists
//! - [ ] Code blocks with background color
//! - [ ] Links as NSURL attributes
//! - [ ] Strikethrough formatting
//!
//! ### Implemented Features:
//! - **Image embedding** ✅: Both local and remote images are loaded via NSImage and embedded
//!   as NSTextAttachment objects using `setImage()` and `attributedStringWithAttachment()`.
//!   Images render inline when pasted into TextEdit, Notes, Mail, Pages, etc.
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

use objc2::rc::{Retained, autoreleasepool};
use objc2::ClassType;
use objc2_foundation::{
    NSAttributedString, NSMutableAttributedString, NSString, NSData, NSURL, NSRange,
};
use objc2_app_kit::{
    NSPasteboard, NSTextAttachment, NSImage,
    NSAttributedStringAttachmentConveniences,
};

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
        unsafe {
            attachment.setImage(Some(&ns_image));
        }

        // Create attributed string from attachment
        let attachment_string = unsafe {
            NSAttributedString::attributedStringWithAttachment(&attachment)
        };

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
    fn test_heading() {
        let ast = parse_markdown("# Heading 1");
        let result = mdast_to_nsattributed_string(&ast, Path::new("."));
        assert!(result.is_ok());
    }
}
