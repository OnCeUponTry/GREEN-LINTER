use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
struct Config {
    country: String,
}

fn config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home)
        .join(".config")
        .join("green-linter")
        .join("config.json")
}

pub fn load_country() -> Option<String> {
    let path = config_path();
    let content = std::fs::read_to_string(path).ok()?;
    let config: Config = serde_json::from_str(&content).ok()?;
    Some(config.country)
}

pub fn save_country(country: &str) -> Result<(), String> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Cannot create config dir: {}", e))?;
    }
    let config = Config {
        country: country.to_string(),
    };
    let json =
        serde_json::to_string_pretty(&config).map_err(|e| format!("JSON error: {}", e))?;
    std::fs::write(&path, json).map_err(|e| format!("Cannot write config: {}", e))?;
    Ok(())
}

pub fn prompt_country(valid_codes: &[String]) -> String {
    println!();
    println!("  First run! Enter your country code (ISO 3166-1 alpha-3).");
    println!("  Examples: USA, PER, DEU, BRA, IND, GBR, FRA, JPN, CHN");
    println!("  Use WORLD for global average (483 gCO2/kWh).");
    print!("  > ");
    io::stdout().flush().ok();

    let stdin = io::stdin();
    loop {
        let mut input = String::new();
        if stdin.lock().read_line(&mut input).is_err() {
            return "WORLD".into();
        }
        let code = input.trim().to_uppercase();
        if code.is_empty() {
            return "WORLD".into();
        }
        if valid_codes.contains(&code) {
            return code;
        }
        println!(
            "  Unknown code '{}'. Try again (e.g. USA, PER, DEU, WORLD):",
            code
        );
        print!("  > ");
        io::stdout().flush().ok();
    }
}
