use crate::{buffer::Buffer, config::Config};
use crossterm::{
    cursor::{MoveTo, SavePosition},
    event,
    event::{EnableMouseCapture, Event, KeyEvent},
    execute,
    terminal::{self, DisableLineWrap, EnterAlternateScreen},
    Result,
};
use std::{
    env,
    io::{self, Write},
    path::PathBuf,
};

mod buffer;
mod command;
mod config;

fn main() {
    let config_path = PathBuf::from(match env::var("XDG_CONFIG_HOME") {
        Ok(config_path) => config_path + "/editor/config.yml",
        Err(_) => String::from("./config.yml"),
    });
    let config = match Config::load(&config_path) {
        Ok(conf) => conf,
        Err(e) => {
            println!(
                "Error: {} while trying to load config from path: {:?}",
                e, &config_path
            );
            Config::write_default(&config_path).unwrap();
            Config::default()
        }
    };

    run(&mut io::stdout(), config).unwrap();
}

struct CleanUp;

impl Drop for CleanUp {
    fn drop(&mut self) {
        terminal::disable_raw_mode().expect("Unable to disable raw mode");
    }
}

fn run<W: Write>(w: &mut W, config: Config) -> Result<()> {
    let _cleanup = CleanUp;
    let path = env::args().nth(1).expect("No file argument given!").into();
    let mut editor = Editor::new(path, config);

    terminal::enable_raw_mode()?;
    execute!(
        w,
        SavePosition,
        MoveTo(editor.buffer().line_nr_cols as u16, 0),
        EnterAlternateScreen,
        EnableMouseCapture,
        DisableLineWrap,
    )?;

    editor.draw(w)?;
    loop {
        match event::read()? {
            Event::Resize(width, height) => {
                editor.update_size(width as usize, height as usize);
                editor.draw(w)?;
            }
            Event::Key(event) => editor.handle_keyevent(w, event)?,
            _ => (),
        }
    }
}

struct Editor {
    buffers: Vec<Buffer>,
    config: Config,
    current_buffer: usize,
    width: usize,
    height: usize,
}

impl Editor {
    pub fn new(path: PathBuf, config: Config) -> Self {
        let (width, height) = terminal::size().unwrap();
        Editor {
            buffers: vec![Buffer::new(path, config.clone())],
            config,
            current_buffer: 0,
            width: width as usize,
            height: height as usize,
        }
    }

    pub fn update_size(&mut self, width: usize, height: usize) {
        self.buffer_mut().update_size(width, height);
        self.width = width;
        self.height = height;
    }

    pub fn draw<W: Write>(&mut self, w: &mut W) -> Result<()> {
        self.buffer_mut().draw_all(w)
    }

    pub fn buffer(&self) -> &Buffer {
        self.buffers
            .get(self.current_buffer)
            .expect("Buffer index was out of range for editor")
    }

    pub fn buffer_mut(&mut self) -> &mut Buffer {
        self.buffers
            .get_mut(self.current_buffer)
            .expect("Buffer index was out of range for editor")
    }

    pub fn handle_keyevent<W: Write>(&mut self, w: &mut W, key_event: KeyEvent) -> Result<()> {
        self.buffer_mut().handle_keyevent(w, key_event)
    }
}
