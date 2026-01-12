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
//! - **Bold text** ✅: Font + `NSInlinePresentationIntent::StronglyEmphasized`
//! - **Italic text** ✅: Font + `NSInlinePresentationIntent::Emphasized`
//! - **Headings** ✅: `NSPresentationIntent::header` + `NSAccessibilityTextHeadingLevelAttribute`
//! - **Inline code** ✅: Monospace font + `NSInlinePresentationIntent::Code`
//! - **Code blocks** ✅: `NSPresentationIntent::codeBlock` with language hint
//! - **Links** ✅: Clickable links using `NSLinkAttributeName`
//! - **Strikethrough** ✅: Visual + `NSInlinePresentationIntent::Strikethrough`
//! - **Lists** ✅: Using `NSTextList` with disc/decimal markers in paragraph style
//! - **Blockquotes** ✅: `NSPresentationIntent::blockQuote` + gray text
//! - **Tables** ✅: Using `NSTextTable` and `NSTextTableBlock` with borders and padding
//!
//! ### References:
//! - NSAttributedString: https://developer.apple.com/documentation/foundation/nsattributedstring
//! - NSTextAttachment: https://developer.apple.com/documentation/appkit/nstextattachment
//! - NSTextTable: https://developer.apple.com/documentation/appkit/nstexttable
//! - Attributed String Programming Guide: https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/AttributedStrings/

use log::{debug, warn};
use markdown::mdast::Node;
use std::path::Path;
use syntect::easy::HighlightLines;
use syntect::util::LinesWithEndings;

use crate::config::ImageConfig;
use crate::highlight::HighlightContext;
use crate::image::{ImageCache, is_remote_url};

use objc2::AnyThread;
use objc2::rc::{Retained, autoreleasepool};
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2_app_kit::{
    NSAttributedStringAttachmentConveniences, NSBackgroundColorAttributeName, NSColor, NSFont,
    NSFontAttributeName, NSFontDescriptorSymbolicTraits, NSFontItalicTrait,
    NSFontTextStyleHeadline, NSFontTextStyleLargeTitle, NSFontTextStyleSubheadline,
    NSFontTextStyleTitle1, NSFontTextStyleTitle2, NSFontTextStyleTitle3,
    NSForegroundColorAttributeName, NSImage, NSLinkAttributeName, NSMutableParagraphStyle,
    NSParagraphStyleAttributeName, NSPasteboard, NSPasteboardWriting,
    NSStrikethroughStyleAttributeName, NSTextAttachment, NSTextBlock, NSTextList,
    NSTextListMarkerDecimal, NSTextListMarkerDisc, NSTextListOptions, NSTextTable,
    NSTextTableBlock,
};
use objc2_foundation::{
    NSAttributedString, NSDictionary, NSInlinePresentationIntent,
    NSInlinePresentationIntentAttributeName, NSMutableAttributedString, NSNumber,
    NSPresentationIntent, NSPresentationIntentAttributeName, NSRange, NSString,
};

/// Result of converting markdown to NSAttributedString
pub struct NativeConversionResult {
    /// The attributed string for clipboard
    pub attr_string: Retained<NSMutableAttributedString>,
    /// Maps generated filenames to original URLs for HTML post-processing
    pub image_urls: std::collections::HashMap<String, String>,
    /// The image config used (affects HTML generation)
    pub image_config: ImageConfig,
}

/// Convert markdown AST to NSMutableAttributedString
///
/// This is the main entry point for macOS clipboard writing. The resulting
/// attributed string can be written directly to NSPasteboard.
///
/// Image handling: Images are always embedded in the NSAttributedString for native apps.
/// The `image_config` only affects HTML generation:
/// - `embed_local: true, embed_remote: true`: Convert to data URIs in HTML
/// - `embed_local: true, embed_remote: false`: Data URIs for local, original URLs for remote
/// - `embed_local: false, embed_remote: false`: Keep original URLs in HTML
pub fn mdast_to_nsattributed_string(
    node: &Node,
    base_dir: &Path,
    image_config: &ImageConfig,
    strict: bool,
    highlight: Option<&HighlightContext>,
    image_cache: &ImageCache,
) -> Result<NativeConversionResult, String> {
    autoreleasepool(|_| {
        let attr_string = NSMutableAttributedString::new();
        let mut ctx =
            AttributedStringContext::new(base_dir, image_config, strict, highlight, image_cache);

        node_to_attributed_string(node, &attr_string, &mut ctx)?;

        Ok(NativeConversionResult {
            attr_string,
            image_urls: ctx.image_urls,
            image_config: image_config.clone(),
        })
    })
}

/// Write NSAttributedString to the macOS pasteboard
///
/// This writes the attributed string directly to NSPasteboard, allowing macOS apps
/// to get rich text with embedded images when pasting.
///
/// HTML handling:
/// - If `use_external_html` is false, auto-generate HTML from NSAttributedString
/// - If `use_external_html` is true and `external_html` is Some, use that HTML
/// - If `text` is provided, it will be written as plain text (e.g., markdown)
pub fn write_to_pasteboard(
    result: &NativeConversionResult,
    use_external_html: bool,
    external_html: Option<&str>,
    text: Option<&str>,
) -> Result<(), String> {
    use objc2_app_kit::{NSPasteboardTypeHTML, NSPasteboardTypeString};

    autoreleasepool(|_| {
        let pasteboard = NSPasteboard::generalPasteboard();
        pasteboard.clearContents();

        // Write the attributed string directly - macOS will automatically provide
        // multiple representations (RTFD, RTF, plain text, etc.)
        // Cast to immutable NSAttributedString for NSPasteboardWriting
        let attr_string: &NSAttributedString = &result.attr_string;
        let protocol_obj: &ProtocolObject<dyn NSPasteboardWriting> =
            ProtocolObject::from_ref(attr_string);
        let objects = objc2_foundation::NSArray::from_slice(&[protocol_obj]);

        if !pasteboard.writeObjects(&objects) {
            return Err("Failed to write attributed string to pasteboard".into());
        }

        // Write HTML - either external (from -f native,html) or auto-generated
        let html_content = if use_external_html {
            external_html.map(|s| s.to_string())
        } else {
            convert_to_html(result)
        };

        if let Some(html) = html_content {
            unsafe {
                let html_string = NSString::from_str(&html);
                pasteboard.setString_forType(&html_string, NSPasteboardTypeHTML);
            }
            debug!("Also wrote HTML to pasteboard");
        }

        // Write plain text if provided (e.g., markdown via -f native,markdown)
        if let Some(text_content) = text {
            unsafe {
                let text_string = NSString::from_str(text_content);
                pasteboard.setString_forType(&text_string, NSPasteboardTypeString);
            }
            debug!("Also wrote plain text to pasteboard");
        }

        debug!("Wrote NSAttributedString to pasteboard");
        Ok(())
    })
}

/// Convert NSAttributedString to HTML, replacing file:// URLs based on image_config
///
/// - embed_local + embed_remote: All images become data URIs
/// - embed_local only: Local images become data URIs, remote keep original URLs
/// - neither: All images keep original URLs
fn convert_to_html(result: &NativeConversionResult) -> Option<String> {
    use base64::Engine;
    use objc2_app_kit::{
        NSAttributedStringDocumentFormats, NSDocumentTypeDocumentAttribute, NSHTMLTextDocumentType,
        NSTextAttachment,
    };

    unsafe {
        let attr_string = &result.attr_string;
        let length = attr_string.length();
        if length == 0 {
            return None;
        }

        // Collect replacement URLs: filename -> replacement (data URI or original URL)
        let mut replacements: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        let attachment_key = NSString::from_str("NSAttachment");

        let mut index: usize = 0;
        while index < length {
            let mut effective_range = NSRange::new(0, 0);
            let attr_value = attr_string.attribute_atIndex_effectiveRange(
                &attachment_key,
                index,
                &mut effective_range,
            );

            if let Some(attachment_obj) = attr_value
                && let Some(attachment) = attachment_obj.downcast_ref::<NSTextAttachment>()
                && let Some(file_wrapper) = attachment.fileWrapper()
                && let Some(filename) = file_wrapper.preferredFilename()
            {
                let filename_str = filename.to_string();

                // Only process images we explicitly handled (image_N.ext or remote_N.ext pattern)
                if !filename_str.starts_with("image_") && !filename_str.starts_with("remote_") {
                    index = effective_range.location + effective_range.length;
                    if effective_range.length == 0 {
                        index += 1;
                    }
                    continue;
                }

                // Get original URL for this image
                let original_url = result.image_urls.get(&filename_str);

                // Decide replacement based on image_config
                let should_use_data_uri = if let Some(url) = original_url {
                    if is_remote_url(url) {
                        result.image_config.embed_remote
                    } else {
                        result.image_config.embed_local
                    }
                } else {
                    // No original URL tracked, default to embed
                    true
                };

                if should_use_data_uri {
                    // Convert to data URI
                    if let Some(ns_data) = file_wrapper.regularFileContents() {
                        let bytes = ns_data.as_bytes_unchecked();
                        let base64_data = base64::engine::general_purpose::STANDARD.encode(bytes);

                        let mime_type = if filename_str.ends_with(".png") {
                            "image/png"
                        } else if filename_str.ends_with(".jpg") || filename_str.ends_with(".jpeg")
                        {
                            "image/jpeg"
                        } else if filename_str.ends_with(".gif") {
                            "image/gif"
                        } else if filename_str.ends_with(".webp") {
                            "image/webp"
                        } else {
                            "application/octet-stream"
                        };

                        let data_uri = format!("data:{};base64,{}", mime_type, base64_data);
                        replacements.insert(filename_str, data_uri);
                    }
                } else if let Some(url) = original_url {
                    // Use original URL
                    replacements.insert(filename_str, url.clone());
                }
            }

            // Move to next range
            index = effective_range.location + effective_range.length;
            if effective_range.length == 0 {
                index += 1;
            }
        }

        // Convert to HTML using native API
        let full_range = NSRange::new(0, length);
        let doc_type_key: &NSString = NSDocumentTypeDocumentAttribute;
        let html_type: &AnyObject = NSHTMLTextDocumentType.as_ref();
        let doc_attrs: Retained<NSDictionary<NSString, AnyObject>> =
            NSDictionary::from_slices(&[doc_type_key], &[html_type]);

        let html_data = attr_string
            .dataFromRange_documentAttributes_error(full_range, &doc_attrs)
            .ok()?;

        // Get bytes from NSData
        let html_bytes = html_data.as_bytes_unchecked();
        let mut html = String::from_utf8(html_bytes.to_vec()).ok()?;

        // Replace file:// URLs with appropriate replacements
        for (filename, replacement) in replacements {
            let file_url = format!("file:///{}", filename);
            html = html.replace(&file_url, &replacement);
        }

        Some(html)
    }
}

/// Context for building attributed string
struct AttributedStringContext<'a> {
    base_dir: &'a Path,
    image_config: &'a ImageConfig,
    strict: bool,
    highlight: Option<&'a HighlightContext>,
    image_cache: &'a ImageCache,
    /// Maps generated filenames (image_N.ext) to original URLs for HTML post-processing
    image_urls: std::collections::HashMap<String, String>,
}

impl<'a> AttributedStringContext<'a> {
    fn new(
        base_dir: &'a Path,
        image_config: &'a ImageConfig,
        strict: bool,
        highlight: Option<&'a HighlightContext>,
        image_cache: &'a ImageCache,
    ) -> Self {
        Self {
            base_dir,
            image_config,
            strict,
            highlight,
            image_cache,
            image_urls: std::collections::HashMap::new(),
        }
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
            debug!("Root node with {} children", root.children.len());
            for (i, child) in root.children.iter().enumerate() {
                debug!("  Root child {}: {:?}", i, std::mem::discriminant(child));
                node_to_attributed_string(child, attr_string, ctx)?;
            }
        }
        Node::Paragraph(para) => {
            debug!("Paragraph node with {} children", para.children.len());
            let temp_string = NSMutableAttributedString::new();
            for child in &para.children {
                node_to_attributed_string(child, &temp_string, ctx)?;
            }
            // Add paragraph break
            append_text(&temp_string, "\n");
            // Apply paragraph spacing
            let range = NSRange::new(0, temp_string.length());
            apply_paragraph_spacing(&temp_string, range);
            attr_string.appendAttributedString(&temp_string);
        }
        Node::Text(text) => {
            debug!("Text node: {:?}", text.value);
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
            debug!(
                "Heading node depth={} with {} children",
                heading.depth,
                heading.children.len()
            );
            let temp_string = NSMutableAttributedString::new();
            for child in &heading.children {
                node_to_attributed_string(child, &temp_string, ctx)?;
            }
            // Include newline in the heading (required for Apple Notes to recognize it)
            append_text(&temp_string, "\n");
            let range = NSRange::new(0, temp_string.length());
            apply_heading(&temp_string, range, heading.depth);
            attr_string.appendAttributedString(&temp_string);
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

            if let Some(highlight_ctx) = ctx.highlight {
                // Syntax highlighted code block
                let syntax = code
                    .lang
                    .as_ref()
                    .map(|lang| highlight_ctx.find_syntax(lang))
                    .unwrap_or_else(|| highlight_ctx.syntax_set.find_syntax_plain_text());

                let mut highlighter = HighlightLines::new(syntax, &highlight_ctx.theme);

                for line in LinesWithEndings::from(&code.value) {
                    if let Ok(ranges) = highlighter.highlight_line(line, &highlight_ctx.syntax_set)
                    {
                        for (style, text) in ranges {
                            let text_without_newline = text.trim_end_matches('\n');
                            if !text_without_newline.is_empty() {
                                append_highlighted_text(
                                    &temp_string,
                                    text_without_newline,
                                    style.foreground,
                                );
                            }
                            // Add newline back if it was there
                            if text.ends_with('\n') {
                                append_text(&temp_string, "\n");
                            }
                        }
                    } else {
                        append_text(&temp_string, line);
                    }
                }
            } else {
                // Plain code block without highlighting
                append_text(&temp_string, &code.value);
            }

            append_text(&temp_string, "\n");
            let range = NSRange::new(0, temp_string.length());
            apply_code_block(&temp_string, range, code.lang.as_deref(), ctx.highlight);
            attr_string.appendAttributedString(&temp_string);
        }
        Node::List(list) => {
            unsafe {
                // Create NSTextList with appropriate marker format
                let marker_format = if list.ordered {
                    NSTextListMarkerDecimal
                } else {
                    NSTextListMarkerDisc
                };

                let start_number = list.start.unwrap_or(1) as isize;
                let text_list = NSTextList::initWithMarkerFormat_options_startingItemNumber(
                    NSTextList::alloc(),
                    marker_format,
                    NSTextListOptions::empty(),
                    start_number,
                );

                // Create array containing just this list
                let lists_array = objc2_foundation::NSArray::from_slice(&[&*text_list]);

                // Process each list item
                for child in &list.children {
                    if let Node::ListItem(item) = child {
                        let item_string = NSMutableAttributedString::new();

                        // Process item content (no manual bullet - NSTextList handles it)
                        for item_child in &item.children {
                            node_to_attributed_string(item_child, &item_string, ctx)?;
                        }

                        // Create paragraph style with the text list
                        let para_style = NSMutableParagraphStyle::new();
                        para_style.setTextLists(&lists_array);

                        // Apply paragraph style to the item
                        let range = NSRange::new(0, item_string.length());
                        item_string.addAttribute_value_range(
                            NSParagraphStyleAttributeName,
                            &*para_style as &AnyObject,
                            range,
                        );

                        attr_string.appendAttributedString(&item_string);
                    }
                }
            }
            append_text(attr_string, "\n");
        }
        Node::ListItem(_) => {
            // List items are handled by the parent List node
            // This branch handles orphaned list items (shouldn't happen in valid markdown)
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

/// Append text with a specific foreground color (for syntax highlighting)
fn append_highlighted_text(
    attr_string: &NSMutableAttributedString,
    text: &str,
    color: syntect::highlighting::Color,
) {
    unsafe {
        let ns_string = NSString::from_str(text);
        let temp_string = NSMutableAttributedString::initWithString(
            NSMutableAttributedString::alloc(),
            &ns_string,
        );

        // Apply foreground color
        let ns_color = NSColor::colorWithRed_green_blue_alpha(
            color.r as f64 / 255.0,
            color.g as f64 / 255.0,
            color.b as f64 / 255.0,
            color.a as f64 / 255.0,
        );
        let range = NSRange::new(0, temp_string.length());
        temp_string.addAttribute_value_range(
            NSForegroundColorAttributeName,
            &ns_color as &AnyObject,
            range,
        );

        attr_string.appendAttributedString(&temp_string);
    }
}

/// Apply bold formatting to a range
///
/// Applies both visual bold font and semantic StronglyEmphasized intent.
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

        // Apply semantic inline presentation intent (StronglyEmphasized)
        let intent = NSInlinePresentationIntent::StronglyEmphasized;
        let intent_value = NSNumber::new_usize(intent.0);
        attr_string.addAttribute_value_range(
            NSInlinePresentationIntentAttributeName,
            &*intent_value as &AnyObject,
            range,
        );
    }
}

/// Apply italic formatting to a range
///
/// Applies both visual italic font and semantic Emphasized intent.
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

        // Apply semantic inline presentation intent (Emphasized)
        let intent = NSInlinePresentationIntent::Emphasized;
        let intent_value = NSNumber::new_usize(intent.0);
        attr_string.addAttribute_value_range(
            NSInlinePresentationIntentAttributeName,
            &*intent_value as &AnyObject,
            range,
        );
    }
}

/// Apply heading formatting to a range
///
/// Uses paragraph style with headerLevel and preferred font for text style.
/// Also applies NSPresentationIntent for semantic structure.
///
/// Key insight: The range MUST include the trailing newline for Apple Notes
/// to recognize the heading. This is handled by the caller.
fn apply_heading(attr_string: &NSMutableAttributedString, range: NSRange, depth: u8) {
    // Use a static counter for unique identity values
    static INTENT_ID: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(100);

    unsafe {
        // Clamp depth to 1-6
        let clamped_depth = depth.clamp(1, 6);

        // 1. Apply paragraph style with headerLevel and spacing
        // This is what Apple Notes needs to recognize headings
        let para_style = NSMutableParagraphStyle::new();
        para_style.setHeaderLevel(clamped_depth as isize);
        // Add spacing before heading (except h1) and after all headings
        let spacing_before = match clamped_depth {
            1 => 0.0,
            2 => 12.0,
            _ => 8.0,
        };
        let spacing_after = match clamped_depth {
            1 | 2 => 8.0,
            _ => 4.0,
        };
        para_style.setParagraphSpacingBefore(spacing_before);
        para_style.setParagraphSpacing(spacing_after);
        attr_string.addAttribute_value_range(
            NSParagraphStyleAttributeName,
            &*para_style as &AnyObject,
            range,
        );

        // 2. Apply preferred font for text style
        let text_style = match clamped_depth {
            1 => NSFontTextStyleLargeTitle,
            2 => NSFontTextStyleTitle1,
            3 => NSFontTextStyleTitle2,
            4 => NSFontTextStyleTitle3,
            5 => NSFontTextStyleHeadline,
            _ => NSFontTextStyleSubheadline,
        };
        let options: Retained<NSDictionary<NSString, AnyObject>> = NSDictionary::new();
        let heading_font = NSFont::preferredFontForTextStyle_options(text_style, &options);
        attr_string.addAttribute_value_range(
            NSFontAttributeName,
            &*heading_font as &AnyObject,
            range,
        );

        // 3. Apply semantic NSPresentationIntent for header
        // This provides semantic structure for apps that support it
        let identity = INTENT_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst) as isize;
        let header_intent = NSPresentationIntent::headerIntentWithIdentity_level_nestedInsideIntent(
            identity,
            clamped_depth as isize,
            None,
        );
        attr_string.addAttribute_value_range(
            NSPresentationIntentAttributeName,
            &*header_intent as &AnyObject,
            range,
        );
    }
}

/// Apply paragraph spacing to a range
///
/// Adds spacing after paragraphs for visual separation between blocks.
fn apply_paragraph_spacing(attr_string: &NSMutableAttributedString, range: NSRange) {
    unsafe {
        let para_style = NSMutableParagraphStyle::new();
        para_style.setParagraphSpacing(6.0); // spacing after paragraph
        attr_string.addAttribute_value_range(
            NSParagraphStyleAttributeName,
            &*para_style as &AnyObject,
            range,
        );
    }
}

/// Apply monospace font to a range (for inline code)
///
/// Applies both visual monospace font and semantic Code intent.
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

        // Apply semantic inline presentation intent (Code)
        let intent = NSInlinePresentationIntent::Code;
        let intent_value = NSNumber::new_usize(intent.0);
        attr_string.addAttribute_value_range(
            NSInlinePresentationIntentAttributeName,
            &*intent_value as &AnyObject,
            range,
        );
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
/// Applies both visual strikethrough and semantic Strikethrough intent.
fn apply_strikethrough(attr_string: &NSMutableAttributedString, range: NSRange) {
    unsafe {
        // NSUnderlineStyleSingle = 1
        let style = NSNumber::new_i32(1);

        // Apply strikethrough style
        attr_string.addAttribute_value_range(
            NSStrikethroughStyleAttributeName,
            &*style as &AnyObject,
            range,
        );

        // Apply semantic inline presentation intent (Strikethrough)
        let intent = NSInlinePresentationIntent::Strikethrough;
        let intent_value = NSNumber::new_usize(intent.0);
        attr_string.addAttribute_value_range(
            NSInlinePresentationIntentAttributeName,
            &*intent_value as &AnyObject,
            range,
        );
    }
}

/// Apply code block formatting to a range
///
/// Applies visual formatting (monospace font, background from theme or light gray) and
/// semantic NSPresentationIntent with optional language hint.
fn apply_code_block(
    attr_string: &NSMutableAttributedString,
    range: NSRange,
    language: Option<&str>,
    highlight: Option<&HighlightContext>,
) {
    // Use a static counter for unique identity values
    static INTENT_ID: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1000);

    unsafe {
        // Apply semantic code block presentation intent
        let identity = INTENT_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst) as isize;
        let lang_hint = language.map(NSString::from_str);
        let code_intent =
            NSPresentationIntent::codeBlockIntentWithIdentity_languageHint_nestedInsideIntent(
                identity,
                lang_hint.as_deref(),
                None,
            );
        attr_string.addAttribute_value_range(
            NSPresentationIntentAttributeName,
            &*code_intent as &AnyObject,
            range,
        );

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

        // Apply background color from theme or default light gray
        let bg_color = if let Some(ctx) = highlight {
            if let Some(bg) = ctx.theme.settings.background {
                NSColor::colorWithRed_green_blue_alpha(
                    bg.r as f64 / 255.0,
                    bg.g as f64 / 255.0,
                    bg.b as f64 / 255.0,
                    bg.a as f64 / 255.0,
                )
            } else {
                NSColor::colorWithRed_green_blue_alpha(0.95, 0.95, 0.95, 1.0)
            }
        } else {
            NSColor::colorWithRed_green_blue_alpha(0.95, 0.95, 0.95, 1.0)
        };
        attr_string.addAttribute_value_range(
            NSBackgroundColorAttributeName,
            &bg_color as &AnyObject,
            range,
        );
    }
}

/// Apply blockquote formatting to a range
///
/// Applies visual formatting (gray text) and semantic NSPresentationIntent.
fn apply_blockquote(attr_string: &NSMutableAttributedString, range: NSRange) {
    // Use a static counter for unique identity values
    static INTENT_ID: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(2000);

    unsafe {
        // Apply semantic blockquote presentation intent
        let identity = INTENT_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst) as isize;
        let quote_intent =
            NSPresentationIntent::blockQuoteIntentWithIdentity_nestedInsideIntent(identity, None);
        attr_string.addAttribute_value_range(
            NSPresentationIntentAttributeName,
            &*quote_intent as &AnyObject,
            range,
        );

        // Apply gray color to blockquotes
        let gray_color = NSColor::colorWithRed_green_blue_alpha(0.5, 0.5, 0.5, 1.0);
        attr_string.addAttribute_value_range(
            NSForegroundColorAttributeName,
            &gray_color as &AnyObject,
            range,
        );
    }
}

/// Render image as a clickable link (fallback when embedding fails)
fn render_image_as_link(attr_string: &NSMutableAttributedString, url: &str, alt: &str) {
    let start = attr_string.length();
    append_text(attr_string, if alt.is_empty() { url } else { alt });
    let end = attr_string.length();
    let range = NSRange::new(start, end - start);
    apply_link(attr_string, range, url);
}

/// Add an image as NSTextAttachment
///
/// For NSAttributedString, images are always embedded for optimal clipboard behavior.
/// The image_config affects how HTML is generated later (whether to use data URIs or URLs).
fn embed_image(
    attr_string: &NSMutableAttributedString,
    url: &str,
    alt: &str,
    ctx: &mut AttributedStringContext,
) -> Result<(), String> {
    use objc2_foundation::NSFileWrapper;

    let is_remote = is_remote_url(url);

    // For NSAttributedString, we always need to load image data for the clipboard.
    // However, we only optimize if the corresponding embed flag is set, so that
    // the HTML output (which respects embed flags) stays consistent with NSAttributedString.
    // If embed is disabled, both NSAttributedString and HTML use the original unoptimized image.
    let (should_optimize_local, should_optimize_remote) = (
        ctx.image_config.optimize_local && ctx.image_config.embed_local,
        ctx.image_config.optimize_remote && ctx.image_config.embed_remote,
    );

    let load_config = ImageConfig {
        embed_local: true,  // always load for native clipboard
        embed_remote: true, // always load for native clipboard
        optimize_local: should_optimize_local,
        optimize_remote: should_optimize_remote,
        max_dimension: ctx.image_config.max_dimension,
        quality: ctx.image_config.quality,
    };

    // Use the ImageCache for consistent behavior with HTML/RTF
    let embedded = match ctx
        .image_cache
        .get_or_load(url, ctx.base_dir, &load_config, ctx.strict)
    {
        Ok(Some(img)) => img,
        Ok(None) => {
            // Skipped (e.g., data URL) - render as link
            render_image_as_link(attr_string, url, alt);
            return Ok(());
        }
        Err(e) => {
            warn!("Failed to load image: {} - {}", url, e);
            render_image_as_link(attr_string, url, alt);
            return Ok(());
        }
    };

    // Create NSData from the bytes
    let ns_data = objc2_foundation::NSData::with_bytes(&embedded.data);

    // Create NSImage from data
    let ns_image = match NSImage::initWithData(NSImage::alloc(), &ns_data) {
        Some(img) if img.isValid() => img,
        _ => {
            warn!("Failed to create valid NSImage from data: {}", url);
            render_image_as_link(attr_string, url, alt);
            return Ok(());
        }
    };

    // Determine file extension from mime type
    let extension = match embedded.mime_type.as_str() {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => "png",
    };

    // Generate unique filename
    static IMAGE_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let counter = IMAGE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let filename = if is_remote {
        format!("remote_{}.{}", counter, extension)
    } else {
        format!("image_{}.{}", counter, extension)
    };

    // Set the image name - this is what HTML conversion uses for src attribute
    ns_image.setName(Some(&NSString::from_str(&filename)));

    // Create file wrapper with the image data
    let file_wrapper = NSFileWrapper::initRegularFileWithContents(NSFileWrapper::alloc(), &ns_data);
    file_wrapper.setPreferredFilename(Some(&NSString::from_str(&filename)));

    // Create attachment with image and file wrapper
    let attachment = NSTextAttachment::new();
    attachment.setImage(Some(&ns_image));
    attachment.setFileWrapper(Some(&file_wrapper));

    // Create attributed string from attachment
    let attachment_string = NSAttributedString::attributedStringWithAttachment(&attachment);
    attr_string.appendAttributedString(&attachment_string);

    // Track original URL for HTML post-processing
    // For local files, store the absolute path so HTML can reference it
    let tracked_url = if is_remote {
        url.to_string()
    } else {
        let abs_path = ctx.base_dir.join(url);
        format!(
            "file://{}",
            abs_path.canonicalize().unwrap_or(abs_path).display()
        )
    };
    ctx.image_urls.insert(filename, tracked_url);

    debug!("Image embedded with fileWrapper: {}", url);
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
                    // Set border width (1.0 point)
                    text_block.setWidth_type_forLayer(
                        1.0,
                        objc2_app_kit::NSTextBlockValueType::AbsoluteValueType,
                        objc2_app_kit::NSTextBlockLayer::Border,
                    );

                    // Set border color (light gray)
                    let border_color = NSColor::colorWithRed_green_blue_alpha(0.8, 0.8, 0.8, 1.0);
                    text_block.setBorderColor(Some(&border_color));

                    // Set padding (4.0 points)
                    text_block.setWidth_type_forLayer(
                        4.0,
                        objc2_app_kit::NSTextBlockValueType::AbsoluteValueType,
                        objc2_app_kit::NSTextBlockLayer::Padding,
                    );

                    // Create paragraph style with the table block
                    let paragraph_style = NSMutableParagraphStyle::new();
                    let blocks_array =
                        objc2_foundation::NSArray::from_slice(&[&text_block as &NSTextBlock]);
                    paragraph_style.setTextBlocks(&blocks_array);

                    // Apply paragraph style to the entire cell content
                    unsafe {
                        let full_range = NSRange::new(0, cell_string.length());
                        cell_string.addAttribute_value_range(
                            NSParagraphStyleAttributeName,
                            &paragraph_style as &AnyObject,
                            full_range,
                        );
                    }

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
    use crate::image::ImageCache;
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

    fn test_image_config() -> ImageConfig {
        ImageConfig {
            embed_local: true,
            embed_remote: true,
            optimize_local: false,
            optimize_remote: false,
            max_dimension: 1200,
            quality: 80,
        }
    }

    #[test]
    fn test_basic_text() {
        let ast = parse_markdown("Hello world");
        let cache = ImageCache::new();
        let config = test_image_config();
        let result =
            mdast_to_nsattributed_string(&ast, Path::new("."), &config, false, None, &cache);
        assert!(result.is_ok());
        let conversion = result.unwrap();
        assert!(conversion.attr_string.length() > 0);
    }

    #[test]
    fn test_bold_text() {
        let ast = parse_markdown("**bold**");
        let cache = ImageCache::new();
        let config = test_image_config();
        let result =
            mdast_to_nsattributed_string(&ast, Path::new("."), &config, false, None, &cache);
        assert!(result.is_ok());
    }

    #[test]
    fn test_italic_text() {
        let ast = parse_markdown("*italic*");
        let cache = ImageCache::new();
        let config = test_image_config();
        let result =
            mdast_to_nsattributed_string(&ast, Path::new("."), &config, false, None, &cache);
        assert!(result.is_ok());
    }

    #[test]
    fn test_bold_italic() {
        let ast = parse_markdown("***bold and italic***");
        let cache = ImageCache::new();
        let config = test_image_config();
        let result =
            mdast_to_nsattributed_string(&ast, Path::new("."), &config, false, None, &cache);
        assert!(result.is_ok());
    }

    #[test]
    fn test_heading() {
        let ast = parse_markdown("# Heading 1");
        let cache = ImageCache::new();
        let config = test_image_config();
        let result =
            mdast_to_nsattributed_string(&ast, Path::new("."), &config, false, None, &cache);
        assert!(result.is_ok());
    }

    #[test]
    fn test_inline_code() {
        let ast = parse_markdown("`code`");
        let cache = ImageCache::new();
        let config = test_image_config();
        let result =
            mdast_to_nsattributed_string(&ast, Path::new("."), &config, false, None, &cache);
        assert!(result.is_ok());
    }

    #[test]
    fn test_link() {
        let ast = parse_markdown("[example](https://example.com)");
        let cache = ImageCache::new();
        let config = test_image_config();
        let result =
            mdast_to_nsattributed_string(&ast, Path::new("."), &config, false, None, &cache);
        assert!(result.is_ok());
    }

    #[test]
    fn test_strikethrough() {
        let ast = parse_markdown("~~deleted~~");
        let cache = ImageCache::new();
        let config = test_image_config();
        let result =
            mdast_to_nsattributed_string(&ast, Path::new("."), &config, false, None, &cache);
        assert!(result.is_ok());
    }

    #[test]
    fn test_mixed_formatting() {
        let ast = parse_markdown("**bold** and `code` and [link](url) and ~~strike~~");
        let cache = ImageCache::new();
        let config = test_image_config();
        let result =
            mdast_to_nsattributed_string(&ast, Path::new("."), &config, false, None, &cache);
        assert!(result.is_ok());
    }

    #[test]
    fn test_code_block() {
        let ast = parse_markdown("```rust\nfn main() {}\n```");
        let cache = ImageCache::new();
        let config = test_image_config();
        let result =
            mdast_to_nsattributed_string(&ast, Path::new("."), &config, false, None, &cache);
        assert!(result.is_ok());
    }

    #[test]
    fn test_list() {
        let ast = parse_markdown("- Item 1\n- Item 2\n- Item 3");
        let cache = ImageCache::new();
        let config = test_image_config();
        let result =
            mdast_to_nsattributed_string(&ast, Path::new("."), &config, false, None, &cache);
        assert!(result.is_ok());
    }

    #[test]
    fn test_blockquote() {
        let ast = parse_markdown("> This is a quote\n> with multiple lines");
        let cache = ImageCache::new();
        let config = test_image_config();
        let result =
            mdast_to_nsattributed_string(&ast, Path::new("."), &config, false, None, &cache);
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
        let cache = ImageCache::new();
        let config = test_image_config();
        let result =
            mdast_to_nsattributed_string(&ast, Path::new("."), &config, false, None, &cache);
        assert!(result.is_ok());
    }

    #[test]
    fn test_table_with_formatting() {
        let ast = parse_markdown(
            "| **Bold** | *Italic* |\n\
             |----------|----------|\n\
             | `code`   | [link](url) |",
        );
        let cache = ImageCache::new();
        let config = test_image_config();
        let result =
            mdast_to_nsattributed_string(&ast, Path::new("."), &config, false, None, &cache);
        assert!(result.is_ok());
    }
}
