#![allow(unused_imports)]

use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::{error::Error, collections::BTreeMap};
use geohog::map_load::{countries_from_shapefile, Country };
use geohog::net;
use itertools::*;

use crossterm::{event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},execute,terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},};
use std::{io,time::{Duration, Instant},};
use ratatui::{backend::{Backend, CrosstermBackend},layout::{Constraint, Direction, Layout, Rect},style::{Color, Style},text::Span,widgets::{canvas::{Canvas, Map, MapResolution, Rectangle, Shape, Painter, Line},Block, Borders,},Frame, Terminal,};
use ipgeolocate::{Locator, Service};

#[derive(Debug)]
struct GeoLocation {
    ip: String,
    lat: f64,
    long: f64,
}

struct App {
    countries: [BTreeMap<String, Country>; 3],
    viewport: ViewPort,
    host: Arc<Mutex<Option<GeoLocation>>>, 
    endpoints: Arc<Mutex<Vec<GeoLocation>>>,
}

impl App {
    fn new(countries: [BTreeMap<String, Country>; 3]) -> Self {

        let endpoints = Arc::new(Mutex::new(Vec::new()));
        let host = Arc::new(Mutex::new(None));

        App { 
            countries, 
            viewport: ViewPort {
                x: -180.0,
                y: -90.0,
                width: 360.0,
                height: 180.0,
            },
            host,
            endpoints,
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let low = countries_from_shapefile("assets/ne_110m_admin_0_countries/ne_110m_admin_0_countries.shp")?;
    let med = countries_from_shapefile("assets/ne_50m_admin_0_countries/ne_50m_admin_0_countries.shp")?;
    let high = countries_from_shapefile("assets/ne_10m_admin_0_countries/ne_10m_admin_0_countries.shp")?;

    let countries = [low, med, high];

    // set up terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let app = App::new(countries);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let event_tick_rate = Duration::from_millis(250);
    let ping_tick_rate = Duration::from_secs(10);
    let app_return = run_app(&mut terminal, app, event_tick_rate, ping_tick_rate);

    // restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;


    Ok(())
}

fn run_app <B: Backend>(
    terminal: &mut Terminal<B>,
    mut app: App,
    event_tick_rate: Duration,
    ping_tick_rate: Duration,
) -> Result<(), Box<dyn std::error::Error>> {
    let start_tick = Instant::now();
    let mut last_event_tick = start_tick;
    let mut last_ping_tick = start_tick - Duration::from_secs(9);
    loop {
        terminal.draw(|f| ui(f, &app))?;

        let timeout = event_tick_rate
            .checked_sub(last_event_tick.elapsed())
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

        if last_event_tick.elapsed() >= event_tick_rate {
            //app.on_tick();
            last_event_tick = Instant::now();
        }

        if last_ping_tick.elapsed() >= ping_tick_rate {

            let host = app.host.clone();
            if host.lock().unwrap().is_none() {
                tokio::spawn(geolocate_host(host));
            }

            let endpoints = app.endpoints.clone();
            tokio::spawn(geolocate_endpoints(endpoints));

            last_ping_tick = Instant::now();
        }

    }
}

macro_rules! line {
    ($x1:expr, $y1:expr, $x2:expr, $y2:expr, $color:expr $(,)?) => {
        &Line { x1: $x1, y1: $y1, x2: $x2, y2: $y2, color: $color }
    }
}

fn ui(f: &mut Frame, app: &App) {
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
            let host_lock = app.host.lock().unwrap();
            if let Some(host) = host_lock.as_ref() {
                for endpoint in app.endpoints.lock().unwrap().iter() {
                    let line = line!(
                        host.long, host.lat,
                        endpoint.long, endpoint.lat,
                        Color::Yellow,
                    );
                    ctx.draw(line);
                }
            }
            std::mem::drop(host_lock);

            // Paint additional countries
            let paint_queue = vec!["USA", "FRA", "BRA", "RUS", "CHN", "NGA", "GMB"];

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
                            "GMB" => Color::LightGreen,
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
        
            
        })
        .x_bounds(app.viewport.x_bounds())
        .y_bounds(app.viewport.y_bounds());
    f.render_widget(canvas, panes[0]);
}

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

async fn geolocate_endpoints(
    endpoint_locations: Arc<Mutex<Vec<GeoLocation>>>,
) -> Result<(), String> {
    let service = Service::IpApi;
    let connections = net::get_tcp().map_err(|e| e.to_string())?;

    let mut new_endpoint_locations = Vec::new();

    for connection in connections {
        let geolocate_response = Locator::get(&connection.remote_address, service).await
            .ok()
            .map(|response| {
                GeoLocation {
                    ip: connection.remote_address.to_owned(),
                    lat: response.latitude.parse::<f64>().unwrap(),
                    long: response.longitude.parse::<f64>().unwrap(),
                }
            });

        if let Some(location) = geolocate_response {
            new_endpoint_locations.push(location);
        }
    }

    *endpoint_locations.lock().unwrap() = new_endpoint_locations;

    Ok(())
}

async fn geolocate_host(host_location: Arc<Mutex<Option<GeoLocation>>>) -> Result<(), String> {
    let service = Service::IpApi;

    let ip_raw = match public_ip::addr().await.unwrap() {
        IpAddr::V4(ipaddr) => ipaddr,
        IpAddr::V6(_) => { return Err("IPV6 not supported".to_owned())}
    };

    let ip = ip_raw.octets()
        .map(|byte| byte.to_string())
        .iter()
        .map(|s| s.as_ref())
        .intersperse(".")
        .collect::<String>();

    let geolocate = Locator::get(&ip, service).await
        .ok()
        .map(|response| {
            GeoLocation {
                ip: ip.to_owned(),
                lat: response.latitude.parse::<f64>().unwrap(),
                long: response.longitude.parse::<f64>().unwrap(),
            }
        });

    *host_location.lock().unwrap() = geolocate;
    Ok(())
}