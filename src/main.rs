#![allow(unused_imports)]

use std::{error::Error, collections::BTreeMap};
use map_view::map_load::{countries_from_shapefile, Country, StyleCountry};

/* Check these later */
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{
    io,
    time::{Duration, Instant},
};
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::Span,
    widgets::{
        canvas::{Canvas, Map, MapResolution, Rectangle, Shape, Painter},
        Block, Borders,
    },
    Frame, Terminal,
};
/* ================= */

const VIEWS: [([f64; 2], [f64; 2]); 2] = [
    ([-181.0, 181.0], [-91.0, 91.0]),
    ([-140.0, -50.0], [0.0, 50.0]),
];

struct App {
    countries: BTreeMap<String, Country>,
    selected: String,
    viewport: usize,
}

impl App {
    fn new(countries: BTreeMap<String, Country>) -> Self {
        App { 
            countries, 
            selected: "USA".to_string(),
            viewport: 0,
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let countries = countries_from_shapefile("ne_110m_admin_0_countries/ne_110m_admin_0_countries.shp")?;

    // set up terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let app = App::new(countries);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let tick_rate = Duration::from_millis(250);
    let app_return = run_app(&mut terminal, app, tick_rate);

    // restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    if let Err(err) = app_return {
        eprintln!("Error: {:?}", err);
    }

    Ok(())
}

fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    mut app: App,
    tick_rate: Duration,
) -> io::Result<()> {
    let mut last_tick = Instant::now();
    loop {
        terminal.draw(|f| ui(f, &app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                #[allow(clippy::single_match)]
                match key.code {
                    KeyCode::Char('q') => {
                        return Ok(());
                    },
                    KeyCode::Char('w') => {
                        app.viewport = 0;
                    },
                    KeyCode::Char('a') => {
                        app.viewport = 1;
                    }
                    _ => {}
                }
            }
        }
        if last_tick.elapsed() >= tick_rate {
            //app.on_tick();
            last_tick = Instant::now();
        }

    }
}

macro_rules! line {
    ($x1:expr, $y1:expr, $x2:expr, $y2:expr, $color:expr) => {
        Line {
            x1: $x1,
            y1: $y1,
            x2: $x2,
            y2: $y2,
            color: $color,
        }
    }
}

fn ui<B: Backend>(f: &mut Frame<B>, app: &App) {
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(100)].as_ref())
        .split(f.size());

    let canvas = Canvas::default()
        .block(Block::default().borders(Borders::ALL).title("World"))
        .paint(|ctx| {
            /*
            let country = app.countries.get(&app.selected).unwrap();
            ctx.draw(country);
            */
            for country in &app.countries {
                if country.1.data.subregion == "Western Europe" {
                    let tmp_country = StyleCountry {
                        country: country.1,
                        style: Color::Blue,
                    };
                    ctx.draw(&tmp_country);
                } else {
                    ctx.draw(country.1);
                }
            }
            /*
            let map = Map { 
                resolution: MapResolution::High,
                color: Color::Reset,
            };
            ctx.draw(&map);
            */

            let usa = app.countries.get("USA").unwrap();
            let usa_style = StyleCountry {
                country: usa,
                style: Color::Yellow,
            };
            ctx.draw(&usa_style);

            let rus = app.countries.get("RUS").unwrap();
            let rus_style = StyleCountry {
                country: rus,
                style: Color::Red,
            };
            ctx.draw(&rus_style);
        })
        .x_bounds(VIEWS[app.viewport].0)
        .y_bounds(VIEWS[app.viewport].1);
    f.render_widget(canvas, panes[0]);
}

