use std::{
    collections::{HashMap, HashSet},
    fs::{self},
    path::{Path, MAIN_SEPARATOR},
};

use clap::Parser;
use dependabot_config::v2::{Dependabot, PackageEcosystem, Schedule, Update};
use walkdir::{DirEntry, WalkDir};

#[derive(Parser)]
struct Cli {
    #[clap(parse(from_os_str))]
    path: std::path::PathBuf,
}

fn is_target(mapping: &HashMap<String, PackageEcosystem>, entry: &DirEntry) -> bool {
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

#[derive(Clone)]
struct FoundTarget {
    ecosystem: Option<PackageEcosystem>,
    path: Option<String>,
    file_name: Option<String>,
}

fn find_targets(
    mapping: HashMap<String, PackageEcosystem>,
    ignored_dirs: HashSet<String>,
    walk: WalkDir,
    root: String,
) -> Vec<FoundTarget> {
    walk.follow_links(true)
        .into_iter()
        .filter_entry(|entry| !is_ignored(&ignored_dirs, entry))
        .filter_map(|entry| entry.ok())
        .filter(|entry| is_target(&mapping, entry))
        .map(|entry| {
            let file_name = entry.file_name().to_str().map(String::from);
            let ecosystem = Some(*mapping.get(&file_name.clone().unwrap()).unwrap());
            let path = entry.path().to_path_buf().parent().map(|file_path: &Path| {
                file_path
                    .strip_prefix(&root)
                    .expect("Couldn't strip prefix")
                    .to_str()
                    .map_or(String::from(MAIN_SEPARATOR), String::from)
            });

            let path = if path == Some(String::from("")) {
                Some(String::from(MAIN_SEPARATOR))
            } else {
                path
            };

            FoundTarget {
                file_name,
                path,
                ecosystem,
            }
        })
        .collect()
}

fn found_to_update(found_target: &FoundTarget) -> Update {
    Update::new(
        found_target.ecosystem.unwrap().to_owned(),
        found_target.path.as_ref().unwrap().to_string(),
        Schedule::new(dependabot_config::v2::Interval::Weekly),
    )
}

fn main() {
    let args = Cli::parse();
    let scanned_root = args.path;
    let scanned_directory = &scanned_root.as_os_str().to_str().map(String::from);
    let dependabot_config_file_path = Path::new(&scanned_root)
        .join(".github")
        .join("dependabot.yaml");
    println!("Scanning directory {}.", scanned_directory.clone().unwrap());

    let ignored_dirs = HashSet::from([".git", "target", "node_modules"].map(|s| s.to_string()));
    let mapping = HashMap::from(
        [
            ("package.json", PackageEcosystem::Npm),
            ("package-lock.json", PackageEcosystem::Npm),
            ("yarn.lock", PackageEcosystem::Npm),
            ("Dockerfile", PackageEcosystem::Docker),
            ("Cargo.toml", PackageEcosystem::Cargo),
            ("requirements.in", PackageEcosystem::Pip),
            ("requirements.txt", PackageEcosystem::Pip),
            ("pyproject.toml", PackageEcosystem::Pip),
            ("poetry.lock", PackageEcosystem::Pip),
            ("Pipfile", PackageEcosystem::Pip),
            ("Pipfile.lock", PackageEcosystem::Pip),
            ("setup.py", PackageEcosystem::Pip),
            ("Gemfile.lock", PackageEcosystem::Bundler),
            ("Gemfile", PackageEcosystem::Bundler),
            ("composer.json", PackageEcosystem::Composer),
            ("composer.lock", PackageEcosystem::Composer),
            ("mix.exs", PackageEcosystem::Hex),
            ("mix.lock", PackageEcosystem::Hex),
            ("build.gradle", PackageEcosystem::Gradle),
            ("build.gradle.kts", PackageEcosystem::Gradle),
            ("pom.xml", PackageEcosystem::Maven),
            (".terraform.lock.hcl", PackageEcosystem::Terraform),
            // ("pubspec.yaml", PackageEcosystem::Pub),
            ("*.csproj", PackageEcosystem::Nuget), // TODO: make this work
        ]
        .map(|p| (p.0.to_string(), p.1)),
    );

    let walk_dir = WalkDir::new(scanned_root);
    let found = find_targets(
        mapping,
        ignored_dirs,
        walk_dir,
        scanned_directory.clone().unwrap(),
    );

    if found.is_empty() {
        println!("Found no targets.");
        std::process::exit(0);
    }

    println!(
        "Found package managers: {}.",
        found
            .clone()
            .into_iter()
            .map(|f| format!("found {} in {}", f.file_name.unwrap(), f.path.unwrap()))
            .collect::<Vec<String>>()
            .join(", ")
    );

    let updates = found.iter().map(found_to_update).collect();
    let dependabot_config: Dependabot = Dependabot::new(updates);
    println!(
        "Writing dependabot config to file {}",
        dependabot_config_file_path.to_str().unwrap()
    );

    if let Some(p) = dependabot_config_file_path.parent() {
        match fs::create_dir_all(p) {
            Ok(it) => it,
            Err(err) => println!("Couldn't create .github directory: {}", err),
        }
    };
    match fs::write(dependabot_config_file_path, &dependabot_config.to_string()) {
        Ok(it) => it,
        Err(err) => println!("Couldn't create dependabot.yaml: {}", err),
    };
    println!("Done!");
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
