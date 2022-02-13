use serde::{Serialize, Deserialize};
use crossterm::style::{Color, Attributes};
use std::collections::HashMap;

struct Config {
    hl_styles: HashMap<String, Style>
}

//#[derive(Serialize, Deserialize)]
struct Style {
    fg: Option<Color>,
    bg: Option<Color>,
    attr: Option<Attributes>
}

impl Default for Config {
    fn default() -> Self {
        let hl_styles: HashMap<String, Style> = vec![
            ("attribute",
            Style {
                fg: Some(Color::Blue),
                bg: None,
                attr: None,
            }),
            ("constant",
            Style {
                fg: Some(Color::DarkYellow),
                bg: None,
                attr: None,
            }),
            ("function.builtin",
            Style {
                fg: Some(Color::Blue),
                bg: None,
                attr: None,
            }),
            ("function",
            Style {
                fg: Some(Color::Blue),
                bg: None,
                attr: None,
            }),
            ("keyword",
            Style {
                fg: Some(Color::Magenta),
                bg: None,
                attr: None,
            }),
            ("operator",
            Style {
                fg: None,
                bg: None,
                attr: None,
            }),
            ("property",
            Style {
                fg: Some(Color::Blue),
                bg: None,
                attr: None,
            }),
            ("punctuation",
            Style {
                fg: None,
                bg: None,
                attr: None,
            }),
            ("punctuation.bracket",
            Style {
                fg: None,
                bg: None,
                attr: None,
            }),
            ("punctuation.delimiter",
            Style {
                fg: Some(Color::DarkYellow),
                bg: None,
                attr: None,
            }),
            ("string",
            Style {
                fg: Some(Color::Green),
                bg: None,
                attr: None,
            }),
            ("string.special",
            Style {
                fg: Some(Color::Cyan),
                bg: None,
                attr: None,
            }),
            ("tag",
            Style {
                fg: Some(Color::Yellow),
                bg: None,
                attr: None,
            }),
            ("type",
            Style {
                fg: Some(Color::Blue),
                bg: None,
                attr: None,
            }),
            ("type.builtin",
            Style {
                fg: Some(Color::Yellow),
                bg: None,
                attr: None,
            }),
            ("variable",
            Style {
                fg: None,
                bg: None,
                attr: None,
            }),
            ("variable.builtin",
            Style {
                fg: Some(Color::Grey),
                bg: None,
                attr: None,
            }),
            ("variable.parameter",
            Style {
                fg: None,
                bg: None,
                attr: None,
            }),
        ].into_iter().map(|(key, val)| (key.to_string(), val)).collect();
        Config { hl_styles }
    }
}
