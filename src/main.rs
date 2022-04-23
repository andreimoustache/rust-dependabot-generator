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

fn main() {
    let args = Cli::parse();
    println!("Scanning directory {}.", args.path.to_str().unwrap());

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

    let mut found: HashSet<String> = HashSet::new();

    for entry in WalkDir::new(args.path)
        .follow_links(true)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| is_target(&mapping, &entry))
    {
        let found_target = entry.file_name().to_str().unwrap();
        let found_for = mapping.get(found_target).unwrap();
        found.insert(found_for.to_string());
    }

    println!(
        "Found package managers: {}.",
        found
            .into_iter()
            .map(String::from)
            .collect::<Vec<String>>()
            .join(", ")
    );
}
