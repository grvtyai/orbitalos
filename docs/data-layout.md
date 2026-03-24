# OrbitalOS Data Layout

## Goal

OrbitalOS stores application data locally on the machine and does not depend on
cloud services for normal operation.

The storage model is designed around:

- one shared local SQLite database for structured app data
- XDG-aligned config, cache, and data directories
- filesystem storage for user-facing files, attachments, and exports

This gives OrbitalOS a clean foundation for cross-app data sharing without
duplicating the same content between apps.

## Current Layout

On Ubuntu and other Linux systems, OrbitalOS currently resolves paths like this:

- data root: `~/.local/share/orbitalos/`
- config root: `~/.config/orbitalos/`
- cache root: `~/.cache/orbitalos/`
- documents root: `~/Documents/OrbitalOS/`
- shared database: `~/.local/share/orbitalos/orbital.db`
- attachments: `~/Documents/OrbitalOS/attachments/`

These paths are based on the XDG directory convention and are created by
`orbital-core`.

## Storage Responsibilities

### Shared SQLite Database

The shared SQLite database is the main source of truth for structured data.

This includes:

- notes and note metadata
- planner items later
- tags
- links between entities
- attachment references
- change history

The database is shared so OrbitalOS apps can work with the same local data
model instead of synchronizing copies between separate app silos.

### Filesystem Storage

The filesystem is used for larger or user-facing files.

This includes:

- imported documents
- attachment payloads
- exports such as PDF, Markdown, or HTML
- future media or preview assets

The database stores references and metadata, while the file contents live on
disk.

## Recommended OrbitalOS App Pattern

As the project grows, apps should follow this pattern:

- shared structured data in `~/.local/share/orbitalos/orbital.db`
- app-specific data folders in `~/.local/share/orbitalos/<app>/`
- app-specific config in `~/.config/orbitalos/<app>/`
- app-specific cache in `~/.cache/orbitalos/<app>/`
- user-facing documents in `~/Documents/OrbitalOS/<AppName>/`

Examples:

- `Drift` note exports: `~/Documents/OrbitalOS/Drift/`
- `Blink` captured files: `~/Documents/OrbitalOS/Blink/`
- `Prism` generated previews or cache metadata: database + cache

## Why This Fits OrbitalOS

This layout is a good fit because:

- it survives `git pull` and repo updates
- it keeps code and user data separate
- it supports local-first cross-app integration
- it follows Linux desktop conventions instead of inventing custom paths
- it scales naturally toward Planner, Viewer, Snapshot, and Mail

## Practical Rule

Use this rule of thumb:

- if data is structured and queried often, put it in SQLite
- if data is a file the user may open, export, import, or move, keep it on disk

That split should remain the default for OrbitalOS going forward.
