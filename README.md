# OrbitalOS

Main repository for OrbitalOS, a Linux-first personal desktop environment and
app suite built on Ubuntu LTS.

## Current Direction

OrbitalOS is currently moving from system branding work into application
infrastructure work.

Current milestone:
- Phase 1.1 completed: branding and base-system customization
- Phase 1.2 completed: shared Rust core library
- Phase 1.3 in progress: `Drift` foundations and shared local data layer

## Stack

- Rust for shared core and app logic
- GTK4 + Libadwaita for desktop UI
- SQLite for local structured data
- Filesystem storage for attachments and documents
- PDF export later in the app lifecycle

## Repository Layout

- `orbital-assets/` shared visual assets
- `crates/orbital-core/` shared domain and platform library
- `docs/` project notes and architecture decisions
- `scripts/` setup and bootstrap scripts
- `Backups/` local project snapshots, ignored by git

## Current Features

The current `orbital-core` foundation already includes:

- shared OrbitalOS app identifiers and naming
- XDG-aware paths for data, config, cache, documents, and attachments
- a shared SQLite database bootstrap
- automatic schema migrations
- a first common schema for notes, tags, links, attachments, and change history
- a first `NoteRepository` for create, load, list, save, and archive flows

This is the base that `Drift` and later `Vector`, `Vlink`, and `Prism` will
share so data can stay local and cross-app aware.

## Ubuntu VM Bootstrap

For a fresh Ubuntu LTS development or integration VM, run:

```bash
curl -fsSL https://raw.githubusercontent.com/grvtyai/orbitalos/main/scripts/bootstrap-ubuntu-lts.sh | bash
```

This installs the current OrbitalOS base development dependencies:
- build tools
- GTK4 + Libadwaita development packages
- SQLite development packages
- Rust via `rustup`

If you want to install the toolchain and immediately clone and test the repo on
Ubuntu LTS:

```bash
curl -fsSL https://raw.githubusercontent.com/grvtyai/orbitalos/main/scripts/bootstrap-ubuntu-lts.sh | bash && source "$HOME/.cargo/env" && git clone https://github.com/grvtyai/orbitalos.git ~/orbitalos && cd ~/orbitalos && cargo check -p orbital-core && cargo test -p orbital-core
```

If the repo already exists on the VM, use:

```bash
cd ~/orbitalos && git pull && source "$HOME/.cargo/env" && cargo check -p orbital-core && cargo test -p orbital-core
```

## First Development Goal

Build a stable shared core that can be used by:
- `Drift` (Notes)
- `Vlink` (Snapshot)
- `Prism` (Viewer)

The core library is intentionally UI-agnostic so app crates can reuse the same
types, paths, identifiers, and storage conventions.

## Phase 1 Status

Completed:
- Phase 1.1 system branding on Ubuntu LTS
- Phase 1.2 `orbital-core` workspace foundation
- Phase 1.3 backend groundwork for `Drift`

Next:
- add the first GTK4 + Libadwaita `Drift` app crate
- connect the `Drift` UI to the shared note repository
- build the minimal note list and editor flow
