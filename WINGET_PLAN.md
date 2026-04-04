# winget-cli Feature Parity Roadmap

**Windows investigation (winget + Scoop as stand-ins):** see
[docs/WINDOWS_PACKAGE_MANAGER_INVESTIGATION.md](docs/WINDOWS_PACKAGE_MANAGER_INVESTIGATION.md)
for a phased research plan (manifests, shims, sources, spikes) before locking
implementation on Windows.

## Overview

This document maps winget-cli's command surface onto wax, identifies gaps, and
defines a phased implementation plan for bringing wax to full UX parity with the
Windows Package Manager client.

winget-cli organises everything around the concept of **sources** (named package
repositories, e.g. `winget`, `msstore`, or a private REST endpoint) and
**manifests** (YAML files describing a package).  wax is built around Homebrew
formulae and taps.  The mapping is not always 1-to-1, but the user-visible verbs
are close enough that each winget command can be given a natural home in wax.

---

## Winget Command → wax Mapping

| winget command | wax equivalent today | gap? |
|---|---|---|
| `winget install <pkg>` | `wax install <pkg>` | No — covered |
| `winget upgrade [<pkg>]` | `wax upgrade [<pkg>]` | No — covered |
| `winget uninstall <pkg>` | `wax uninstall <pkg>` (aliases: `rm`, `remove`) | No — covered |
| `winget search <query>` | `wax search <query>` (aliases: `s`, `find`) | No — covered |
| `winget list` | `wax list` (aliases: `ls`) | Partial — wax `list` does not show available-upgrade column or filter by source |
| `winget show <pkg>` | `wax info <pkg>` (alias: `show`) | No — covered |
| `winget source add/remove/list/update/reset/export` | `wax tap add/remove/list/update` | Partial — tap covers Git-based sources; no REST source support, no `reset` or `export` subcommands |
| `winget settings` | none | **Gap** — no persistent settings/config command |
| `winget export -o <file>` | `wax bundle dump` | Partial — `bundle dump` writes a Waxfile; no JSON export format compatible with winget |
| `winget import -i <file>` | `wax bundle [--file]` | Partial — reads Waxfile.toml; no winget JSON import |
| `winget hash --file <path>` | none | **Gap** — no built-in file hasher for manifest authoring |
| `winget validate --manifest <path>` | none | **Gap** — no manifest linter/validator |
| `winget features` | none | **Gap** — no feature-flag / experimental-feature listing |
| `winget --info` | `wax doctor` | Partial — doctor repairs issues; no read-only info dump (versions, paths, policies) |
| `winget pin add/remove/list/reset` | `wax pin` / `wax unpin` | Partial — pin/unpin exist; no `pin list` subcommand, no `pin reset` |
| `winget complete` | `wax completions` | No — covered |
| `winget download` | none | **Gap** — no standalone package download-only command |
| `winget repair` | `wax reinstall` | Partial — reinstall covers the use-case; no `--repair` semantics for app-specific repair installers |

---

## What wax Already Has

- `install` — install formulae and casks, with `--user`/`--global`/`--build-from-source`/`--dry-run`
- `upgrade` — upgrade one or all packages, optional `--system` pass-through
- `uninstall` / `reinstall` — remove and re-install, with `--all` and `--dry-run`
- `search` — full-text search across formulae and casks
- `info` (alias `show`) — detailed package metadata
- `list` (alias `ls`) — installed packages
- `outdated` — packages with available updates
- `tap` — multi-source management via Git repos (add, remove, list, update)
- `bundle` — install from Waxfile (`bundle`) and dump to Waxfile (`bundle dump`)
- `lock` / `sync` — lockfile-based environment reproducibility
- `pin` / `unpin` — version pinning
- `doctor` — system health check with optional `--fix`
- `services` — background service management (list, start, stop, restart)
- `system` — OS-level package manager integration with generation/rollback support
- `deps` / `uses` — dependency graph queries
- `audit` — detect deprecated, disabled, or outdated installed packages
- `completions` — shell completion generation

---

## Gaps to Close

1. **`wax source`** — a first-class source management command distinct from `tap`, supporting REST-based private sources (analogous to winget's REST source protocol) and the `reset` and `export` subcommands.
2. **`wax settings`** — a command to view and edit persistent wax configuration (JSON or TOML), open the config file in `$EDITOR`, and print the effective config path.
3. **`wax hash`** — compute SHA-256 (and optionally SHA-512) of a local file; output in a format suitable for pasting into a formula or manifest.
4. **`wax validate`** — lint a Waxfile, formula `.rb`, or future manifest YAML for structural correctness and policy compliance.
5. **`wax features`** — list experimental feature flags with current enabled/disabled state; allow toggling via `wax settings`.
6. **`wax info --info` / `wax --info`** — read-only diagnostic dump (wax version, config paths, log path, active sources, policy overrides) without triggering repairs.
7. **`wax list --upgradable`** — add an `--upgradable` (alias `--updates`) flag to `wax list` that annotates or filters to packages with an available newer version.
8. **`wax list --source <name>`** — filter `list` output by tap/source.
9. **`wax pin list`** — add a `list` subcommand to `pin`/`unpin` to show all pinned packages and their pinned versions.
10. **`wax download`** — download a formula's bottle or cask artifact to a local directory without installing it.
11. **`wax export` / `wax import`** — first-class aliases or subcommands that produce/consume a portable JSON format (compatible with winget's export schema) in addition to the existing Waxfile.toml.

---

## Implementation Plan

### Phase 1 — Discoverability and Diagnostics (low risk, high value)

**Goal:** Bring wax's self-reporting and UX polish to winget parity with no new
external dependencies.

- [ ] 1.1 Add `wax --info` flag (or `wax info --wax`) that prints wax version,
      install prefix, cellar path, tap directory, log path, cache path, and active
      taps.  Implemented in `src/commands/info.rs` or a new
      `src/commands/wax_info.rs`.
- [ ] 1.2 Add `wax features` subcommand.  Add a `[features]` table to the wax
      config file (see 2.1); `wax features` reads and pretty-prints it.  Stub out
      at least three gated features: `rest-sources`, `winget-import`, and
      `parallel-downloads`.
- [ ] 1.3 Add `wax pin list` subcommand in `src/commands/pin.rs` — iterate the
      pins store and print package name + pinned version in a table.
- [ ] 1.4 Add `--upgradable` flag to `wax list` in `src/commands/list.rs`.  Reuse
      the version-comparison logic already in `src/commands/outdated.rs`.

### Phase 2 — Settings and Configuration

**Goal:** Give users a stable, documented configuration surface.

- [ ] 2.1 Define a `WaxConfig` struct (TOML) in a new `src/config.rs`.  Fields:
      `telemetry_enabled`, `progress_style` (`bar`|`spinner`|`none`),
      `default_scope` (`user`|`global`), `parallel_downloads` (u8), and a
      `[features]` table.  Store at `$XDG_CONFIG_HOME/wax/config.toml` (fallback
      `~/.config/wax/config.toml`).
- [ ] 2.2 Add `wax settings` subcommand in `src/commands/settings.rs`:
      - `wax settings` — print current config as pretty-printed TOML to stdout.
      - `wax settings edit` — open the config file in `$EDITOR`.
      - `wax settings get <key>` — print a single value.
      - `wax settings set <key> <value>` — update a single value.
      - `wax settings reset` — overwrite config with defaults.
- [ ] 2.3 Wire `WaxConfig` into the `Cli` initialisation path so downstream
      commands can read `default_scope`, `progress_style`, and `parallel_downloads`.

### Phase 3 — Manifest Tooling (hash and validate)

**Goal:** Support formula and manifest authors, mirroring winget's `hash` and
`validate` commands.

- [ ] 3.1 Add `wax hash` subcommand in `src/commands/hash.rs`:
      - `wax hash --file <path>` — compute SHA-256, print as `sha256: <hex>`.
      - `--sha512` flag for SHA-512.
      - `--url <url>` — download to a temp file and hash (reuse existing download
        helpers from `src/bottle.rs` or `src/install.rs`).
- [ ] 3.2 Add `wax validate` subcommand in `src/commands/validate.rs`:
      - `wax validate --manifest <path>` — parse a Waxfile.toml or formula `.rb`
        and report structural errors.
      - `wax validate --formula <name>` — fetch the formula from the cache and
        validate its fields (version, url, sha256, license).
      - Exit code 0 = valid, 1 = warnings, 2 = errors (matches winget convention).

### Phase 4 — Source Management (REST sources)

**Goal:** Extend the tap system to support winget-style REST-based private sources.

- [ ] 4.1 Add `wax source` as an alias-group over the existing `wax tap` commands,
      exposing the same `add`, `remove`, `list`, `update` subcommands.  This is a
      thin alias layer so users familiar with winget terminology find the right
      command immediately.
- [ ] 4.2 Add `wax source reset [<name>]` — remove all user-added sources and
      restore the default set (analogous to `winget source reset`).
- [ ] 4.3 Add `wax source export` — write the current source list to stdout as JSON
      or TOML for backup/transfer.
- [ ] 4.4 Design and implement a `RestSource` backend in `src/tap.rs` (or a new
      `src/rest_source.rs`).  The REST API contract should be compatible with the
      [winget REST source](https://github.com/microsoft/winget-cli-restsource) spec
      so that private winget repositories can be consumed by wax without a separate
      conversion step.  Gate behind `features.rest-sources`.

### Phase 5 — Export / Import and Download

**Goal:** Portable, cross-tool package lists and offline workflows.

- [ ] 5.1 Add `wax export` top-level command (alias for `wax bundle dump`) that
      writes a JSON file following the winget export schema
      (`PackageIdentifier`, `Version`, `Source` fields per entry).  Accept
      `--output <path>` flag.
- [ ] 5.2 Add `wax import` top-level command (alias for `wax bundle`) that reads
      the JSON export format in addition to Waxfile.toml.  Auto-detect format by
      file extension or leading `{`.
- [ ] 5.3 Add `wax download` subcommand in `src/commands/download.rs`:
      - `wax download <pkg> [--output-dir <dir>]` — download the bottle (or cask
        artifact) to a local directory without installing it.
      - `--skip-sha256` flag for trusted-network scenarios.
      - Useful for air-gapped environments.

### Phase 6 — Polish and Parity Audit

- [ ] 6.1 Update `wax doctor` to also check for config file validity and report
      any unknown or deprecated config keys.
- [ ] 6.2 Add `--source <name>` filter flag to `wax search`, `wax list`, and
      `wax upgrade` so users can target a specific tap/source.
- [ ] 6.3 Ensure `wax audit` covers the same categories as `winget` integrity
      checks: missing files, wrong hash, version mismatch with source.
- [ ] 6.4 Write integration tests for each new command (hash, validate, settings,
      source, export, import, download) following the pattern in existing test
      modules.
- [ ] 6.5 Update `docs/` and the README feature table to reflect the completed
      winget parity surface.

---

## Notes on Platform Scope

winget is Windows-only; wax also targets macOS and Linux.  Command-level parity
in this document helps users moving between platforms.  **If wax on Windows is
positioned to replace winget and Scoop**, the assumptions below need to be
revisited after the investigation in
[docs/WINDOWS_PACKAGE_MANAGER_INVESTIGATION.md](docs/WINDOWS_PACKAGE_MANAGER_INVESTIGATION.md)
(installer types, shims, store/MSIX, bucket JSON vs winget YAML).

Previously this section stated that Windows-specific installer types need not be
replicated; that only holds for a **Unix-first** wax.  A Windows **stand-in**
build should explicitly define which installer families (MSI, EXE, ZIP, MSIX,
etc.) are in scope for v1 versus delegated or out-of-scope.

Cross-platform value *regardless* of Windows depth:

1. Familiar muscle memory for users who switch between Windows and
   macOS/Linux.
2. Shared conventions (REST source protocol, export/import lists) for private
   registries.
3. A consistent `export`/`import` cycle where formats align.
