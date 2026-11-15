# StorageScout — Project Plan

A native desktop application, written purely in **Rust** with **egui/eframe**, that scans the
whole computer (every drive, every file, installed and portable programs) and presents the
results in sorted lists, groups and visualisations so the user can understand what is using
disk space, jump to any location, and clean up safely.

---

## 1. Goals

| # | Goal | Notes |
|---|------|-------|
| G1 | Scan all drives (or a chosen subset / a custom folder) | Fast, parallel, cancellable, live progress |
| G2 | Show *what is taking space* | Drive overview, folder tree sorted by size, treemap |
| G3 | Group content | By category (video, audio, images, documents, archives, programs, code, system, other) and by extension |
| G4 | Inventory programs | Installed (Windows registry, Store packages) and *not installed* / portable executables found on disk |
| G5 | Find waste | Largest files, old files, duplicates, empty folders, temp/cache folders, dev build artefacts |
| G6 | Act on results | Open file, show in Explorer, copy path, move to Recycle Bin, permanent delete (confirmed), run uninstaller |
| G7 | Export | CSV / JSON report of scan results |

Non-goals (for now): CI workflows, auto-updates, network shares, real-time file watching.

---

## 2. Technology

| Concern | Crate | Why |
|---------|-------|-----|
| GUI | `eframe` / `egui`, `egui_extras` (tables) | Immediate-mode, pure Rust, cross-platform |
| Parallel walk | `rayon` | Work-stealing recursion over directories |
| Channels / sync | `crossbeam-channel`, std atomics | Progress + results from worker threads |
| Drives | `sysinfo` | Mount points, total/free space, file system, removable flag |
| Installed programs | `winreg` (Windows only) | Uninstall keys (HKLM, HKLM\WOW6432Node, HKCU) + AppX repository |
| Recycle bin | `trash` | Safe, reversible delete |
| Open / reveal | `open` + `explorer /select,` | Launch files, reveal in file manager |
| Hashing (duplicates) | `xxhash-rust` (xxh3) | Pure Rust, very fast |
| Folder picker | `rfd` | Native "choose folder" dialog |
| Export | `serde`, `serde_json`, `csv` | Reports |
| Time | `chrono` | Human dates |

Everything else (size formatting, categorisation, treemap layout, analysis) is hand-written.

---

## 3. Architecture

```
src/
├── main.rs              entry point, window setup
├── app.rs               StorageScoutApp: global state, tab routing, scan lifecycle
├── model/
│   ├── mod.rs
│   ├── tree.rs          FileTree arena (flat Vec<Node>, contiguous children)
│   └── category.rs      extension -> Category mapping
├── scanner/
│   ├── mod.rs           ScanHandle (spawn, progress, cancel, result)
│   └── walker.rs        parallel recursive directory walker
├── analysis/
│   ├── mod.rs           Analysis built once after a scan
│   ├── duplicates.rs    size -> partial hash -> full hash pipeline (background)
│   └── cleanup.rs       known temp/cache/dev-artefact rules
├── system/
│   ├── drives.rs        drive enumeration
│   ├── programs.rs      installed + portable programs
│   └── actions.rs       open, reveal, trash, delete, uninstall
├── ui/
│   ├── mod.rs           shared widgets (size bar, file context menu, confirm dialog)
│   ├── overview.rs      drives + category summary
│   ├── explorer.rs      drill-down folder browser sorted by size
│   ├── treemap.rs       squarified treemap view
│   ├── categories.rs    category / group browser
│   ├── extensions.rs    per-extension statistics
│   ├── files.rs         largest files & old files lists (filterable, multi-select)
│   ├── duplicates.rs    duplicate groups
│   ├── programs.rs      installed + portable programs
│   └── cleanup.rs       cleanup suggestions with batch actions
└── util/
    ├── format.rs        human sizes, dates, counts
    └── export.rs        CSV / JSON export
```

### 3.1 Data model — `FileTree`
* Flat arena `Vec<Node>`; each node has `name`, `parent`, `children: Range<u32>` (children are
  stored contiguously), `size` (recursive for dirs), `file_count`, `modified`, `kind` (file/dir),
  `category`, `deleted` flag.
* Roots = scanned drives/folders. Full path is rebuilt on demand by walking parents.
* After a delete, the node is flagged and its size is subtracted from all ancestors, so every view
  stays consistent without rescanning.

### 3.2 Scanner
* Runs on a background thread; uses rayon to recurse into sub-directories in parallel.
* Uses `symlink_metadata` — symlinks and junctions are **not followed** (avoids loops and double
  counting).
* Shared `Progress` (atomics: files, dirs, bytes, errors, current path) polled by the UI each frame.
* Cancellation via `AtomicBool` checked at every directory.
* Access-denied paths are counted and listed in an "Errors" panel instead of failing the scan.
* Output: recursive temp tree → flattened into the `FileTree` arena (BFS, children sorted by size).

### 3.3 Analysis (after scan)
* Category totals and file counts, extension stats, top-N largest files, old files
  (not modified for N days), empty folders, executables list.
* Duplicates: on demand, background thread: group by size (≥ min size) → hash first 64 KiB →
  full hash. Shows wasted bytes per group.
* Cleanup rules: `%TEMP%`, `C:\Windows\Temp`, browser caches (Chrome, Edge, Firefox, Brave),
  crash dumps, Windows Update download cache, thumbnail cache, package-manager caches
  (npm, pip, cargo, gradle, nuget), dev artefacts (`node_modules`, Rust `target`, `__pycache__`,
  `.gradle`, `build`/`dist` next to a project manifest), old log files.

### 3.4 Programs
* **Installed** (Windows): registry Uninstall keys → name, version, publisher, install date,
  install location, estimated size, uninstall command, source (machine/user/32-bit). Real size
  from the scan tree when the install location was scanned.
* **Store apps**: AppX package repository in HKCU.
* **Portable / not installed**: `.exe` files from the scan that are outside Windows / Program
  Files and not inside any registered install location, grouped by folder.
* Non-Windows fallback: `.desktop` files (Linux) / `/Applications` bundles (macOS).

### 3.5 UI
* Top bar: drive picker (checkboxes), "Scan folder…", Scan / Cancel, live progress, export.
* Left navigation: Overview · Explorer · Treemap · Categories · Extensions · Largest files ·
  Old files · Duplicates · Programs · Cleanup · Errors.
* Every file/folder row has a context menu: Open, Show in Explorer, Copy path,
  Move to Recycle Bin, Delete permanently.
* Lists support search filter, sorting and multi-select with batch actions and a total of the
  selection.
* All destructive actions go through a confirmation dialog showing count and total size;
  Recycle Bin is the default, permanent delete is explicit.

---

## 4. Safety rules
1. Never delete without a confirmation dialog.
2. Default action is *Move to Recycle Bin*.
3. Protected paths (Windows dir, Program Files, system root files, drive roots) are blocked from
   deletion with a clear message.
4. Uninstallers are only launched from the registry's own `UninstallString`, after confirmation.

---

## 5. Milestones

| Milestone | Content | Status |
|-----------|---------|--------|
| M1 | Cargo project, app shell, navigation, drive list | Done |
| M2 | Parallel scanner, progress, cancel, FileTree arena | Done |
| M3 | Explorer view, category & extension groups, largest/old files | Done |
| M4 | Treemap | Done |
| M5 | Programs inventory (installed, Store, portable) | Done |
| M6 | Actions: open / reveal / copy / trash / delete with safety rules | Done |
| M7 | Duplicates finder, cleanup suggestions | Done |
| M8 | Export, polish, README, tests for core logic | Done |

Added along the way: a separate *Build Output* category (`.pdb`, `.rlib`, `.o` …), a *Search*
view with wildcards, a *Selection* view, *Empty folders*, and command-line arguments
(`storagescout <paths…> --view=<name>`).

### Ideas for later
* Measure allocated size on disk (compression, sparse files, hard links counted once).
* Read NTFS MFT directly for even faster scans (needs administrator rights).

## 7. Version 2 (branch `v2`)

Approved by the owner on 2026-11-15. Design: dark navy theme, one selectable accent colour,
Inter / Space Grotesk fonts, line icons, ring charts and gauges, grouped sidebar that collapses
to icons, light theme kept. Screens: dashboard, scanning, sunburst explorer, update prompt
(see `screenshots/`).

### 7.1 Self-update (via GitHub Releases)
1. A release is built, signed and published when `main` gets a new version.
2. The app checks the latest release at start-up, at most once a day (can be turned off).
3. Prompt: what's new, *Install and restart* / *Remind me later* / *Skip this version*.
4. The installer must match `SHA256SUMS.txt` and be signed with thumbprint
   `A7AAB0826FF0EE4B5CC7BC4B78722CE255D0BC2E`, otherwise nothing is installed.
5. Silent install, restart into the new version; settings and history are kept.
6. PRIVACY.md: the only network use is the update check, which sends no personal data.
7. Because the privacy statement promises no network use unless the user asks for it, the first
   start asks once whether to check for updates; nothing is sent before the user agrees.

Implemented in `src/update.rs` (GitHub API, download, SHA-256, Authenticode check pinned to the
thumbprint), `src/ui/update.rs` (dialog and state), `src/ui/settings.rs` (Settings page) and
`installer/storagescout.iss` (`/RELAUNCH=1` starts the new version after a silent install).

### 7.2 Features
| # | Feature | Phase |
|---|---------|-------|
| F1 | New theme, fonts, icons, sidebar, dashboard, scanning screen | 1 |
| F2 | Self-update (7.1) | 2 |
| F3 | Scan history and "what grew" | 3 |
| F4 | Sunburst explorer with details panel | 3 |
| F5 | Quarantine (restore for 30 days) | 4 |
| F6 | Media viewer: photos, videos (with sound), PDF documents inside the app | 4 |
| F7 | Faster rescans (only changed folders) | 5 |
| F8 | Similar photos (perceptual hash) | 5 |
| F9 | Startup apps | 5 |
| F10 | Leftover finder (uninstalled programs) | 5 |
| F11 | Scheduled scans and low-space alerts | 5 |
| F12 | Command palette (Ctrl+K) | 5 |
| F13 | Exclusions | 5 |
| F14 | HTML report export | 5 |
| F15 | Large file alerts | 5 |
| F16 | Stale downloads | 5 |
| F17 | Developer caches (Docker, WSL, Android SDK, Visual Studio, JetBrains) | 6 |
| F18 | Game library view (Steam, Epic, GOG, Xbox) | 6 |
| F19 | Hidden space (shadow copies, restore points, hiberfil, pagefile, WinSxS) | 6 |
| F20 | Empty and tiny-file clutter | 5 |
| F21 | Media details (video length/resolution/codec, photo date/camera) | 6 |
| F22 | Unused language packs and duplicate fonts | 6 |
| F23 | Cleanup preview (dry run) | 4 |
| F24 | Undo history | 4 |
| F25 | Protected paths | 4 |
| F26 | Secure delete (off by default, SSD warning) | 6 |
| F27 | NTFS compression for rarely used folders | 6 |
| F28 | Move a folder to another drive and leave a link | 6 |
| F29 | Archive old files | 6 |
| F30 | Drive health (SMART, temperature, SSD wear) | 7 |
| F31 | Space forecast | 3 |
| F32 | Program last-used dates | 6 |
| F33 | Bulk uninstall with leftover cleanup | 6 |
| F34 | Browser profiles | 6 |
| F35 | Windows optional features | 7 |
| F36 | Cloud folders that could be online-only | 7 |
| F37 | Saved searches and filters | 5 |
| F38 | Tags and notes | 5 |
| F39 | Compare two folders | 6 |
| F40 | Drag and drop a folder to scan | 5 |
| F41 | "Scan with StorageScout" in the Explorer right-click menu (optional at install) | 7 |
| F42 | System tray mode | 7 |
| F43 | Keyboard shortcuts and cheat sheet (`?`) | 5 |
| F44 | Translatable interface (English, Urdu) | 7 |

## 6. Workflow
* Work happens on the `dev` branch; commits are pushed regularly to `origin/dev`.
* `.github/workflows/release.yml` builds on GitHub-hosted Windows runners: unsigned test builds
  for pushes and pull requests, self-signed builds (OpenSSL certificate, signtool) and a draft release for `v*` tags.
* Installer: Inno Setup script in `installer/`. Licence: MIT. Policies: `PRIVACY.md`,
  `SIGNING.md`.
* Build: `cargo run --release` (release mode recommended — scanning is much faster).
