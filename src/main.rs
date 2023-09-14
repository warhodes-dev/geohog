#![allow(unused_imports)]

use std::{error::Error, collections::BTreeMap};
use map_view::map_load::{countries_from_shapefile, Country };
use itertools::*;

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
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::Span,
    widgets::{
        canvas::{Canvas, Map, MapResolution, Rectangle, Shape, Painter, Line},
        Block, Borders,
    },
    Frame, Terminal,
};
/* ================= */

struct ViewPort {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl ViewPort {
    fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        ViewPort { x, y, width, height }
    }

    fn zoom_in(&mut self) {
        self.x += 2.0;
        self.y += 1.0;
        self.width -= 4.0;
        self.height -= 2.0;
    }

    fn zoom_out(&mut self) {
        self.x -= 2.0;
        self.y -= 1.0;
        self.width += 4.0;
        self.height += 2.0;
    }

    fn move_up(&mut self) {
        self.y += 2.0;
    }

    fn move_down(&mut self) {
        self.y -= 2.0;
    }

    fn move_west(&mut self) {
        self.x -= 2.0;
    }

    fn move_east(&mut self) {
        self.x += 2.0;
    }

    fn x_bounds(&self) -> [f64; 2] {
        [self.x, self.x + self.width]
    }

    fn y_bounds(&self) -> [f64; 2] {
        [self.y, self.y + self.height]
    }
}

struct App {
    countries: [BTreeMap<String, Country>; 3],
    selected: String,
    viewport: ViewPort,
}

impl App {
    fn new(countries: [BTreeMap<String, Country>; 3]) -> Self {
        App { 
            countries, 
            selected: "USA".to_string(),
            viewport: ViewPort {
                x: -180.0,
                y: -90.0,
                width: 360.0,
                height: 180.0,
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let low = countries_from_shapefile("ne_110m_admin_0_countries/ne_110m_admin_0_countries.shp")?;
    let med = countries_from_shapefile("ne_50m_admin_0_countries/ne_50m_admin_0_countries.shp")?;
    let high = countries_from_shapefile("ne_10m_admin_0_countries/ne_10m_admin_0_countries.shp")?;

    let countries = [low, med, high];

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
                    KeyCode::Esc => {
                        return Ok(());
                    },
                    KeyCode::Char('q') => {
                        app.viewport.zoom_out();
                    },
                    KeyCode::Char('e') => {
                        app.viewport.zoom_in();
                    },
                    KeyCode::Char('w') => {
                        app.viewport.move_up();
                    },
                    KeyCode::Char('a') => {
                        app.viewport.move_west();
                    },
                    KeyCode::Char('s') => {
                        app.viewport.move_down();
                    },
                    KeyCode::Char('d') => {
                        app.viewport.move_east();
                    },
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
    ($x1:expr, $y1:expr, $x2:expr, $y2:expr, $color:expr $(,)?) => {
        &Line { x1: $x1, y1: $y1, x2: $x2, y2: $y2, color: $color }
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

            let countries = match app.viewport.width {
                m if (70.0..=360.0).contains(&m) => &app.countries[0],
                m if (15.0..70.0).contains(&m) => &app.countries[1],
                m if (0.0..15.0).contains(&m) => &app.countries[2],
                _ => &app.countries[0],
            };

            // Paint base map
            for country in countries.values() {
                for shape in &country.shapes {
                    let point_pairs = shape.iter().tuple_windows();

                    for (&(x1, y1), &(x2, y2)) in point_pairs {
                        let segment = line!( 
                            x1, y1,
                            x2, y2,
                            Color::Reset,
                        );
                        ctx.draw(segment);
                    }
                }
            }

            // Paint geolocated lines

            // Paint additional countries
            /*
            let paint_queue = vec!["USA", "FRA", "BRA", "RUS", "CHN", "NGA"];

            for tag in paint_queue {
                if let Some(country) = &countries.get(tag) {
                    for shape in &country.shapes {
                        let point_pairs = shape.iter().tuple_windows();

                        let color = match tag {
                            "USA" => Color::Cyan,
                            "FRA" => Color::Blue,
                            "BRA" => Color::Green,
                            "RUS" => Color::Red,
                            "CHN" => Color::LightRed,
                            "NGA" => Color::Yellow,
                            _ => Color::Reset,
                        };

                        for (&(x1, y1), &(x2, y2)) in point_pairs {
                            let segment = line!( 
                                x1, y1,
                                x2, y2,
                                color,
                            );
                            ctx.draw(segment);
                        }
                    }

                    let style = &country.style;
                    ctx.print(
                        style.label_x - (style.label.len() as f64) / 2.0,
                        style.label_y,
                        style.label.to_owned(),
                    );

                }
            }
            */
        
            
        })
        .x_bounds(app.viewport.x_bounds())
        .y_bounds(app.viewport.y_bounds());
    f.render_widget(canvas, panes[0]);
}

