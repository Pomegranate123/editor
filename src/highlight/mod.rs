use crate::{
    buffer::Buffer,
    config::HighlightStyles,
    highlight::language::Language,
};
use crossterm::style::ContentStyle;
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlight};

pub mod language;

pub struct Highlighter {
    hl: tree_sitter_highlight::Highlighter,
    conf: Option<HighlightConfiguration>,
    style: HighlightStyles,
    cache: Option<Vec<HighlightEvent>>,
}

impl Highlighter {
    pub fn new(lang: Option<Language>, style: HighlightStyles) -> Self {
        Self {
            hl: tree_sitter_highlight::Highlighter::new(),
            conf: lang.map(|l| {
                let mut hl_conf: HighlightConfiguration = l.into();
                hl_conf.configure(&style.types);
                hl_conf
            }),
            style,
            cache: None,
        }
    }

    pub fn has_hl(&self) -> bool {
        self.cache.is_some()
    }

    pub fn get_hl(&self) -> &[HighlightEvent] {
        self.cache.as_ref().unwrap()
    }

    pub fn update_hl(&mut self, buf: &Buffer) {
        match &self.conf {
            None => self.cache = Some(vec![HighlightEvent::Source {start: 0, end: buf.text.len_bytes() - 1}]),
            Some(conf) => {
                self.cache = Some(
                    self.hl.highlight(
                        &conf,
                        &buf.text.bytes().collect::<Vec<u8>>(),
                        None,
                        |_| None,
                    )
                    .unwrap()
                    .map(|event| event.unwrap())
                    .collect()
                );
            }
        }
    }

    pub fn get_style(&self, hl_type: &Highlight) -> &ContentStyle {
        self.style.styles.get(hl_type.0).expect("Style index out of bounds for HighlightStyles instance. Perhaps the amount of types does not match the amount of styles.")
    }
}

