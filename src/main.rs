use anyhow::Result;

pub mod ui {
    use std::io::{self, Stdout};

    use anyhow::Result;
    use crossterm::{event::{DisableMouseCapture, EnableMouseCapture}, terminal::{self, EnterAlternateScreen, LeaveAlternateScreen}};
    use ratatui::{backend::{Backend, CrosstermBackend}, Terminal};

    #[derive(Debug)]
    pub struct Tui {
        terminal: Terminal<CrosstermBackend<Stdout>>,
    }

    impl Tui {
        pub fn initialize() -> Result<Self> {
            terminal::enable_raw_mode()?;
            let mut stdout = io::stdout();
            crossterm::execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
            let backend = CrosstermBackend::new(stdout);
            let terminal = Terminal::new(backend)?;

            Ok(Tui {
                terminal,
            })
        }

        fn restore_terminal() {
            terminal::disable_raw_mode()
                .expect("Failed to disable raw mode");
            let mut stdout = io::stdout();
            crossterm::execute!(stdout, LeaveAlternateScreen, DisableMouseCapture)
                .expect("Failed to disable terminal features");
            println!("Terminal restored.")
        }


    }

    impl Drop for Tui {
        fn drop(&mut self) {
            Tui::restore_terminal();
        }
    }
}

use clap::Parser;
use geohog::config::Config;
use ui::Tui;
use geohog::log;

fn main() -> Result<()> {
    let config = Config::parse();
    log::setup_trace(&config);

    let tui = Tui::initialize()?;

    Ok(())
}