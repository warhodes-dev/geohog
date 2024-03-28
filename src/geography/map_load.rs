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

pub struct CountryStyle {
    pub label: String,
    pub label_x: f64, 
    pub label_y: f64,
    //pub color: ?
}

pub struct Country {
    pub data: CountryData,
    pub style: CountryStyle,
    pub shape_data: ShapeData,
}

pub struct ShapeData {
    pub low: ShapeSet,
    pub med: ShapeSet,
    pub high: ShapeSet,
}

pub type ShapeSet = Vec<Vec<(f64, f64)>>;

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

        let shape_data = ShapeData {
            low: collect_shapes(&low_shape_data),
            med: collect_shapes(&med_shape_data),
            high: collect_shapes(&high_shape_data),
        };

        let country = Country{ data, shape_data, style };
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

fn collect_shapes(shape: &Shape) -> ShapeSet {
    let mut shapes = Vec::new();
    let polygon = match shape {
        Shape::Polygon(p) => p,
        _ => panic!("non polygon"),
    };
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