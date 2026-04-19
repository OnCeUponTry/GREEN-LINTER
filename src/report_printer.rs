use std::collections::HashMap;

use colored::Colorize;
use serde::Serialize;

use crate::co2_estimator::Co2Estimator;
use crate::eco_checks::{Finding, FindingCategory};
use crate::project_scanner::ProjectInfo;

#[derive(Serialize)]
struct JsonReport<'a> {
    project: String,
    project_type: String,
    country: &'a str,
    intensity_gco2_kwh: f64,
    findings: Vec<JsonFinding>,
    summary: JsonSummary,
}

#[derive(Serialize)]
struct JsonFinding {
    category: String,
    title: String,
    detail: String,
    wasted_bytes: u64,
    co2_grams: f64,
    file: Option<String>,
    line: Option<usize>,
}

#[derive(Serialize)]
struct JsonSummary {
    total_findings: usize,
    total_wasted_bytes: u64,
    total_co2_grams: f64,
    by_category: HashMap<String, usize>,
}

pub fn print_json(project: &ProjectInfo, findings: &[Finding], estimator: &Co2Estimator) {
    let types: Vec<&str> = project
        .project_type
        .iter()
        .map(|t| match t {
            crate::project_scanner::ProjectType::Node => "Node",
            crate::project_scanner::ProjectType::Docker => "Docker",
        })
        .collect();

    let mut total_wasted: u64 = 0;
    let mut by_category: HashMap<String, usize> = HashMap::new();
    let json_findings: Vec<JsonFinding> = findings
        .iter()
        .map(|f| {
            let co2 = if f.wasted_bytes > 0 {
                total_wasted += f.wasted_bytes;
                estimator.bytes_to_gco2(f.wasted_bytes)
            } else {
                0.0
            };
            *by_category.entry(f.category.label().to_string()).or_insert(0) += 1;
            JsonFinding {
                category: f.category.label().to_string(),
                title: f.title.clone(),
                detail: f.detail.clone(),
                wasted_bytes: f.wasted_bytes,
                co2_grams: co2,
                file: f.file.clone(),
                line: f.line,
            }
        })
        .collect();

    let report = JsonReport {
        project: project.root.display().to_string(),
        project_type: types.join(" + "),
        country: estimator.country_code(),
        intensity_gco2_kwh: estimator.intensity(),
        summary: JsonSummary {
            total_findings: findings.len(),
            total_wasted_bytes: total_wasted,
            total_co2_grams: estimator.bytes_to_gco2(total_wasted),
            by_category,
        },
        findings: json_findings,
    };

    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}

pub fn print_report(project: &ProjectInfo, findings: &[Finding], estimator: &Co2Estimator) {
    println!();
    println!("{}", "=== green-linter report ===".green().bold());
    println!();

    // Project info
    let types: Vec<&str> = project
        .project_type
        .iter()
        .map(|t| match t {
            crate::project_scanner::ProjectType::Node => "Node",
            crate::project_scanner::ProjectType::Docker => "Docker",
        })
        .collect();
    println!(
        "  {} {}",
        "Project:".white().bold(),
        project.root.display()
    );
    println!(
        "  {} {}",
        "Type:".white().bold(),
        types.join(" + ")
    );
    println!(
        "  {} {} ({})",
        "CO2 region:".white().bold(),
        estimator.country_code(),
        format!("{:.2} gCO2/kWh", estimator.intensity()).cyan()
    );
    println!();

    if findings.is_empty() {
        println!(
            "  {} No waste detected. Your project is clean!",
            "OK".green().bold()
        );
        print_footer();
        return;
    }

    // Group findings
    println!(
        "  {} {} finding(s):",
        "Found".yellow().bold(),
        findings.len()
    );

    // Category summary
    let mut cat_counts: HashMap<&str, usize> = HashMap::new();
    for f in findings {
        *cat_counts.entry(f.category.label()).or_insert(0) += 1;
    }
    let mut cats: Vec<_> = cat_counts.into_iter().collect();
    cats.sort_by(|a, b| b.1.cmp(&a.1));
    let summary_parts: Vec<String> = cats
        .iter()
        .map(|(label, count)| format!("{} {}", count, label))
        .collect();
    println!("  {}", summary_parts.join(" | ").cyan());
    println!();

    let mut total_wasted: u64 = 0;

    for (i, finding) in findings.iter().enumerate() {
        let icon = match &finding.category {
            FindingCategory::DockerBaseImage
            | FindingCategory::DockerCache
            | FindingCategory::DockerLayerBloat
            | FindingCategory::DockerStructure
            | FindingCategory::DockerOrphaned => "D",
            FindingCategory::HeavyDependency
            | FindingCategory::NodePhantom
            | FindingCategory::NodeDuplicate
            | FindingCategory::NodeDeprecated => "N",
            FindingCategory::BuildArtifact => "A",
            FindingCategory::MissingLockfile
            | FindingCategory::LockfileConflict => "L",
        };

        let category_color = match &finding.category {
            FindingCategory::DockerBaseImage
            | FindingCategory::DockerCache
            | FindingCategory::DockerLayerBloat
            | FindingCategory::DockerStructure => finding.category.label().blue(),
            FindingCategory::DockerOrphaned => finding.category.label().yellow(),
            FindingCategory::HeavyDependency => finding.category.label().magenta(),
            FindingCategory::NodePhantom => finding.category.label().cyan(),
            FindingCategory::NodeDuplicate => finding.category.label().magenta(),
            FindingCategory::NodeDeprecated => finding.category.label().red(),
            FindingCategory::BuildArtifact => finding.category.label().red(),
            FindingCategory::MissingLockfile
            | FindingCategory::LockfileConflict => finding.category.label().yellow(),
        };

        let location = match (&finding.file, finding.line) {
            (Some(f), Some(l)) => format!("{}:{}", f, l),
            (Some(f), None) => f.clone(),
            _ => String::new(),
        };

        println!(
            "  {} [{}] {} {}",
            format!("{}.", i + 1).white().bold(),
            icon.cyan().bold(),
            category_color.bold(),
            if !location.is_empty() {
                format!("({})", location).dimmed().to_string()
            } else {
                String::new()
            }
        );
        println!("     {}", finding.title.white());
        println!("     {}", finding.detail.white());

        if finding.wasted_bytes > 0 {
            let gco2 = estimator.bytes_to_gco2(finding.wasted_bytes);
            println!(
                "     {} {:.4} gCO2 ({})",
                "~".yellow(),
                gco2,
                format_bytes(finding.wasted_bytes).red()
            );
            total_wasted += finding.wasted_bytes;
        }
        println!();
    }

    // Total CO2
    if total_wasted > 0 {
        let total_gco2 = estimator.bytes_to_gco2(total_wasted);
        println!("{}", "--- CO2 Impact Summary ---".green().bold());
        println!(
            "  Total waste measured: {}",
            format_bytes(total_wasted).red().bold()
        );
        println!(
            "  Estimated CO2:       {:.4} gCO2",
            total_gco2
        );
        println!(
            "  Calculation: {}",
            estimator.format_chain(total_wasted).dimmed()
        );
        println!(
            "  Sources: Ember Climate 2023 (CC-BY), Aslan et al. 2018",
        );
        println!();
    }

    print_footer();
}

fn print_footer() {
    println!();
    println!(
        "  {}",
        "All suggestions are informational. Test alternatives before migrating."
            .yellow()
            .dimmed()
    );
    println!(
        "  {}",
        "This scan's footprint: ~0.002g CO2".green().dimmed()
    );
    println!(
        "  {}",
        "green-linter v0.1.0 -- https://github.com/OnCeUponTry/green-linter"
            .dimmed()
    );
    println!();
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}
