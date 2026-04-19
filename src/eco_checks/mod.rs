pub mod build_artifacts;
pub mod docker_waste;
pub mod heavy_deps;
pub mod lockfile_audit;

use crate::project_scanner::ProjectInfo;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Finding {
    pub category: FindingCategory,
    pub title: String,
    pub detail: String,
    pub wasted_bytes: u64,
    pub file: Option<String>,
    pub line: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub enum FindingCategory {
    DockerBaseImage,
    DockerCache,
    DockerLayerBloat,
    DockerStructure,
    DockerOrphaned,
    HeavyDependency,
    NodePhantom,
    NodeDuplicate,
    NodeDeprecated,
    BuildArtifact,
    MissingLockfile,
    LockfileConflict,
}

impl FindingCategory {
    pub fn label(&self) -> &str {
        match self {
            Self::DockerBaseImage => "Docker Base Image",
            Self::DockerCache => "Docker Cache",
            Self::DockerLayerBloat => "Docker Layer Bloat",
            Self::DockerStructure => "Docker Structure",
            Self::DockerOrphaned => "Docker Orphaned",
            Self::HeavyDependency => "Heavy Dependency",
            Self::NodePhantom => "Phantom Dependency",
            Self::NodeDuplicate => "Duplicate Dependency",
            Self::NodeDeprecated => "Deprecated Dependency",
            Self::BuildArtifact => "Build Artifact",
            Self::MissingLockfile => "Missing Lockfile",
            Self::LockfileConflict => "Lockfile Conflict",
        }
    }
}

pub fn run_all_checks(project: &ProjectInfo) -> Vec<Finding> {
    let mut findings = Vec::new();
    docker_waste::check(project, &mut findings);
    heavy_deps::check(project, &mut findings);
    build_artifacts::check(project, &mut findings);
    lockfile_audit::check(project, &mut findings);
    findings
}
