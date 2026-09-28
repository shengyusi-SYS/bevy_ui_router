use std::{fs, path::Path};
use toml::Value;

pub const BEVY_VERSION: &str = "0.19.x";
const CRATES_IO: &str = "registry+https://github.com/rust-lang/crates.io-index";

pub fn is_bevy_package(name: &str) -> bool {
    name == "bevy" || name.starts_with("bevy_") && !name.starts_with("bevy_ui_router")
}

fn package_name<'a>(alias: &'a str, dependency: &'a Value) -> &'a str {
    dependency
        .get("package")
        .and_then(Value::as_str)
        .unwrap_or(alias)
}

fn replaced_package(identity: &str) -> &str {
    let (url, fragment) = identity.split_once('#').unwrap_or((identity, ""));
    let package = if !fragment.is_empty() && !fragment.starts_with(|c: char| c.is_ascii_digit()) {
        fragment
    } else if url.contains("://") {
        url.split('?')
            .next()
            .unwrap()
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap()
    } else {
        url
    };
    package.split(['@', ':']).next().unwrap()
}

/// Check source policy without freezing dependency aliases, features or table layout.
pub fn manifest_violations(manifest: &Value) -> Vec<String> {
    fn check_sections(table: &Value, violations: &mut Vec<String>) {
        for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
            let Some(entries) = table.get(section).and_then(Value::as_table) else {
                continue;
            };
            for (alias, dependency) in entries {
                let package = package_name(alias, dependency);
                if !is_bevy_package(package) {
                    continue;
                }
                for field in ["git", "rev", "branch", "path", "registry", "registry-index"] {
                    if dependency.get(field).is_some() {
                        violations
                            .push(format!("{package}: source override `{field}` is forbidden"));
                    }
                }
            }
        }
    }

    let mut violations = Vec::new();
    check_sections(manifest, &mut violations);
    if let Some(workspace) = manifest.get("workspace") {
        check_sections(workspace, &mut violations);
    }
    if let Some(targets) = manifest.get("target").and_then(Value::as_table) {
        for target in targets.values() {
            check_sections(target, &mut violations);
        }
    }
    if let Some(registries) = manifest.get("patch").and_then(Value::as_table) {
        for entries in registries.values().filter_map(Value::as_table) {
            for (alias, dependency) in entries {
                let package = package_name(alias, dependency);
                if is_bevy_package(package) {
                    violations.push(format!("{package}: Bevy patch is forbidden"));
                }
            }
        }
    }
    if let Some(entries) = manifest.get("replace").and_then(Value::as_table) {
        for identity in entries.keys() {
            let package = replaced_package(identity);
            if is_bevy_package(package) {
                violations.push(format!("{package}: Bevy replacement is forbidden"));
            }
        }
    }
    violations
}

/// All Bevy entries are selected by name, never by the version being validated.
pub fn lock_violations(lock: &Value) -> Vec<String> {
    let mut violations = Vec::new();
    for package in lock["package"].as_array().expect("lock package list") {
        let name = package["name"].as_str().expect("package name");
        if !is_bevy_package(name) {
            continue;
        }
        let supported = package
            .get("version")
            .and_then(Value::as_str)
            .and_then(|version| version.strip_prefix("0.19."))
            .is_some_and(|patch| {
                !patch.is_empty() && patch.bytes().all(|byte| byte.is_ascii_digit())
            });
        if !supported {
            violations.push(format!("{name}: must resolve to Bevy {BEVY_VERSION}"));
        }
        if package.get("source").and_then(Value::as_str) != Some(CRATES_IO) {
            violations.push(format!("{name}: must resolve from crates.io"));
        }
    }
    violations
}

/// `base` is two levels above the config file, including CARGO_HOME configs.
pub fn config_violations(config: &Value, base: &Path) -> Vec<String> {
    let mut violations = manifest_violations(config);
    if let Some(paths) = config.get("paths").and_then(Value::as_array) {
        for path in paths {
            let manifest = base
                .join(path.as_str().expect("Cargo override path"))
                .join("Cargo.toml");
            let source = fs::read_to_string(&manifest)
                .unwrap_or_else(|error| panic!("{}: {error}", manifest.display()));
            let package: Value = toml::from_str(&source).expect("override Cargo manifest");
            let name = package["package"]["name"]
                .as_str()
                .expect("override package name");
            if is_bevy_package(name) {
                violations.push(format!("{name}: Cargo paths override is forbidden"));
            }
        }
    }
    let Some(source) = config
        .get("source")
        .and_then(|sources| sources.get("crates-io"))
    else {
        return violations;
    };
    for field in ["replace-with", "directory", "local-registry", "git"] {
        if source.get(field).is_some() {
            violations.push(format!("crates.io source override `{field}` is forbidden"));
        }
    }
    if let Some(registry) = source.get("registry").and_then(Value::as_str) {
        if !matches!(
            registry,
            "https://github.com/rust-lang/crates.io-index" | "sparse+https://index.crates.io/"
        ) {
            violations.push("crates.io registry override is forbidden".to_owned());
        }
    }
    violations
}
