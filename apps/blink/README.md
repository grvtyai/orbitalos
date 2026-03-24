# Blink

`Blink` is the OrbitalOS snapshot app.

It will sit alongside `Drift` on the same shared OrbitalOS foundation so later
cross-app sync and shared local data flows can evolve without changing the app
shape again.

## Current Status

The current `Blink` crate is a first workspace scaffold.

It already includes:

- a dedicated Rust app crate inside `apps/blink/`
- GTK4 + Libadwaita app bootstrap
- shared OrbitalOS app identity through `orbital-core`
- shared path discovery through `orbital-core`
- a first placeholder window describing the planned snapshot direction

## Direction

`Blink` is intended to become the local snapshot layer for OrbitalOS.

Near-term focus:

- define the first snapshot capture flow
- align storage conventions with `orbital-core`
- keep app structure parallel to `Drift` where shared conventions make sense
- prepare a shape that can later sync cleanly with other OrbitalOS apps

## Install And Run

From the repo root on Ubuntu LTS:

```bash
source "$HOME/.cargo/env"
cargo check -p blink
GSK_RENDERER=cairo LIBGL_ALWAYS_SOFTWARE=1 cargo run -p blink
```

## Storage

`Blink` is being aligned with the shared OrbitalOS local app layout:

- shared OrbitalOS app identifiers
- shared config/data/cache path discovery
- shared database and domain foundations as they expand

See also:

- [`../../README.md`](../../README.md)
- [`../../crates/orbital-core`](../../crates/orbital-core)
