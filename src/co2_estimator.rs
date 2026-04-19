use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Serialize)]
pub struct WasteProfile {
    pub emoji: &'static str,
    pub label: &'static str,
    pub message: &'static str,
}

#[derive(Deserialize)]
struct CarbonData {
    countries: HashMap<String, f64>,
}

// Aslan et al. (2018): "Electricity Intensity of Internet Data Transmission"
// Energy per byte of network data: 0.06 kWh/GB
const ENERGY_PER_GB_KWH: f64 = 0.06;

// Standard home LED bulb (replaces traditional 60W incandescent)
const LED_BULB_WATTS: f64 = 9.0;

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

    /// Convert wasted bytes to equivalent hours of a standard LED bulb.
    /// Country-independent: grid intensity cancels out (same grid powers both waste and bulb).
    pub fn lightbulb_hours(bytes: u64) -> f64 {
        let gb = bytes as f64 / (1024.0 * 1024.0 * 1024.0);
        let kwh = gb * ENERGY_PER_GB_KWH;
        kwh / (LED_BULB_WATTS / 1000.0)
    }

    /// Waste profile: 3-tier RAGE scale with sarcastic message.
    /// Thresholds: <100 MB = CHILL, 100–500 MB = ANGRY, >500 MB = RAGEST.
    /// Inspired by SWD Digital Carbon Ratings & Green Software Foundation SCI.
    pub fn waste_profile(bytes: u64) -> WasteProfile {
        let mb = bytes as f64 / (1024.0 * 1024.0);
        let idx = (bytes / 1024) as usize;

        if mb < 100.0 {
            let msgs: &[&str] = &[
                "Nice! You actually read the docs.",
                "Your project is clean. Was this an accident?",
                "Respect. Not many devs make it here.",
            ];
            WasteProfile { emoji: "\u{1f60c}", label: "CHILL", message: msgs[idx % msgs.len()] }
        } else if mb < 500.0 {
            let msgs: &[&str] = &[
                "Hey, wake up! Your deps are collecting dust.",
                "Your node_modules sent a distress signal.",
                "Time for spring cleaning, don't you think?",
            ];
            WasteProfile { emoji: "\u{1f620}", label: "ANGRY", message: msgs[idx % msgs.len()] }
        } else {
            let msgs: &[&str] = &[
                "Are you a trainee? This needs urgent cleanup.",
                "Your project weighs more than my first laptop.",
                "Delete node_modules. Breathe. Start over.",
            ];
            WasteProfile { emoji: "\u{1f480}", label: "RAGEST", message: msgs[idx % msgs.len()] }
        }
    }
}
