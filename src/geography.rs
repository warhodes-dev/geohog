use std::collections::BTreeMap;

use self::map_load::{countries_from_shapefile, Country};
use anyhow::Result;

pub mod map_load;

pub fn get_geography() -> Result<BTreeMap<String, Country>> {
    // WARNING: Hardcoded paths. TODO: Config for these? 
    // These could be an option. "Do you want to fetch high resolution map data?"
    let low_res_shapefile = "geo_data/ne_110m_admin_0_countries/ne_110m_admin_0_countries.shp";
    let med_res_shapefile = "geo_data/ne_50m_admin_0_countries/ne_50m_admin_0_countries.shp";
    let high_res_shapefile = "geo_data/ne_10m_admin_0_countries/ne_10m_admin_0_countries.shp";

    let geography = countries_from_shapefile(
        low_res_shapefile,
        med_res_shapefile,
        high_res_shapefile,
    )?;

    Ok(geography)
}