use std::io::Write;
use std::process::{Command, Stdio};
use tempfile::TempDir;

fn mdcopy_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mdcopy"))
}

fn run_with_stdin(args: &[&str], input: &str) -> (String, String, bool) {
    let mut cmd = mdcopy_cmd();
    cmd.args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().expect("Failed to spawn mdcopy");

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(input.as_bytes())
            .expect("Failed to write stdin");
    }

    let output = child.wait_with_output().expect("Failed to wait for mdcopy");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (stdout, stderr, output.status.success())
}

fn run_with_args(args: &[&str]) -> (String, String, bool) {
    let output = mdcopy_cmd()
        .args(args)
        .output()
        .expect("Failed to run mdcopy");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (stdout, stderr, output.status.success())
}

#[test]
fn test_help_flag() {
    let (stdout, _, success) = run_with_args(&["--help"]);
    assert!(success);
    assert!(stdout.contains("mdcopy"));
    assert!(stdout.contains("Convert markdown"));
    assert!(stdout.contains("--input"));
    assert!(stdout.contains("--output"));
}

#[test]
fn test_list_themes() {
    let (stdout, _, success) = run_with_args(&["--list-themes"]);
    assert!(success);
    assert!(stdout.contains("Available themes:"));
    assert!(stdout.contains("base16-ocean.dark"));
    assert!(stdout.contains("base16-ocean.light"));
}

#[test]
fn test_basic_markdown_to_stdout() {
    let (stdout, _, success) = run_with_stdin(&["-o", "-"], "# Hello World");
    assert!(success);
    assert!(stdout.contains("<h1>Hello World</h1>"));
}

#[test]
fn test_bold_text() {
    let (stdout, _, success) = run_with_stdin(&["-o", "-"], "**bold**");
    assert!(success);
    assert!(stdout.contains("<strong>bold</strong>"));
}

#[test]
fn test_italic_text() {
    let (stdout, _, success) = run_with_stdin(&["-o", "-"], "*italic*");
    assert!(success);
    assert!(stdout.contains("<em>italic</em>"));
}

#[test]
fn test_code_block_with_highlighting() {
    let (stdout, _, success) = run_with_stdin(&["-o", "-"], "```rust\nfn main() {}\n```");
    assert!(success);
    // With syntax highlighting enabled by default, we should see styled spans
    assert!(stdout.contains("<pre"));
    assert!(stdout.contains("style="));
}

#[test]
fn test_code_block_without_highlighting() {
    let (stdout, _, success) = run_with_stdin(
        &["-o", "-", "--highlight", "false"],
        "```rust\nfn main() {}\n```",
    );
    assert!(success);
    assert!(stdout.contains("<pre><code"));
    assert!(stdout.contains("class=\"language-rust\""));
}

#[test]
fn test_highlight_theme_selection() {
    let (stdout, _, success) = run_with_stdin(
        &["-o", "-", "--highlight-theme", "Solarized (dark)"],
        "```rust\nfn main() {}\n```",
    );
    assert!(success);
    // Solarized dark has a specific background color
    assert!(stdout.contains("background-color:#002b36"));
}

#[test]
fn test_input_from_file() {
    let temp_dir = TempDir::new().unwrap();
    let input_path = temp_dir.path().join("input.md");
    std::fs::write(&input_path, "# Test").unwrap();

    let (stdout, _, success) = run_with_args(&["-i", input_path.to_str().unwrap(), "-o", "-"]);
    assert!(success);
    assert!(stdout.contains("<h1>Test</h1>"));
}

#[test]
fn test_output_to_file() {
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output.html");

    let (_, _, success) = run_with_stdin(&["-o", output_path.to_str().unwrap()], "# Test");
    assert!(success);

    let content = std::fs::read_to_string(&output_path).unwrap();
    assert!(content.contains("<h1>Test</h1>"));
}

#[test]
fn test_config_file() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.toml");
    std::fs::write(&config_path, "[highlight]\nenable = false\n").unwrap();

    let (stdout, _, success) = run_with_stdin(
        &["-o", "-", "-c", config_path.to_str().unwrap()],
        "```rust\nfn main() {}\n```",
    );
    assert!(success);
    // With highlighting disabled via config, should see plain code block
    assert!(stdout.contains("<pre><code"));
}

#[test]
fn test_embed_mode_none() {
    let (stdout, _, success) = run_with_stdin(&["-o", "-", "-e", "none"], "![alt](image.png)");
    assert!(success);
    assert!(stdout.contains("src=\"image.png\""));
}

#[test]
fn test_strict_mode_missing_image() {
    let temp_dir = TempDir::new().unwrap();
    let input_path = temp_dir.path().join("input.md");
    std::fs::write(&input_path, "![alt](nonexistent.png)").unwrap();

    let (_, stderr, success) = run_with_args(&[
        "-i",
        input_path.to_str().unwrap(),
        "-o",
        "-",
        "--strict",
        "-r",
        temp_dir.path().to_str().unwrap(),
    ]);
    assert!(!success);
    assert!(stderr.contains("not found") || stderr.contains("Error"));
}

#[test]
fn test_graceful_mode_missing_image() {
    let temp_dir = TempDir::new().unwrap();
    let input_path = temp_dir.path().join("input.md");
    std::fs::write(&input_path, "![alt](nonexistent.png)").unwrap();

    let (stdout, _, success) = run_with_args(&[
        "-i",
        input_path.to_str().unwrap(),
        "-o",
        "-",
        "-r",
        temp_dir.path().to_str().unwrap(),
    ]);
    // Should succeed even with missing image
    assert!(success);
    assert!(stdout.contains("nonexistent.png"));
}

#[test]
fn test_local_image_embedding() {
    let temp_dir = TempDir::new().unwrap();
    let input_path = temp_dir.path().join("input.md");
    let image_path = temp_dir.path().join("test.png");

    // Create a minimal PNG file (just header)
    std::fs::write(
        &image_path,
        [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A],
    )
    .unwrap();

    std::fs::write(&input_path, "![test](test.png)").unwrap();

    let (stdout, _, success) = run_with_args(&[
        "-i",
        input_path.to_str().unwrap(),
        "-o",
        "-",
        "-r",
        temp_dir.path().to_str().unwrap(),
    ]);
    assert!(success);
    // Image should be embedded as data URL
    assert!(stdout.contains("data:image/png;base64"));
}

#[test]
fn test_gfm_table() {
    let (stdout, _, success) = run_with_stdin(&["-o", "-"], "| A | B |\n|---|---|\n| 1 | 2 |");
    assert!(success);
    assert!(stdout.contains("<table>"));
    assert!(stdout.contains("<th>"));
    assert!(stdout.contains("<td>"));
}

#[test]
fn test_gfm_strikethrough() {
    let (stdout, _, success) = run_with_stdin(&["-o", "-"], "~~deleted~~");
    assert!(success);
    assert!(stdout.contains("<del>deleted</del>"));
}

#[test]
fn test_verbose_output() {
    let (_, stderr, success) = run_with_stdin(&["-o", "-", "-v"], "# Test");
    assert!(success);
    assert!(stderr.contains("Read") || stderr.contains("Generated"));
}

#[test]
fn test_quiet_mode() {
    let (_, stderr, success) = run_with_stdin(&["-o", "-", "-q"], "# Test");
    assert!(success);
    // In quiet mode, there should be minimal stderr output
    assert!(stderr.is_empty() || !stderr.contains("INFO"));
}

#[test]
fn test_dark_mode_theme() {
    let (stdout, _, success) = run_with_stdin(
        &["-o", "-", "--highlight-dark"],
        "```rust\nfn main() {}\n```",
    );
    assert!(success);
    // Should use the dark theme (base16-ocean.dark by default)
    assert!(stdout.contains("<pre"));
}

#[test]
fn test_light_mode_theme() {
    let (stdout, _, success) = run_with_stdin(
        &["-o", "-", "--highlight-light"],
        "```rust\nfn main() {}\n```",
    );
    assert!(success);
    // Should use the light theme
    assert!(stdout.contains("<pre"));
}

#[test]
fn test_custom_theme_dark() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.toml");
    std::fs::write(
        &config_path,
        "[highlight]\ntheme_dark = \"Solarized (dark)\"\n",
    )
    .unwrap();

    let (stdout, _, success) = run_with_stdin(
        &[
            "-o",
            "-",
            "-c",
            config_path.to_str().unwrap(),
            "--highlight-dark",
        ],
        "```rust\nfn main() {}\n```",
    );
    assert!(success);
    // Should use Solarized dark theme
    assert!(stdout.contains("background-color:#002b36"));
}

#[test]
fn test_complex_document() {
    let markdown = r#"# Title

This is a **paragraph** with *formatting*.

## Code Example

```rust
fn main() {
    println!("Hello, world!");
}
```

- List item 1
- List item 2

| Column A | Column B |
|----------|----------|
| Cell 1   | Cell 2   |

> A blockquote

---

[A link](https://example.com)
"#;

    let (stdout, _, success) = run_with_stdin(&["-o", "-"], markdown);
    assert!(success);
    assert!(stdout.contains("<h1>Title</h1>"));
    assert!(stdout.contains("<strong>paragraph</strong>"));
    assert!(stdout.contains("<em>formatting</em>"));
    assert!(stdout.contains("<pre"));
    assert!(stdout.contains("<ul>"));
    assert!(stdout.contains("<table>"));
    assert!(stdout.contains("<blockquote>"));
    assert!(stdout.contains("<hr"));
    assert!(stdout.contains("href=\"https://example.com\""));
}
