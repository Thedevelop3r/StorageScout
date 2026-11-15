//! Cleanup suggestions: temp folders, caches, dumps, developer build artefacts.

use std::path::PathBuf;

use crate::model::{FileTree, NodeId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Risk {
    /// Regenerated automatically, safe to remove.
    Safe,
    /// Usually safe, but check before removing.
    Review,
}

impl Risk {
    pub fn label(self) -> &'static str {
        match self {
            Risk::Safe => "Safe",
            Risk::Review => "Review",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleanKind {
    /// Delete what is inside the folder, keep the folder.
    Contents,
    /// Delete the targets themselves.
    Whole,
}

#[derive(Debug, Clone)]
pub struct CleanupItem {
    pub group: &'static str,
    pub title: String,
    pub description: &'static str,
    pub risk: Risk,
    pub kind: CleanKind,
    pub targets: Vec<NodeId>,
    pub size: u64,
}

fn env_path(var: &str) -> Option<PathBuf> {
    std::env::var_os(var).map(PathBuf::from)
}

/// Build all cleanup suggestions for a scanned tree.
pub fn suggestions(tree: &FileTree) -> Vec<CleanupItem> {
    let mut items: Vec<CleanupItem> = Vec::new();
    let mut add = |group, title: &str, description, risk, kind, targets: Vec<NodeId>| {
        let mut targets = targets;
        targets.sort_unstable();
        targets.dedup();
        let size: u64 = targets.iter().map(|&t| tree.get(t).size).sum();
        if size > 0 {
            items.push(CleanupItem { group, title: title.to_owned(), description, risk, kind, targets, size });
        }
    };
    let find = |p: Option<PathBuf>| p.and_then(|p| tree.find_path(&p)).filter(|&id| !tree.is_gone(id));
    let find_all = |paths: Vec<Option<PathBuf>>| -> Vec<NodeId> { paths.into_iter().filter_map(find).collect() };

    // ----- Windows / system -------------------------------------------------------------
    #[cfg(windows)]
    {
        let sysroot = env_path("SystemRoot").unwrap_or_else(|| PathBuf::from("C:\\Windows"));
        let local = env_path("LOCALAPPDATA");
        let program_data = env_path("ProgramData");

        add("System", "User temporary files",
            "Files programs left in your %TEMP% folder. Files in use are skipped.",
            Risk::Safe, CleanKind::Contents, find_all(vec![env_path("TEMP"), env_path("TMP")]));
        add("System", "Windows temporary files",
            "System wide temp folder (C:\\Windows\\Temp). Needs administrator rights for some files.",
            Risk::Safe, CleanKind::Contents, find_all(vec![Some(sysroot.join("Temp"))]));
        add("System", "Windows Update download cache",
            "Already installed update packages. Windows downloads them again if needed.",
            Risk::Safe, CleanKind::Contents, find_all(vec![Some(sysroot.join("SoftwareDistribution").join("Download"))]));
        add("System", "Delivery Optimization cache",
            "Update files shared with other PCs.",
            Risk::Safe, CleanKind::Contents, find_all(vec![Some(sysroot.join(
                r"ServiceProfiles\NetworkService\AppData\Local\Microsoft\Windows\DeliveryOptimization\Cache"))]));
        add("System", "Crash dumps",
            "Memory dumps written when programs or Windows crashed.",
            Risk::Safe, CleanKind::Contents, find_all(vec![
                local.as_ref().map(|l| l.join("CrashDumps")),
                Some(sysroot.join("Minidump")),
                Some(sysroot.join("LiveKernelReports")),
            ]));
        add("System", "Windows memory dump",
            "Full memory dump from the last system crash (MEMORY.DMP).",
            Risk::Safe, CleanKind::Whole, find_all(vec![Some(sysroot.join("MEMORY.DMP"))]));
        add("System", "Windows Error Reporting",
            "Archived and queued error reports.",
            Risk::Safe, CleanKind::Contents, find_all(vec![
                program_data.as_ref().map(|p| p.join(r"Microsoft\Windows\WER\ReportArchive")),
                program_data.as_ref().map(|p| p.join(r"Microsoft\Windows\WER\ReportQueue")),
                local.as_ref().map(|l| l.join(r"Microsoft\Windows\WER")),
            ]));
        // Thumbnail cache files
        if let Some(explorer) = find(local.as_ref().map(|l| l.join(r"Microsoft\Windows\Explorer"))) {
            let thumbs: Vec<NodeId> = tree
                .children(explorer)
                .filter(|&c| {
                    let n = tree.get(c);
                    !n.is_dir && (n.name.starts_with("thumbcache_") || n.name.starts_with("iconcache_"))
                })
                .collect();
            add("System", "Thumbnail & icon cache",
                "Picture previews; Windows rebuilds them when needed. Files in use are skipped.",
                Risk::Safe, CleanKind::Whole, thumbs);
        }
        // Windows.old on every root
        let olds: Vec<NodeId> = tree
            .roots
            .iter()
            .filter_map(|&r| tree.children(r).find(|&c| tree.get(c).name.eq_ignore_ascii_case("Windows.old")))
            .collect();
        add("System", "Previous Windows installation (Windows.old)",
            "Lets you roll back a Windows upgrade. Remove only if you do not plan to go back. Needs administrator rights.",
            Risk::Review, CleanKind::Whole, olds);
        // Recycle bins
        let bins: Vec<NodeId> = tree
            .roots
            .iter()
            .filter_map(|&r| tree.children(r).find(|&c| tree.get(c).name.eq_ignore_ascii_case("$Recycle.Bin")))
            .collect();
        add("System", "Recycle Bin",
            "Files you already deleted. Cleaning permanently removes them.",
            Risk::Review, CleanKind::Contents, bins);

        // ----- Browsers ----------------------------------------------------------------
        if let Some(local) = &local {
            let chromium = [
                ("Google Chrome", local.join(r"Google\Chrome\User Data")),
                ("Microsoft Edge", local.join(r"Microsoft\Edge\User Data")),
                ("Brave", local.join(r"BraveSoftware\Brave-Browser\User Data")),
                ("Vivaldi", local.join(r"Vivaldi\User Data")),
                ("Opera", local.join(r"Opera Software\Opera Stable")),
            ];
            for (browser, user_data) in chromium {
                let Some(ud) = find(Some(user_data)) else { continue };
                let mut targets = Vec::new();
                let mut profiles: Vec<NodeId> = tree.children(ud).filter(|&c| tree.get(c).is_dir).collect();
                profiles.push(ud);
                for prof in profiles {
                    for c in tree.children(prof) {
                        let n = tree.get(c);
                        if n.is_dir && matches!(&*n.name, "Cache" | "Code Cache" | "GPUCache" | "Service Worker" | "ShaderCache" | "GrShaderCache" | "DawnCache") {
                            if &*n.name == "Service Worker" {
                                if let Some(cs) = tree.children(c).find(|&x| &*tree.get(x).name == "CacheStorage") {
                                    targets.push(cs);
                                }
                            } else {
                                targets.push(c);
                            }
                        }
                    }
                }
                add("Browsers", &format!("{browser} cache"),
                    "Cached web content. Close the browser first; sites load a bit slower the first time.",
                    Risk::Safe, CleanKind::Contents, targets);
            }
            if let Some(profiles) = find(Some(local.join(r"Mozilla\Firefox\Profiles"))) {
                let targets: Vec<NodeId> = tree
                    .children(profiles)
                    .filter_map(|p| tree.children(p).find(|&c| &*tree.get(c).name == "cache2"))
                    .collect();
                add("Browsers", "Firefox cache",
                    "Cached web content. Close Firefox first.",
                    Risk::Safe, CleanKind::Contents, targets);
            }
        }
    }

    // ----- Package manager caches (all platforms) -------------------------------------------
    let home = env_path("USERPROFILE").or_else(|| env_path("HOME"));
    let local = env_path("LOCALAPPDATA");
    let appdata = env_path("APPDATA");
    let j = |base: &Option<PathBuf>, rel: &str| base.as_ref().map(|b| b.join(rel));
    let pkg_caches: Vec<(&str, Vec<Option<PathBuf>>, Risk)> = vec![
        ("npm cache", vec![j(&local, "npm-cache"), j(&appdata, "npm-cache"), j(&home, ".npm/_cacache")], Risk::Safe),
        ("Yarn cache", vec![j(&local, "Yarn/Cache"), j(&home, ".cache/yarn")], Risk::Safe),
        ("pnpm store", vec![j(&local, "pnpm/store"), j(&home, ".local/share/pnpm/store")], Risk::Safe),
        ("pip cache", vec![j(&local, "pip/Cache"), j(&home, ".cache/pip")], Risk::Safe),
        ("Cargo download cache", vec![j(&home, ".cargo/registry/cache"), j(&home, ".cargo/git/db")], Risk::Safe),
        ("Gradle caches", vec![j(&home, ".gradle/caches")], Risk::Safe),
        ("NuGet packages", vec![j(&home, ".nuget/packages")], Risk::Review),
        ("Maven repository", vec![j(&home, ".m2/repository")], Risk::Review),
        ("Go module cache", vec![j(&home, "go/pkg/mod")], Risk::Review),
    ];
    for (name, paths, risk) in pkg_caches {
        add("Developer caches", name,
            "Downloaded packages kept by a package manager. They are downloaded again on the next build.",
            risk, CleanKind::Contents, find_all(paths));
    }

    // ----- Pattern based scan ---------------------------------------------------------------
    let mut node_modules = Vec::new();
    let mut rust_targets = Vec::new();
    let mut pycache = Vec::new();
    let mut logs = Vec::new();
    let mut dumps = Vec::new();
    let mut stack: Vec<NodeId> = tree.roots.clone();
    while let Some(id) = stack.pop() {
        for c in tree.children(id) {
            let n = tree.get(c);
            if n.is_dir {
                match &*n.name {
                    "node_modules" => {
                        node_modules.push(c);
                        continue;
                    }
                    "__pycache__" => {
                        pycache.push(c);
                        continue;
                    }
                    "target" if tree.children(c).any(|x| &*tree.get(x).name == "CACHEDIR.TAG") => {
                        rust_targets.push(c);
                        continue;
                    }
                    _ => {}
                }
                stack.push(c);
            } else {
                let lower = n.name.to_ascii_lowercase();
                if lower.ends_with(".log") && n.size >= 5 * 1024 * 1024 {
                    logs.push(c);
                } else if lower.ends_with(".dmp") && n.size >= 1024 * 1024 {
                    dumps.push(c);
                }
            }
        }
    }
    add("Developer build artefacts", "node_modules folders",
        "JavaScript dependencies; restored with `npm install`. Remove from projects you are not working on.",
        Risk::Review, CleanKind::Whole, node_modules);
    add("Developer build artefacts", "Rust target folders",
        "Rust build output; recreated with `cargo build`.",
        Risk::Safe, CleanKind::Whole, rust_targets);
    add("Developer build artefacts", "Python __pycache__",
        "Compiled Python bytecode; recreated automatically.",
        Risk::Safe, CleanKind::Whole, pycache);
    add("Logs & dumps", "Large log files (≥ 5 MB)",
        "Log files written by programs. Check before removing if you need them for troubleshooting.",
        Risk::Review, CleanKind::Whole, logs);
    add("Logs & dumps", "Memory dump files (.dmp)",
        "Crash dumps scattered across the disk.",
        Risk::Safe, CleanKind::Whole, dumps);

    // ----- Downloads ------------------------------------------------------------------------
    if let Some(downloads) = find(home.as_ref().map(|h| h.join("Downloads"))) {
        let installers: Vec<NodeId> = tree
            .children(downloads)
            .filter(|&c| {
                let n = tree.get(c);
                let l = n.name.to_ascii_lowercase();
                !n.is_dir && (l.ends_with(".exe") || l.ends_with(".msi") || l.ends_with(".iso") || l.ends_with(".dmg"))
            })
            .collect();
        add("Downloads", "Installers & disk images in Downloads",
            "Setup files you probably already ran.",
            Risk::Review, CleanKind::Whole, installers);
    }

    items.sort_by(|a, b| a.group.cmp(b.group).then(b.size.cmp(&a.size)));
    items
}
