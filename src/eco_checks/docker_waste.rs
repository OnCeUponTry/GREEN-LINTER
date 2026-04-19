use crate::eco_checks::{Finding, FindingCategory};
use crate::project_scanner::ProjectInfo;

// Known heavy images and their alpine equivalents (sizes in bytes)
// Sources: Docker Hub official image sizes as of 2026-04
struct ImageInfo {
    pattern: &'static str,
    size_mb: u64,
    alpine_mb: u64,
}

const HEAVY_IMAGES: &[ImageInfo] = &[
    ImageInfo { pattern: "ubuntu", size_mb: 77, alpine_mb: 7 },
    ImageInfo { pattern: "debian", size_mb: 124, alpine_mb: 7 },
    ImageInfo { pattern: "centos", size_mb: 231, alpine_mb: 7 },
    ImageInfo { pattern: "fedora", size_mb: 187, alpine_mb: 7 },
    ImageInfo { pattern: "node:", size_mb: 350, alpine_mb: 50 },
    ImageInfo { pattern: "python:", size_mb: 340, alpine_mb: 50 },
];

pub fn check(project: &ProjectInfo, findings: &mut Vec<Finding>) {
    if let Some(df) = &project.dockerfile {
        check_single_dockerfile(project, df, findings);
    }
    for df in &project.extra_dockerfiles {
        check_single_dockerfile(project, df, findings);
    }
    check_project_level(project, findings);
    check_orphaned_dockerfiles(project, findings);
}

fn check_project_level(project: &ProjectInfo, findings: &mut Vec<Finding>) {
    let has_any_docker = project.dockerfile.is_some() || !project.extra_dockerfiles.is_empty();
    if !has_any_docker {
        return;
    }
    if !project.has_dockerignore {
        let context_size = estimate_build_context(project);
        findings.push(Finding {
            category: FindingCategory::DockerStructure,
            title: "Missing .dockerignore".into(),
            detail: format!(
                "No .dockerignore found. Entire directory (~{}) sent as build context. node_modules, .git, dist/ included.",
                format_bytes(context_size)
            ),
            wasted_bytes: context_size,
            file: None,
            line: None,
        });
    }
}

fn check_single_dockerfile(
    project: &ProjectInfo,
    dockerfile: &crate::project_scanner::DockerfileInfo,
    findings: &mut Vec<Finding>,
) {
    let fname = &dockerfile.filename;

    for (i, line) in dockerfile.lines.iter().enumerate() {
        let trimmed = line.trim();
        if !trimmed.to_uppercase().starts_with("FROM ") {
            continue;
        }
        let image = trimmed[5..].trim().split_whitespace().next().unwrap_or("");
        let image_lower = image.to_lowercase();
        let line_num = i + 1;

        // Pattern 1: Heavy base image (also covers Pattern 3 — no slim/alpine)
        let mut matched_heavy = false;
        for info in HEAVY_IMAGES {
            if image_lower.contains(info.pattern)
                && !image_lower.contains("alpine")
                && !image_lower.contains("slim")
            {
                let delta_mb = info.size_mb - info.alpine_mb;
                let delta_bytes = delta_mb * 1024 * 1024;
                findings.push(Finding {
                    category: FindingCategory::DockerBaseImage,
                    title: format!("Heavy base image: {}", image),
                    detail: format!(
                        "{} = ~{}MB. alpine equivalent = ~{}MB. Delta: {}MB",
                        image, info.size_mb, info.alpine_mb, delta_mb
                    ),
                    wasted_bytes: delta_bytes,
                    file: Some(fname.clone()),
                    line: Some(line_num),
                });
                matched_heavy = true;
                break;
            }
        }

        // Pattern 2: :latest or no tag (non-reproducible)
        if image.contains(":latest") || (!image.contains(':') && !image.contains("scratch")) {
            findings.push(Finding {
                category: FindingCategory::DockerBaseImage,
                title: format!("Non-pinned image tag: {}", image),
                detail: format!(
                    "'{}' has no version pin. Builds are non-reproducible across time.",
                    image
                ),
                wasted_bytes: 0,
                file: Some(fname.clone()),
                line: Some(line_num),
            });
        }

        // Pattern 3: No -slim/-alpine variant (skip if already caught by Pattern 1)
        if !matched_heavy
            && !image_lower.contains("alpine")
            && !image_lower.contains("slim")
            && !image_lower.contains("scratch")
            && (image_lower.starts_with("node:") || image_lower.starts_with("python:"))
        {
            let base = if image_lower.starts_with("node:") { "node" } else { "python" };
            findings.push(Finding {
                category: FindingCategory::DockerBaseImage,
                title: format!("No slim/alpine variant: {}", image),
                detail: format!(
                    "'{}' uses full image (~350MB). {}-alpine is ~50MB. Delta: ~300MB",
                    image, base
                ),
                wasted_bytes: 300 * 1024 * 1024,
                file: Some(fname.clone()),
                line: Some(line_num),
            });
        }
    }

    check_cache_patterns(dockerfile, findings);
    check_layer_bloat(dockerfile, findings);
    check_structure(project, dockerfile, findings);
}

fn check_cache_patterns(
    dockerfile: &crate::project_scanner::DockerfileInfo,
    findings: &mut Vec<Finding>,
) {
    let fname = &dockerfile.filename;
    let mut copy_all_line: Option<usize> = None;
    let mut install_line: Option<usize> = None;

    for (i, line) in dockerfile.lines.iter().enumerate() {
        let trimmed = line.trim().to_uppercase();
        let line_num = i + 1;

        // Pattern 4: COPY . . before npm install (cache invalidation)
        if trimmed.starts_with("COPY . ") || trimmed.starts_with("COPY ./") {
            if copy_all_line.is_none() {
                copy_all_line = Some(line_num);
            }
        }
        if trimmed.contains("NPM INSTALL") || trimmed.contains("YARN INSTALL") || trimmed.contains("PNPM INSTALL") {
            install_line = Some(line_num);
        }

        // Pattern 5: ADD where COPY suffices
        if trimmed.starts_with("ADD ") && !trimmed.contains("HTTP") && !trimmed.contains(".TAR") && !trimmed.contains(".GZ") {
            findings.push(Finding {
                category: FindingCategory::DockerCache,
                title: format!("ADD used where COPY suffices (line {})", line_num),
                detail: "ADD has extra functionality (URL fetch, tar extraction). COPY is explicit and cache-friendly.".into(),
                wasted_bytes: 0,
                file: Some(fname.clone()),
                line: Some(line_num),
            });
        }

        // Pattern 6: apt-get without --no-install-recommends
        if trimmed.contains("APT-GET INSTALL") && !trimmed.contains("--NO-INSTALL-RECOMMENDS") {
            findings.push(Finding {
                category: FindingCategory::DockerCache,
                title: format!("apt-get without --no-install-recommends (line {})", line_num),
                detail: "Recommended packages add ~30-100MB of unnecessary dependencies.".into(),
                wasted_bytes: 50 * 1024 * 1024,
                file: Some(fname.clone()),
                line: Some(line_num),
            });
        }
    }

    if let (Some(copy_ln), Some(inst_ln)) = (copy_all_line, install_line) {
        if copy_ln < inst_ln {
            findings.push(Finding {
                category: FindingCategory::DockerCache,
                title: format!("COPY . before install (lines {}/{})", copy_ln, inst_ln),
                detail: format!(
                    "COPY . . at line {} invalidates cache for install at line {}. Copy package.json first, install, then COPY the rest.",
                    copy_ln, inst_ln
                ),
                wasted_bytes: 0,
                file: Some(fname.clone()),
                line: Some(copy_ln),
            });
        }
    }
}

fn check_layer_bloat(
    dockerfile: &crate::project_scanner::DockerfileInfo,
    findings: &mut Vec<Finding>,
) {
    let fname = &dockerfile.filename;
    let mut consecutive_runs: Vec<usize> = Vec::new();

    // Multi-stage awareness: find the last FROM line index
    let from_indices: Vec<usize> = dockerfile
        .lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.trim().to_uppercase().starts_with("FROM "))
        .map(|(i, _)| i)
        .collect();
    let is_multistage = from_indices.len() > 1;
    let last_from_idx = from_indices.last().copied().unwrap_or(0);

    for (i, line) in dockerfile.lines.iter().enumerate() {
        let trimmed = line.trim().to_uppercase();
        let line_num = i + 1;

        // Pattern 7: apt-get without cleanup
        if trimmed.contains("APT-GET INSTALL") && !trimmed.contains("RM -RF") && !trimmed.contains("CLEAN") {
            findings.push(Finding {
                category: FindingCategory::DockerLayerBloat,
                title: format!("apt-get without cleanup (line {})", line_num),
                detail: "apt cache stays in layer (~30-50MB). Add: && rm -rf /var/lib/apt/lists/*".into(),
                wasted_bytes: 40 * 1024 * 1024,
                file: Some(fname.clone()),
                line: Some(line_num),
            });
        }

        // Pattern 8: Consecutive RUN commands
        if trimmed.starts_with("RUN ") {
            consecutive_runs.push(line_num);
        } else if !trimmed.is_empty() && !trimmed.starts_with('#') {
            if consecutive_runs.len() > 1 {
                findings.push(Finding {
                    category: FindingCategory::DockerLayerBloat,
                    title: format!("{} consecutive RUN commands (lines {}-{})", consecutive_runs.len(), consecutive_runs[0], consecutive_runs.last().unwrap()),
                    detail: format!(
                        "{} separate RUN = {} layers. Combine with && to reduce image size.",
                        consecutive_runs.len(), consecutive_runs.len()
                    ),
                    wasted_bytes: 0,
                    file: Some(fname.clone()),
                    line: Some(consecutive_runs[0]),
                });
            }
            consecutive_runs.clear();
        }

        // Pattern 9: npm install without --omit=dev
        // In multi-stage builds, only flag in the final (production) stage —
        // builder stages need devDependencies for compilation
        let in_final_stage = !is_multistage || i >= last_from_idx;
        if in_final_stage
            && (trimmed.contains("NPM INSTALL") || trimmed.contains("NPM CI"))
            && !trimmed.contains("--OMIT=DEV")
            && !trimmed.contains("--PRODUCTION")
            && !trimmed.contains("NODE_ENV=PRODUCTION")
        {
            findings.push(Finding {
                category: FindingCategory::DockerLayerBloat,
                title: format!("npm install includes devDependencies (line {})", line_num),
                detail: "Production image includes devDependencies. Add --omit=dev to exclude them.".into(),
                wasted_bytes: 0,
                file: Some(fname.clone()),
                line: Some(line_num),
            });
        }
    }

    // Check remaining consecutive runs at end of file
    if consecutive_runs.len() > 1 {
        findings.push(Finding {
            category: FindingCategory::DockerLayerBloat,
            title: format!(
                "{} consecutive RUN commands (lines {}-{})",
                consecutive_runs.len(),
                consecutive_runs[0],
                consecutive_runs.last().unwrap()
            ),
            detail: format!(
                "{} separate RUN = {} layers. Combine with && to reduce image size.",
                consecutive_runs.len(),
                consecutive_runs.len()
            ),
            wasted_bytes: 0,
            file: Some(fname.clone()),
            line: Some(consecutive_runs[0]),
        });
    }
}

fn check_structure(
    _project: &ProjectInfo,
    dockerfile: &crate::project_scanner::DockerfileInfo,
    findings: &mut Vec<Finding>,
) {
    let fname = &dockerfile.filename;
    let content_upper: String = dockerfile.lines.join("\n").to_uppercase();

    // Pattern 10: No multi-stage when build tools present
    let from_count = dockerfile.lines.iter().filter(|l| l.trim().to_uppercase().starts_with("FROM ")).count();
    let has_build_tools = content_upper.contains("GCC")
        || content_upper.contains("G++")
        || content_upper.contains("MAKE")
        || content_upper.contains("BUILD-ESSENTIAL")
        || content_upper.contains("NPM RUN BUILD");

    if from_count <= 1 && has_build_tools {
        findings.push(Finding {
            category: FindingCategory::DockerStructure,
            title: "No multi-stage build with build tools".into(),
            detail: "Build tools (gcc, make, build-essential, npm run build) remain in production image. Use multi-stage to drop them.".into(),
            wasted_bytes: 200 * 1024 * 1024,
            file: Some(fname.clone()),
            line: None,
        });
    }

    // Pattern 12: No USER directive (runs as root)
    let has_user = dockerfile.lines.iter().any(|l| l.trim().to_uppercase().starts_with("USER "));
    if !has_user {
        findings.push(Finding {
            category: FindingCategory::DockerStructure,
            title: "No USER directive (runs as root)".into(),
            detail: "Container runs as root. Add USER directive for security.".into(),
            wasted_bytes: 0,
            file: Some(fname.clone()),
            line: None,
        });
    }
}

fn estimate_build_context(project: &ProjectInfo) -> u64 {
    let mut size = 0u64;
    if project.has_node_modules {
        size += project.node_modules_size;
    }
    if project.has_dist {
        size += project.dist_size;
    }
    size
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

fn check_orphaned_dockerfiles(project: &ProjectInfo, findings: &mut Vec<Finding>) {
    if project.compose_referenced.is_empty() {
        return;
    }
    let all_dockerfiles: Vec<&str> = {
        let mut names = Vec::new();
        if let Some(df) = &project.dockerfile {
            names.push(df.filename.as_str());
        }
        for df in &project.extra_dockerfiles {
            names.push(df.filename.as_str());
        }
        names
    };
    let ref_sources: String = {
        let mut sources = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&project.root) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let n = name.to_string_lossy().to_lowercase();
                if (n.ends_with(".yml") || n.ends_with(".yaml") || n.ends_with(".sh")
                    || n.starts_with("makefile") || n == "justfile")
                    && entry.path().is_file()
                {
                    sources.push(entry.file_name().to_string_lossy().to_string());
                }
            }
        }
        sources.sort();
        sources.join(", ")
    };
    for name in &all_dockerfiles {
        let is_referenced = project
            .compose_referenced
            .iter()
            .any(|r| r == *name);
        if !is_referenced {
            findings.push(Finding {
                category: FindingCategory::DockerOrphaned,
                title: format!("Orphaned Dockerfile: {}", name),
                detail: format!(
                    "'{}' is not referenced by any config file. Searched: [{}]. Dead config wastes maintenance and causes confusion.",
                    name, ref_sources
                ),
                wasted_bytes: 0,
                file: Some(name.to_string()),
                line: None,
            });
        }
    }
}
