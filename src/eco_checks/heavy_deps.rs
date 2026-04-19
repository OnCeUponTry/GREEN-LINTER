use crate::eco_checks::{Finding, FindingCategory};
use crate::project_scanner::ProjectInfo;
use std::collections::HashSet;
use std::path::Path;

struct HeavyDep {
    name: &'static str,
    size_kb: u64,
    alternative: &'static str,
    alt_size_kb: u64,
    import_names: &'static [&'static str],
    risky_patterns: &'static [&'static str],
    safe_note: &'static str,
    risky_note: &'static str,
}

const HEAVY_DEPS: &[HeavyDep] = &[
    HeavyDep {
        name: "moment",
        size_kb: 4400,
        alternative: "dayjs",
        alt_size_kb: 2,
        import_names: &["moment"],
        risky_patterns: &[".duration(", ".tz(", ".utcOffset(", "defineLocale("],
        safe_note: "dayjs is API-compatible for formatting/parsing. Drop-in for basic usage.",
        risky_note: "Uses duration/timezone features. dayjs needs plugins: dayjs/plugin/duration, dayjs/plugin/timezone.",
    },
    HeavyDep {
        name: "lodash",
        size_kb: 1500,
        alternative: "native ES6+ or lodash-es",
        alt_size_kb: 0,
        import_names: &["lodash", "_"],
        risky_patterns: &[".cloneDeep(", ".merge(", ".template(", ".memoize(", ".curry(", ".flowRight("],
        safe_note: "map/filter/find/reduce/debounce all have native equivalents.",
        risky_note: "Uses deep utilities (cloneDeep/merge/template) with no native equivalent. Consider lodash-es for tree-shaking.",
    },
    HeavyDep {
        name: "underscore",
        size_kb: 700,
        alternative: "native ES6+",
        alt_size_kb: 0,
        import_names: &["underscore", "_"],
        risky_patterns: &[".template(", ".chain("],
        safe_note: "Most methods have ES6+ equivalents (map, filter, reduce, find).",
        risky_note: "Uses template/chain which need custom replacements.",
    },
    HeavyDep {
        name: "request",
        size_kb: 800,
        alternative: "native fetch",
        alt_size_kb: 0,
        import_names: &["request"],
        risky_patterns: &[".pipe(", ".form(", "request.jar(", ".auth("],
        safe_note: "fetch() is native since Node 18. Basic GET/POST is straightforward.",
        risky_note: "Uses streaming/cookies/auth helpers. Migration needs custom wrappers.",
    },
    HeavyDep {
        name: "axios",
        size_kb: 450,
        alternative: "native fetch",
        alt_size_kb: 0,
        import_names: &["axios"],
        risky_patterns: &[".interceptors", ".create(", "axios.all(", "CancelToken", "cancelToken"],
        safe_note: "fetch() handles basic HTTP. Add response.json() calls.",
        risky_note: "Uses interceptors/custom instances. Needs wrapper to replicate.",
    },
    HeavyDep {
        name: "bluebird",
        size_kb: 560,
        alternative: "native Promise",
        alt_size_kb: 0,
        import_names: &["bluebird"],
        risky_patterns: &["Promise.map(", "Promise.each(", "Promise.reduce(", "Promise.props(", ".spread("],
        safe_note: "Native Promise + Promise.all covers basic async patterns.",
        risky_note: "Uses concurrent utilities (map/each/reduce). Replace with Promise.all + Array methods.",
    },
    HeavyDep {
        name: "express-validator",
        size_kb: 350,
        alternative: "zod",
        alt_size_kb: 13,
        import_names: &["express-validator"],
        risky_patterns: &[".custom(", "oneOf(", "validationResult("],
        safe_note: "zod schemas handle validation with better TypeScript support.",
        risky_note: "Uses custom validators/oneOf. Different validation approach with zod.",
    },
    HeavyDep {
        name: "node-uuid",
        size_kb: 120,
        alternative: "crypto.randomUUID()",
        alt_size_kb: 0,
        import_names: &["node-uuid"],
        risky_patterns: &["v1(", "v3(", "v5("],
        safe_note: "crypto.randomUUID() is drop-in for UUID v4. Node 19+.",
        risky_note: "Uses v1/v3/v5 UUIDs. Keep uuid package for those variants.",
    },
    HeavyDep {
        name: "uuid",
        size_kb: 120,
        alternative: "crypto.randomUUID()",
        alt_size_kb: 0,
        import_names: &["uuid"],
        risky_patterns: &["v1(", "v3(", "v5("],
        safe_note: "crypto.randomUUID() is drop-in for UUID v4. Node 19+.",
        risky_note: "Uses v1/v3/v5 UUIDs. Keep uuid package for those variants.",
    },
    HeavyDep {
        name: "chalk",
        size_kb: 150,
        alternative: "picocolors",
        alt_size_kb: 3,
        import_names: &["chalk"],
        risky_patterns: &[".rgb(", ".hex(", ".bgRgb(", ".keyword(", ".level"],
        safe_note: "picocolors covers bold, dim, red, green, blue, cyan, etc.",
        risky_note: "Uses RGB/hex colors. picocolors only supports named colors.",
    },
    HeavyDep {
        name: "colors",
        size_kb: 80,
        alternative: "picocolors",
        alt_size_kb: 3,
        import_names: &["colors"],
        risky_patterns: &[".rainbow", ".america", ".trap", ".zalgo"],
        safe_note: "picocolors covers basic named colors.",
        risky_note: "Uses fancy styles (rainbow/america). picocolors does not support those.",
    },
    HeavyDep {
        name: "commander",
        size_kb: 250,
        alternative: "util.parseArgs (native Node 18+)",
        alt_size_kb: 0,
        import_names: &["commander", "Command"],
        risky_patterns: &[".command(", ".addCommand(", ".action("],
        safe_note: "parseArgs handles basic flags and positional args.",
        risky_note: "Uses subcommands. parseArgs does not support subcommand routing.",
    },
    HeavyDep {
        name: "yargs",
        size_kb: 400,
        alternative: "util.parseArgs (native Node 18+)",
        alt_size_kb: 0,
        import_names: &["yargs"],
        risky_patterns: &[".command(", ".middleware(", ".completion("],
        safe_note: "parseArgs handles basic flags and positional args.",
        risky_note: "Uses subcommands/middleware. parseArgs does not support those.",
    },
    HeavyDep {
        name: "node-sass",
        size_kb: 15000,
        alternative: "sass (Dart)",
        alt_size_kb: 5000,
        import_names: &["node-sass"],
        risky_patterns: &[],
        safe_note: "sass (Dart) is API-compatible. Drop-in replacement. Pure JS, no native binary.",
        risky_note: "",
    },
    HeavyDep {
        name: "imagemin",
        size_kb: 3500,
        alternative: "sharp",
        alt_size_kb: 1500,
        import_names: &["imagemin"],
        risky_patterns: &["imageminJpegtran", "imageminPngquant", "imageminSvgo", "imageminGifsicle"],
        safe_note: "sharp handles resize, convert, compress with native performance.",
        risky_note: "Uses specific imagemin plugins. sharp has different plugin architecture.",
    },
    HeavyDep {
        name: "jquery",
        size_kb: 290,
        alternative: "native DOM API",
        alt_size_kb: 0,
        import_names: &["jquery", "jQuery"],
        risky_patterns: &[".ajax(", ".animate(", ".delegate(", ".live(", ".deferred("],
        safe_note: "querySelector + fetch + addEventListener replace basic jQuery.",
        risky_note: "Uses AJAX/animation/delegation. Needs fetch + CSS transitions + event delegation.",
    },
    HeavyDep {
        name: "async",
        size_kb: 500,
        alternative: "native async/await",
        alt_size_kb: 0,
        import_names: &["async"],
        risky_patterns: &[".waterfall(", ".auto(", ".retry(", ".cargo(", ".queue("],
        safe_note: "async/await + Promise.all replace series/parallel patterns.",
        risky_note: "Uses waterfall/auto/queue. Needs manual refactoring of control flow.",
    },
    HeavyDep {
        name: "mkdirp",
        size_kb: 40,
        alternative: "fs.mkdirSync({recursive:true})",
        alt_size_kb: 0,
        import_names: &["mkdirp"],
        risky_patterns: &[],
        safe_note: "Drop-in. fs.mkdirSync(path, {recursive: true}). Node 10+.",
        risky_note: "",
    },
    HeavyDep {
        name: "rimraf",
        size_kb: 120,
        alternative: "fs.rmSync({recursive:true})",
        alt_size_kb: 0,
        import_names: &["rimraf"],
        risky_patterns: &[],
        safe_note: "Drop-in. fs.rmSync(path, {recursive: true, force: true}). Node 14+.",
        risky_note: "",
    },
    HeavyDep {
        name: "glob",
        size_kb: 200,
        alternative: "fs.globSync (Node 22+)",
        alt_size_kb: 0,
        import_names: &["glob"],
        risky_patterns: &[".hasMagic(", "Glob(", ".stream("],
        safe_note: "fs.globSync() covers basic patterns. Node 22+.",
        risky_note: "Uses advanced glob features. fs.globSync() may not cover all patterns.",
    },
];

#[derive(Debug)]
enum UsageVerdict {
    Safe,
    Partial { risky_found: Vec<String> },
    NotImported,
    NotAnalyzed,
}

struct UsageAnalysis {
    files_using: Vec<String>,
    verdict: UsageVerdict,
}

fn analyze_usage(dep: &HeavyDep, project: &ProjectInfo) -> UsageAnalysis {
    if project.source_files.is_empty() {
        return UsageAnalysis {
            files_using: vec![],
            verdict: UsageVerdict::NotAnalyzed,
        };
    }

    let mut files_set: HashSet<String> = HashSet::new();
    for name in dep.import_names {
        if let Some(files) = project.import_index.get(*name) {
            for f in files {
                files_set.insert(f.clone());
            }
        }
    }
    let files_using: Vec<String> = files_set.into_iter().collect();

    if files_using.is_empty() {
        return UsageAnalysis {
            files_using: vec![],
            verdict: UsageVerdict::NotImported,
        };
    }

    let mut risky_found = Vec::new();
    for file_path in &files_using {
        if let Some(source) = project.source_files.iter().find(|s| &s.relative_path == file_path) {
            for pattern in dep.risky_patterns {
                if source.content.contains(pattern) && !risky_found.contains(&pattern.to_string()) {
                    risky_found.push(pattern.to_string());
                }
            }
        }
    }

    let verdict = if risky_found.is_empty() {
        UsageVerdict::Safe
    } else {
        UsageVerdict::Partial { risky_found }
    };

    UsageAnalysis {
        files_using,
        verdict,
    }
}

pub fn check(project: &ProjectInfo, findings: &mut Vec<Finding>) {
    let pkg = match &project.package_json {
        Some(p) => p,
        None => return,
    };

    let all_deps: Vec<&(String, String)> = pkg
        .dependencies
        .iter()
        .chain(pkg.dev_dependencies.iter())
        .collect();

    for dep in &all_deps {
        let dep_name = dep.0.as_str();
        for heavy in HEAVY_DEPS {
            if dep_name == heavy.name {
                let delta_kb = heavy.size_kb.saturating_sub(heavy.alt_size_kb);
                let delta_bytes = delta_kb * 1024;

                let analysis = analyze_usage(heavy, project);

                let verdict_label = match &analysis.verdict {
                    UsageVerdict::Safe => "SAFE to replace",
                    UsageVerdict::Partial { .. } => "PARTIAL -- some usage needs attention",
                    UsageVerdict::NotImported => "NOT IMPORTED -- safe to remove",
                    UsageVerdict::NotAnalyzed => "check usage before replacing",
                };

                let note = match &analysis.verdict {
                    UsageVerdict::Safe => heavy.safe_note.to_string(),
                    UsageVerdict::Partial { risky_found } => {
                        format!(
                            "{} Detected: {}",
                            heavy.risky_note,
                            risky_found.join(", ")
                        )
                    }
                    UsageVerdict::NotImported => {
                        "Installed but not imported in any source file. Safe to remove entirely, or replace with lighter alternative.".to_string()
                    }
                    UsageVerdict::NotAnalyzed => {
                        if heavy.risky_patterns.is_empty() {
                            heavy.safe_note.to_string()
                        } else {
                            format!("No .js/.ts files found to analyze. {}", heavy.safe_note)
                        }
                    }
                };

                let files_info = if !analysis.files_using.is_empty() {
                    let max_show = 3;
                    let shown: Vec<&str> = analysis.files_using.iter().take(max_show).map(|s| s.as_str()).collect();
                    let extra = if analysis.files_using.len() > max_show {
                        format!(" +{} more", analysis.files_using.len() - max_show)
                    } else {
                        String::new()
                    };
                    format!(
                        " Used in {} file(s): {}{}.",
                        analysis.files_using.len(),
                        shown.join(", "),
                        extra
                    )
                } else {
                    String::new()
                };

                findings.push(Finding {
                    category: FindingCategory::HeavyDependency,
                    title: format!(
                        "Heavy dependency: {} (~{}KB) -- {}",
                        dep_name, heavy.size_kb, verdict_label
                    ),
                    detail: format!(
                        "Consider: {} (~{}KB). Delta: {}KB.{} {}",
                        heavy.alternative, heavy.alt_size_kb, delta_kb, files_info, note
                    ),
                    wasted_bytes: delta_bytes,
                    file: Some("package.json".into()),
                    line: None,
                });
                break;
            }
        }
    }

    let heavy_dep_names: HashSet<&str> = all_deps.iter()
        .filter(|dep| HEAVY_DEPS.iter().any(|h| h.name == dep.0.as_str()))
        .map(|dep| dep.0.as_str())
        .collect();

    check_phantom_deps(project, findings, &heavy_dep_names);
    check_duplicate_deps(project, findings);
    check_deprecated_deps(project, findings);
}

fn check_phantom_deps(project: &ProjectInfo, findings: &mut Vec<Finding>, heavy_dep_names: &HashSet<&str>) {
    let pkg = match &project.package_json {
        Some(p) => p,
        None => return,
    };
    if project.source_files.is_empty() {
        return;
    }

    let mut imported_names: Vec<&str> = Vec::new();
    let mut phantom_candidates: Vec<&str> = Vec::new();

    for (name, _version) in &pkg.dependencies {
        if name.starts_with("@types/") {
            continue;
        }
        if project.import_index.contains_key(name.as_str()) {
            imported_names.push(name);
        } else {
            phantom_candidates.push(name);
        }
    }

    if phantom_candidates.is_empty() {
        return;
    }

    for (name, _version) in &pkg.dev_dependencies {
        if name.starts_with("@types/") {
            continue;
        }
        if project.import_index.contains_key(name.as_str()) {
            imported_names.push(name);
        }
    }

    let framework_required = build_peer_graph(&project.root, &imported_names);

    for name in phantom_candidates {
        if heavy_dep_names.contains(name) {
            continue;
        }
        if framework_required.contains(name) {
            continue;
        }
        findings.push(Finding {
            category: FindingCategory::NodePhantom,
            title: format!("Phantom dependency: {}", name),
            detail: format!(
                "'{}' is listed in dependencies but not imported in {} source file(s) \
                and not found as a peer/transitive dependency of any imported package. \
                Likely removable. Note: dynamic requires or config-only usage may not be detected.",
                name, project.source_files.len()
            ),
            wasted_bytes: 0,
            file: Some("package.json".into()),
            line: None,
        });
    }
}

fn build_peer_graph(root: &Path, imported_names: &[&str]) -> HashSet<String> {
    let mut required = HashSet::new();
    let node_modules = root.join("node_modules");

    if !node_modules.is_dir() {
        return required;
    }

    for dep_name in imported_names {
        let pkg_path = node_modules.join(dep_name).join("package.json");
        let content = match std::fs::read_to_string(&pkg_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let json: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(serde_json::Value::Object(peers)) = json.get("peerDependencies") {
            for key in peers.keys() {
                required.insert(key.clone());
            }
        }
        if let Some(serde_json::Value::Object(deps)) = json.get("dependencies") {
            for key in deps.keys() {
                required.insert(key.clone());
            }
        }
    }

    required
}

struct DuplicateGroup {
    purpose: &'static str,
    members: &'static [&'static str],
}

const DUPLICATE_GROUPS: &[DuplicateGroup] = &[
    DuplicateGroup { purpose: "utility belt", members: &["lodash", "underscore", "ramda"] },
    DuplicateGroup { purpose: "date handling", members: &["moment", "dayjs", "date-fns", "luxon"] },
    DuplicateGroup { purpose: "HTTP client", members: &["axios", "request", "got", "node-fetch", "superagent", "undici"] },
    DuplicateGroup { purpose: "schema validation", members: &["joi", "yup", "zod", "ajv", "superstruct"] },
    DuplicateGroup { purpose: "CLI argument parsing", members: &["commander", "yargs", "meow", "minimist", "arg"] },
    DuplicateGroup { purpose: "UUID generation", members: &["uuid", "node-uuid", "nanoid", "cuid"] },
    DuplicateGroup { purpose: "terminal colors", members: &["chalk", "colors", "picocolors", "colorette", "kleur", "ansi-colors"] },
];

fn check_duplicate_deps(project: &ProjectInfo, findings: &mut Vec<Finding>) {
    let pkg = match &project.package_json {
        Some(p) => p,
        None => return,
    };

    let all_dep_names: Vec<&str> = pkg.dependencies.iter()
        .chain(pkg.dev_dependencies.iter())
        .map(|(name, _)| name.as_str())
        .collect();

    for group in DUPLICATE_GROUPS {
        let found: Vec<&&str> = all_dep_names.iter()
            .filter(|name| group.members.contains(name))
            .collect();
        if found.len() >= 2 {
            let names: Vec<&str> = found.iter().map(|n| **n).collect();
            findings.push(Finding {
                category: FindingCategory::NodeDuplicate,
                title: format!("Duplicate {} packages: {}", group.purpose, names.join(" + ")),
                detail: format!(
                    "{} packages serve the same purpose ({}): {}. Keep one, remove the rest to reduce node_modules size and maintenance burden.",
                    found.len(), group.purpose, names.join(", ")
                ),
                wasted_bytes: 0,
                file: Some("package.json".into()),
                line: None,
            });
        }
    }
}

struct DeprecatedInfo {
    name: &'static str,
    since: &'static str,
    replacement: &'static str,
    reason: &'static str,
}

const DEPRECATED_DEPS: &[DeprecatedInfo] = &[
    DeprecatedInfo { name: "request", since: "2020-02", replacement: "native fetch (Node 18+) or undici", reason: "Fully deprecated, no security patches" },
    DeprecatedInfo { name: "node-uuid", since: "2016", replacement: "uuid", reason: "Renamed to uuid" },
    DeprecatedInfo { name: "nomnom", since: "2015", replacement: "commander or yargs", reason: "Abandoned, no maintainer" },
    DeprecatedInfo { name: "jade", since: "2016", replacement: "pug", reason: "Renamed due to trademark" },
    DeprecatedInfo { name: "istanbul", since: "2018", replacement: "nyc or c8", reason: "Superseded by nyc/c8" },
    DeprecatedInfo { name: "tslint", since: "2019", replacement: "@typescript-eslint/eslint-plugin", reason: "Deprecated in favor of ESLint" },
    DeprecatedInfo { name: "left-pad", since: "2016", replacement: "String.prototype.padStart()", reason: "Native method since ES2017" },
    DeprecatedInfo { name: "colors", since: "2022-01", replacement: "picocolors or chalk", reason: "Maintainer injected malicious code, abandoned" },
    DeprecatedInfo { name: "node-sass", since: "2020-10", replacement: "sass (Dart Sass)", reason: "LibSass deprecated, no new features" },
    DeprecatedInfo { name: "querystring", since: "2019", replacement: "URLSearchParams (native)", reason: "Node.js legacy, native alternative available" },
];

fn check_deprecated_deps(project: &ProjectInfo, findings: &mut Vec<Finding>) {
    let pkg = match &project.package_json {
        Some(p) => p,
        None => return,
    };

    let all_deps: Vec<&str> = pkg.dependencies.iter()
        .chain(pkg.dev_dependencies.iter())
        .map(|(name, _)| name.as_str())
        .collect();

    for deprecated in DEPRECATED_DEPS {
        if all_deps.contains(&deprecated.name) {
            findings.push(Finding {
                category: FindingCategory::NodeDeprecated,
                title: format!("Deprecated package: {} (since {})", deprecated.name, deprecated.since),
                detail: format!(
                    "'{}' is deprecated: {}. Replace with: {}.",
                    deprecated.name, deprecated.reason, deprecated.replacement
                ),
                wasted_bytes: 0,
                file: Some("package.json".into()),
                line: None,
            });
        }
    }
}
