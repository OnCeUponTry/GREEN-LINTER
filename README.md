# green-linter

**Static project structure auditor that detects computational waste and estimates CO2 impact.**

green-linter scans your project files (package.json, Dockerfiles, source code) *without executing anything* and reports measurable waste: heavy dependencies with usage analysis, Docker anti-patterns with size deltas, orphaned Dockerfiles, build artifacts in your repo, and missing lockfiles. Every finding includes concrete numbers -- not opinions.

It then translates that waste into estimated CO2 using peer-reviewed data from [Ember Climate](https://ember-climate.org/) (country-level carbon intensity, CC-BY-4.0) and [Aslan et al. 2018](https://doi.org/10.1111/jiec.12630) (energy per byte).

## Quick Start

```bash
# Install from crates.io
cargo install green-linter

# Scan a project (first run prompts for country, saves to config)
green-linter ./my-project

# Scan with explicit country code (209 countries, ISO alpha-3)
green-linter ./my-project --country PER

# JSON output for CI/CD pipelines
green-linter ./my-project --country USA --json

# Examples: USA, DEU, FRA, GBR, BRA, IND, CHN, JPN, PER
```

### Exit Codes

| Code | Meaning |
|------|---------|
| `0` | No findings (project is clean) or no supported project detected |
| `1` | One or more findings detected |

Use exit codes in CI to fail builds when waste is detected:

```bash
green-linter . --country USA || echo "Waste detected!"
```

### Country Auto-Detection

On first run, green-linter asks for your country code interactively and saves it to `~/.config/green-linter/config.json`. Subsequent runs use the saved value automatically. Override anytime with `--country`.

Resolution chain: `--country` flag > saved config > interactive prompt > WORLD average.

## What It Detects

### Docker (13 patterns across 5 categories)

| Category | # | Pattern | What It Reports |
|----------|---|---------|-----------------|
| **Base Image** | 1 | Heavy base (ubuntu, debian, centos) | Size vs alpine equivalent (e.g. 77MB vs 7MB) |
| **Base Image** | 2 | Non-pinned tag (:latest or missing) | Non-reproducible builds |
| **Base Image** | 3 | No slim/alpine variant | Size delta |
| **Cache** | 4 | COPY . . before install | Line numbers, cache invalidation |
| **Cache** | 5 | ADD where COPY suffices | Line number |
| **Cache** | 6 | apt-get without --no-install-recommends | Estimated extra MB |
| **Layer Bloat** | 7 | apt-get without cleanup | ~30-50MB cache in layer |
| **Layer Bloat** | 8 | Consecutive RUN commands | Layer count |
| **Layer Bloat** | 9 | npm install with devDependencies | devDep count |
| **Structure** | 10 | No multi-stage with build tools | Tool names |
| **Structure** | 11 | Missing .dockerignore | Build context size (reported once per project) |
| **Structure** | 12 | No USER directive | Runs as root |
| **Orphan** | 13 | Dockerfile not referenced by any config | Lists searched files |

#### Multi-Dockerfile Support

green-linter detects all Dockerfile variants in your project root:
- `Dockerfile` (standard)
- `Dockerfile.*` (e.g., `Dockerfile.prod`, `Dockerfile.dev`)
- `*.Dockerfile` (e.g., `backend.Dockerfile`)
- `Containerfile` and `Containerfile.*` (Podman)

Each file is audited independently with all 12 structural patterns.

#### Orphaned Dockerfile Detection (Pattern 13)

green-linter scans your project for config files that reference Dockerfiles:
- **Compose files**: `docker-compose*.yml`, `compose*.yml` -- looks for `dockerfile:` keys and implicit `build:` directives
- **Shell scripts**: `.sh` files -- looks for `docker build -f`, `buildah bud -f`, `podman build -f`
- **Makefiles/Justfiles**: same patterns as shell scripts

Any Dockerfile not referenced by any config file is reported as orphaned. The finding shows exactly which files were searched for transparency.

If no config files exist, the orphan check is skipped entirely (zero false positives).

### Node.js Dependencies (20 heavy packages with usage analysis)

green-linter scans your source code (.js, .ts, .jsx, .tsx, .mjs, .cjs) to determine HOW you use each heavy dependency:

| Verdict | Meaning | Example |
|---------|---------|---------|
| **SAFE** | Imported but only uses lightweight features | `moment` with only `.format()` calls |
| **PARTIAL** | Uses features that pull in heavy sub-modules | `moment` with `.tz()` or `.duration()` |
| **Not analyzed** | Not imported in source (may be unused) | Listed in package.json but not used |

Heavy packages detected with alternatives:

| Heavy Package | Size | Alternative | Savings |
|---------------|------|-------------|---------|
| moment | ~4.4MB | dayjs (2KB) | 4398KB |
| node-sass | ~15MB | sass (5MB) | 10000KB |
| lodash | ~1.5MB | lodash-es or native | 1500KB |
| request | ~800KB | native fetch | 800KB |
| axios | ~400KB | native fetch | 400KB |
| ... and 15 more | | | |

### Build Artifacts

- `node_modules/` committed to repo (with real size in MB)
- `dist/` committed to repo (with real size in MB)

### Missing Lockfile

Detects missing `package-lock.json`, `yarn.lock`, or `pnpm-lock.yaml` -- non-reproducible builds waste CI compute on every run.

## Output Modes

### Terminal (default)

Colored report with category summary, detailed findings, and CO2 chain:

```
Found 18 finding(s):
14 Phantom Dependency | 2 Build Artifact | 1 Heavy Dependency | 1 Lockfile Conflict

1. [N] Heavy Dependency (package.json)
   Heavy dependency: axios (~450KB) -- PARTIAL -- some usage needs attention
   ...
```

### JSON (`--json`)

Machine-readable output for CI/CD integration:

```bash
green-linter . --country PER --json | jq '.summary'
```

```json
{
  "total_findings": 18,
  "total_wasted_bytes": 551704957,
  "total_co2_grams": 8.995,
  "by_category": {
    "Phantom Dependency": 14,
    "Build Artifact": 2,
    "Heavy Dependency": 1,
    "Lockfile Conflict": 1
  }
}
```

## CO2 Estimation

Every finding with measurable bytes gets a CO2 estimate:

```
605.96 MB x 0.06 kWh/GB x 291.77 gCO2/kWh (PER) = 10.3595 gCO2
```

- **Bytes wasted**: measured from real package sizes and image deltas
- **Energy per byte**: 0.06 kWh/GB ([Aslan et al. 2018](https://doi.org/10.1111/jiec.12630))
- **Carbon intensity**: per-country from [Ember Climate 2023](https://ember-climate.org/data/data-tools/data-explorer/) (209 countries, CC-BY-4.0)
- **Default**: World average (483.18 gCO2/kWh)

## Principles

1. **Zero false positives**: Every finding is a measurable fact, not an opinion
2. **Verifiable**: Every number can be independently confirmed
3. **Reproducible**: Same input always produces same output
4. **Offline**: No network required, all data embedded in the binary
5. **Single binary**: No runtime dependencies
6. **Read-only**: Never modifies your project files

## Install

### From crates.io (recommended)

```bash
cargo install green-linter
```

### From source

```bash
git clone https://github.com/OnCeUponTry/green-linter.git
cd green-linter
cargo install --path .
```

### Pre-built binary

Download from [Releases](https://github.com/OnCeUponTry/green-linter/releases). The binary is statically linked -- runs on any Linux x86_64 without dependencies.

## Data Sources

- **Carbon intensity**: [Ember Climate - Yearly Electricity Data 2023](https://ember-climate.org/data/data-tools/data-explorer/) (CC-BY-4.0)
- **Energy per byte**: Aslan, J., Mayers, K., Koomey, J.G. et al. (2018). "Electricity Intensity of Internet Data Transmission: Untangling the Estimates." *Journal of Industrial Ecology*, 22(4), 785-798. [DOI: 10.1111/jiec.12630](https://doi.org/10.1111/jiec.12630)
- **Docker image sizes**: Docker Hub official images (verified April 2026)
- **npm package sizes**: npm registry unpacked sizes (verified April 2026)

## License

GPL-3.0-or-later. See [LICENSE](LICENSE) for the full text.

Copyright (C) 2026 Carlos Enrique Castro Lazaro
