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
- `Vlink` snapshot app, planned
- `Prism` viewer app, planned
- `Vector` planner app, planned
- `Relay` mail app, planned
- `Control` settings app, planned
- `Dock` files app, planned

The app directory overview also lives in [`apps/README.md`](apps/README.md).

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

Current `Drift` features already include:

- local notebook with page list and note editor
- shared SQLite-backed note storage through `orbital-core`
- autosave for title and body editing
- inline formatting basics such as bold, italic, underline, strike, lists, and color
- block-based canvas groundwork with a movable and resizable text block
- configurable grid density through a dedicated settings window
- local per-app settings persisted in the Drift config directory
- app branding in the header with logo and dedicated app documentation

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

To build and run the current `Drift` app on Ubuntu LTS:

```bash
cd ~/orbitalos && git pull && source "$HOME/.cargo/env" && cargo check -p drift && GSK_RENDERER=cairo LIBGL_ALWAYS_SOFTWARE=1 cargo run -p drift
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
- Phase 1.3 shared local storage groundwork for `Drift`
- Phase 1.3 first running `Drift` UI with editor, toolbar, grid canvas, and settings

Next:
- deepen the block editor model in `Drift`
- add more block types like image and code
- continue stabilizing drag, resize, and layout behavior
