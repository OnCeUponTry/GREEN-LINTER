use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Deserialize)]
struct CarbonData {
    countries: HashMap<String, f64>,
}

// Aslan et al. (2018): "Electricity Intensity of Internet Data Transmission"
// Energy per byte of network data: 0.06 kWh/GB
const ENERGY_PER_GB_KWH: f64 = 0.06;

const CARBON_DATA_JSON: &str = include_str!("../data/carbon_intensity.json");

fn carbon_data() -> &'static HashMap<String, f64> {
    static DATA: OnceLock<HashMap<String, f64>> = OnceLock::new();
    DATA.get_or_init(|| {
        let data: CarbonData = serde_json::from_str(CARBON_DATA_JSON)
            .expect("embedded carbon data is valid JSON");
        data.countries
    })
}

pub struct Co2Estimator {
    country_code: String,
    intensity_gco2_per_kwh: f64,
}

impl Co2Estimator {
    pub fn new(country_code: &str) -> Result<Self, String> {
        let code = country_code.to_uppercase();
        let intensity = carbon_data().get(&code).copied().ok_or_else(|| {
            format!(
                "Unknown country code '{}'. Use ISO 3166-1 alpha-3 (e.g. USA, PER, DEU). Use WORLD for global average.",
                code
            )
        })?;

        Ok(Self {
            country_code: code,
            intensity_gco2_per_kwh: intensity,
        })
    }

    pub fn valid_country_codes() -> Vec<String> {
        carbon_data().keys().cloned().collect()
    }

    /// Convert wasted bytes to estimated CO2 in grams
    pub fn bytes_to_gco2(&self, bytes: u64) -> f64 {
        let gb = bytes as f64 / (1024.0 * 1024.0 * 1024.0);
        let kwh = gb * ENERGY_PER_GB_KWH;
        kwh * self.intensity_gco2_per_kwh
    }

    pub fn country_code(&self) -> &str {
        &self.country_code
    }

    pub fn intensity(&self) -> f64 {
        self.intensity_gco2_per_kwh
    }

    /// Format the calculation chain for display
    pub fn format_chain(&self, bytes: u64) -> String {
        let mb = bytes as f64 / (1024.0 * 1024.0);
        let gb = bytes as f64 / (1024.0 * 1024.0 * 1024.0);
        let kwh = gb * ENERGY_PER_GB_KWH;
        let gco2 = kwh * self.intensity_gco2_per_kwh;
        format!(
            "{:.2} MB x {:.2} kWh/GB x {:.2} gCO2/kWh ({}) = {:.4} gCO2",
            mb, ENERGY_PER_GB_KWH, self.intensity_gco2_per_kwh, self.country_code, gco2
        )
    }
}
