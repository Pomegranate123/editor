use crate::{config::Config, render::BufferRenderer};
use crossterm::{
    cursor::{RestorePosition, SavePosition},
    event,
    event::{DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{self, DisableLineWrap, EnableLineWrap, EnterAlternateScreen, LeaveAlternateScreen},
    Result,
};
use std::{
    env,
    io::{self, Write},
    path::PathBuf,
    process,
};

mod action;
mod buffer;
mod config;
mod input;
mod rect;
mod render;
mod utils;

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

    let _cleanup = CleanUp;
    let path = env::args().nth(1).expect("No file argument given!").into();

    let mut editor = Editor::new(path, config);
    editor.run(&mut io::stdout()).unwrap();
}

struct CleanUp;

impl Drop for CleanUp {
    fn drop(&mut self) {
        terminal::disable_raw_mode().expect("Unable to disable raw mode");
    }
}

struct Editor {
    buffers: Vec<BufferRenderer>,
    _config: Config,
    current_buffer: usize,
    width: u16,
    height: u16,
}

impl Editor {
    pub fn new(path: PathBuf, config: Config) -> Self {
        let (width, height) = terminal::size().unwrap();
        Editor {
            buffers: vec![BufferRenderer::new(path, config.clone())],
            _config: config,
            current_buffer: 0,
            width,
            height,
        }
    }

    pub fn run<W: Write>(&mut self, w: &mut W) -> Result<()> {
        terminal::enable_raw_mode()?;
        execute!(
            w,
            SavePosition,
            EnterAlternateScreen,
            EnableMouseCapture,
            DisableLineWrap,
        )?;
        self.buffer_mut().draw_all(w)?;
        loop {
            let input = event::read()?;
            if let Event::Key(event) = input {
                if event.code == KeyCode::Char('c') && event.modifiers == KeyModifiers::CONTROL {
                    self.quit(w)?;
                }
            }
            self.handle_input(w, input)?;
        }
    }

    pub fn update_size(&mut self, width: u16, height: u16) {
        self.buffer_mut().update_size(width, height);
        self.width = width;
        self.height = height;
    }

    pub fn buffer_mut(&mut self) -> &mut BufferRenderer {
        self.buffers
            .get_mut(self.current_buffer)
            .expect("BufferRenderer index was out of range for editor")
    }

    pub fn handle_input<W: Write>(&mut self, w: &mut W, event: Event) -> Result<()> {
        match event {
            Event::Resize(width, height) => {
                self.update_size(width, height);
                self.buffer_mut().draw_all(w)?;
            }
            Event::Key(event) => self.buffer_mut().handle_keyevent(w, event)?,
            Event::Mouse(_event) => (),
        }
        Ok(())
    }

    #[allow(unused)]
    /// Cleans up and quits the application
    fn quit<W: Write>(&mut self, w: &mut W) -> Result<()> {
        execute!(
            w,
            DisableMouseCapture,
            LeaveAlternateScreen,
            RestorePosition,
            EnableLineWrap,
        )?;
        terminal::disable_raw_mode()?;
        process::exit(0);
    }
}
