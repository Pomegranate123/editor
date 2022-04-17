use std::path::Path;
use tree_sitter_highlight::HighlightConfiguration;

#[derive(Debug)]
pub enum Language {
    Rust,
    Toml,
}

pub fn detect(path: &Path) -> Option<Language> {
    Some(match path.extension() {
        None => return None,
        Some(extension) => match extension.to_str().unwrap() {
            "rs" => Language::Rust,
            "toml" => Language::Toml,
            _ => return None
        }
    })
}

impl From<Language> for HighlightConfiguration {
    fn from(lang: Language) -> HighlightConfiguration {
        match lang {
            Language::Rust => {
                HighlightConfiguration::new(
                tree_sitter_rust::language(),
                tree_sitter_rust::HIGHLIGHT_QUERY,
                "", "").unwrap()
            }
            Language::Toml => {
                HighlightConfiguration::new(
                tree_sitter_toml::language(),
                tree_sitter_toml::HIGHLIGHT_QUERY,
                "", "").unwrap()
            }
        }
    }
}
