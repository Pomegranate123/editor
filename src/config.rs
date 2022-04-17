use crossterm::style::{self, ContentStyle};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct HighlightStyles {
    pub types: Vec<String>,
    pub styles: Vec<ContentStyle>,
}

impl HighlightStyles {
    pub fn new(types: Vec<String>, styles: Vec<ContentStyle>) -> Self {
        Self { types, styles }
    }
}

#[derive(Clone)]
pub struct Config {
    pub line_nr_active: ContentStyle,
    pub line_nr_column: ContentStyle,
    pub hl: HighlightStyles,
}

impl Config {
    pub fn load(file: &std::path::Path) -> Result<Config, Box<dyn std::error::Error>> {
        let contents = std::fs::read_to_string(file)?;
        let conf: SerDeConfig = serde_yaml::from_str(&contents)?;
        Ok(conf.into())
    }

    pub fn write_default(file: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
        if file.exists() {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "File already exists",
            )));
        }

        let conf = SerDeConfig::default();
        let contents = serde_yaml::to_string(&conf)?;
        std::fs::create_dir_all(file.parent().unwrap())?;
        std::fs::write(file, &contents)?;
        Ok(())
    }
}

impl From<SerDeConfig> for Config {
    fn from(c: SerDeConfig) -> Self {
        Config {
            line_nr_active: c.line_nr_active.into(),
            line_nr_column: c.line_nr_column.into(),
            hl: HighlightStyles::new(c.hl.keys().cloned().collect(), c.hl.into_values().map(ContentStyle::from).collect()),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct SerDeConfig {
    line_nr_active: Style,
    line_nr_column: Style,
    hl: HashMap<String, Style>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Style {
    fg: Option<Color>,
    bg: Option<Color>,
    attr: Attributes,
}

impl Style {
    fn new() -> Self {
        Style {
            fg: None,
            bg: None,
            attr: Attributes(vec![]),
        }
    }

    fn fg(mut self, color: Color) -> Self {
        self.fg = Some(color);
        self
    }

    fn bg(mut self, color: Color) -> Self {
        self.bg = Some(color);
        self
    }

    fn attr(mut self, attr: Attribute) -> Self {
        self.attr.0.push(attr);
        self
    }
}

impl From<Style> for ContentStyle {
    fn from(s: Style) -> ContentStyle {
        ContentStyle {
            foreground_color: Some(s.fg.unwrap_or(Color::Reset).into()),
            background_color: Some(s.bg.unwrap_or(Color::Reset).into()),
            attributes: s.attr.into(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
enum Attribute {
    Bold,
    Italic,
    Underlined,
    Dim,
    Reversed,
    Hidden,
    CrossedOut,
}

impl From<Attribute> for style::Attribute {
    fn from(a: Attribute) -> style::Attribute {
        match a {
            Attribute::Bold => style::Attribute::Bold,
            Attribute::Italic => style::Attribute::Italic,
            Attribute::Underlined => style::Attribute::Underlined,
            Attribute::Dim => style::Attribute::Dim,
            Attribute::Reversed => style::Attribute::Reverse,
            Attribute::Hidden => style::Attribute::Hidden,
            Attribute::CrossedOut => style::Attribute::CrossedOut,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
struct Attributes(Vec<Attribute>);

impl From<Attributes> for style::Attributes {
    fn from(a: Attributes) -> style::Attributes {
        let mut attributes = style::Attributes::default();
        for attribute in a.0 {
            attributes.set(attribute.into())
        }
        attributes
    }
}

#[derive(Serialize, Deserialize, Clone)]
enum Color {
    Reset,
    Black,
    DarkGrey,
    Red,
    DarkRed,
    Green,
    DarkGreen,
    Yellow,
    DarkYellow,
    Blue,
    DarkBlue,
    Magenta,
    DarkMagenta,
    Cyan,
    DarkCyan,
    White,
    Grey,
    Rgb { r: u8, g: u8, b: u8 },
    AnsiValue(u8),
}

impl From<Color> for style::Color {
    fn from(c: Color) -> style::Color {
        match c {
            Color::Reset => style::Color::Reset,
            Color::Black => style::Color::Black,
            Color::DarkGrey => style::Color::DarkGrey,
            Color::Red => style::Color::Red,
            Color::DarkRed => style::Color::DarkRed,
            Color::Green => style::Color::Green,
            Color::DarkGreen => style::Color::DarkGreen,
            Color::Yellow => style::Color::Yellow,
            Color::DarkYellow => style::Color::DarkYellow,
            Color::Blue => style::Color::Blue,
            Color::DarkBlue => style::Color::DarkBlue,
            Color::Magenta => style::Color::Magenta,
            Color::DarkMagenta => style::Color::DarkMagenta,
            Color::Cyan => style::Color::Cyan,
            Color::DarkCyan => style::Color::DarkCyan,
            Color::White => style::Color::White,
            Color::Grey => style::Color::Grey,
            Color::Rgb { r, g, b } => style::Color::Rgb { r, g, b },
            Color::AnsiValue(v) => style::Color::AnsiValue(v),
        }
    }
}

impl Default for SerDeConfig {
    fn default() -> Self {
        let hl_types = vec![
            "attribute",
            "constant",
            "function.builtin",
            "function",
            "keyword",
            "property",
            "punctuation.delimiter",
            "string",
            "string.special",
            "tag",
            "type",
            "type.builtin",
            "variable.builtin",
        ]
        .into_iter()
        .map(String::from);
        let hl_styles = vec![
            Style::new().fg(Color::Blue),
            Style::new().fg(Color::DarkYellow),
            Style::new().fg(Color::Blue),
            Style::new().fg(Color::Blue),
            Style::new().fg(Color::Magenta),
            Style::new().fg(Color::Blue),
            Style::new().fg(Color::DarkYellow),
            Style::new().fg(Color::Green),
            Style::new().fg(Color::Cyan),
            Style::new().fg(Color::Yellow),
            Style::new().fg(Color::Blue),
            Style::new().fg(Color::Yellow),
            Style::new().fg(Color::Grey),
        ];
        SerDeConfig {
            line_nr_active: Style::new()
                .fg(Color::White)
                .bg(Color::Black)
                .attr(Attribute::Bold),
            line_nr_column: Style::new()
                .fg(Color::Rgb {
                    r: 80,
                    g: 80,
                    b: 80,
                })
                .bg(Color::Black),
            hl: hl_types.zip(hl_styles.into_iter()).collect(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        SerDeConfig::default().into()
    }
}
