use anyhow::Result;

pub mod ui {
    use std::io::{self, Stdout};

    use anyhow::Result;
    use crossterm::{
        event::{DisableMouseCapture, EnableMouseCapture}, 
        terminal::{self, EnterAlternateScreen, LeaveAlternateScreen}
    };
    use ratatui::{
        backend::CrosstermBackend, prelude::*, widgets::{Block, Borders, Paragraph, Widget} 
    };

    #[derive(Debug)]
    pub struct Tui {
        terminal: Terminal<CrosstermBackend<Stdout>>,
        app: App,
    }

    impl Tui {
        pub fn initialize() -> Result<Self> {
            terminal::enable_raw_mode()?;
            let mut stdout = io::stdout();
            crossterm::execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
            let backend = CrosstermBackend::new(stdout);
            let terminal = Terminal::new(backend)?;

            let app = App::new();

            Ok(Tui {terminal, app})
        }

        pub fn restore_terminal() {
            terminal::disable_raw_mode()
                .expect("Failed to disable raw mode");
            let mut stdout = io::stdout();
            crossterm::execute!(stdout, LeaveAlternateScreen, DisableMouseCapture)
                .expect("Failed to disable terminal features");
            println!("Terminal restored.")
        }

        pub fn run(&mut self) -> Result<()> {
            loop {
                self.draw()?;

                //check for keypresses
            }
        }

        fn draw(&mut self) -> Result<()> {
            let terminal = &mut self.terminal;
            let app = &mut self.app;
            terminal.draw(|f| f.render_widget(app, f.size()))?;
            Ok(())
        }
    }
    

    impl Drop for Tui {
        fn drop(&mut self) {
            Tui::restore_terminal();
        }
    }

    #[derive(Debug)]
    struct App;

    impl App {
        fn new() -> Self {
            App
        }

        fn render_header(&self, area: Rect, buf: &mut Buffer) {
            Paragraph::new("Geohog")
                .bold()
                .centered()
                .render(area, buf);
        }

        fn render_list(&self, area: Rect, buf: &mut Buffer) {
            let outer_block = Block::new()
                .borders(Borders::NONE)
                .title_alignment(Alignment::Left)
                .title("Connections");
            let inner_block = Block::new()
                .borders(Borders::NONE);

            let outer_area = area;
            let inner_area = outer_block.inner(outer_area);

            outer_block.render(outer_area, buf);
        }   

        fn render_info(&self, area: Rect, buf: &mut Buffer) {

        }
    }

    impl Widget for &mut App {
        fn render(self, area: Rect, buf: &mut Buffer)
        where Self: Sized {
            let vertical = Layout::vertical([
                Constraint::Length(2),
                Constraint::Min(0),
            ]);
            let [header_area, main_area] = vertical.areas(area);
            
            let horizontal = Layout::horizontal([
                Constraint::Percentage(70),
                Constraint::Percentage(30)
            ]);
            let [list_area, info_area] = horizontal.areas(main_area);
            self.render_header(header_area, buf);
            self.render_list(list_area, buf);
            self.render_info(info_area, buf);
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