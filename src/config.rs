use crate::EmbedMode;
use log::{debug, trace};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

/// Highlight configuration from file
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct FileHighlightConfig {
    pub enable: Option<bool>,
    pub theme: Option<String>,
    pub themes_dir: Option<String>,
    pub syntaxes_dir: Option<String>,
    #[serde(default)]
    pub languages: HashMap<String, String>,
}

/// Configuration loaded from file
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct FileConfig {
    pub input: Option<String>,
    pub output: Option<String>,
    pub root: Option<String>,
    pub embed: Option<String>,
    pub strict: Option<bool>,
    #[serde(default)]
    pub highlight: FileHighlightConfig,
}

/// Resolved highlight configuration
#[derive(Debug)]
pub struct HighlightConfig {
    pub enable: bool,
    pub theme: String,
    pub themes_dir: Option<PathBuf>,
    pub syntaxes_dir: Option<PathBuf>,
    pub languages: HashMap<String, String>,
}

impl Default for HighlightConfig {
    fn default() -> Self {
        Self {
            enable: true,
            theme: "base16-ocean.dark".to_string(),
            themes_dir: None,
            syntaxes_dir: None,
            languages: default_language_mappings(),
        }
    }
}

/// Resolved configuration with all sources merged
#[derive(Debug)]
pub struct Config {
    pub input: PathBuf,
    pub output: Option<PathBuf>,
    pub root: Option<PathBuf>,
    pub embed: EmbedMode,
    pub strict: bool,
    pub highlight: HighlightConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            input: PathBuf::from("-"),
            output: None,
            root: None,
            embed: EmbedMode::Local,
            strict: false,
            highlight: HighlightConfig::default(),
        }
    }
}

/// Default language alias mappings
fn default_language_mappings() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("js".to_string(), "JavaScript".to_string());
    m.insert("ts".to_string(), "TypeScript".to_string());
    m.insert("py".to_string(), "Python".to_string());
    m.insert("rb".to_string(), "Ruby".to_string());
    m.insert("rs".to_string(), "Rust".to_string());
    m.insert("sh".to_string(), "Bourne Again Shell (bash)".to_string());
    m.insert("bash".to_string(), "Bourne Again Shell (bash)".to_string());
    m.insert("zsh".to_string(), "Bourne Again Shell (bash)".to_string());
    m.insert("yml".to_string(), "YAML".to_string());
    m.insert("md".to_string(), "Markdown".to_string());
    m.insert("dockerfile".to_string(), "Dockerfile".to_string());
    m
}

/// Get the XDG config directory ($XDG_CONFIG_HOME or ~/.config)
fn xdg_config_dir() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| dirs::home_dir().map(|p| p.join(".config")))
}

/// Get the default config file path
/// On macOS, checks ~/Library/Application Support/mdcopy/ first, then $XDG_CONFIG_HOME/mdcopy/
pub fn default_config_path() -> Option<PathBuf> {
    default_config_dir().map(|p| p.join("config.toml"))
}

/// Get the default config directory
/// On macOS, checks ~/Library/Application Support/mdcopy/ first, then $XDG_CONFIG_HOME/mdcopy/
pub fn default_config_dir() -> Option<PathBuf> {
    // Primary location (platform standard)
    let primary = dirs::config_local_dir().map(|p| p.join("mdcopy"));

    // On macOS, fall back to XDG config dir if primary doesn't exist
    #[cfg(target_os = "macos")]
    {
        if primary.as_ref().is_some_and(|p| p.exists()) {
            return primary;
        }
        // Check XDG-style fallback
        let fallback = xdg_config_dir().map(|p| p.join("mdcopy"));
        if fallback.as_ref().is_some_and(|p| p.exists()) {
            return fallback;
        }
        // Neither exists, return primary (will be created there if needed)
        primary
    }

    #[cfg(not(target_os = "macos"))]
    {
        primary
    }
}

/// Load configuration from a TOML file
pub fn load_config_file(path: &PathBuf) -> Option<FileConfig> {
    match std::fs::read_to_string(path) {
        Ok(content) => match toml::from_str(&content) {
            Ok(config) => {
                debug!("Loaded config from {:?}", path);
                Some(config)
            }
            Err(e) => {
                log::warn!("Failed to parse config file {:?}: {}", path, e);
                None
            }
        },
        Err(e) => {
            if e.kind() != std::io::ErrorKind::NotFound {
                log::warn!("Failed to read config file {:?}: {}", path, e);
            } else {
                trace!("No config file at {:?}", path);
            }
            None
        }
    }
}

/// Load a setting from environment variable
fn env_var(name: &str) -> Option<String> {
    let key = format!("MDCOPY_{}", name.to_uppercase());
    std::env::var(&key).ok().map(|v| {
        trace!("Found env var {}={}", key, v);
        v
    })
}

fn parse_bool(s: &str) -> Option<bool> {
    match s.to_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn parse_embed_mode(s: &str) -> Option<EmbedMode> {
    match s.to_lowercase().as_str() {
        "all" => Some(EmbedMode::All),
        "local" => Some(EmbedMode::Local),
        "none" => Some(EmbedMode::None),
        _ => None,
    }
}

/// CLI argument values for highlight settings
pub struct CliHighlightArgs {
    pub enable: Option<bool>,
    pub theme: Option<String>,
    pub themes_dir: Option<PathBuf>,
    pub syntaxes_dir: Option<PathBuf>,
}

/// CLI argument values (None means not specified)
pub struct CliArgs {
    pub input: Option<PathBuf>,
    pub output: Option<PathBuf>,
    pub root: Option<PathBuf>,
    pub embed: Option<EmbedMode>,
    pub strict: Option<bool>,
    pub highlight: CliHighlightArgs,
}

impl HighlightConfig {
    /// Get the theme name
    pub fn effective_theme(&self) -> &str {
        &self.theme
    }

    /// Get the themes directory (custom or default)
    pub fn get_themes_dir(&self) -> Option<PathBuf> {
        self.themes_dir
            .clone()
            .or_else(|| default_config_dir().map(|p| p.join("themes")))
    }

    /// Get the syntaxes directory (custom or default)
    pub fn get_syntaxes_dir(&self) -> Option<PathBuf> {
        self.syntaxes_dir
            .clone()
            .or_else(|| default_config_dir().map(|p| p.join("syntaxes")))
    }
}

impl Config {
    /// Build configuration with precedence: CLI > env vars > config file > defaults
    #[allow(clippy::field_reassign_with_default)]
    pub fn build(cli: CliArgs, config_path: Option<PathBuf>) -> Self {
        let mut config = Config::default();

        // Load config file (lowest priority after defaults)
        let file_config = config_path
            .or_else(default_config_path)
            .and_then(|p| load_config_file(&p))
            .unwrap_or_default();

        // Apply config file values
        if let Some(v) = file_config.input {
            config.input = PathBuf::from(v);
        }
        if let Some(v) = file_config.output {
            config.output = Some(PathBuf::from(v));
        }
        if let Some(v) = file_config.root {
            config.root = Some(PathBuf::from(v));
        }
        if let Some(v) = file_config.embed.and_then(|s| parse_embed_mode(&s)) {
            config.embed = v;
        }
        if let Some(v) = file_config.strict {
            config.strict = v;
        }

        // Apply highlight config from file
        if let Some(v) = file_config.highlight.enable {
            config.highlight.enable = v;
        }
        if let Some(v) = file_config.highlight.theme {
            config.highlight.theme = v;
        }
        if let Some(v) = file_config.highlight.themes_dir {
            config.highlight.themes_dir = Some(PathBuf::from(v));
        }
        if let Some(v) = file_config.highlight.syntaxes_dir {
            config.highlight.syntaxes_dir = Some(PathBuf::from(v));
        }
        for (k, v) in file_config.highlight.languages {
            config.highlight.languages.insert(k, v);
        }

        // Apply environment variables (higher priority than config file)
        if let Some(v) = env_var("input") {
            config.input = PathBuf::from(v);
        }
        if let Some(v) = env_var("output") {
            config.output = Some(PathBuf::from(v));
        }
        if let Some(v) = env_var("root") {
            config.root = Some(PathBuf::from(v));
        }
        if let Some(v) = env_var("embed").and_then(|s| parse_embed_mode(&s)) {
            config.embed = v;
        }
        if let Some(v) = env_var("strict").and_then(|s| parse_bool(&s)) {
            config.strict = v;
        }

        // Highlight env vars (MDCOPY_HIGHLIGHT_*)
        if let Some(v) = env_var("highlight").and_then(|s| parse_bool(&s)) {
            config.highlight.enable = v;
        }
        if let Some(v) = env_var("highlight_theme") {
            config.highlight.theme = v;
        }
        if let Some(v) = env_var("highlight_themes_dir") {
            config.highlight.themes_dir = Some(PathBuf::from(v));
        }
        if let Some(v) = env_var("highlight_syntaxes_dir") {
            config.highlight.syntaxes_dir = Some(PathBuf::from(v));
        }

        // Apply CLI arguments (highest priority)
        if let Some(v) = cli.input {
            config.input = v;
        }
        if let Some(v) = cli.output {
            config.output = Some(v);
        }
        if let Some(v) = cli.root {
            config.root = Some(v);
        }
        if let Some(v) = cli.embed {
            config.embed = v;
        }
        if let Some(v) = cli.strict {
            config.strict = v;
        }

        // Highlight CLI args
        if let Some(v) = cli.highlight.enable {
            config.highlight.enable = v;
        }
        if let Some(v) = cli.highlight.theme {
            config.highlight.theme = v;
        }
        if let Some(v) = cli.highlight.themes_dir {
            config.highlight.themes_dir = Some(v);
        }
        if let Some(v) = cli.highlight.syntaxes_dir {
            config.highlight.syntaxes_dir = Some(v);
        }

        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn empty_cli_args() -> CliArgs {
        CliArgs {
            input: None,
            output: None,
            root: None,
            embed: None,
            strict: None,
            highlight: CliHighlightArgs {
                enable: None,
                theme: None,
                themes_dir: None,
                syntaxes_dir: None,
            },
        }
    }

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.input, PathBuf::from("-"));
        assert!(config.output.is_none());
        assert!(config.root.is_none());
        assert_eq!(config.embed, crate::EmbedMode::Local);
        assert!(!config.strict);
        assert!(config.highlight.enable);
        assert_eq!(config.highlight.theme, "base16-ocean.dark");
    }

    #[test]
    fn test_default_language_mappings() {
        let mappings = default_language_mappings();
        assert_eq!(mappings.get("js"), Some(&"JavaScript".to_string()));
        assert_eq!(mappings.get("ts"), Some(&"TypeScript".to_string()));
        assert_eq!(mappings.get("py"), Some(&"Python".to_string()));
        assert_eq!(mappings.get("rs"), Some(&"Rust".to_string()));
        assert_eq!(
            mappings.get("sh"),
            Some(&"Bourne Again Shell (bash)".to_string())
        );
    }

    #[test]
    fn test_parse_bool() {
        assert_eq!(parse_bool("true"), Some(true));
        assert_eq!(parse_bool("TRUE"), Some(true));
        assert_eq!(parse_bool("1"), Some(true));
        assert_eq!(parse_bool("yes"), Some(true));
        assert_eq!(parse_bool("on"), Some(true));

        assert_eq!(parse_bool("false"), Some(false));
        assert_eq!(parse_bool("FALSE"), Some(false));
        assert_eq!(parse_bool("0"), Some(false));
        assert_eq!(parse_bool("no"), Some(false));
        assert_eq!(parse_bool("off"), Some(false));

        assert_eq!(parse_bool("invalid"), None);
        assert_eq!(parse_bool(""), None);
    }

    #[test]
    fn test_parse_embed_mode() {
        assert_eq!(parse_embed_mode("all"), Some(crate::EmbedMode::All));
        assert_eq!(parse_embed_mode("ALL"), Some(crate::EmbedMode::All));
        assert_eq!(parse_embed_mode("local"), Some(crate::EmbedMode::Local));
        assert_eq!(parse_embed_mode("none"), Some(crate::EmbedMode::None));
        assert_eq!(parse_embed_mode("invalid"), None);
    }

    #[test]
    fn test_highlight_config_effective_theme() {
        let config = HighlightConfig {
            theme: "custom-theme".to_string(),
            ..Default::default()
        };
        assert_eq!(config.effective_theme(), "custom-theme");
    }

    #[test]
    fn test_load_config_file_valid() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        let mut file = std::fs::File::create(&config_path).unwrap();
        writeln!(file, "strict = true").unwrap();
        writeln!(file, "embed = \"all\"").unwrap();
        writeln!(file, "[highlight]").unwrap();
        writeln!(file, "enable = false").unwrap();
        writeln!(file, "theme = \"my-theme\"").unwrap();

        let config = load_config_file(&config_path);
        assert!(config.is_some());
        let config = config.unwrap();
        assert_eq!(config.strict, Some(true));
        assert_eq!(config.embed, Some("all".to_string()));
        assert_eq!(config.highlight.enable, Some(false));
        assert_eq!(config.highlight.theme, Some("my-theme".to_string()));
    }

    #[test]
    fn test_load_config_file_not_found() {
        let config = load_config_file(&PathBuf::from("/nonexistent/config.toml"));
        assert!(config.is_none());
    }

    #[test]
    fn test_load_config_file_invalid_toml() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        let mut file = std::fs::File::create(&config_path).unwrap();
        writeln!(file, "invalid toml [ content").unwrap();

        let config = load_config_file(&config_path);
        assert!(config.is_none());
    }

    #[test]
    fn test_load_config_file_with_languages() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        let mut file = std::fs::File::create(&config_path).unwrap();
        writeln!(file, "[highlight.languages]").unwrap();
        writeln!(file, "jsx = \"JavaScript\"").unwrap();
        writeln!(file, "tsx = \"TypeScript\"").unwrap();

        let config = load_config_file(&config_path).unwrap();
        assert_eq!(
            config.highlight.languages.get("jsx"),
            Some(&"JavaScript".to_string())
        );
        assert_eq!(
            config.highlight.languages.get("tsx"),
            Some(&"TypeScript".to_string())
        );
    }

    #[test]
    fn test_config_build_defaults() {
        let cli = empty_cli_args();
        let config = Config::build(cli, None);

        assert_eq!(config.input, PathBuf::from("-"));
        assert!(config.output.is_none());
        assert_eq!(config.embed, crate::EmbedMode::Local);
        assert!(!config.strict);
        assert!(config.highlight.enable);
    }

    #[test]
    fn test_config_build_cli_overrides() {
        let cli = CliArgs {
            input: Some(PathBuf::from("input.md")),
            output: Some(PathBuf::from("output.html")),
            root: Some(PathBuf::from("/custom/root")),
            embed: Some(crate::EmbedMode::All),
            strict: Some(true),
            highlight: CliHighlightArgs {
                enable: Some(false),
                theme: Some("custom".to_string()),
                themes_dir: Some(PathBuf::from("/themes")),
                syntaxes_dir: Some(PathBuf::from("/syntaxes")),
            },
        };

        let config = Config::build(cli, None);

        assert_eq!(config.input, PathBuf::from("input.md"));
        assert_eq!(config.output, Some(PathBuf::from("output.html")));
        assert_eq!(config.root, Some(PathBuf::from("/custom/root")));
        assert_eq!(config.embed, crate::EmbedMode::All);
        assert!(config.strict);
        assert!(!config.highlight.enable);
        assert_eq!(config.highlight.theme, "custom");
        assert_eq!(config.highlight.themes_dir, Some(PathBuf::from("/themes")));
        assert_eq!(
            config.highlight.syntaxes_dir,
            Some(PathBuf::from("/syntaxes"))
        );
    }

    #[test]
    fn test_config_build_from_file() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        let mut file = std::fs::File::create(&config_path).unwrap();
        writeln!(file, "input = \"from-file.md\"").unwrap();
        writeln!(file, "strict = true").unwrap();
        writeln!(file, "[highlight]").unwrap();
        writeln!(file, "theme = \"file-theme\"").unwrap();

        let cli = empty_cli_args();
        let config = Config::build(cli, Some(config_path));

        assert_eq!(config.input, PathBuf::from("from-file.md"));
        assert!(config.strict);
        assert_eq!(config.highlight.theme, "file-theme");
    }

    #[test]
    fn test_config_build_cli_overrides_file() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        let mut file = std::fs::File::create(&config_path).unwrap();
        writeln!(file, "input = \"from-file.md\"").unwrap();
        writeln!(file, "strict = true").unwrap();

        let cli = CliArgs {
            input: Some(PathBuf::from("from-cli.md")),
            strict: Some(false),
            ..empty_cli_args()
        };

        let config = Config::build(cli, Some(config_path));

        // CLI should override file
        assert_eq!(config.input, PathBuf::from("from-cli.md"));
        assert!(!config.strict);
    }

    #[test]
    fn test_highlight_config_get_themes_dir_custom() {
        let config = HighlightConfig {
            themes_dir: Some(PathBuf::from("/custom/themes")),
            ..Default::default()
        };
        assert_eq!(
            config.get_themes_dir(),
            Some(PathBuf::from("/custom/themes"))
        );
    }

    #[test]
    fn test_highlight_config_get_syntaxes_dir_custom() {
        let config = HighlightConfig {
            syntaxes_dir: Some(PathBuf::from("/custom/syntaxes")),
            ..Default::default()
        };
        assert_eq!(
            config.get_syntaxes_dir(),
            Some(PathBuf::from("/custom/syntaxes"))
        );
    }

    #[test]
    fn test_file_config_default() {
        let config = FileConfig::default();
        assert!(config.input.is_none());
        assert!(config.output.is_none());
        assert!(config.root.is_none());
        assert!(config.embed.is_none());
        assert!(config.strict.is_none());
        assert!(config.highlight.enable.is_none());
    }
}
