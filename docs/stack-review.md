# OrbitalOS Stack Review

## Short Answer

For a Linux-first desktop OS layer and app suite on Ubuntu LTS, your stack choice
is strong:

- Rust is a good long-term choice for shared app logic and system-facing code
- GTK4 + Libadwaita is the right native UI direction for GNOME-based Ubuntu
- SQLite + filesystem storage is a pragmatic local-first storage model

The main tradeoff is that this is intentionally Linux-native, not a
cross-platform-first stack. For OrbitalOS, that is a feature, not a problem.

## Why Rust Fits OrbitalOS

Rust is a better foundation than Python for the parts of this project that need
to remain stable across multiple apps:

- shared domain types
- local database and storage code
- background services later
- file import/export pipelines
- stronger guarantees around ownership and error handling

Python is still great for build scripts, import tools, migration helpers, and
one-off utilities. But for the core app layer, Rust gives you a safer and more
maintainable base once the project grows beyond a single app.

## Why GTK4 + Libadwaita Fits Ubuntu LTS

GTK4 is the native widget toolkit behind modern GNOME applications, and
Libadwaita builds on top of GTK4 with app structure, adaptive widgets, and
design patterns that match the GNOME ecosystem.

That matters for OrbitalOS because your project is:

- Linux-first
- Ubuntu-based
- desktop-shell-adjacent
- likely to benefit from native dialogs, accessibility, theming, and packaging

If you were optimizing for rapid cross-platform shipping, a web stack would be a
better fit. But for a native Ubuntu experience, GTK4 + Libadwaita is the better
architectural match.

## What This Means in Practice

Recommended rule of thumb:

- keep `orbital-core` free of GTK dependencies
- put UI-specific code in app crates
- share only domain, storage, IDs, paths, and services through the core

That split keeps the core testable and reusable while still letting each app use
Libadwaita naturally.

## Ubuntu LTS Notes

Ubuntu 24.04 LTS provides the necessary native packages for this stack,
including:

- `libgtk-4-dev`
- `libadwaita-1-dev`
- `libsqlite3-dev`
- `pkg-config`
- `build-essential`

Suggested development setup inside Ubuntu:

```bash
sudo apt update
sudo apt install build-essential pkg-config libgtk-4-dev libadwaita-1-dev libsqlite3-dev
curl https://sh.rustup.rs -sSf | sh
```

## Recommendation

Proceed with:

- Rust for all OrbitalOS apps
- GTK4 + Libadwaita for app UI
- SQLite + filesystem storage
- a shared `orbital-core` crate as the first development milestone

This is a sensible and future-proof starting point for the roadmap you outlined.
