use std::collections::{HashMap, HashSet};

use clap::Parser;
use walkdir::{DirEntry, WalkDir};

#[derive(Parser)]
struct Cli {
    #[clap(parse(from_os_str))]
    path: std::path::PathBuf,
}

fn is_target(mapping: &HashMap<&str, &str>, entry: &DirEntry) -> bool {
    return entry
        .file_name()
        .to_str()
        .map(|s| mapping.contains_key(s))
        .unwrap_or(false);
}

fn is_ignored(ignored_dirs: &HashSet<&str>, entry: &DirEntry) -> bool {
    return entry
        .file_name()
        .to_str()
        .map(|s| ignored_dirs.contains(s))
        .unwrap_or(false);
}

fn find_targets(
    mapping: HashMap<&str, &str>,
    ignored_dirs: HashSet<&str>,
    walk: WalkDir,
) -> HashSet<String> {
    let mut found: HashSet<String> = HashSet::new();

    for entry in walk
        .follow_links(true)
        .into_iter()
        .filter_entry(|entry| !is_ignored(&ignored_dirs, entry))
        .filter_map(|entry| entry.ok())
        .filter(|entry| is_target(&mapping, entry))
    {
        let found_target = entry.file_name().to_str().unwrap();
        let found_for = mapping.get(found_target).unwrap();
        found.insert(found_for.to_string());
    }

    found
}

fn main() {
    let args = Cli::parse();
    println!("Scanning directory {}.", args.path.to_str().unwrap());

    let ignored_dirs = HashSet::from([".git", "target"]);
    let mapping = HashMap::from([
        ("package.json", "npm"),
        ("package-lock.json", "npm"),
        ("yarn.lock", "npm"),
        ("Dockerfile", "docker"),
        ("Cargo.toml", "cargo"),
        ("requirements.txt", "pip"),
        ("pyproject.toml", "pip"),
        ("poetry.lock", "pip"),
    ]);

    let walk_dir = WalkDir::new(args.path);
    let found = find_targets(mapping, ignored_dirs, walk_dir);

    println!(
        "Found package managers: {}.",
        found
            .into_iter()
            .map(String::from)
            .collect::<Vec<String>>()
            .join(", ")
    );
}
