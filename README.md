# Packit

A lightweight, self-contained Linux package manager and package builder written in **Rust**. Build, distribute, install, and manage applications independently of traditional distribution repositories.

Packit provides a simple packaging workflow for bundling upstream applications into distributable `.pack` packages.

## Installation

### Build from source

```bash
git clone https://github.com/im-ali-ahmad/packit.git
cd packit

cargo build --release

mkdir -p ~/.local/bin
cp target/release/packit ~/.local/bin/
```

### Install using the installation script

```bash
curl -fsSL "https://raw.githubusercontent.com/im-ali-ahmad/packit/main/install.sh" -o packit-install.sh

chmod +x packit-install.sh
./packit-install.sh

rm -f packit-install.sh
```

Make sure `~/.local/bin` is included in your `PATH` when installing locally.

## Commands

| Command                           | Description                                                           |
| --------------------------------- | --------------------------------------------------------------------- |
| `init-project-local <PATH>`       | Initialize a package manifest from a local project directory.         |
| `init-tar-local <PATH>`           | Initialize a package manifest from a local TAR archive.               |
| `init-tar-remote <URL>`           | Initialize a package manifest from a remote TAR archive (HTTP/HTTPS). |
| `build <MANIFEST_FILE>`           | Build a distributable package from a manifest.                        |
| `build <MANIFEST_FILE> --install` | Build and automatically install the resulting package.                |
| `install <PACKAGE_FILE>`          | Install a locally built package.                                      |
| `install --remote <URL>`          | Download and install a remote package (HTTP/HTTPS).                   |
| `remove <PACKAGE_UID>`            | Uninstall an installed package.                                       |
| `search [PACKAGE_UID]`            | Display package details or list all installed packages.               |
| `list`                            | List all installed packages.                                          |
| `version [--only-numbers]`        | Display the current Packit version.                                   |
| `help`                            | Display the help message.                                             |

## Usage Examples

### Initialize a package

From a local project directory:

```bash
packit init-project-local ./my-project
```

From a local TAR archive:

```bash
packit init-tar-local ./source.tar.gz
```

From a remote TAR archive:

```bash
packit init-tar-remote https://example.com/source.tar.gz
```

### Build a package

```bash
packit build packit.toml
```

Build and install automatically:

```bash
packit build packit.toml --install
```

### Install packages

Install a locally built package:

```bash
packit install ./example.pack
```

Install directly from a remote URL:

```bash
packit install --remote https://example.com/example.pack
```

### Manage installed packages

Display all installed packages:

```bash
packit list
```

Alternatively:

```bash
packit search
```

Display information about a specific package:

```bash
packit search com.example.package
```

Uninstall a package:

```bash
packit remove com.example.package
```

## Manifest Example

The following example demonstrates how to create a package manifest for Microsoft Visual Studio Code lates version for x64. For arm64, replace 'x64' with 'arm64'.

Create `vscode-install.toml`:

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
author = ["microsoft"]
version = "bash($PARENT/VSCode-linux-x64/bin/code --version | head -n 1)"
license = "Proprietary"
```

Build and install:

```bash
packit build vscode-install.toml --install
```

The resulting package will be built and installed automatically.

Verify the installation:

```bash
packit search microsoft.vscode
```

## Quick Workflow

A typical Packit workflow consists of three steps:

```bash
# Initialize a manifest
packit init-tar-remote https://example.com/source.tar.gz

# Build the distributable package
packit build package.toml

# Install the package
packit install package.pack
```

Alternatively, combine building and installation:

```bash
packit build package.toml --install
```

## Package Management

Packit supports managing installed packages through their unique package identifiers (UIDs).

```bash
# List installed packages
packit list

# Search for a specific package
packit search com.example.package

# Remove an installed package
packit remove com.example.package
```

## Version

Display the installed Packit version:

```bash
packit version
```

Display only the version number:

```bash
packit version --only-numbers
```

## License

Packit is open-source software distributed under the [MIT License](LICENSE).
