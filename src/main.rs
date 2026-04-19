use clap::Parser;
use std::path::PathBuf;
use std::process;

mod co2_estimator;
mod config;
mod eco_checks;
mod project_scanner;
mod report_printer;

use co2_estimator::Co2Estimator;
use eco_checks::run_all_checks;
use project_scanner::scan_project;
use report_printer::{print_json, print_report};

#[derive(Parser)]
#[command(name = "green-linter", version, about = "Detect computational waste in project structure and estimate CO2 impact")]
struct Cli {
    /// Path to the project directory to audit
    #[arg(default_value = ".")]
    path: PathBuf,

    /// ISO 3166-1 alpha-3 country code for CO2 intensity (e.g. USA, PER, DEU)
    #[arg(long)]
    country: Option<String>,

    /// Output findings as JSON instead of colored text
    #[arg(long)]
    json: bool,
}

fn main() {
    let cli = Cli::parse();

    let path = match cli.path.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error: cannot access '{}': {}", cli.path.display(), e);
            process::exit(1);
        }
    };

    if !path.is_dir() {
        eprintln!("Error: '{}' is not a directory", path.display());
        process::exit(1);
    }

    // Resolve country: explicit flag > saved config > interactive prompt
    let country_code = if let Some(explicit) = cli.country {
        explicit
    } else if let Some(saved) = config::load_country() {
        saved
    } else {
        let valid = Co2Estimator::valid_country_codes();
        let code = config::prompt_country(&valid);
        if let Err(e) = config::save_country(&code) {
            eprintln!("Warning: could not save config: {}", e);
        }
        code
    };

    let estimator = match Co2Estimator::new(&country_code) {
        Ok(e) => e,
        Err(msg) => {
            eprintln!("Error: {}", msg);
            process::exit(1);
        }
    };

    let project = scan_project(&path);

    if project.project_type.is_empty() {
        eprintln!("No supported project detected (Node/Docker). Nothing to audit.");
        eprintln!("Supported: package.json (Node), Dockerfile (Docker)");
        eprintln!("Want more ecosystems? Open an issue: https://github.com/OnCeUponTry/green-linter/issues");
        process::exit(0);
    }

    let findings = run_all_checks(&project);

    if cli.json {
        print_json(&project, &findings, &estimator);
    } else {
        print_report(&project, &findings, &estimator);
    }

    if !findings.is_empty() {
        process::exit(1);
    }
}
