//! Generate `THIRD-PARTY-LICENSES.md` from the locked dependency graph.
//!
//! The file is aggregated: license texts that only differ in copyright lines
//! are stored once, and the copyright lines are listed per crate. Data files
//! that happen to contain a license header (Unicode test tables, Adobe glyph
//! lists, …) are not treated as license files.
//!
//! Run from the repository root:
//!
//! ```text
//! cargo run -p image-view --example gen_third_party_licenses
//! ```

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const TARGETS: &[&str] = &[
    "x86_64-pc-windows-msvc",
    "x86_64-unknown-linux-gnu",
    "aarch64-apple-darwin",
];

struct Package {
    name: String,
    version: String,
    license: String,
    repository: String,
    manifest_path: PathBuf,
    license_file: Option<PathBuf>,
}

struct LicenseGroup {
    label: String,
    text: String,
    crates: BTreeSet<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let metadata = cargo_metadata(&root)?;
    let (packages, workspace, dependencies) = parse_metadata(&metadata)?;
    let reachable = reachable_packages(&workspace, &dependencies);

    let mut third_party: Vec<&Package> = reachable
        .iter()
        .filter(|id| !workspace.contains(*id))
        .filter_map(|id| packages.get(id))
        .collect();
    third_party.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.version.cmp(&b.version)));

    let entries: Vec<(&Package, Vec<PathBuf>)> = third_party
        .iter()
        .map(|package| (*package, license_files(package)))
        .collect();

    let mut groups: BTreeMap<(String, String), LicenseGroup> = BTreeMap::new();
    let mut notices: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut unknown_files = Vec::new();
    let mut skipped_copyleft = Vec::new();

    for (package, files) in &entries {
        let crate_name = format!("{} {}", package.name, package.version);
        for file in files {
            let Ok(bytes) = fs::read(file) else {
                continue;
            };
            let raw = String::from_utf8_lossy(&bytes).replace("\r\n", "\n");
            let raw = raw.trim();
            if raw.is_empty() {
                continue;
            }

            let license = detect_license(raw);
            if is_copyleft(license) && has_permissive_alternative(&package.license) {
                skipped_copyleft.push(format!("{}: {}", crate_name, file.display()));
                continue;
            }
            if license.is_none() {
                unknown_files.push(format!("{}: {}", crate_name, file.display()));
            }

            for line in raw.lines() {
                if let Some(notice) = copyright_notice(line) {
                    notices
                        .entry(crate_name.clone())
                        .or_default()
                        .insert(notice);
                }
            }

            let text = tidy(&remove_copyright_lines(raw));
            if text.is_empty() {
                continue;
            }
            let label = license.unwrap_or("Other").to_owned();
            let group = groups
                .entry((label.clone(), text.clone()))
                .or_insert_with(|| LicenseGroup {
                    label,
                    text,
                    crates: BTreeSet::new(),
                });
            group.crates.insert(crate_name.clone());
        }
    }

    let mut groups: Vec<LicenseGroup> = groups.into_values().collect();
    groups.sort_by(|a, b| {
        a.label
            .cmp(&b.label)
            .then_with(|| a.crates.iter().next().cmp(&b.crates.iter().next()))
            .then_with(|| a.text.cmp(&b.text))
    });

    let mut label_counts: BTreeMap<&str, usize> = BTreeMap::new();
    for group in &groups {
        *label_counts.entry(group.label.as_str()).or_default() += 1;
    }

    let mut out = String::new();
    writeln!(out, "# Third-Party Licenses")?;
    writeln!(out)?;
    writeln!(
        out,
        "image-view is distributed with the third-party Rust crates listed below."
    )?;
    writeln!(out)?;
    writeln!(
        out,
        "This file is generated from `Cargo.lock` for the Windows x64, Linux x64 and macOS arm64 release targets. Do not edit it manually; regenerate with:"
    )?;
    writeln!(out)?;
    writeln!(out, "```powershell")?;
    writeln!(
        out,
        "cargo run -p image-view --example gen_third_party_licenses"
    )?;
    writeln!(out, "```")?;
    writeln!(out)?;
    writeln!(
        out,
        "Where a crate offers a choice of licenses, this notice relies on the permissive option; alternative copyleft texts are intentionally not reproduced."
    )?;

    writeln!(out)?;
    writeln!(out, "## Components")?;
    writeln!(out)?;
    writeln!(
        out,
        "| Package | Version | License | License file | Repository |"
    )?;
    writeln!(out, "| --- | --- | --- | --- | --- |")?;
    for (package, files) in &entries {
        writeln!(
            out,
            "| `{}` | {} | {} | {} | {} |",
            cell(&package.name),
            cell(&package.version),
            cell(&package.license),
            if files.is_empty() {
                "not bundled"
            } else {
                "included below"
            },
            repository_cell(&package.repository),
        )?;
    }

    writeln!(out)?;
    writeln!(out, "## License texts")?;
    writeln!(out)?;
    writeln!(
        out,
        "Copyright lines are collected in the [Copyright notices](#copyright-notices) section so identical license texts can be stored once."
    )?;
    writeln!(out)?;
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    for group in &groups {
        let variant = {
            let entry = seen.entry(group.label.as_str()).or_default();
            *entry += 1;
            *entry
        };
        let total = label_counts[group.label.as_str()];
        if total == 1 {
            writeln!(out, "### {}", group.label)?;
        } else {
            writeln!(out, "### {} ({variant}/{total})", group.label)?;
        }
        writeln!(out)?;
        writeln!(out, "Applies to:")?;
        for crate_name in &group.crates {
            writeln!(out, "- `{crate_name}`")?;
        }
        writeln!(out)?;
        writeln!(out, "~~~~text")?;
        writeln!(out, "{}", group.text)?;
        writeln!(out, "~~~~")?;
        writeln!(out)?;
    }

    writeln!(out, "## Copyright notices")?;
    writeln!(out)?;
    writeln!(
        out,
        "These notices were extracted from the bundled license files listed above."
    )?;
    writeln!(out)?;
    for (crate_name, lines) in &notices {
        writeln!(out, "### `{crate_name}`")?;
        writeln!(out)?;
        for line in lines {
            writeln!(out, "- {line}")?;
        }
        writeln!(out)?;
    }

    let missing: Vec<&Package> = entries
        .iter()
        .filter(|(_, files)| files.is_empty())
        .map(|(package, _)| *package)
        .collect();
    if !missing.is_empty() {
        writeln!(out, "## Packages without a bundled license file")?;
        writeln!(out)?;
        writeln!(
            out,
            "These crates declare a license in their package metadata but do not ship a license file. The declared license texts are included above; see the repository for copyright details."
        )?;
        writeln!(out)?;
        writeln!(out, "| Package | Version | License | Repository |")?;
        writeln!(out, "| --- | --- | --- | --- |")?;
        for package in missing {
            writeln!(
                out,
                "| `{}` | {} | {} | {} |",
                cell(&package.name),
                cell(&package.version),
                cell(&package.license),
                repository_cell(&package.repository),
            )?;
        }
    }

    let path = root.join("THIRD-PARTY-LICENSES.md");
    fs::write(&path, out)?;
    println!(
        "wrote {} ({} crates, {} license texts, {} notice groups, {} without a license file)",
        path.display(),
        third_party.len(),
        groups.len(),
        notices.len(),
        entries.iter().filter(|(_, files)| files.is_empty()).count()
    );
    if !unknown_files.is_empty() {
        println!("notes without a recognized license id:");
        for file in &unknown_files {
            println!("  {file}");
        }
    }
    if !skipped_copyleft.is_empty() {
        println!("skipped copyleft alternatives (permissive option available):");
        for file in &skipped_copyleft {
            println!("  {file}");
        }
    }
    Ok(())
}

fn cargo_metadata(root: &Path) -> Result<Value, Box<dyn std::error::Error>> {
    let mut command = Command::new("cargo");
    command
        .current_dir(root)
        .args(["metadata", "--locked", "--format-version", "1"]);
    for target in TARGETS {
        command.args(["--filter-platform", target]);
    }
    let output = command.output()?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into());
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}

type Metadata = (
    HashMap<String, Package>,
    HashSet<String>,
    HashMap<String, Vec<String>>,
);

fn parse_metadata(metadata: &Value) -> Result<Metadata, Box<dyn std::error::Error>> {
    let package_values = metadata["packages"]
        .as_array()
        .ok_or("cargo metadata has no packages")?;
    let mut packages = HashMap::with_capacity(package_values.len());
    for value in package_values {
        let id = value["id"].as_str().ok_or("package has no id")?.to_owned();
        let manifest_path = PathBuf::from(value["manifest_path"].as_str().unwrap_or_default());
        let directory = manifest_path.parent().unwrap_or_else(|| Path::new("."));
        let license_file = value["license_file"]
            .as_str()
            .filter(|path| !path.is_empty())
            .map(|path| directory.join(path));
        packages.insert(
            id,
            Package {
                name: value["name"].as_str().unwrap_or_default().to_owned(),
                version: value["version"].as_str().unwrap_or_default().to_owned(),
                license: value["license"].as_str().unwrap_or("UNKNOWN").to_owned(),
                repository: value["repository"].as_str().unwrap_or_default().to_owned(),
                manifest_path,
                license_file,
            },
        );
    }

    let workspace = metadata["workspace_members"]
        .as_array()
        .ok_or("cargo metadata has no workspace members")?
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();

    let node_values = metadata["resolve"]["nodes"]
        .as_array()
        .ok_or("cargo metadata has no resolve nodes")?;
    let mut dependencies = HashMap::with_capacity(node_values.len());
    for node in node_values {
        let id = node["id"]
            .as_str()
            .ok_or("resolve node has no id")?
            .to_owned();
        let mut deps = Vec::new();
        for dep in node["deps"].as_array().into_iter().flatten() {
            let Some(package) = dep["pkg"].as_str() else {
                continue;
            };
            let kinds = dep["dep_kinds"].as_array();
            let non_dev = kinds.is_none_or(|kinds| {
                kinds
                    .iter()
                    .any(|kind| kind["kind"].as_str() != Some("dev"))
            });
            if non_dev {
                deps.push(package.to_owned());
            }
        }
        dependencies.insert(id, deps);
    }

    Ok((packages, workspace, dependencies))
}

fn reachable_packages(
    workspace: &HashSet<String>,
    dependencies: &HashMap<String, Vec<String>>,
) -> HashSet<String> {
    let mut seen = HashSet::new();
    let mut stack: Vec<String> = workspace.iter().cloned().collect();
    while let Some(id) = stack.pop() {
        if !seen.insert(id.clone()) {
            continue;
        }
        if let Some(children) = dependencies.get(&id) {
            stack.extend(children.iter().cloned());
        }
    }
    seen
}

/// License files shipped with the crate itself. Nested directories are only
/// searched when the crate root has no license file (for example font packages
/// that keep their licenses under `fonts/`); this avoids picking up licenses of
/// vendored sub-projects and data files.
fn license_files(package: &Package) -> Vec<PathBuf> {
    let Some(directory) = package.manifest_path.parent() else {
        return Vec::new();
    };

    let mut files = top_level_license_files(directory);
    if files.is_empty()
        && let Some(file) = &package.license_file
        && file.is_file()
    {
        files.push(file.clone());
    }
    if files.is_empty() {
        collect_license_files(directory, &mut files);
    }
    files.sort();
    files.dedup();
    files
}

fn top_level_license_files(directory: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(directory) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && is_license_file(path))
        .collect()
}

fn collect_license_files(directory: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if path.is_dir() {
            if matches!(
                name.as_str(),
                "target" | ".git" | "src" | "tests" | "examples" | "benches"
            ) {
                continue;
            }
            collect_license_files(&path, files);
        } else if is_license_file(&path) {
            files.push(path);
        }
    }
}

fn is_license_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let lower = name.to_ascii_lowercase();
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    // Source, markup and font files can be named `copying.rs` or `notice.html`;
    // only actual text license files are relevant.
    if matches!(
        extension.as_str(),
        "rs" | "toml"
            | "json"
            | "lock"
            | "yml"
            | "yaml"
            | "xml"
            | "html"
            | "css"
            | "js"
            | "ts"
            | "c"
            | "h"
            | "cpp"
            | "hpp"
            | "py"
            | "sh"
            | "bat"
            | "ps1"
            | "ttf"
            | "otf"
            | "woff"
            | "woff2"
    ) {
        return false;
    }

    if lower.starts_with("license")
        || lower.starts_with("licence")
        || lower.starts_with("copying")
        || lower.starts_with("notice")
        || lower.starts_with("copyright")
        || lower.starts_with("unlicense")
        || lower == "ofl.txt"
        || lower == "ufl.txt"
    {
        return true;
    }
    if (extension == "txt" || extension == "md") && lower.contains("license") {
        return true;
    }

    // Some font packages ship their license as a plain `.txt` file whose name
    // does not mention "license" (for example Hack-Regular.txt). Only accept
    // small files with a recognizable license text, so Unicode/Adobe data
    // tables are skipped.
    if extension == "txt"
        && let Ok(metadata) = fs::metadata(path)
        && metadata.len() < 64 * 1024
        && let Ok(bytes) = fs::read(path)
    {
        let sample = String::from_utf8_lossy(&bytes[..bytes.len().min(4096)]);
        return detect_license(&sample).is_some();
    }
    false
}

fn detect_license(text: &str) -> Option<&'static str> {
    let lower = text.to_ascii_lowercase();
    // Distinctive names first: font, Unicode, Boost and CDLA texts embed
    // MIT/BSD-style permission paragraphs inside their own terms.
    if lower.contains("sil open font license") {
        Some("OFL-1.1")
    } else if lower.contains("ubuntu font licence") || lower.contains("ubuntu font license") {
        Some("Ubuntu-font-1.0")
    } else if lower.contains("unicode, inc. license agreement")
        || lower.contains("unicode license v3")
    {
        Some("Unicode-3.0")
    } else if lower.contains("boost software license") {
        Some("BSL-1.0")
    } else if lower.contains("community data license agreement") {
        Some("CDLA-Permissive-2.0")
    } else if lower.contains("cc0 1.0 universal") || lower.contains("creative commons legal code") {
        Some("CC0-1.0")
    } else if lower
        .contains("this is free and unencumbered software released into the public domain")
    {
        Some("Unlicense")
    } else if lower.contains("mozilla public license version 2.0") {
        Some("MPL-2.0")
    } else if lower.contains("gnu lesser general public license") {
        Some("LGPL-2.1")
    } else if lower.contains("gnu general public license") {
        Some("GPL-2.0")
    } else if lower.contains("apache license") && lower.contains("version 2.0") {
        Some("Apache-2.0")
    } else if lower.contains("permission is hereby granted, free of charge") {
        Some("MIT")
    } else if lower.contains("redistribution and use in source and binary forms") {
        Some("BSD")
    } else if lower.contains("permission to use, copy, modify, and")
        && lower.contains("distribute this software")
    {
        Some("ISC")
    } else if lower.contains("this software is provided 'as-is'") {
        Some("Zlib")
    } else {
        None
    }
}

fn is_copyleft(license: Option<&str>) -> bool {
    matches!(license, Some("GPL-2.0" | "LGPL-2.1"))
}

/// `Apache-2.0 OR GPL-2.0-only` can be satisfied with the permissive branch, so
/// the GPL text does not need to be redistributed.
fn has_permissive_alternative(expression: &str) -> bool {
    expression
        .split(" OR ")
        .any(|part| !part.to_ascii_uppercase().contains("GPL"))
}

fn copyright_notice(line: &str) -> Option<String> {
    let trimmed = line.trim().trim_start_matches(['#', '*', ';', '/']).trim();
    let lower = trimmed.to_ascii_lowercase();
    if !(lower.starts_with("copyright") || lower.starts_with("(c)") || lower.starts_with('©')) {
        return None;
    }
    // Headings such as "COPYRIGHT AND PERMISSION NOTICE" or CC0's
    // "Copyright and Related Rights" are part of the terms, not notices.
    if !(lower.contains("(c)") || trimmed.contains('©') || has_year(trimmed)) {
        return None;
    }
    if lower.contains("[yyyy]") || lower.contains("[name of") {
        return None;
    }
    Some(trimmed.to_owned())
}

fn has_year(text: &str) -> bool {
    text.as_bytes()
        .windows(4)
        .any(|window| window.iter().all(u8::is_ascii_digit) && matches!(window[0], b'1' | b'2'))
}

fn remove_copyright_lines(text: &str) -> String {
    text.lines()
        .filter(|line| copyright_notice(line).is_none())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Trim trailing whitespace and collapse blank-line runs so texts that differ
/// only in formatting are stored once.
fn tidy(text: &str) -> String {
    let mut out = String::new();
    let mut blank = 0;
    for line in text.lines() {
        let line = line.trim_end();
        if line.trim().is_empty() {
            blank += 1;
            continue;
        }
        if !out.is_empty() {
            out.push('\n');
            if blank > 0 {
                out.push('\n');
            }
        }
        blank = 0;
        out.push_str(line);
    }
    out
}

fn cell(text: &str) -> String {
    text.replace('|', "\\|").replace(['\r', '\n'], " ")
}

fn repository_cell(repository: &str) -> String {
    if repository.is_empty() {
        "-".to_owned()
    } else {
        cell(repository)
    }
}
