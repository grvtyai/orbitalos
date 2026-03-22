#!/usr/bin/env bash

set -euo pipefail

if [[ "${EUID}" -eq 0 ]]; then
  echo "Please run this script as a regular user, not as root."
  echo "It uses sudo for system packages and installs rustup into your user home."
  exit 1
fi

if ! command -v sudo >/dev/null 2>&1; then
  echo "sudo is required but was not found."
  exit 1
fi

APT_PACKAGES=(
  build-essential
  pkg-config
  curl
  git
  sqlite3
  libgtk-4-dev
  libadwaita-1-dev
  libsqlite3-dev
)

echo "==> Updating apt package lists"
sudo apt update

echo "==> Installing Ubuntu dependencies for OrbitalOS development"
sudo apt install -y "${APT_PACKAGES[@]}"

if [[ ! -x "${HOME}/.cargo/bin/rustup" ]]; then
  echo "==> Installing Rust with rustup"
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
else
  echo "==> rustup already installed, updating toolchain metadata"
  "${HOME}/.cargo/bin/rustup" self update
fi

source "${HOME}/.cargo/env"

echo "==> Ensuring stable Rust toolchain is installed and active"
rustup toolchain install stable
rustup default stable

echo "==> Tool versions"
rustc --version
cargo --version

echo
echo "OrbitalOS Ubuntu LTS bootstrap complete."
echo "Next step:"
echo "  cargo check"

