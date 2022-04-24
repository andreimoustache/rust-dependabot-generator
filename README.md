# rust-dependabot-generator
_Generates a `dependabot` config for a given directory._

Scans a directory for package manager config files, and generates the [`dependabot.yaml`](https://docs.github.com/en/code-security/dependabot/dependabot-version-updates/configuration-options-for-the-dependabot.yml-file).

# Usage

`cargo run ~/workspace/rust-dependabot-generator`

```
Scanning directory ~/rust-dependabot-generator.
Found package managers: cargo.
```
