# OrbitalOS

Main repository for OrbitalOS, a Linux-first personal desktop environment and
app suite built on Ubuntu LTS.

## Current Direction

OrbitalOS is currently moving from system branding work into application
infrastructure work.

Current milestone:
- Phase 1.1 completed: branding and base-system customization
- Phase 1.2 started: shared Rust core library

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

## First Development Goal

Build a stable shared core that can be used by:
- `Drift` (Notes)
- `Vlink` (Snapshot)
- `Prism` (Viewer)

The core library is intentionally UI-agnostic so app crates can reuse the same
types, paths, identifiers, and storage conventions.
