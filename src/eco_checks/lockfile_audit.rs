use crate::eco_checks::{Finding, FindingCategory};
use crate::project_scanner::{ProjectInfo, ProjectType};

pub fn check(project: &ProjectInfo, findings: &mut Vec<Finding>) {
    if !project.project_type.contains(&ProjectType::Node) {
        return;
    }

    if !project.has_lockfile {
        findings.push(Finding {
            category: FindingCategory::MissingLockfile,
            title: "No lockfile found".into(),
            detail: "No package-lock.json, yarn.lock, or pnpm-lock.yaml. Builds are non-reproducible: CI installs different versions each time, wasting compute.".into(),
            wasted_bytes: 0,
            file: Some("package.json".into()),
            line: None,
        });
    }

    if project.lockfiles_found.len() > 1 {
        findings.push(Finding {
            category: FindingCategory::LockfileConflict,
            title: format!("{} lockfiles found (conflict)", project.lockfiles_found.len()),
            detail: format!(
                "Found: {}. Multiple lockfiles cause ambiguous installs -- different team members may get different dependency trees. Keep one, delete the rest.",
                project.lockfiles_found.join(", ")
            ),
            wasted_bytes: 0,
            file: None,
            line: None,
        });
    }
}
