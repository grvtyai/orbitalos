# Blink

![Blink Logo](../../orbital-assets/logos/blink_logo.png)

`Blink` is the OrbitalOS snapshot app.

It will sit alongside `Drift` on the same shared OrbitalOS foundation so later
cross-app sync and shared local data flows can evolve without changing the app
shape again.

It is being built in Rust with GTK4 and Libadwaita on top of the shared
`orbital-core` library.

## Current Scope

Current `Blink` already includes:

- a dedicated Rust app crate inside `apps/blink/`
- GTK4 + Libadwaita app bootstrap
- shared OrbitalOS app identity through `orbital-core`
- shared path discovery and shared SQLite bootstrap through `orbital-core`
- a first persistent snapshot model and repository in the shared core
- a local snapshot library with sidebar list and detail panel
- manual `New Snapshot` creation for storage and UI testing
- `Import Image` with file copy into Blink app data storage
- direct screenshot capture into the Blink app data directory
- image preview, file path, and MIME type details
- hover delete action with confirmation dialog

## Product Direction

`Blink` is intended to become the local snapshot layer for OrbitalOS.

The product plan is currently grouped like this:

- basics:
  capture flows, local snapshot library, annotation core, export, hotkeys, and
  delay capture
- next:
  OCR and text extraction, redaction helpers, video snips, search, tags, and
  cross-links into `Drift`
- nice to have:
  smart framing, color picking, QR detection, GIF export, and richer markup
  polish

## Install And Run

`Blink` uses the shared OrbitalOS Ubuntu bootstrap from the repo root:

```bash
curl -fsSL https://raw.githubusercontent.com/grvtyai/orbitalos/main/scripts/bootstrap-ubuntu-lts.sh | bash
```

This bootstrap currently installs the Linux capture tools Blink expects for
simple direct screenshots:

- `grim`
- `slurp`

From the repo root on Ubuntu LTS:

```bash
source "$HOME/.cargo/env"
cargo check -p blink
GSK_RENDERER=cairo LIBGL_ALWAYS_SOFTWARE=1 cargo run -p blink
```

If the repo is already cloned and you want the full refresh flow:

```bash
cd ~/orbitalos && git pull && source "$HOME/.cargo/env" && cargo check -p blink && GSK_RENDERER=cairo LIBGL_ALWAYS_SOFTWARE=1 cargo run -p blink
```

If Blink's Linux capture requirements change, rerun the bootstrap script after
pulling so the Ubuntu VM gets the updated packages as well.

## Storage

`Blink` is being aligned with the shared OrbitalOS local app layout:

- shared OrbitalOS app identifiers
- shared config/data/cache path discovery
- shared database and domain foundations as they expand
- imported image files stored under the Blink app data directory
- a future snapshot model that can stay compatible with `Drift` and later apps

See also:

- [`../../README.md`](../../README.md)
- [`../../crates/orbital-core`](../../crates/orbital-core)

## Phase Plan

The current implementation plan for `Blink` is:

- Phase 1:
  build the stable foundation with screenshot capture, local persistence,
  snapshot listing, metadata, export, annotation basics, and shared data
  conventions for future sync
- Phase 2:
  expand into OCR, text extraction, redaction helpers, video snips, pause and
  resume recording, tagging, search, and first `Drift` integration points
- Phase 3:
  add smart capture tools, color picker, QR detection, GIF export, richer
  polish features, and broader cross-app workflows on top of the stabilized
  shared core

## Near-Term Direction

The next major steps for `Blink` are:

- define the first real screenshot capture flow
- refine snapshot metadata editing beyond the current import and test entry flow
- keep app structure parallel to `Drift` where shared conventions make sense
- prepare the storage and metadata shape for later cross-app sync
