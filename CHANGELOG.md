# Changelog

## [0.2.0] - 2025-01-09

### Added
- `--format` / `-f` flag to select clipboard formats (html, rtf, markdown)
- Markdown format option embeds images as data URLs in markdown text
- Demo recording in README

### Changed
- Improved paste compatibility for Google Docs and email clients
  - Lists: unwrap `<p>` tags in tight lists to avoid extra line spacing
  - Code blocks: use `<div>` with inline styles instead of `<pre>` for continuous background
  - Tables: use old-school HTML attributes (`cellpadding`, `nowrap`) for better compatibility
- Fixed syntax highlighting state handling for multi-line code blocks

### Removed
- Integration tests (replaced by comprehensive unit tests)

## [0.1.1] - 2025-01-08

### Fixed
- Homebrew tap authentication
- Git credential handling for releases

## [0.1.0] - 2025-01-08

Initial release.

### Features
- Convert Markdown to clipboard with HTML and RTF formats
- GitHub Flavored Markdown support
- Syntax highlighting with customizable themes
- Image embedding (local and remote)
- Configuration via file, environment variables, or CLI
