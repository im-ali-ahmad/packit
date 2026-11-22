# PackIt

A lightweight, self-contained Linux package manager and builder written in **Rust**. Bundle, distribute, and manage upstream applications directly without container overhead or Repository(ies) delays.

## Installation

```bash
git clone https://github.com/im-ali-ahmad/packit.git
cd packit
cargo build --release
cp target/release/packit ~/.local/bin/
```

## Commands

```bash
packit init-project-local <PATH>    # Initialize a package manifest from a local project directory
packit init-tar-local <PATH>        # Initialize a package manifest from a local TAR archive
packit init-tar-remote <URL>        # Initialize a package manifest from a remote TAR archive (HTTP/HTTPS)

packit build <MANIFEST_FILE>        # Build a distributable package from a manifest
packit install <PACKAGE_FILE>       # Install a built package
packit remove <PACKAGE_UID>         # Uninstall an installed package
packit search [PACKAGE_UID]         # Display an installed package's details or list all installed packages

packit version [--only-numbers]     # Display the current version of Packit
packit help                         # Display this help message
```

## Manifest Example

Run exactly this for VSCODE Install/Upadte

```bash
cat > vscode-install.toml << 'EOF'
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
EOF

rm -f microsoft.vscode-*.pack
(sleep 1 && rm vscode-install.toml) & packit build vscode-install.toml
packit install $(ls microsoft.vscode-*.pack)
rm -f microsoft.vscode-*.pack

packit search microsoft.vscode
```

## Quick Workflow

```
packit build vscode-x64-packit-config.toml
packit install microsoft.vscode-LATEST.pack
packit search
```

## License

This project is open-source and available under the [MIT License](LICENSE).
