use clap_verbosity_flag::{InfoLevel, Verbosity};
use log::{debug, error, info, warn};
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    fs::{self},
    path::{Path, MAIN_SEPARATOR},
};

use clap::Parser;
use dependabot_config::v2::{Dependabot, PackageEcosystem, Schedule, Update};
use itertools::Itertools;
use walkdir::{DirEntry, WalkDir};

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(value_parser)]
    path: std::path::PathBuf,

    #[clap(flatten)]
    verbose: Verbosity<InfoLevel>,
}

fn is_target(mapping: &HashMap<String, PackageEcosystem>, entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .is_some_and(|s| mapping.contains_key(&s.to_string()))
}

fn is_ignored(ignored_dirs: &HashSet<String>, entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .is_some_and(|s| ignored_dirs.contains(&s.to_string()))
}

/// Allow grouping of filenames, i.e. npm (package.json and yarn.lock)
#[derive(Clone, Hash, Debug)]
struct FoundTarget {
    ecosystem: PackageEcosystem,
    path: String,
    file_names: BTreeSet<String>,
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
struct EcosystemPath {
    ecosystem: PackageEcosystem,
    path: String,
}

fn find_targets(
    mapping: HashMap<String, PackageEcosystem>,
    ignored_dirs: HashSet<String>,
    walk: WalkDir,
    root: String,
) -> Vec<FoundTarget> {
    let grouped = walk
        .follow_links(true)
        .into_iter()
        .filter_entry(|entry| !is_ignored(&ignored_dirs, entry))
        .filter_map(|entry| entry.ok())
        .filter(|entry| is_target(&mapping, entry))
        .map(|entry| {
            let file_name = entry.file_name().to_str().map(String::from).unwrap();
            let ecosystem = *mapping.get(&file_name).unwrap();
            let path = entry
                .path()
                .to_path_buf()
                .parent()
                .map(|file_path: &Path| {
                    file_path
                        .strip_prefix(&root)
                        .expect("Couldn't strip prefix")
                        .to_str()
                        .map_or(String::from(MAIN_SEPARATOR), String::from)
                })
                .map(|s| {
                    if s.is_empty() {
                        String::from(MAIN_SEPARATOR)
                    } else {
                        s
                    }
                })
                .expect("should resolve parent");
            (EcosystemPath { ecosystem, path }, file_name)
        })
        .into_group_map_by(|a| a.0.clone());
    let mut found = Vec::new();
    for (ecopath, groups) in grouped {
        found.push(FoundTarget {
            ecosystem: ecopath.ecosystem,
            path: ecopath.path,
            file_names: BTreeSet::from_iter(groups.iter().map(|p| p.1.clone())),
        })
    }
    // Sort by ecosystem first, then path. Files are already sorted.
    found.sort_by(|a, b| {
        a.ecosystem
            .to_string()
            .cmp(&b.ecosystem.to_string())
            .then(a.path.cmp(&b.path))
    });
    found
}

fn found_to_update(found_target: &FoundTarget) -> Update {
    Update::new(
        found_target.ecosystem,
        found_target.path.clone(),
        Schedule::new(dependabot_config::v2::Interval::Weekly),
    )
}

fn main() {
    let args = Cli::parse();
    env_logger::Builder::new()
        .filter_level(args.verbose.log_level_filter())
        .init();

    let scanned_root = args.path;
    if !scanned_root.is_dir() {
        warn!("Root path is not a directory");
        return;
    }
    let scanned_directory = &scanned_root.as_os_str().to_str().map(String::from);
    let dependabot_config_file_path = Path::new(&scanned_root)
        .join(".github")
        .join("dependabot.yaml");

    info!("Scanning directory {}.", scanned_directory.clone().unwrap());

    let ignored_dirs = HashSet::from([".git", "target", "node_modules"].map(|s| s.to_string()));
    debug!("Ignoring {:?}", &ignored_dirs);

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
            ("packages.config", PackageEcosystem::Nuget),
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
        info!("Found no targets.");
        std::process::exit(0);
    }

    let managers = found
        .iter()
        .map(|p| p.ecosystem.to_string())
        .unique()
        .collect::<Vec<String>>()
        .join(", ");
    let values = found
        .iter()
        .map(|f: &FoundTarget| {
            format!(
                "{}: /{} ({})",
                f.ecosystem,
                if f.path == MAIN_SEPARATOR.to_string() {
                    String::new()
                } else {
                    f.path.clone()
                },
                f.file_names.iter().join(", ")
            )
        })
        .collect::<Vec<String>>()
        .join("\n");
    info!("Found package managers {managers}:\n{values}");

    let updates = found.iter().map(found_to_update).collect();
    let dependabot_config: Dependabot = Dependabot::new(updates);
    debug!(
        "Writing dependabot config to file {}",
        dependabot_config_file_path.to_str().unwrap()
    );

    if let Some(p) = dependabot_config_file_path.parent() {
        match fs::create_dir_all(p) {
            Ok(it) => it,
            Err(err) => error!("Couldn't create .github directory: {err}"),
        }
    };
    match fs::write(dependabot_config_file_path, dependabot_config.to_string()) {
        Ok(it) => it,
        Err(err) => error!("Couldn't create dependabot.yaml: {err}"),
    };
    info!("Done!");
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
