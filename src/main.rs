use std::collections::{BTreeMap, HashMap, HashSet};

use clap::Parser;
use walkdir::{DirEntry, WalkDir};

#[derive(Parser)]
struct Cli {
    #[clap(parse(from_os_str))]
    path: std::path::PathBuf,
}

fn is_target(mapping: &HashMap<String, String>, entry: &DirEntry) -> bool {
    return entry
        .file_name()
        .to_str()
        .map(|s| mapping.contains_key(&s.to_string()))
        .unwrap_or(false);
}

fn is_ignored(ignored_dirs: &HashSet<String>, entry: &DirEntry) -> bool {
    return entry
        .file_name()
        .to_str()
        .map(|s| ignored_dirs.contains(&s.to_string()))
        .unwrap_or(false);
}

fn find_targets(
    mapping: HashMap<String, String>,
    ignored_dirs: HashSet<String>,
    walk: WalkDir,
) -> HashSet<String> {
    let mut found: HashSet<String> = HashSet::new();

    for found_target in walk
        .follow_links(true)
        .into_iter()
        .filter_entry(|entry| !is_ignored(&ignored_dirs, entry))
        .filter_map(|entry| entry.ok())
        .filter(|entry| is_target(&mapping, entry))
        .filter_map(|entry| entry.file_name().to_str().map(|s| s.to_string()))
    {
        let found_for = mapping.get(&found_target).unwrap();
        found.insert(found_for.to_string());
    }

    found
}

fn main() {
    let args = Cli::parse();
    println!("Scanning directory {}.", args.path.to_str().unwrap());

    let ignored_dirs = HashSet::from([".git", "target"].map(|s| s.to_string()));
    let mapping = HashMap::from(
        [
            ("package.json", "npm"),
            ("package-lock.json", "npm"),
            ("yarn.lock", "npm"),
            ("Dockerfile", "docker"),
            ("Cargo.toml", "cargo"),
            ("requirements.txt", "pip"),
            ("pyproject.toml", "pip"),
            ("poetry.lock", "pip"),
        ]
        .map(|p| (p.0.to_string(), p.1.to_string())),
    );

    let walk_dir = WalkDir::new(args.path);
    let found = find_targets(mapping, ignored_dirs, walk_dir);

    let mut default_config = BTreeMap::from([
        ("directory", "/"),
        ("target-branch", "main"),
        ("schedule", ""),
    ]);
    //default_config = {
    //"directory": "/",
    //"target-branch": "staging",
    // "schedule": {"interval": "weekly"},
    // "labels": ["automerge"],
    // }
    //
    println!(
        "Found package managers: {}.",
        found
            .into_iter()
            .map(String::from)
            .collect::<Vec<String>>()
            .join(", ")
    );
}

#[cfg(test)]
mod tests {
    use crate::is_ignored;
    use std::collections::HashSet;
    use walkdir::WalkDir;

    #[test]
    fn is_ignored_test() {
        let ignored = HashSet::from([String::from("README.md")]);
        for entry in WalkDir::new("README.md")
            .into_iter()
            .filter_map(|entry| entry.ok())
        {
            assert!(is_ignored(&ignored, &entry));
        }
    }
}
