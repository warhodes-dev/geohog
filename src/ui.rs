use std::io::{self, Stdout};

use anyhow::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture}, 
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen}
};
use ratatui::{
    prelude::*,
    backend::CrosstermBackend, 
    widgets::{Block, Borders, Paragraph, Widget}
};

use crate::net::NetClient;

pub mod app;

pub struct Tui {
    pub terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl Tui {
    pub fn initialize() -> Result<Self> {
        terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        crossterm::execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        Ok(Tui{terminal})
    }

    pub fn restore_terminal() {
        terminal::disable_raw_mode()
            .expect("Disable raw mode");
        let mut stdout = io::stdout();
        crossterm::execute!(stdout, LeaveAlternateScreen, DisableMouseCapture)
            .expect("Disable terminal features");
        println!("Terminal restored.")
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        Tui::restore_terminal();
    }
}