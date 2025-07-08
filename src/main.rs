use clap_verbosity_flag::{InfoLevel, Verbosity};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use log::{debug, error, info, warn};
use std::path::MAIN_SEPARATOR_STR;
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    fs::{self},
    path::{Path, MAIN_SEPARATOR},
    sync::LazyLock,
};

use clap::Parser;
use dependabot_config::v2::{Dependabot, PackageEcosystem, Schedule, Update};
use itertools::Itertools;
use walkdir::{DirEntry, WalkDir};
use PackageEcosystem::{
    Bun, Bundler, Cargo, Composer, Devcontainers, Docker, DockerCompose, DotnetSdk, Elm,
    GithubActions, Gitsubmodule, Gomod, Gradle, Helm, Maven, Mix, Npm, Nuget, Pip, Pub, Swift,
    Terraform, Uv,
};

static ECOSYSTEMS: LazyLock<HashMap<PackageEcosystem, GlobSet>> = LazyLock::new(|| {
    HashMap::from([
        (Bun, patterns_to_globset(&PATTERNS_BUN)),
        (Bundler, patterns_to_globset(&PATTERNS_BUNDLER)),
        (Cargo, patterns_to_globset(&PATTERNS_CARGO)),
        (Composer, patterns_to_globset(&PATTERNS_COMPOSER)),
        (Devcontainers, patterns_to_globset(&PATTERNS_DEVCONTAINERS)),
        (Docker, patterns_to_globset(&PATTERNS_DOCKER)),
        (DockerCompose, patterns_to_globset(&PATTERNS_DOCKER_COMPOSE)),
        (DotnetSdk, patterns_to_globset(&PATTERNS_DOTNET_SDK)),
        (Mix, patterns_to_globset(&PATTERNS_MIX)),
        (Elm, patterns_to_globset(&PATTERNS_ELM)),
        (Gitsubmodule, patterns_to_globset(&PATTERNS_GITSUBMODULES)),
        (GithubActions, patterns_to_globset(&PATTERNS_GITHUBACTIONS)),
        (Gomod, patterns_to_globset(&PATTERNS_GOMOD)),
        (Gradle, patterns_to_globset(&PATTERNS_GRADLE)),
        (Helm, patterns_to_globset(&PATTERNS_HELM)),
        (Maven, patterns_to_globset(&PATTERNS_MAVEN)),
        (Npm, patterns_to_globset(&PATTERNS_NPM)),
        (Nuget, patterns_to_globset(&PATTERNS_NUGET)),
        (Pip, patterns_to_globset(&PATTERNS_PIP)),
        (Pub, patterns_to_globset(&PATTERNS_PUB)),
        (Swift, patterns_to_globset(&PATTERNS_SWIFT)),
        (Terraform, patterns_to_globset(&PATTERNS_TERRAFORM)),
        (Uv, patterns_to_globset(&PATTERNS_UV)),
    ])
});

// Globs are defined in each FileFetcher class:
// https://github.com/dependabot/dependabot-core/blob/HEAD/helm/lib/dependabot/helm/file_fetcher.rb#L9
const PATTERNS_BUN: [&str; 1] = ["bun.lock"];
const PATTERNS_BUNDLER: [&str; 1] = ["Gemfile{,.lock}"];
const PATTERNS_CARGO: [&str; 1] = ["Cargo.{toml,lock}"];
const PATTERNS_COMPOSER: [&str; 1] = ["composer.{json,lock}"];
const PATTERNS_DEVCONTAINERS: [&str; 1] = ["{,.}devcontainer{,-lock}.json"];
const PATTERNS_DOCKER: [&str; 1] = ["*Dockerfile*"];
const PATTERNS_DOCKER_COMPOSE: [&str; 1] = ["*docker-compose*.y{,a}ml"];
const PATTERNS_DOTNET_SDK: [&str; 1] = ["global.json"];
const PATTERNS_ELM: [&str; 1] = ["elm.json"];
const PATTERNS_GITSUBMODULES: [&str; 1] = [".gitmodules"];
const PATTERNS_GITHUBACTIONS: [&str; 1] = [".github/workflows/*.y{,a}ml"]; // path separators!
const PATTERNS_GOMOD: [&str; 1] = ["go.{mod,sum}"];
const PATTERNS_GRADLE: [&str; 3] = [
    "build.gradle{,.kts}",
    "gradle.build", // (older versions)
    "settings.gradle",
];
const PATTERNS_HELM: [&str; 1] = [
    // "*.y{,a}ml", // doesn't really make sense to allow any YAML file
    "Chart.lock",
];
const PATTERNS_MAVEN: [&str; 1] = ["pom.xml"];
const PATTERNS_MIX: [&str; 1] = ["mix.{exs,lock}"]; // Elixir
const PATTERNS_NPM: [&str; 3] = ["package{,-lock}.json", "{deno,yarn}.lock", "pnpm-lock.yaml"];
const PATTERNS_NUGET: [&str; 3] = [
    "*.csproj",
    "project.json",    // (older .NET Core projects)
    "packages.config", // (legacy NuGet projects)
];
const PATTERNS_PIP: [&str; 5] = [
    "pyproject.toml",
    "setup.{cfg,py}",
    "requirements*.{in,txt}",
    "poetry.lock",
    "Pipfile{,.lock}",
];
const PATTERNS_PUB: [&str; 1] = ["pubspec.{yaml,lock}"]; // Dart
const PATTERNS_SWIFT: [&str; 1] = ["Package.{swift,resolved}"];
const PATTERNS_TERRAFORM: [&str; 3] = [
    "{main,variables}.tf",
    "terraform.tfstate",
    ".terraform.lock.hcl", // hidden file!
];
const PATTERNS_UV: [&str; 1] = ["uv.lock"];

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(value_parser)]
    path: std::path::PathBuf,

    #[clap(flatten)]
    verbose: Verbosity<InfoLevel>,
}

fn find_ecosystem<P: AsRef<Path> + ?Sized>(path: &P, root: &String) -> Option<PackageEcosystem> {
    let filename = path.as_ref().file_name()?;
    for (ecosystem, globset) in ECOSYSTEMS.iter() {
        if ecosystem == &GithubActions {
            if matches!(path.as_ref().strip_prefix(root), Ok(sub) if globset.is_match(sub)) {
                return Some(*ecosystem); // .github/workflows/*.y{,a}ml
            }
        } else if globset.is_match(filename) {
            return Some(*ecosystem);
        }
    }
    None
}

fn patterns_to_globset(x: &[&str]) -> GlobSet {
    let mut npm = GlobSetBuilder::new();
    x.iter()
        .map(|p| {
            GlobBuilder::new(p)
                .empty_alternates(true)
                .build()
                .ok()
                .unwrap()
        })
        .for_each(|g| {
            npm.add(g);
        });
    npm.build().unwrap()
}

fn is_ignored(ignored_dirs: &HashSet<String>, entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .is_some_and(|s| ignored_dirs.contains(&s.to_string()))
}

/// Allow grouping of filenames, i.e. npm (package.json and yarn.lock)
#[derive(Clone, Hash, Debug, PartialEq)]
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

fn find_targets(ignored_dirs: HashSet<String>, walk: WalkDir, root: String) -> Vec<FoundTarget> {
    // .github/workflows/*.y{,a}ml has to be stripped to /
    // https://containers.dev/guide/dependabot
    // .devcontainer/{,subfolder/}devcontainer{,-lock}.json should be stripped to /
    let gha_or_empty = |e, p: &str| {
        if p.is_empty()
            || e == GithubActions
            || (e == Devcontainers
                && (p == ".devcontainer"
                    || p.starts_with(&(".devcontainer".to_owned() + MAIN_SEPARATOR_STR))))
        {
            MAIN_SEPARATOR.to_string()
        } else {
            p.to_string()
        }
    };
    let grouped = walk
        .follow_links(true)
        .into_iter()
        .filter_entry(|entry| !is_ignored(&ignored_dirs, entry))
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let ecosystem = find_ecosystem(entry.path(), &root)?; // Skip unknown paths
            let file_name = entry.file_name().to_str().map(String::from)?;
            let path = entry
                .path()
                .to_path_buf()
                .parent()
                .and_then(|file_path: &Path| {
                    file_path
                        .strip_prefix(&root)
                        .expect("Couldn't strip prefix")
                        .to_str()
                        .map(|p| gha_or_empty(ecosystem, p))
                })
                .unwrap_or_else(|| MAIN_SEPARATOR.to_string());
            Some((EcosystemPath { ecosystem, path }, file_name))
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

    info!("Scanning directory {}", scanned_directory.clone().unwrap());

    let ignored_dirs = HashSet::from([".git", "target", "node_modules"].map(|s| s.to_string()));
    debug!("Ignoring {:?}", &ignored_dirs);

    let walk_dir = WalkDir::new(scanned_root);
    let found = find_targets(ignored_dirs, walk_dir, scanned_directory.clone().unwrap());

    if found.is_empty() {
        info!("Found no targets.");
        std::process::exit(0);
    }

    let managers = found_managers(&found);
    let values = found_values(&found);
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

fn found_managers(found: &[FoundTarget]) -> String {
    found
        .iter()
        .map(|p| p.ecosystem.to_string())
        .unique()
        .collect::<Vec<String>>()
        .join(", ")
}

fn found_values(found: &[FoundTarget]) -> String {
    found
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
        .join("\n")
}

#[cfg(test)]
mod tests {
    use crate::{find_ecosystem, find_targets, found_managers, found_values, is_ignored};
    use dependabot_config::v2::PackageEcosystem::{
        Bun, Bundler, Cargo, Composer, Devcontainers, Docker, DockerCompose, DotnetSdk, Elm,
        GithubActions, Gitsubmodule, Gomod, Gradle, Helm, Maven, Mix, Npm, Nuget, Pip, Pub, Swift,
        Terraform, Uv,
    };
    use std::collections::HashSet;
    use std::fs;
    use tempfile::tempdir;
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

    #[test]
    fn deduplication_and_sorting_test() {
        let files = [
            "package.json",
            "package-lock.json",
            "Cargo.toml",
            ".github/workflows/ci.yml",
        ];
        let tmpdir = tempdir().expect("Couldn't create temp dir");
        files.iter().for_each(|f| {
            let path = tmpdir.path().join(f);
            fs::create_dir_all(&path.parent().expect("parent")).expect("created subdirs");
            fs::write(&path, "").expect("Couldn't create file")
        });

        let found = find_targets(
            HashSet::new(),
            WalkDir::new(tmpdir.path()),
            tmpdir.path().to_str().unwrap().to_string(),
        );

        let managers = found_managers(&found);
        let values = found_values(&found);
        assert_eq!(
            (
                "cargo, github-actions, npm",
                "cargo: / (Cargo.toml)\ngithub-actions: / (ci.yml)\nnpm: / (package-lock.json, package.json)"
            ),
            (managers.as_str(), values.as_str())
        );
    }

    #[test]
    fn find_ecosystem_test() {
        #[rustfmt::skip]
        let files = [
            "unknown",
            "a/bun.lock",
            "a/Gemfile", "Gemfile.lock",
            "a/Cargo.toml", "Cargo.lock",
            "a/composer.json", "composer.lock",
            "a/devcontainer.json", ".devcontainer.json", ".devcontainer-lock.json",
            "a/Dockerfile", "Dockerfile.alpine", "local.Dockerfile", "myDockerfile.alpine",
            "a/docker-compose.yml", "mydocker-compose.yml", "docker-compose-prod.yaml",
            "a/global.json",
            "a/elm.json",
            ".gitmodules",
            ".github/workflows/ci.yml",
            "a/go.mod", "go.sum",
            "a/build.gradle", "build.gradle.kts", "gradle.build", "settings.gradle",
            "a/Chart.lock",
            "a/pom.xml",
            "a/mix.exs", "mix.lock",
            "a/package.json", "package-lock.json", "deno.lock", "yarn.lock", "pnpm-lock.yaml",
            "a/b.csproj", "project.json", "packages.config",
            "a/pyproject.toml", "setup.cfg", "setup.py", "requirements.in", "requirements-dev.in", "requirements.txt", "requirements-prod.txt", "poetry.lock", "Pipfile", "Pipfile.lock",
            "a/pubspec.yaml", "pubspec.lock",
            "a/Package.swift", "Package.resolved",
            "a/main.tf", "variables.tf", "terraform.tfstate", ".terraform.lock.hcl",
            "a/uv.lock",
        ];
        #[rustfmt::skip]
        let expected = vec![
            None,
            Some(Bun),
            Some(Bundler), Some(Bundler),
            Some(Cargo), Some(Cargo),
            Some(Composer), Some(Composer),
            Some(Devcontainers), Some(Devcontainers), Some(Devcontainers),
            Some(Docker), Some(Docker), Some(Docker), Some(Docker),
            Some(DockerCompose), Some(DockerCompose), Some(DockerCompose),
            Some(DotnetSdk),
            Some(Elm),
            Some(Gitsubmodule),
            Some(GithubActions),
            Some(Gomod), Some(Gomod),
            Some(Gradle), Some(Gradle), Some(Gradle), Some(Gradle),
            Some(Helm),
            Some(Maven),
            Some(Mix), Some(Mix),
            Some(Npm), Some(Npm), Some(Npm), Some(Npm), Some(Npm),
            Some(Nuget), Some(Nuget), Some(Nuget),
            Some(Pip), Some(Pip), Some(Pip), Some(Pip), Some(Pip), Some(Pip), Some(Pip), Some(Pip), Some(Pip), Some(Pip),
            Some(Pub), Some(Pub),
            Some(Swift), Some(Swift),
            Some(Terraform), Some(Terraform), Some(Terraform), Some(Terraform),
            Some(Uv)
        ];
        let results: Vec<_> = files
            .into_iter()
            .map(|f| find_ecosystem(f, &String::new()))
            .collect();
        assert_eq!(results, expected);
    }
}
