use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

pub struct ArtifactDir {
    pub name: String,
    pub size: u64,
}

pub struct ProjectInfo {
    pub root: PathBuf,
    pub project_type: Vec<ProjectType>,
    pub package_json: Option<PackageJsonInfo>,
    pub dockerfile: Option<DockerfileInfo>,
    pub extra_dockerfiles: Vec<DockerfileInfo>,
    pub compose_referenced: Vec<String>,
    pub source_files: Vec<SourceFile>,
    pub has_node_modules: bool,
    pub node_modules_size: u64,
    pub has_dist: bool,
    pub dist_size: u64,
    pub has_lockfile: bool,
    pub has_dockerignore: bool,
    pub extra_artifacts: Vec<ArtifactDir>,
    pub lockfiles_found: Vec<String>,
    pub import_index: HashMap<String, Vec<String>>,
}

pub struct SourceFile {
    pub relative_path: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProjectType {
    Node,
    Docker,
}

pub struct PackageJsonInfo {
    pub dependencies: Vec<(String, String)>,
    pub dev_dependencies: Vec<(String, String)>,
}

pub struct DockerfileInfo {
    pub lines: Vec<String>,
    pub filename: String,
}

pub fn scan_project(root: &Path) -> ProjectInfo {
    let mut project_type = Vec::new();
    let mut package_json = None;
    let mut dockerfile = None;
    let mut extra_dockerfiles = Vec::new();

    let pkg_path = root.join("package.json");
    if pkg_path.exists() {
        project_type.push(ProjectType::Node);
        package_json = parse_package_json(&pkg_path);
    }

    let docker_path = root.join("Dockerfile");
    if docker_path.exists() {
        project_type.push(ProjectType::Docker);
        dockerfile = parse_dockerfile(&docker_path, "Dockerfile");
    }

    // Scan for all Dockerfile variants in root directory
    // Patterns: Dockerfile.*, *.Dockerfile, Containerfile, Containerfile.*
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            let name_lower = name_str.to_lowercase();

            if !entry.path().is_file() {
                continue;
            }

            let is_docker = name_str.starts_with("Dockerfile.")
                || name_lower.ends_with(".dockerfile")
                || name_str == "Containerfile"
                || name_str.starts_with("Containerfile.");

            if is_docker {
                if !project_type.contains(&ProjectType::Docker) {
                    project_type.push(ProjectType::Docker);
                }
                if let Some(df) = parse_dockerfile(&entry.path(), &name_str) {
                    if dockerfile.is_none() {
                        dockerfile = Some(df);
                    } else {
                        extra_dockerfiles.push(df);
                    }
                }
            }
        }
    }

    let (has_node_modules, node_modules_size) = check_dir_size(&root.join("node_modules"));
    let (has_dist, dist_size) = check_dir_size(&root.join("dist"));

    let mut lockfiles_found = Vec::new();
    if root.join("package-lock.json").exists() {
        lockfiles_found.push("package-lock.json".to_string());
    }
    if root.join("yarn.lock").exists() {
        lockfiles_found.push("yarn.lock".to_string());
    }
    if root.join("pnpm-lock.yaml").exists() {
        lockfiles_found.push("pnpm-lock.yaml".to_string());
    }
    let has_lockfile = !lockfiles_found.is_empty();

    let has_dockerignore = root.join(".dockerignore").exists();

    let extra_artifacts = detect_extra_artifacts(root);

    let source_files = scan_source_files(root);
    let import_index = build_import_index(&source_files);

    let compose_referenced = scan_compose_dockerfiles(root);

    ProjectInfo {
        root: root.to_path_buf(),
        project_type,
        package_json,
        dockerfile,
        extra_dockerfiles,
        compose_referenced,
        source_files,
        has_node_modules,
        node_modules_size,
        has_dist,
        dist_size,
        has_lockfile,
        has_dockerignore,
        extra_artifacts,
        lockfiles_found,
        import_index,
    }
}

fn parse_package_json(path: &Path) -> Option<PackageJsonInfo> {
    let content = fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;

    let deps = extract_deps(json.get("dependencies"));
    let dev_deps = extract_deps(json.get("devDependencies"));

    Some(PackageJsonInfo {
        dependencies: deps,
        dev_dependencies: dev_deps,
    })
}

fn extract_deps(val: Option<&serde_json::Value>) -> Vec<(String, String)> {
    match val {
        Some(serde_json::Value::Object(map)) => map
            .iter()
            .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("*").to_string()))
            .collect(),
        _ => Vec::new(),
    }
}

fn parse_dockerfile(path: &Path, name: &str) -> Option<DockerfileInfo> {
    let content = fs::read_to_string(path).ok()?;
    let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    Some(DockerfileInfo {
        lines,
        filename: name.to_string(),
    })
}

fn check_dir_size(path: &Path) -> (bool, u64) {
    if !path.exists() || !path.is_dir() {
        return (false, 0);
    }
    (true, dir_size_recursive(path))
}

fn dir_size_recursive(path: &Path) -> u64 {
    let mut total: u64 = 0;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            if meta.is_file() {
                #[cfg(unix)]
                {
                    total += meta.blocks() * 512;
                }
                #[cfg(not(unix))]
                {
                    total += meta.len();
                }
            } else if meta.is_dir() {
                total += dir_size_recursive(&entry.path());
            }
        }
    }
    total
}

fn detect_extra_artifacts(root: &Path) -> Vec<ArtifactDir> {
    const ARTIFACT_DIRS: &[&str] = &[
        ".next", ".nuxt", "build", "out", "coverage",
        ".parcel-cache", "storybook-static", ".turbo",
    ];
    let mut artifacts = Vec::new();
    for name in ARTIFACT_DIRS {
        let dir_path = root.join(name);
        if dir_path.is_dir() {
            let size = dir_size_recursive(&dir_path);
            artifacts.push(ArtifactDir {
                name: name.to_string(),
                size,
            });
        }
    }
    artifacts
}

fn scan_compose_dockerfiles(root: &Path) -> Vec<String> {
    let mut referenced = Vec::new();
    let entries = match fs::read_dir(root) {
        Ok(e) => e,
        Err(_) => return referenced,
    };
    for entry in entries.flatten() {
        if !entry.path().is_file() {
            continue;
        }
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        let name_lower = name_str.to_lowercase();

        let content = match fs::read_to_string(entry.path()) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // Compose/YAML files: look for "dockerfile:" key and implicit "build:" references
        if name_lower.ends_with(".yml") || name_lower.ends_with(".yaml") {
            let mut has_build_directive = false;
            let mut explicit_dockerfile_count = 0;
            let mut build_directive_count = 0;

            for line in content.lines() {
                let trimmed = line.trim();
                if let Some(val) = trimmed.strip_prefix("dockerfile:") {
                    explicit_dockerfile_count += 1;
                    let df_ref = val.trim().trim_matches('"').trim_matches('\'');
                    if let Some(basename) = df_ref.rsplit('/').next() {
                        if !basename.is_empty() && !referenced.contains(&basename.to_string()) {
                            referenced.push(basename.to_string());
                        }
                    }
                }
                // Detect "build:" directives (both "build: ." and "build:" with sub-keys)
                if trimmed.starts_with("build:") {
                    has_build_directive = true;
                    build_directive_count += 1;
                }
            }

            // If compose has build directives without explicit dockerfile for all of them,
            // Docker Compose implicitly uses "Dockerfile" for those services
            if has_build_directive && build_directive_count > explicit_dockerfile_count {
                if !referenced.contains(&"Dockerfile".to_string()) {
                    referenced.push("Dockerfile".to_string());
                }
            }
        }

        // Shell scripts and Makefiles: look for "docker build -f", "buildah bud -f", "podman build -f"
        let is_script = name_lower.ends_with(".sh")
            || name_lower.starts_with("makefile")
            || name_lower == "justfile";
        if is_script {
            for line in content.lines() {
                let trimmed = line.trim();
                for prefix in &["docker build -f ", "buildah bud -f ", "podman build -f ",
                                 "docker build --file ", "docker buildx build -f ",
                                 "docker compose build"] {
                    if let Some(rest) = trimmed.to_lowercase().find(prefix).and_then(|pos| {
                        Some(&trimmed[pos + prefix.len()..])
                    }) {
                        let token = rest.split_whitespace().next().unwrap_or("");
                        let basename = token.rsplit('/').next().unwrap_or(token);
                        if !basename.is_empty() && !referenced.contains(&basename.to_string()) {
                            referenced.push(basename.to_string());
                        }
                    }
                }
            }
        }
    }
    referenced
}

const SOURCE_EXTENSIONS: &[&str] = &["js", "ts", "jsx", "tsx", "mjs", "cjs"];
const SKIP_DIRS: &[&str] = &["node_modules", "dist", ".git", "build", "coverage", ".next", ".nuxt"];
const MAX_FILE_SIZE: u64 = 512 * 1024; // 512KB per file

fn scan_source_files(root: &Path) -> Vec<SourceFile> {
    let mut files = Vec::new();
    scan_source_dir(root, root, &mut files);
    files
}

fn scan_source_dir(root: &Path, dir: &Path, files: &mut Vec<SourceFile>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if SKIP_DIRS.iter().any(|s| *s == name_str.as_ref()) {
                continue;
            }
            scan_source_dir(root, &path, files);
        } else if path.is_file() {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");
            if !SOURCE_EXTENSIONS.contains(&ext) {
                continue;
            }
            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            if meta.len() > MAX_FILE_SIZE {
                continue;
            }
            if let Ok(content) = fs::read_to_string(&path) {
                let relative = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();
                files.push(SourceFile {
                    relative_path: relative,
                    content,
                });
            }
        }
    }
}

fn build_import_index(source_files: &[SourceFile]) -> HashMap<String, Vec<String>> {
    let mut index: HashMap<String, Vec<String>> = HashMap::new();
    for source in source_files {
        let mut seen_in_file: HashSet<String> = HashSet::new();
        for line in source.content.lines() {
            for raw in extract_import_paths(line) {
                let pkg = normalize_package_name(&raw);
                if !pkg.is_empty() && seen_in_file.insert(pkg.clone()) {
                    index.entry(pkg).or_default().push(source.relative_path.clone());
                }
            }
        }
    }
    index
}

fn extract_import_paths(line: &str) -> Vec<String> {
    let mut paths = Vec::new();
    let markers: &[(&str, char)] = &[
        ("require('", '\''),
        ("require(\"", '"'),
        ("from '", '\''),
        ("from \"", '"'),
        ("import '", '\''),
        ("import \"", '"'),
    ];
    for &(marker, quote) in markers {
        let mut search_from = 0;
        while search_from < line.len() {
            let start = match line[search_from..].find(marker) {
                Some(s) => s,
                None => break,
            };
            let abs_start = search_from + start + marker.len();
            if abs_start >= line.len() {
                break;
            }
            match line[abs_start..].find(quote) {
                Some(end) => {
                    let path = &line[abs_start..abs_start + end];
                    if !path.is_empty() {
                        paths.push(path.to_string());
                    }
                    search_from = abs_start + end + 1;
                }
                None => break,
            }
        }
    }
    paths
}

fn normalize_package_name(raw: &str) -> String {
    if raw.starts_with('.') || raw.starts_with('/') {
        return String::new();
    }
    if raw.starts_with('@') {
        let parts: Vec<&str> = raw.splitn(3, '/').collect();
        return if parts.len() >= 2 {
            format!("{}/{}", parts[0], parts[1])
        } else {
            raw.to_string()
        };
    }
    match raw.find('/') {
        Some(pos) => raw[..pos].to_string(),
        None => raw.to_string(),
    }
}
