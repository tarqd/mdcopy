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

/// Image embed configuration from file
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct FileImageEmbedConfig {
    pub local: Option<bool>,
    pub remote: Option<bool>,
    pub optimize_local: Option<bool>,
    pub optimize_remote: Option<bool>,
    pub max_dimension: Option<u32>,
    pub quality: Option<u8>,
}

/// Image configuration from file (wrapper for nested [image.embed])
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct FileImageConfig {
    #[serde(default)]
    pub embed: FileImageEmbedConfig,
}

/// Configuration loaded from file
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct FileConfig {
    pub input: Option<String>,
    pub output: Option<String>,
    pub root: Option<String>,
    pub strict: Option<bool>,
    pub prosemirror: Option<bool>,
    #[serde(default)]
    pub highlight: FileHighlightConfig,
    #[serde(default)]
    pub image: FileImageConfig,
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

/// Resolved image configuration
#[derive(Debug, Clone)]
pub struct ImageConfig {
    pub embed_local: bool,
    pub embed_remote: bool,
    pub optimize_local: bool,
    pub optimize_remote: bool,
    pub max_dimension: u32,
    pub quality: u8,
}

impl Default for ImageConfig {
    fn default() -> Self {
        Self {
            embed_local: true,
            embed_remote: false,
            optimize_local: true,
            optimize_remote: false,
            max_dimension: 1200,
            quality: 80,
        }
    }
}

/// Source of a configuration value
#[derive(Debug, Clone)]
pub enum ConfigSource {
    /// Default value
    Default,
    /// From config file
    File(PathBuf),
    /// From environment variable
    Env(String),
    /// From CLI argument
    Cli,
}

impl std::fmt::Display for ConfigSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigSource::Default => write!(f, "default"),
            ConfigSource::File(path) => write!(f, "config: {}", path.display()),
            ConfigSource::Env(var) => write!(f, "env: {}", var),
            ConfigSource::Cli => write!(f, "cli"),
        }
    }
}

/// Tracks the source of each configuration value
#[derive(Debug, Clone)]
pub struct ConfigSources {
    pub embed_local: ConfigSource,
    pub embed_remote: ConfigSource,
    pub optimize_local: ConfigSource,
    pub optimize_remote: ConfigSource,
    pub max_dimension: ConfigSource,
    pub quality: ConfigSource,
    pub strict: ConfigSource,
    pub highlight_enable: ConfigSource,
    pub highlight_theme: ConfigSource,
}

impl Default for ConfigSources {
    fn default() -> Self {
        Self {
            embed_local: ConfigSource::Default,
            embed_remote: ConfigSource::Default,
            optimize_local: ConfigSource::Default,
            optimize_remote: ConfigSource::Default,
            max_dimension: ConfigSource::Default,
            quality: ConfigSource::Default,
            strict: ConfigSource::Default,
            highlight_enable: ConfigSource::Default,
            highlight_theme: ConfigSource::Default,
        }
    }
}

impl ConfigSources {
    /// Format current settings with their sources for display (used in --help)
    pub fn format_settings(&self, config: &Config) -> String {
        let mut lines = Vec::new();
        lines.push(format!(
            "  embed_local: {} ({})",
            config.image.embed_local, self.embed_local
        ));
        lines.push(format!(
            "  embed_remote: {} ({})",
            config.image.embed_remote, self.embed_remote
        ));
        lines.push(format!(
            "  optimize_local: {} ({})",
            config.image.optimize_local, self.optimize_local
        ));
        lines.push(format!(
            "  optimize_remote: {} ({})",
            config.image.optimize_remote, self.optimize_remote
        ));
        lines.push(format!(
            "  max_dimension: {} ({})",
            config.image.max_dimension, self.max_dimension
        ));
        lines.push(format!(
            "  quality: {} ({})",
            config.image.quality, self.quality
        ));
        lines.push(format!("  strict: {} ({})", config.strict, self.strict));
        lines.push(format!(
            "  highlight: {} ({})",
            config.highlight.enable, self.highlight_enable
        ));
        lines.push(format!(
            "  highlight_theme: {} ({})",
            config.highlight.theme, self.highlight_theme
        ));
        lines.join("\n")
    }
}

/// Resolved configuration with all sources merged
#[derive(Debug)]
pub struct Config {
    pub input: PathBuf,
    pub output: Option<PathBuf>,
    pub root: Option<PathBuf>,
    pub strict: bool,
    /// Emit ProseMirror slice marker for Confluence paste compatibility
    pub prosemirror: bool,
    pub highlight: HighlightConfig,
    pub image: ImageConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            input: PathBuf::from("-"),
            output: None,
            root: None,
            strict: false,
            prosemirror: true,
            highlight: HighlightConfig::default(),
            image: ImageConfig::default(),
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
#[cfg(target_os = "macos")]
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

/// CLI argument values for highlight settings
pub struct CliHighlightArgs {
    pub enable: Option<bool>,
    pub theme: Option<String>,
    pub themes_dir: Option<PathBuf>,
    pub syntaxes_dir: Option<PathBuf>,
}

/// CLI argument values for image settings
pub struct CliImageArgs {
    pub embed_local: Option<bool>,
    pub embed_remote: Option<bool>,
    pub optimize_local: Option<bool>,
    pub optimize_remote: Option<bool>,
    pub max_dimension: Option<u32>,
    pub quality: Option<u8>,
}

/// CLI argument values (None means not specified)
pub struct CliArgs {
    pub input: Option<PathBuf>,
    pub output: Option<PathBuf>,
    pub root: Option<PathBuf>,
    pub strict: Option<bool>,
    pub prosemirror: Option<bool>,
    pub highlight: CliHighlightArgs,
    pub image: CliImageArgs,
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
    /// Returns the config along with source tracking for each value
    #[allow(clippy::field_reassign_with_default)]
    pub fn build(cli: CliArgs, config_path: Option<PathBuf>) -> (Self, ConfigSources) {
        let mut config = Config::default();
        let mut sources = ConfigSources::default();

        // Determine which config file to use and load it
        let resolved_config_path = config_path.or_else(default_config_path);
        let (file_config, config_file_path) = resolved_config_path
            .and_then(|p| load_config_file(&p).map(|c| (c, p)))
            .map(|(c, p)| (c, Some(p)))
            .unwrap_or((FileConfig::default(), None));

        // Helper to create file source
        let file_source = |path: &Option<PathBuf>| -> ConfigSource {
            path.as_ref()
                .map(|p| ConfigSource::File(p.clone()))
                .unwrap_or(ConfigSource::Default)
        };

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
        if file_config.strict.is_some() {
            config.strict = file_config.strict.unwrap();
            sources.strict = file_source(&config_file_path);
        }
        if let Some(v) = file_config.prosemirror {
            config.prosemirror = v;
        }

        // Apply highlight config from file
        if file_config.highlight.enable.is_some() {
            config.highlight.enable = file_config.highlight.enable.unwrap();
            sources.highlight_enable = file_source(&config_file_path);
        }
        if file_config.highlight.theme.is_some() {
            config.highlight.theme = file_config.highlight.theme.unwrap();
            sources.highlight_theme = file_source(&config_file_path);
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

        // Apply image config from file
        if file_config.image.embed.local.is_some() {
            config.image.embed_local = file_config.image.embed.local.unwrap();
            sources.embed_local = file_source(&config_file_path);
        }
        if file_config.image.embed.remote.is_some() {
            config.image.embed_remote = file_config.image.embed.remote.unwrap();
            sources.embed_remote = file_source(&config_file_path);
        }
        if file_config.image.embed.optimize_local.is_some() {
            config.image.optimize_local = file_config.image.embed.optimize_local.unwrap();
            sources.optimize_local = file_source(&config_file_path);
        }
        if file_config.image.embed.optimize_remote.is_some() {
            config.image.optimize_remote = file_config.image.embed.optimize_remote.unwrap();
            sources.optimize_remote = file_source(&config_file_path);
        }
        if file_config.image.embed.max_dimension.is_some() {
            config.image.max_dimension = file_config.image.embed.max_dimension.unwrap();
            sources.max_dimension = file_source(&config_file_path);
        }
        if file_config.image.embed.quality.is_some() {
            config.image.quality = file_config.image.embed.quality.unwrap();
            sources.quality = file_source(&config_file_path);
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
        if let Some(v) = env_var("strict").and_then(|s| parse_bool(&s)) {
            config.strict = v;
            sources.strict = ConfigSource::Env("MDCOPY_STRICT".to_string());
        }
        if let Some(v) = env_var("prosemirror").and_then(|s| parse_bool(&s)) {
            config.prosemirror = v;
        }

        // Highlight env vars (MDCOPY_HIGHLIGHT_*)
        if let Some(v) = env_var("highlight").and_then(|s| parse_bool(&s)) {
            config.highlight.enable = v;
            sources.highlight_enable = ConfigSource::Env("MDCOPY_HIGHLIGHT".to_string());
        }
        if let Some(v) = env_var("highlight_theme") {
            config.highlight.theme = v;
            sources.highlight_theme = ConfigSource::Env("MDCOPY_HIGHLIGHT_THEME".to_string());
        }
        if let Some(v) = env_var("highlight_themes_dir") {
            config.highlight.themes_dir = Some(PathBuf::from(v));
        }
        if let Some(v) = env_var("highlight_syntaxes_dir") {
            config.highlight.syntaxes_dir = Some(PathBuf::from(v));
        }

        // Image env vars (MDCOPY_IMAGE_EMBED_*)
        if let Some(v) = env_var("image_embed_local").and_then(|s| parse_bool(&s)) {
            config.image.embed_local = v;
            sources.embed_local = ConfigSource::Env("MDCOPY_IMAGE_EMBED_LOCAL".to_string());
        }
        if let Some(v) = env_var("image_embed_remote").and_then(|s| parse_bool(&s)) {
            config.image.embed_remote = v;
            sources.embed_remote = ConfigSource::Env("MDCOPY_IMAGE_EMBED_REMOTE".to_string());
        }
        if let Some(v) = env_var("image_embed_optimize_local").and_then(|s| parse_bool(&s)) {
            config.image.optimize_local = v;
            sources.optimize_local =
                ConfigSource::Env("MDCOPY_IMAGE_EMBED_OPTIMIZE_LOCAL".to_string());
        }
        if let Some(v) = env_var("image_embed_optimize_remote").and_then(|s| parse_bool(&s)) {
            config.image.optimize_remote = v;
            sources.optimize_remote =
                ConfigSource::Env("MDCOPY_IMAGE_EMBED_OPTIMIZE_REMOTE".to_string());
        }
        if let Some(v) = env_var("image_embed_max_dimension").and_then(|s| s.parse().ok()) {
            config.image.max_dimension = v;
            sources.max_dimension =
                ConfigSource::Env("MDCOPY_IMAGE_EMBED_MAX_DIMENSION".to_string());
        }
        if let Some(v) = env_var("image_embed_quality").and_then(|s| s.parse().ok()) {
            config.image.quality = v;
            sources.quality = ConfigSource::Env("MDCOPY_IMAGE_EMBED_QUALITY".to_string());
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
        if let Some(v) = cli.strict {
            config.strict = v;
            sources.strict = ConfigSource::Cli;
        }
        if let Some(v) = cli.prosemirror {
            config.prosemirror = v;
        }

        // Highlight CLI args
        if let Some(v) = cli.highlight.enable {
            config.highlight.enable = v;
            sources.highlight_enable = ConfigSource::Cli;
        }
        if let Some(v) = cli.highlight.theme {
            config.highlight.theme = v;
            sources.highlight_theme = ConfigSource::Cli;
        }
        if let Some(v) = cli.highlight.themes_dir {
            config.highlight.themes_dir = Some(v);
        }
        if let Some(v) = cli.highlight.syntaxes_dir {
            config.highlight.syntaxes_dir = Some(v);
        }

        // Image CLI args
        if let Some(v) = cli.image.embed_local {
            config.image.embed_local = v;
            sources.embed_local = ConfigSource::Cli;
        }
        if let Some(v) = cli.image.embed_remote {
            config.image.embed_remote = v;
            sources.embed_remote = ConfigSource::Cli;
        }
        if let Some(v) = cli.image.optimize_local {
            config.image.optimize_local = v;
            sources.optimize_local = ConfigSource::Cli;
        }
        if let Some(v) = cli.image.optimize_remote {
            config.image.optimize_remote = v;
            sources.optimize_remote = ConfigSource::Cli;
        }
        if let Some(v) = cli.image.max_dimension {
            config.image.max_dimension = v;
            sources.max_dimension = ConfigSource::Cli;
        }
        if let Some(v) = cli.image.quality {
            config.image.quality = v;
            sources.quality = ConfigSource::Cli;
        }

        (config, sources)
    }

    /// Output current configuration as TOML
    pub fn to_toml(&self) -> String {
        let input_line = if self.input.as_os_str() != "-" {
            format!("input = {:?}\n", self.input.display().to_string())
        } else {
            String::new()
        };
        let output_line = self
            .output
            .as_ref()
            .map(|p| format!("output = {:?}\n", p.display().to_string()))
            .unwrap_or_default();
        let root_line = self
            .root
            .as_ref()
            .map(|p| format!("root = {:?}\n", p.display().to_string()))
            .unwrap_or_default();
        let themes_dir_line = self
            .highlight
            .themes_dir
            .as_ref()
            .map(|p| format!("themes_dir = {:?}\n", p.display().to_string()))
            .unwrap_or_default();
        let syntaxes_dir_line = self
            .highlight
            .syntaxes_dir
            .as_ref()
            .map(|p| format!("syntaxes_dir = {:?}\n", p.display().to_string()))
            .unwrap_or_default();

        format!(
            "{input_line}{output_line}{root_line}strict = {strict}

[highlight]
enable = {highlight_enable}
theme = {highlight_theme:?}
{themes_dir_line}{syntaxes_dir_line}
[image.embed]
local = {embed_local}
remote = {embed_remote}
optimize_local = {optimize_local}
optimize_remote = {optimize_remote}
max_dimension = {max_dimension}
quality = {quality}",
            strict = self.strict,
            highlight_enable = self.highlight.enable,
            highlight_theme = self.highlight.theme,
            embed_local = self.image.embed_local,
            embed_remote = self.image.embed_remote,
            optimize_local = self.image.optimize_local,
            optimize_remote = self.image.optimize_remote,
            max_dimension = self.image.max_dimension,
            quality = self.image.quality,
        )
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
            strict: None,
            prosemirror: None,
            highlight: CliHighlightArgs {
                enable: None,
                theme: None,
                themes_dir: None,
                syntaxes_dir: None,
            },
            image: CliImageArgs {
                embed_local: None,
                embed_remote: None,
                optimize_local: None,
                optimize_remote: None,
                max_dimension: None,
                quality: None,
            },
        }
    }

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.input, PathBuf::from("-"));
        assert!(config.output.is_none());
        assert!(config.root.is_none());
        assert!(!config.strict);
        assert!(config.highlight.enable);
        assert_eq!(config.highlight.theme, "base16-ocean.dark");
        assert!(config.image.embed_local);
        assert!(!config.image.embed_remote);
        assert!(config.image.optimize_local);
        assert!(!config.image.optimize_remote);
        assert_eq!(config.image.max_dimension, 1200);
        assert_eq!(config.image.quality, 80);
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
        writeln!(file, "[highlight]").unwrap();
        writeln!(file, "enable = false").unwrap();
        writeln!(file, "theme = \"my-theme\"").unwrap();
        writeln!(file, "[image.embed]").unwrap();
        writeln!(file, "local = true").unwrap();
        writeln!(file, "remote = true").unwrap();

        let config = load_config_file(&config_path);
        assert!(config.is_some());
        let config = config.unwrap();
        assert_eq!(config.strict, Some(true));
        assert_eq!(config.highlight.enable, Some(false));
        assert_eq!(config.highlight.theme, Some("my-theme".to_string()));
        assert_eq!(config.image.embed.local, Some(true));
        assert_eq!(config.image.embed.remote, Some(true));
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
        let (config, _sources) = Config::build(cli, None);

        assert_eq!(config.input, PathBuf::from("-"));
        assert!(config.output.is_none());
        assert!(config.image.embed_local);
        assert!(!config.image.embed_remote);
        assert!(!config.strict);
        assert!(config.highlight.enable);
    }

    #[test]
    fn test_config_build_cli_overrides() {
        let cli = CliArgs {
            input: Some(PathBuf::from("input.md")),
            output: Some(PathBuf::from("output.html")),
            root: Some(PathBuf::from("/custom/root")),
            strict: Some(true),
            prosemirror: None,
            highlight: CliHighlightArgs {
                enable: Some(false),
                theme: Some("custom".to_string()),
                themes_dir: Some(PathBuf::from("/themes")),
                syntaxes_dir: Some(PathBuf::from("/syntaxes")),
            },
            image: CliImageArgs {
                embed_local: Some(true),
                embed_remote: Some(true),
                optimize_local: Some(false),
                optimize_remote: Some(false),
                max_dimension: Some(800),
                quality: Some(75),
            },
        };

        let (config, sources) = Config::build(cli, None);

        assert_eq!(config.input, PathBuf::from("input.md"));
        assert_eq!(config.output, Some(PathBuf::from("output.html")));
        assert_eq!(config.root, Some(PathBuf::from("/custom/root")));
        assert!(config.image.embed_local);
        assert!(config.image.embed_remote);
        assert!(config.strict);
        assert!(!config.highlight.enable);
        assert_eq!(config.highlight.theme, "custom");
        assert_eq!(config.highlight.themes_dir, Some(PathBuf::from("/themes")));
        assert_eq!(
            config.highlight.syntaxes_dir,
            Some(PathBuf::from("/syntaxes"))
        );
        assert!(!config.image.optimize_local);
        assert!(!config.image.optimize_remote);
        assert_eq!(config.image.max_dimension, 800);
        assert_eq!(config.image.quality, 75);

        // Verify sources are tracked as CLI
        assert!(matches!(sources.embed_local, ConfigSource::Cli));
        assert!(matches!(sources.strict, ConfigSource::Cli));
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
        let (config, sources) = Config::build(cli, Some(config_path.clone()));

        assert_eq!(config.input, PathBuf::from("from-file.md"));
        assert!(config.strict);
        assert_eq!(config.highlight.theme, "file-theme");

        // Verify sources are tracked as file
        assert!(matches!(sources.strict, ConfigSource::File(ref p) if p == &config_path));
        assert!(matches!(sources.highlight_theme, ConfigSource::File(_)));
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

        let (config, sources) = Config::build(cli, Some(config_path));

        // CLI should override file
        assert_eq!(config.input, PathBuf::from("from-cli.md"));
        assert!(!config.strict);

        // Verify CLI overrode file source
        assert!(matches!(sources.strict, ConfigSource::Cli));
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
        assert!(config.strict.is_none());
        assert!(config.highlight.enable.is_none());
        assert!(config.image.embed.local.is_none());
        assert!(config.image.embed.remote.is_none());
        assert!(config.image.embed.optimize_local.is_none());
        assert!(config.image.embed.optimize_remote.is_none());
        assert!(config.image.embed.max_dimension.is_none());
        assert!(config.image.embed.quality.is_none());
    }
}
