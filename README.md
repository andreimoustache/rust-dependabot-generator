# rust-dependabot-generator
_Generates a `dependabot` config for a given directory._

![CI](https://github.com/andreimoustache/rust-dependabot-generator/actions/workflows/ci.yaml/badge.svg)

Scans a directory for package manager config files, and generates the [`dependabot.yaml`](https://docs.github.com/en/code-security/dependabot/dependabot-version-updates/configuration-options-for-the-dependabot.yml-file).

**Note** This is a work in progress, and not everything works properly, assumptions are made,
things are hardcoded :ok_hand:, but it provides at least a starting point for a config file.

# Usage

```shell
dependabot-generator .
``

```
Scanning directory .
Found package managers cargo, docker, github-actions:
    cargo: / (Cargo.lock, Cargo.toml)
    docker: / (Dockerfile)
    github-actions: / (ci.yaml, release.yaml)
```

The program will write the dependabot `yaml` file to disk.

# Acknowledgements

This is based on [Taiki Endo](https://github.com/taiki-e)'s great work with
[dependabot-config](https://github.com/taiki-e/dependabot-config) crate; without it,
I wouldn't have been able to get such a complete version working in such little time.
