# Ubuntu Bootstrap

This repository includes a single bootstrap script for Ubuntu LTS virtual
machines:

- `scripts/bootstrap-ubuntu-lts.sh`

## What It Installs

The script installs the packages currently needed to build OrbitalOS apps:

- `build-essential`
- `pkg-config`
- `curl`
- `git`
- `sqlite3`
- `libgtk-4-dev`
- `libadwaita-1-dev`
- `libsqlite3-dev`

It also installs Rust via `rustup` for the current user and activates the
stable toolchain.

## One-Liner

Run this on an Ubuntu LTS VM as your normal user account:

```bash
curl -fsSL https://raw.githubusercontent.com/grvtyai/orbitalos/main/scripts/bootstrap-ubuntu-lts.sh | bash
```

The script uses `sudo` for system packages and keeps Rust in `~/.cargo`.

## VM Usage

Recommended usage with your current setup:

- Dev VM: run the bootstrap script immediately, then build and test here
- OrbitalOS VM: run the same script only when you are ready to integrate and
  test application builds inside the branded system VM

## After Bootstrap

Inside a cloned repo:

```bash
source "$HOME/.cargo/env"
cargo check
```
