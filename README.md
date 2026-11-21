# PackIt

A lightweight, self-contained Linux package manager and builder written in **Rust**. Bundle, distribute, and manage upstream applications directly without container overhead or AUR delays.

## Installation

```bash
git clone https://github.com/im-ali-ahmad/packit.git
cd packit
cargo build --release
cp target/release/packit ~/.local/bin/
```

## Commands

```bash
packit init-project-local <PATH>   Initialize manifest from a local folder
packit init-tar-local <PATH>       Initialize manifest from a local .tar archive
packit init-tar-remote <URL>       Initialize manifest from a remote .tar URL
packit build <MANIFEST_FILE>       Build a distributable .pack package
packit install <PACKAGE_FILE>      Install a built package
packit remove <PACKAGE_UID>        Uninstall a package by UID
packit search <PACKAGE_UID>        Show metadata for an installed package
packit list                        List all installed packages
packit version                     Print current version
packit help                        Show help message
```

## Manifest Example

Save as `vscode-x64-packit-config.toml`:

```toml
type = "TARREMOTE"
path = "https://code.visualstudio.com/sha/download?build=stable&os=linux-x64"

[[commands]]
file = "VSCode-linux-x64/bin/code"
name = "code"

[project]
uid = "microsoft.vscode"
name = "VSCode"
description = "A free, cross-platform code editor."
author = [ "microsoft" ]
version = "LATEST"
license = "Proprietary"
```

## Quick Workflow

```
packit build vscode-x64-packit-config.toml
packit install microsoft.vscode-LATEST.pack
packit list
```

## License

This project is open-source and available under the [MIT License](LICENSE).
