use tree_sitter_highlight::{Highlighter, HighlightConfiguration, HighlightEvent};
use std::path::Path;

const HIGHLIGHT_NAMES: &[&'static str; 18] = &[
    "attribute",
    "constant",
    "function.builtin",
    "function",
    "keyword",
    "operator",
    "property",
    "punctuation",
    "punctuation.bracket",
    "punctuation.delimiter",
    "string",
    "string.special",
    "tag",
    "type",
    "type.builtin",
    "variable",
    "variable.builtin",
    "variable.parameter",
];

pub fn get_hl_conf(path: &Path) -> Option<HighlightConfiguration> {
    eprintln!("{}", path.extension().unwrap().to_str().unwrap());
    let mut hl_conf = match path.extension() {
        None => return None,
        Some(extension) => {
            match extension.to_str().unwrap() {
                "rs" => HighlightConfiguration::new(
                    tree_sitter_rust::language(),
                    tree_sitter_rust::HIGHLIGHT_QUERY,
                    "",
                    "").unwrap(),
                _ => return None,
                "toml" => HighlightConfiguration::new(
                    tree_sitter_toml::language(),
                    tree_sitter_toml::HIGHLIGHT_QUERY,
                    "",
                    "").unwrap(),
            }
        }
    };

    hl_conf.configure(HIGHLIGHT_NAMES);
    Some(hl_conf)
}
