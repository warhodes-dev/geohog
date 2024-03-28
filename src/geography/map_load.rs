use std::error::Error;
use std::io::Write;
use std::collections::BTreeMap;
use geo_types::Polygon;
use shapefile::{Reader, dbase, Shape, PolygonRing};
use serde_derive::Serialize;
use ratatui::{widgets::canvas::Painter, style::Color};
use std::fs;
use anyhow::Result;
use itertools::izip;

/// A collection of country data, both quantitative and geospatial
pub struct Country {
    pub data: CountryData,
    pub style: CountryStyle,
    pub rings: RingLOD,
}

/// Demographic, political, and administrative data about a country
pub struct CountryData {
    pub tag: String,
    pub name: String,
    pub name_short: String,
    pub name_long: String,
    pub adm_type: String,
    pub adm_name: String,
    pub economy: String,
    pub income_group: String,
    pub population: u64,
    pub gdp: i64,
    pub continent: String,
    pub subregion: String,
}

/// Style suggestions regarding labeling and rendering a country
pub struct CountryStyle {
    pub label: String,
    pub label_x: f64, 
    pub label_y: f64,
    //pub color: ?
}

/// Contains 3 levels of detail of country border/'ring' data
// 
// A 'ring' in this context is a full loop of points that comprise a
// geographic boundary, such as the official outer borders of a country,
// enclave/exclave borders, or borders surrounding a natural boundary
// such as islands (exterior boundary) or lakes (interior boundary).
pub struct RingLOD {
    pub low: Option<Vec<Ring>>,
    pub med: Option<Vec<Ring>>,
    pub high: Vec<Ring>,
}

/// A full loop of points that comprise a geographic boundary
pub type Ring = Vec<(f64, f64)>;

pub fn countries_from_shapefile(
    low_res_path: &str,
    med_res_path: &str,
    high_res_path: &str,
) -> Result<BTreeMap<String, Country>> {
    let mut countries = BTreeMap::<String, Country>::new();

    let mut high_res_reader = shapefile::Reader::from_path(high_res_path)?;
    let mut med_res_reader = shapefile::Reader::from_path(med_res_path)?;
    let mut low_res_reader = shapefile::Reader::from_path(low_res_path)?;
    
    let high_res_record = high_res_reader.iter_shapes_and_records();
    let med_res_record = med_res_reader.iter_shapes_and_records();
    let low_res_record = low_res_reader.iter_shapes_and_records();

    let records = izip!(high_res_record, med_res_record, low_res_record);

    for (high_record, med_record, low_record) in records {
        // Use high resolution record as the base
        let (high_shape_data, record) = high_record?;
        let (med_shape_data, _) = med_record?;
        let (low_shape_data, _) = low_record?;

        let (data, style) = parse_country_data(record);
        let tag = data.tag.clone();

        let rings = RingLOD {
            low: Some(collect_rings(&low_shape_data)),
            med: Some(collect_rings(&med_shape_data)),
            high: collect_rings(&high_shape_data),
        };

        let country = Country{ data, rings, style };
        countries.insert(tag, country);
    }

    Ok(countries)
}

macro_rules! get_char_entry {
    ($attr:literal, $record:ident) => {
        if let Some(dbase::FieldValue::Character(Some(entry))) = $record.get($attr) {
            entry.to_owned()
        } else {
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

/// Collect a shape record into a more managable Vector of Rings
fn collect_rings(shape: &Shape) -> Vec<Ring> {
    let mut simple_rings = Vec::new();
    let polygonal_shape = match shape {
        Shape::Polygon(p) => p,
        _ => panic!("non polygon"),
    };
    let shape_rings = polygonal_shape.rings();
    for shape_ring in shape_rings {
        let simple_ring = match shape_ring {
            PolygonRing::Outer(rv) => rv,
            PolygonRing::Inner(rv) => rv,
        }.iter().map(|vp| { (vp.x, vp.y) }).collect();
        simple_rings.push(simple_ring);
    }
    simple_rings
}

fn parse_country_data(record: dbase::Record) -> (CountryData, CountryStyle) {
    let tag = get_char_entry!("GU_A3", record);
    let name = get_char_entry!("GEOUNIT", record);
    let name_short = get_char_entry!("ABBREV", record);
    let name_long = get_char_entry!("FORMAL_EN", record);
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