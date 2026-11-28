#!/bin/bash
set -Eeuo pipefail
EXECUTABLE_URL="https://github.com/im-ali-ahmad/packit/releases/latest/download"

ARCH=$(uname -m)
case "$ARCH" in #(
  x86_64|amd64)
    EXECUTABLE_URL="$EXECUTABLE_URL/packit-x86_64-linux"
    ;;          #(
  aarch64|arm64|armv8*)
    EXECUTABLE_URL="$EXECUTABLE_URL/packit-arm64-linux"
    ;;          #(
  armv7*|armhf)
    EXECUTABLE_URL="$EXECUTABLE_URL/packit-armhf-linux"
    ;;          #(
  *)
    echo "Unsupported architecture: $ARCH"
    exit 1
    ;;
esac

README_URL="https://raw.githubusercontent.com/im-ali-ahmad/packit/main/README.md"
LICENSE_URL="https://raw.githubusercontent.com/im-ali-ahmad/packit/main/LICENSE"

mkdir -p tmp-install-packit
cd tmp-install-packit

curl -fL $EXECUTABLE_URL -o packit
curl -fL $README_URL -o README.md
curl -fL $LICENSE_URL -o LICENSE
tar -czvf packit.tar.gz packit README.md LICENSE

chmod +x packit
VERSION=$(./packit version --only-numbers)

cat > packit-config.toml << EOF
type = "TARLOCAL"
path = "packit.tar.gz"

[[commands]]
file = "packit"
name = "packit"

[project]
uid = "packit.cli"
name = "PackIt"
description = "A simple tool to package your project into a distributable format."
author = ["im-ali-ahmad"]
version = "$VERSION"
license = "MIT"
EOF

./packit build packit-config.toml --install

cd -
rm -rf tmp-install-packit
