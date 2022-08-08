use std::error::Error;
use std::io::Write;
use std::collections::BTreeMap;
use geo_types::Polygon;
use shapefile::{Reader, dbase, Shape, PolygonRing};
use serde_derive::Serialize;
use tui::{widgets::canvas::Painter, style::Color};
use std::fs;

#[derive(Debug, Serialize)]
pub struct CountryData {
    pub tag: String,
    pub name: String,
    pub name_short: String,
    pub name_long: String,
    pub adm_type: String, // for now
    pub adm_name: String, // for now. this should be adm_tag eventually
    pub economy: String, // definitely for now
    pub income_group: String, // definitely for now
    pub population: u64,
    pub gdp: i64, 
    pub continent: String,
    pub subregion: String,
}

pub struct Country {
    pub data: CountryData,
    pub shape: Vec<(f64, f64)>,
}

pub fn countries_from_shapefile(path: &str) -> Result<BTreeMap<String, Country>, Box<dyn Error>> {
    let mut countries = BTreeMap::<String, Country>::new();

    let mut reader = Reader::from_path(path)?;
    for shape_record in reader.iter_shapes_and_records() {
        let (shape, record) = shape_record?;

        let points = points_from_shape(&shape);

        let data = parse_country_data(record);
        let tag = data.tag.clone();

        let country = Country{ data, shape: points};
        countries.insert(tag, country);
    }

    /*
    let mut min_x = 0.0;
    let mut min_y = 0.0;
    let mut max_x = 0.0;
    let mut max_y = 0.0;
    for country in &countries {
        for point in &country.1.shape {
            if point.1 < min_y { min_y = point.1 }
            if point.0 < min_x { min_x = point.0 }
            if point.1 > max_y { max_y = point.1 }
            if point.0 > max_x { max_x = point.0 }
        }
    }

    println!("For all countries:");
    println!("    |  X  |  Y  |");
    println!("MAX |{:5}|{:5}|", max_x, max_y);
    println!("MIN |{:5}|{:5}|", min_x, min_y);
    */

    Ok(countries)
}

macro_rules! get_entry {
    ($attr:literal, $record:ident) => {
        if let Some(dbase::FieldValue::Character(Some(entry))) = $record.get($attr) {
            entry
        } else {
            //eprintln!("Failed to read attribute {}.", $attr);
            "unknown"
        }
    };
}

fn points_from_shape(shape: &Shape) -> Vec<(f64, f64)> {
    let mut points = Vec::<(f64, f64)>::new();
    let polygon = if let Shape::Polygon(p) = shape { p } else { panic!("non polygon") };
    let rings = polygon.rings();
    for ring in rings {
        let ringvec = match ring {
            PolygonRing::Outer(rv) => rv,
            PolygonRing::Inner(rv) => rv,
        }.iter().map(|vp| { (vp.x, vp.y) });
        points.extend(ringvec);
    }
    points
}

fn parse_country_data(record: dbase::Record) -> CountryData {
        let tag = get_entry!("GU_A3", record).to_owned();
        let name = get_entry!("GEOUNIT", record).to_owned();
        let name_short = get_entry!("ABBREV", record).to_owned();
        let mut name_long = get_entry!("FORMAL_EN", record).to_owned();
        if name_long == "unknown" { 
            //eprintln!("Substituting for {}", name);
            name_long = name.clone() 
        }
        let adm_type = get_entry!("TYPE", record).to_owned();
        let adm_name = get_entry!("SOVEREIGNT", record).to_owned();
        let economy = get_entry!("ECONOMY", record).to_owned();
        let income_group = get_entry!("INCOME_GRP", record).to_owned();
        let subregion = get_entry!("SUBREGION", record).to_owned();
        let continent = get_entry!("CONTINENT", record).to_owned();

        let population = if let Some(dbase::FieldValue::Numeric(Some(entry))) = record.get("POP_EST") {
            *entry as u64
        } else {
            eprintln!("Failed to read attribute POP_EST for {}", name);
            0
        };

        let gdp = if let Some(dbase::FieldValue::Numeric(Some(entry))) = record.get("GDP_MD") {
            (*entry as i64) * 1_000_000
        } else {
            eprintln!("Failed to read attribute GDP_MD for {}", name);
            0
        };

        CountryData {
            tag,
            name,
            name_short,
            name_long,
            adm_type,
            adm_name,
            economy,
            income_group,
            population,
            gdp,
            subregion,
            continent,
        }
}

pub struct StyleCountry<'a> {
    pub country: &'a Country,
    pub style: Color,
}

impl tui::widgets::canvas::Shape for Country {
    fn draw(&self, painter: &mut Painter) {
        for (x, y) in &self.shape {
            if let Some((x, y)) = painter.get_point(*x, *y) {
                painter.paint(x, y, Color::Reset);
            }
        }
    }
}

impl tui::widgets::canvas::Shape for StyleCountry <'_> {
    fn draw(&self, painter: &mut Painter) {
        for (x, y) in &self.country.shape {
            if let Some((x, y)) = painter.get_point(*x, *y) {
                painter.paint(x, y, self.style);
            }
        }
    }
}
