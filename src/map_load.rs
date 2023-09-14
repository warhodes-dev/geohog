use std::error::Error;
use std::io::Write;
use std::collections::BTreeMap;
use geo_types::Polygon;
use shapefile::{Reader, dbase, Shape, PolygonRing};
use serde_derive::Serialize;
use tui::{widgets::canvas::Painter, style::Color};
use std::fs;

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

pub struct CountryStyle {
    pub label: String,
    pub label_x: f64, 
    pub label_y: f64,
    //pub color: ?
}

pub struct Country {
    pub data: CountryData,
    pub style: CountryStyle,
    pub shapes: Vec<Vec<(f64, f64)>>,
}

pub fn countries_from_shapefile(path: &str) -> Result<BTreeMap<String, Country>, Box<dyn Error>> {
    let mut countries = BTreeMap::<String, Country>::new();

    let mut reader = Reader::from_path(path)?;
    for shape_record in reader.iter_shapes_and_records() {
        let (shape, record) = shape_record?;

        let shapes = collect_shapes(&shape);

        let (data, style) = parse_country_data(record);
        let tag = data.tag.clone();

        let country = Country{ data, shapes, style };
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

macro_rules! get_char_entry {
    ($attr:literal, $record:ident) => {
        if let Some(dbase::FieldValue::Character(Some(entry))) = $record.get($attr) {
            entry.to_owned()
        } else {
            //eprintln!("Failed to read attribute {}.", $attr);
            "unknown".to_owned()
        }
    };
}

macro_rules! get_numeric_entry {
    ($attr:literal, $record:ident) => {
        if let Some(dbase::FieldValue::Numeric(Some(entry))) = $record.get($attr) {
            *entry
        } else {
            0.0f64
        }
    };
}

fn collect_shapes(shape: &Shape) -> Vec<Vec<(f64, f64)>> {
    let mut shapes = Vec::new();
    let polygon = if let Shape::Polygon(p) = shape { p } else { panic!("non polygon") };
    let rings = polygon.rings();
    for ring in rings {
        let ringvec = match ring {
            PolygonRing::Outer(rv) => rv,
            PolygonRing::Inner(rv) => rv,
        }.iter().map(|vp| { (vp.x, vp.y) }).collect();
        shapes.push(ringvec);
    }
    shapes
}

fn parse_country_data(record: dbase::Record) -> (CountryData, CountryStyle) {
    let tag = get_char_entry!("GU_A3", record);
    let name = get_char_entry!("GEOUNIT", record);
    let name_short = get_char_entry!("ABBREV", record);
    let mut name_long = get_char_entry!("FORMAL_EN", record);
    if name_long == "unknown" { 
        //eprintln!("Substituting for {}", name);
        name_long = name.clone() 
    }
    let adm_type = get_char_entry!("TYPE", record);
    let adm_name = get_char_entry!("SOVEREIGNT", record);
    let economy = get_char_entry!("ECONOMY", record);
    let income_group = get_char_entry!("INCOME_GRP", record);
    let subregion = get_char_entry!("SUBREGION", record);
    let continent = get_char_entry!("CONTINENT", record);

    let population = get_numeric_entry!("POP_EST", record) as u64;
    let gdp = get_numeric_entry!("GDP_MD", record) as i64;

    let label_x = get_numeric_entry!("LABEL_X", record);
    let label_y = get_numeric_entry!("LABEL_Y", record);
    let label = name.to_owned();

    ( CountryData {
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
      }, 
      CountryStyle {
        label,
        label_x,
        label_y,
      }
    )
}

/*
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
*/
