# OrbitalOS

Main repository for OrbitalOS, a Linux-first personal desktop environment and
app suite built on Ubuntu LTS.

## Current Direction

OrbitalOS is currently moving from system branding work into application
infrastructure work.

Current milestone:
- Phase 1.1 completed: branding and base-system customization
- Phase 1.2 completed: shared Rust core library
- Phase 1.3 in progress: `Drift` as the first real OrbitalOS app

## Stack

- Rust for shared core and app logic
- GTK4 + Libadwaita for desktop UI
- SQLite for local structured data
- Filesystem storage for attachments and documents
- PDF export later in the app lifecycle

## Repository Layout

- `orbital-assets/` shared visual assets
- `apps/` application crates and app-specific docs
- `crates/orbital-core/` shared domain and platform library
- `docs/` project notes and architecture decisions
- `scripts/` setup and bootstrap scripts
- `Backups/` local project snapshots, ignored by git

## Apps

- [`Drift`](apps/drift/README.md) notes app, currently the primary active app
- [`Blink`](apps/blink/README.md) snapshot app, Phase 1 in progress
- `Prism` viewer app, planned
- `Vector` planner app, planned
- `Relay` mail app, planned
- `Control` settings app, planned
- `Dock` files app, planned

The app directory overview also lives in [`apps/README.md`](apps/README.md).
For app-specific setup, run, and feature details, use the individual app README.

## Current Features

The current `orbital-core` foundation already includes:

- shared OrbitalOS app identifiers and naming
- XDG-aware paths for data, config, cache, documents, and attachments
- a shared SQLite database bootstrap
- automatic schema migrations
- a first common schema for notes, tags, links, attachments, and change history
- a first `NoteRepository` for create, load, list, save, and archive flows

This is the base that `Drift` and later `Vector`, `Blink`, and `Prism` will
share so data can stay local and cross-app aware.

Current `Drift` already includes:

- a local notebook with compact sidebar, page actions, and page reordering
- a block-based canvas editor with text blocks and code blocks
- block drag, resize, duplicate, delete, and right-click menus
- autosave plus global undo/redo across text, blocks, and page actions
- rich-text basics for text blocks, including headings, lists, and checklists
- dedicated app settings with grid density, theme switching, and help links
- local per-app settings persisted in the Drift config directory
- dedicated app documentation in [`apps/drift/README.md`](apps/drift/README.md)

Current `Blink` already includes:

- a dedicated app crate in the workspace
- GTK4 + Libadwaita application bootstrap on top of `orbital-core`
- shared OrbitalOS app identity, path discovery, and database bootstrap
- a first persistent snapshot model and `SnapshotRepository`
- a local snapshot library UI with list and detail view
- manual snapshot creation for UI and storage testing
- image import into the Blink app data directory
- image preview plus file path and MIME metadata in the detail panel
- hover delete action with confirmation dialog

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
- `Blink` (Snapshot)
- `Prism` (Viewer)

The core library is intentionally UI-agnostic so app crates can reuse the same
types, paths, identifiers, and storage conventions.

## Phase 1 Status

Completed:
- Phase 1.1 system branding on Ubuntu LTS
- Phase 1.2 `orbital-core` workspace foundation
- Phase 1.3 shared local storage groundwork for `Drift`
- Phase 1.3 running `Drift` UI with sidebar, toolbar, grid canvas, settings, and multiple block types

Next:
- add more block types like image
- continue stabilizing block behavior and editing quality
- continue Blink Phase 1 with capture-oriented snapshot flows
- start pushing more cross-app conventions into the shared OrbitalOS data model
