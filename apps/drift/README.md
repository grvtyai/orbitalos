# Drift

`Drift` is the first real OrbitalOS app and serves as the local notes
experience for the platform.

It is being built in Rust with GTK4 and Libadwaita on top of the shared
`orbital-core` library.

## Current Scope

Current `Drift` already includes:

- one local notebook with a page list and page editor
- shared SQLite-backed storage through `orbital-core`
- autosave for normal title and body editing
- a formatting toolbar with core text styling actions
- persistent rich-text basics for the current editor flow
- a block-based editor direction with a movable and resizable text block
- a configurable grid shown on the editor canvas
- a dedicated settings window with sections for:
  - Allgemein
  - Personalisierung
  - Hotkeys
  - Hilfe

## Run Locally

From the repo root on Ubuntu LTS:

```bash
source "$HOME/.cargo/env"
cargo check -p drift
GSK_RENDERER=cairo LIBGL_ALWAYS_SOFTWARE=1 cargo run -p drift
```

If the repo is already cloned and you want the full refresh flow:

```bash
cd ~/orbitalos && git pull && source "$HOME/.cargo/env" && cargo check -p drift && GSK_RENDERER=cairo LIBGL_ALWAYS_SOFTWARE=1 cargo run -p drift
```

## Storage

`Drift` uses the shared OrbitalOS local data layout:

- note data in the shared SQLite database
- local config in the Drift app config directory
- no cloud dependency
- no repo-based user data storage

See also:

- [`../../docs/data-layout.md`](../../docs/data-layout.md)
- [`../../crates/orbital-core`](../../crates/orbital-core)

## Near-Term Direction

The next major steps for `Drift` are:

- stabilize the block editor interaction model
- add additional block types such as image and code
- improve layout controls and editor behavior
- deepen the app-specific settings and keyboard workflow
