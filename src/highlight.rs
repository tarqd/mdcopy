use log::{debug, info, trace, warn};
use std::collections::HashMap;
use std::path::PathBuf;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};

pub struct HighlightContext {
    pub syntax_set: SyntaxSet,
    pub theme: Theme,
    language_map: HashMap<String, String>,
}

impl HighlightContext {
    pub fn new(
        theme_name: &str,
        language_map: &HashMap<String, String>,
        themes_dir: Option<&PathBuf>,
        syntaxes_dir: Option<&PathBuf>,
    ) -> Option<Self> {
        let syntax_set = load_syntax_set(syntaxes_dir);
        let theme_set = load_theme_set(themes_dir);

        let theme = theme_set.themes.get(theme_name).cloned().or_else(|| {
            warn!(
                "Theme '{}' not found, available themes: {:?}",
                theme_name,
                theme_set.themes.keys().collect::<Vec<_>>()
            );
            // Fall back to a default theme
            theme_set
                .themes
                .get("base16-ocean.dark")
                .or_else(|| theme_set.themes.values().next())
                .cloned()
        });

        theme.map(|theme| {
            info!("Using theme for syntax highlighting");
            Self {
                syntax_set,
                theme,
                language_map: language_map.clone(),
            }
        })
    }

    /// Find syntax for a language, using the language map for aliases
    pub fn find_syntax(&self, lang: &str) -> &SyntaxReference {
        let lang_lower = lang.to_lowercase();

        // First try the mapped language name
        if let Some(mapped) = self.language_map.get(&lang_lower) {
            if let Some(syntax) = self.syntax_set.find_syntax_by_name(mapped) {
                return syntax;
            }
            // Also try as token (extension)
            if let Some(syntax) = self.syntax_set.find_syntax_by_token(mapped) {
                return syntax;
            }
        }

        // Try direct lookup by token (handles extensions like "rs", "py")
        if let Some(syntax) = self.syntax_set.find_syntax_by_token(lang) {
            return syntax;
        }

        // Try by name
        if let Some(syntax) = self.syntax_set.find_syntax_by_name(lang) {
            return syntax;
        }

        // Fall back to plain text
        self.syntax_set.find_syntax_plain_text()
    }

    pub fn list_themes(themes_dir: Option<&PathBuf>) -> Vec<String> {
        let theme_set = load_theme_set(themes_dir);
        let mut themes: Vec<_> = theme_set.themes.keys().cloned().collect();
        themes.sort();
        themes
    }
}

fn get_config_dir() -> Option<PathBuf> {
    dirs::config_local_dir().map(|p| p.join("mdcopy"))
}

fn load_syntax_set(custom_dir: Option<&PathBuf>) -> SyntaxSet {
    // Determine the syntax directory to use
    let syntax_dir = custom_dir
        .cloned()
        .or_else(|| get_config_dir().map(|p| p.join("syntaxes")));

    // Check if we have custom syntaxes to load
    if let Some(syntax_dir) = syntax_dir {
        if syntax_dir.is_dir() {
            // Build a new syntax set with defaults + custom syntaxes
            let mut builder = SyntaxSet::load_defaults_newlines().into_builder();
            match builder.add_from_folder(&syntax_dir, true) {
                Ok(()) => {
                    info!("Loaded custom syntaxes from {:?}", syntax_dir);
                    let ss = builder.build();
                    debug!("Total syntaxes loaded: {}", ss.syntaxes().len());
                    return ss;
                }
                Err(e) => {
                    warn!(
                        "Failed to load custom syntaxes from {:?}: {}",
                        syntax_dir, e
                    );
                }
            }
        } else {
            trace!("No custom syntax directory at {:?}", syntax_dir);
        }
    }

    // Fall back to just defaults
    let ss = SyntaxSet::load_defaults_newlines();
    debug!("Loaded {} default syntaxes", ss.syntaxes().len());
    ss
}

fn load_theme_set(custom_dir: Option<&PathBuf>) -> ThemeSet {
    let mut theme_set = ThemeSet::load_defaults();
    debug!("Loaded {} default themes", theme_set.themes.len());

    // Determine the theme directory to use
    let theme_dir = custom_dir
        .cloned()
        .or_else(|| get_config_dir().map(|p| p.join("themes")));

    // Load custom themes
    if let Some(theme_dir) = theme_dir {
        if theme_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&theme_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension() == Some(std::ffi::OsStr::new("tmTheme")) {
                        match ThemeSet::get_theme(&path) {
                            Ok(theme) => {
                                let name = path
                                    .file_stem()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or("unknown")
                                    .to_string();
                                info!("Loaded custom theme: {}", name);
                                theme_set.themes.insert(name, theme);
                            }
                            Err(e) => {
                                warn!("Failed to load theme {:?}: {}", path, e);
                            }
                        }
                    }
                }
            }
        } else {
            trace!("No custom theme directory at {:?}", theme_dir);
        }
    }

    theme_set
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_theme_falls_back() {
        // Test MY fallback logic when given an invalid theme name
        let language_map = HashMap::new();
        let ctx = HighlightContext::new("nonexistent-theme-xyz", &language_map, None, None);
        // Should succeed by falling back to a default theme
        assert!(ctx.is_some());
    }

    #[test]
    fn test_find_syntax_uses_language_map() {
        // Test MY language mapping logic
        let mut language_map = HashMap::new();
        language_map.insert("customlang".to_string(), "Rust".to_string());

        let ctx = HighlightContext::new("base16-ocean.dark", &language_map, None, None).unwrap();

        // My code should look up "customlang" in the map and find "Rust"
        let syntax = ctx.find_syntax("customlang");
        assert_eq!(syntax.name, "Rust");
    }

    #[test]
    fn test_find_syntax_case_insensitive_key_lookup() {
        // Test MY case-insensitive key lookup logic
        let mut language_map = HashMap::new();
        language_map.insert("jsx".to_string(), "JavaScript".to_string());

        let ctx = HighlightContext::new("base16-ocean.dark", &language_map, None, None).unwrap();

        // My code lowercases the input, so "JSX" should match "jsx" in the map
        let syntax = ctx.find_syntax("JSX");
        assert_eq!(syntax.name, "JavaScript");
    }

    #[test]
    fn test_find_syntax_unknown_returns_plain_text() {
        // Test MY fallback to plain text logic
        let language_map = HashMap::new();
        let ctx = HighlightContext::new("base16-ocean.dark", &language_map, None, None).unwrap();

        // Unknown language should fall back to plain text
        let syntax = ctx.find_syntax("unknown-language-xyz-123");
        assert_eq!(syntax.name, "Plain Text");
    }

    #[test]
    fn test_list_themes_returns_sorted() {
        // Test that MY list_themes function sorts the output
        let themes = HighlightContext::list_themes(None);
        assert!(!themes.is_empty());

        let mut sorted = themes.clone();
        sorted.sort();
        assert_eq!(themes, sorted, "list_themes should return sorted themes");
    }

    #[test]
    fn test_get_config_dir_appends_mdcopy() {
        // Test that MY config dir function appends "mdcopy" subdirectory
        let config_dir = get_config_dir();
        if let Some(path) = config_dir {
            assert!(path.ends_with("mdcopy"));
        }
    }
}
