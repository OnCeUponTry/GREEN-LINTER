use crate::eco_checks::{Finding, FindingCategory};
use crate::project_scanner::ProjectInfo;

pub fn check(project: &ProjectInfo, findings: &mut Vec<Finding>) {
    if project.has_node_modules {
        let size = project.node_modules_size;
        findings.push(Finding {
            category: FindingCategory::BuildArtifact,
            title: format!("node_modules in repository ({})", format_bytes(size)),
            detail: format!(
                "node_modules/ found in project root: {}. This should be in .gitignore.",
                format_bytes(size)
            ),
            wasted_bytes: size,
            file: Some("node_modules/".into()),
            line: None,
        });
    }

    if project.has_dist {
        let size = project.dist_size;
        findings.push(Finding {
            category: FindingCategory::BuildArtifact,
            title: format!("dist/ in repository ({})", format_bytes(size)),
            detail: format!(
                "dist/ found in project root: {}. Build artifacts should be generated in CI, not committed.",
                format_bytes(size)
            ),
            wasted_bytes: size,
            file: Some("dist/".into()),
            line: None,
        });
    }

    for artifact in &project.extra_artifacts {
        findings.push(Finding {
            category: FindingCategory::BuildArtifact,
            title: format!("{}/ in repository ({})", artifact.name, format_bytes(artifact.size)),
            detail: format!(
                "{}/ found in project root: {}. Build artifacts should be generated in CI, not committed.",
                artifact.name, format_bytes(artifact.size)
            ),
            wasted_bytes: artifact.size,
            file: Some(format!("{}/", artifact.name)),
            line: None,
        });
    }
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
