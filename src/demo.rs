//! Built-in demo data (`--demo`): a fictional computer, used for screenshots and to try the UI.
#![allow(clippy::identity_op)] // `1 * GB` reads better next to `12 * GB`
//!
//! Nothing here reads the real disk or registry. All names, sizes and dates are made up.

use std::path::PathBuf;

use crate::analysis::Analysis;
use crate::analysis::cleanup;
use crate::analysis::duplicates::DupGroup;
use crate::model::tree::{TempDir, TempFile};
use crate::model::{FileTree, NodeId};
use crate::scanner::ScanError;
use crate::system::drives::DriveInfo;
use crate::system::programs::{Program, ProgramSource};
use crate::ui::ScanData;

const GB: u64 = 1024 * 1024 * 1024;
const MB: u64 = 1024 * 1024;
const KB: u64 = 1024;

/// Small deterministic random generator so the demo looks the same every time.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn range(&mut self, lo: u64, hi: u64) -> u64 {
        lo + self.next() % (hi - lo).max(1)
    }
}

struct Builder {
    rng: Rng,
    now: i64,
}

impl Builder {
    fn days_ago(&mut self, lo: u64, hi: u64) -> i64 {
        self.now - (self.rng.range(lo, hi) as i64) * 86_400 - self.rng.range(0, 86_400) as i64
    }

    fn file(&mut self, name: &str, size: u64, age_lo: u64, age_hi: u64) -> TempFile {
        TempFile {
            name: name.into(),
            size,
            modified: self.days_ago(age_lo, age_hi),
        }
    }

    /// `count` files named from `pattern` (`{}` is replaced by a number).
    fn files(
        &mut self,
        pattern: &str,
        count: usize,
        lo: u64,
        hi: u64,
        age_lo: u64,
        age_hi: u64,
    ) -> Vec<TempFile> {
        (1..=count)
            .map(|i| {
                let size = self.rng.range(lo, hi);
                let name = pattern.replace("{}", &format!("{:04}", i));
                self.file(&name, size, age_lo, age_hi)
            })
            .collect()
    }
}

fn dir(name: &str, files: Vec<TempFile>, dirs: Vec<TempDir>) -> TempDir {
    TempDir {
        name: name.into(),
        files,
        dirs,
        ..Default::default()
    }
}

fn finalize(d: &mut TempDir) {
    for c in &mut d.dirs {
        finalize(c);
    }
    d.modified = d
        .files
        .iter()
        .map(|f| f.modified)
        .chain(d.dirs.iter().map(|c| c.modified))
        .max()
        .unwrap_or(0);
    d.finalize();
}

fn find_dir<'a>(root: &'a TempDir, path: &[&str]) -> Option<&'a TempDir> {
    let mut cur = root;
    for name in path {
        cur = cur.dirs.iter().find(|d| &*d.name == *name)?;
    }
    Some(cur)
}

pub fn drives() -> Vec<DriveInfo> {
    vec![
        DriveInfo {
            label: "System".into(),
            mount: PathBuf::from("C:\\"),
            file_system: "NTFS".into(),
            total: 1000 * GB,
            available: 0, // filled from the tree below
            removable: false,
            kind: "SSD".into(),
        },
        DriveInfo {
            label: "Data".into(),
            mount: PathBuf::from("D:\\"),
            file_system: "NTFS".into(),
            total: 2000 * GB,
            available: 0,
            removable: false,
            kind: "HDD".into(),
        },
    ]
}

fn build_c(b: &mut Builder) -> TempDir {
    let system32 = dir(
        "System32",
        {
            let mut f = b.files("api-ms-win-core-{}.dll", 120, 20 * KB, 400 * KB, 30, 300);
            f.push(b.file("ntoskrnl.exe", 11 * MB, 20, 60));
            f
        },
        vec![
            dir(
                "DriverStore",
                b.files("driver_pkg_{}.sys", 300, 200 * KB, 30 * MB, 30, 700),
                vec![],
            ),
            dir(
                "drivers",
                b.files("drv{}.sys", 80, 20 * KB, 4 * MB, 30, 700),
                vec![],
            ),
        ],
    );
    let windows = dir(
        "Windows",
        vec![b.file("MEMORY.DMP", 2 * GB + 300 * MB, 40, 41)],
        vec![
            system32,
            dir(
                "WinSxS",
                b.files("amd64_component_{}.manifest", 600, 2 * MB, 24 * MB, 30, 900),
                vec![],
            ),
            dir(
                "Installer",
                b.files("patch{}.msi", 60, 10 * MB, 180 * MB, 100, 900),
                vec![],
            ),
            dir(
                "Temp",
                b.files("tmp{}.tmp", 140, 50 * KB, 40 * MB, 1, 200),
                vec![],
            ),
            dir(
                "SoftwareDistribution",
                vec![],
                vec![dir(
                    "Download",
                    b.files("update-kb{}.cab", 40, 20 * MB, 300 * MB, 5, 120),
                    vec![],
                )],
            ),
            dir(
                "Fonts",
                b.files("font{}.ttf", 200, 50 * KB, 3 * MB, 300, 2000),
                vec![],
            ),
        ],
    );

    let program = |b: &mut Builder, name: &str, exe: &str, size: u64, files: usize| {
        let mut f = b.files(
            "lib{}.dll",
            files,
            size / (files as u64 * 4),
            size / (files as u64) * 2,
            10,
            400,
        );
        f.push(b.file(exe, size / 12, 10, 200));
        dir(name, f, vec![])
    };
    let games_common = dir(
        "common",
        vec![],
        vec![
            dir(
                "Starfall Odyssey",
                b.files("data_{}.pak", 48, 900 * MB, 2 * GB, 30, 400),
                vec![],
            ),
            dir(
                "Iron Legion",
                b.files("chunk{}.pak", 30, 700 * MB, 1800 * MB, 60, 600),
                vec![],
            ),
            dir(
                "Pixel Racers",
                b.files("assets{}.bundle", 40, 50 * MB, 300 * MB, 10, 200),
                vec![],
            ),
            dir(
                "Harbor Tycoon",
                b.files("level{}.assets", 70, 30 * MB, 250 * MB, 90, 900),
                vec![],
            ),
        ],
    );
    let program_files = dir(
        "Program Files",
        vec![],
        vec![
            program(b, "Photo Studio Pro", "photostudio.exe", 3 * GB, 120),
            program(b, "3D Modeler", "modeler.exe", 1200 * MB, 200),
            program(b, "Office Suite", "office.exe", 2600 * MB, 300),
            program(b, "Web Browser", "browser.exe", 600 * MB, 80),
            program(b, "Video Editor", "videoeditor.exe", 1800 * MB, 140),
            program(b, "Code Editor", "codeeditor.exe", 450 * MB, 160),
            dir(
                "Game Launcher",
                vec![b.file("launcher.exe", 4 * MB, 3, 10)],
                vec![dir("library", vec![], vec![games_common])],
            ),
        ],
    );
    let program_files_x86 = dir(
        "Program Files (x86)",
        vec![],
        vec![
            program(b, "Music Player", "musicplayer.exe", 180 * MB, 40),
            program(b, "PDF Reader", "pdfreader.exe", 420 * MB, 60),
            program(b, "Printer Utility", "printer.exe", 90 * MB, 20),
        ],
    );

    // ---- User profile ----
    let mut photos_2024 = b.files("IMG_{}.jpg", 900, 2 * MB, 9 * MB, 300, 700);
    photos_2024.extend(b.files("VID_{}.mp4", 40, 60 * MB, 900 * MB, 300, 700));
    let pictures = dir(
        "Pictures",
        b.files("wallpaper_{}.png", 20, 1 * MB, 12 * MB, 100, 1500),
        vec![
            dir(
                "Camera Roll",
                b.files("PXL_{}.jpg", 600, 2 * MB, 8 * MB, 1, 300),
                vec![],
            ),
            dir("Trip to the Coast 2024", photos_2024, vec![]),
            dir(
                "Screenshots",
                b.files("Screenshot {}.png", 300, 200 * KB, 3 * MB, 1, 400),
                vec![],
            ),
        ],
    );
    let videos = dir(
        "Videos",
        vec![
            b.file("Family Reunion.mp4", 4 * GB + 200 * MB, 400, 420),
            b.file("Birthday Party.mov", 2 * GB + 700 * MB, 200, 220),
            b.file("Drone Flight 01.mp4", 1 * GB + 900 * MB, 90, 95),
            b.file("Drone Flight 02.mp4", 1 * GB + 600 * MB, 88, 92),
        ],
        vec![dir(
            "Captures",
            b.files("Gameplay {}.mp4", 25, 300 * MB, 2 * GB, 5, 200),
            vec![],
        )],
    );
    let music = dir(
        "Music",
        vec![],
        vec![
            dir(
                "Albums",
                b.files("Track {}.flac", 400, 20 * MB, 60 * MB, 500, 2500),
                vec![],
            ),
            dir(
                "Podcasts",
                b.files("Episode {}.mp3", 120, 30 * MB, 90 * MB, 30, 600),
                vec![],
            ),
        ],
    );
    let documents = dir(
        "Documents",
        {
            let mut f = b.files("Report {}.pdf", 80, 200 * KB, 15 * MB, 10, 1200);
            f.extend(b.files("Budget {}.xlsx", 30, 50 * KB, 3 * MB, 10, 1200));
            f.extend(b.files("Notes {}.docx", 60, 30 * KB, 2 * MB, 10, 1500));
            f.push(b.file("Thesis Final.docx", 48 * MB, 900, 910));
            f
        },
        vec![
            dir(
                "Scans",
                b.files("scan_{}.pdf", 150, 500 * KB, 6 * MB, 100, 2000),
                vec![],
            ),
            dir(
                "Old Backups",
                vec![
                    b.file("phone-backup-2022.zip", 14 * GB, 1100, 1120),
                    b.file("old-laptop.bak", 6 * GB, 1400, 1450),
                ],
                vec![],
            ),
        ],
    );
    let downloads = dir(
        "Downloads",
        vec![
            b.file("linux-distro-24.04.iso", 5 * GB + 800 * MB, 200, 210),
            b.file("game-installer.exe", 3 * GB + 100 * MB, 120, 125),
            b.file("video-editor-setup.exe", 650 * MB, 60, 61),
            b.file("office-suite-setup.msi", 1 * GB + 200 * MB, 300, 301),
            b.file("dataset-archive.zip", 7 * GB, 45, 46),
            b.file("presentation-assets.zip", 900 * MB, 15, 16),
            b.file("music-player-setup.exe", 85 * MB, 400, 401),
            b.file("IMG_0042.jpg", 6 * MB, 20, 21),
        ],
        vec![],
    );
    let projects = dir(
        "Projects",
        vec![],
        vec![
            dir(
                "web-shop",
                b.files("component{}.tsx", 60, 2 * KB, 40 * KB, 1, 60),
                vec![dir(
                    "node_modules",
                    b.files("module{}.js", 900, 5 * KB, 3 * MB, 20, 25),
                    vec![],
                )],
            ),
            dir(
                "image-tool",
                b.files("src{}.rs", 30, 2 * KB, 60 * KB, 1, 30),
                vec![dir(
                    "target",
                    vec![b.file("CACHEDIR.TAG", 177, 3, 4)],
                    vec![
                        dir(
                            "debug",
                            b.files("dep{}.rlib", 400, 2 * MB, 40 * MB, 1, 30),
                            vec![],
                        ),
                        dir(
                            "release",
                            b.files("dep{}.rlib", 200, 2 * MB, 30 * MB, 1, 30),
                            vec![],
                        ),
                    ],
                )],
            ),
            dir(
                "ml-notebooks",
                b.files("experiment{}.ipynb", 25, 100 * KB, 20 * MB, 5, 200),
                vec![
                    dir(
                        "__pycache__",
                        b.files("module{}.pyc", 60, 5 * KB, 200 * KB, 5, 200),
                        vec![],
                    ),
                    dir(
                        "logs",
                        b.files("training{}.log", 12, 6 * MB, 120 * MB, 5, 200),
                        vec![],
                    ),
                ],
            ),
        ],
    );
    let appdata = dir(
        "AppData",
        vec![],
        vec![dir(
            "Local",
            vec![],
            vec![
                dir(
                    "Temp",
                    b.files("tmp{}.tmp", 400, 10 * KB, 30 * MB, 1, 120),
                    vec![],
                ),
                dir(
                    "CrashDumps",
                    b.files("app{}.dmp", 12, 20 * MB, 400 * MB, 10, 300),
                    vec![],
                ),
                dir(
                    "Browser Cache",
                    b.files("f_{}", 1500, 5 * KB, 2 * MB, 1, 60),
                    vec![],
                ),
            ],
        )],
    );
    let alex = dir(
        "Alex",
        vec![],
        vec![
            pictures,
            videos,
            music,
            documents,
            downloads,
            projects,
            appdata,
            dir(
                "Desktop",
                b.files("Shortcut {}.lnk", 12, 1 * KB, 3 * KB, 30, 600),
                vec![],
            ),
        ],
    );
    let users = dir(
        "Users",
        vec![],
        vec![
            alex,
            dir(
                "Public",
                b.files("shared{}.dat", 10, 1 * MB, 20 * MB, 300, 900),
                vec![],
            ),
        ],
    );

    let tools = dir(
        "Tools",
        vec![],
        vec![
            dir(
                "ImageViewer Portable",
                vec![b.file("imageviewer.exe", 14 * MB, 200, 201)],
                vec![dir(
                    "plugins",
                    b.files("plugin{}.dll", 30, 100 * KB, 2 * MB, 200, 201),
                    vec![],
                )],
            ),
            dir(
                "DiskBench",
                vec![b.file("diskbench.exe", 6 * MB, 400, 401)],
                vec![],
            ),
            dir(
                "RetroEmulator",
                vec![
                    b.file("emulator.exe", 22 * MB, 90, 91),
                    b.file("emulator-cli.exe", 4 * MB, 90, 91),
                ],
                vec![dir(
                    "roms",
                    b.files("game{}.bin", 40, 2 * MB, 60 * MB, 300, 900),
                    vec![],
                )],
            ),
        ],
    );

    let mut c = dir(
        "C:\\",
        vec![
            b.file("pagefile.sys", 16 * GB, 0, 1),
            b.file("hiberfil.sys", 12 * GB + 700 * MB, 0, 2),
        ],
        vec![
            windows,
            program_files,
            program_files_x86,
            dir(
                "ProgramData",
                vec![],
                vec![dir(
                    "Package Cache",
                    b.files("pkg{}.msi", 80, 5 * MB, 120 * MB, 100, 900),
                    vec![],
                )],
            ),
            users,
            tools,
            dir(
                "Windows.old",
                vec![],
                vec![dir(
                    "Windows",
                    b.files("old{}.dll", 500, 5 * MB, 60 * MB, 380, 400),
                    vec![],
                )],
            ),
        ],
    );
    finalize(&mut c);
    c
}

fn build_d(b: &mut Builder) -> TempDir {
    let mut d = dir(
        "D:\\",
        vec![],
        vec![
            dir(
                "Media",
                vec![],
                vec![
                    dir(
                        "Movies",
                        b.files("Movie {}.mkv", 70, 2 * GB, 9 * GB, 100, 2500),
                        vec![],
                    ),
                    dir(
                        "TV Shows",
                        vec![],
                        vec![
                            dir(
                                "Ocean Documentary",
                                b.files("S01E{}.mkv", 10, 900 * MB, 1600 * MB, 300, 400),
                                vec![],
                            ),
                            dir(
                                "Space Series",
                                b.files("S02E{}.mp4", 12, 700 * MB, 1300 * MB, 200, 260),
                                vec![],
                            ),
                        ],
                    ),
                    dir(
                        "Photo Archive",
                        b.files("DSC_{}.cr3", 3000, 20 * MB, 35 * MB, 400, 3000),
                        vec![],
                    ),
                    dir(
                        "Music Library",
                        b.files("Song {}.mp3", 4000, 3 * MB, 12 * MB, 500, 4000),
                        vec![],
                    ),
                ],
            ),
            dir(
                "Virtual Machines",
                vec![
                    b.file("ubuntu-dev.vhdx", 64 * GB, 3, 10),
                    b.file("windows-test.vhdx", 48 * GB, 30, 40),
                    b.file("old-server.vmdk", 35 * GB, 700, 720),
                ],
                vec![],
            ),
            dir(
                "Backups",
                vec![
                    b.file("Laptop-2023.vhdx", 180 * GB, 600, 620),
                    b.file("Photos-2021.zip", 42 * GB, 1200, 1210),
                ],
                vec![],
            ),
            dir(
                "Games",
                vec![],
                vec![
                    dir(
                        "Racing Legends",
                        b.files("track{}.pak", 25, 800 * MB, 2 * GB, 100, 300),
                        vec![],
                    ),
                    dir(
                        "Fantasy Quest",
                        b.files("world{}.pak", 40, 600 * MB, 1500 * MB, 200, 500),
                        vec![],
                    ),
                ],
            ),
            dir(
                "Archive",
                b.files("project-{}.7z", 40, 200 * MB, 4 * GB, 400, 3000),
                vec![dir("Empty Folder", vec![], vec![])],
            ),
        ],
    );
    finalize(&mut d);
    d
}

pub fn programs() -> Vec<Program> {
    let p = |name: &str,
             version: &str,
             publisher: &str,
             date: &str,
             loc: Option<&str>,
             size_mb: u64,
             source: ProgramSource| Program {
        name: name.into(),
        version: version.into(),
        publisher: publisher.into(),
        install_date: date.into(),
        install_location: loc.map(PathBuf::from),
        estimated_size: size_mb * MB,
        uninstall_string: if source == ProgramSource::Store {
            None
        } else {
            Some(format!(
                "\"{}\\uninstall.exe\"",
                loc.unwrap_or("C:\\Program Files\\App")
            ))
        },
        source,
        system_component: false,
    };
    use ProgramSource::*;
    vec![
        p(
            "Photo Studio Pro",
            "26.1.0",
            "Example Imaging Inc.",
            "20260412",
            Some("C:\\Program Files\\Photo Studio Pro"),
            2900,
            Machine,
        ),
        p(
            "Office Suite",
            "24.8.2",
            "Open Office Group",
            "20251103",
            Some("C:\\Program Files\\Office Suite"),
            2500,
            Machine,
        ),
        p(
            "Video Editor",
            "5.4.1",
            "Frame Labs",
            "20260918",
            Some("C:\\Program Files\\Video Editor"),
            1700,
            Machine,
        ),
        p(
            "3D Modeler",
            "4.6.0",
            "Example 3D Foundation",
            "20260807",
            Some("C:\\Program Files\\3D Modeler"),
            1150,
            Machine,
        ),
        p(
            "Web Browser",
            "142.0.1",
            "Example Browser Co.",
            "20261101",
            Some("C:\\Program Files\\Web Browser"),
            580,
            Machine,
        ),
        p(
            "Code Editor",
            "1.104.0",
            "Example Dev Tools",
            "20261020",
            Some("C:\\Program Files\\Code Editor"),
            430,
            User,
        ),
        p(
            "Game Launcher",
            "2.10.91",
            "Example Games Inc.",
            "20240115",
            Some("C:\\Program Files\\Game Launcher"),
            40,
            Machine32,
        ),
        p(
            "PDF Reader",
            "2026.003",
            "Docs Software Ltd.",
            "20250622",
            Some("C:\\Program Files (x86)\\PDF Reader"),
            400,
            Machine32,
        ),
        p(
            "Music Player",
            "3.2.7",
            "Sound Apps",
            "20230930",
            Some("C:\\Program Files (x86)\\Music Player"),
            170,
            Machine32,
        ),
        p(
            "Printer Utility",
            "1.18",
            "Printer Maker",
            "20221114",
            Some("C:\\Program Files (x86)\\Printer Utility"),
            85,
            Machine32,
        ),
        p(
            "Runtime Libraries 2015-2026 (x64)",
            "14.50.1",
            "Example Runtime Corp.",
            "20260301",
            None,
            25,
            Machine,
        ),
        p("Weather", "4.53.1", "Example Apps", "", None, 0, Store),
        p(
            "Calculator",
            "11.2508.4",
            "Example Apps",
            "",
            None,
            0,
            Store,
        ),
        p(
            "Photos Viewer",
            "2026.11010.1",
            "Example Apps",
            "",
            None,
            0,
            Store,
        ),
        p("Notes", "6.2.0", "Example Apps", "", None, 0, Store),
    ]
}

fn errors() -> Vec<ScanError> {
    [
        "C:\\System Volume Information",
        "C:\\Windows\\CSC",
        "C:\\Windows\\System32\\LogFiles\\WMI\\RtBackup",
        "D:\\System Volume Information",
    ]
    .iter()
    .map(|p| ScanError {
        path: PathBuf::from(p),
        message: "Access is denied. (os error 5)".into(),
    })
    .collect()
}

/// The full demo dataset: drives (with free space), scan data and duplicate groups.
pub fn scan_data() -> (Vec<DriveInfo>, ScanData, Vec<DupGroup>) {
    let mut b = Builder {
        rng: Rng(0x5eed_1234_abcd),
        now: chrono::Local::now().timestamp(),
    };
    let c = build_c(&mut b);
    let d = build_d(&mut b);
    let mut drives = drives();
    // Leave some space the scan cannot see, like on a real system.
    drives[0].available = drives[0].total.saturating_sub(c.size + 14 * GB);
    drives[1].available = drives[1].total.saturating_sub(d.size + 2 * GB);

    // Copies that the duplicate finder will report: a phone backup of the camera roll and
    // videos copied to the data drive.
    let mut d = d;
    let mut copies = Vec::new();
    for path in [
        &["Users", "Alex", "Pictures", "Camera Roll"][..],
        &["Users", "Alex", "Videos"][..],
    ] {
        if let Some(src) = find_dir(&c, path) {
            copies.extend(src.files.iter().take(60).map(|f| TempFile {
                name: f.name.clone(),
                size: f.size,
                modified: f.modified,
            }));
        }
    }
    let extra: u64 = copies.iter().map(|f| f.size).sum();
    d.dirs.push(dir("Phone Backup", copies, vec![]));
    finalize(&mut d);
    drives[1].available = drives[1].available.saturating_sub(extra);

    let mut tree = FileTree::default();
    tree.add_root(c);
    tree.add_root(d);
    let analysis = Analysis::build(&tree);
    let cleanup = cleanup::suggestions(&tree);

    // Duplicate groups: every (name, size) that occurs more than once.
    let mut by_key: std::collections::HashMap<(&str, u64), Vec<NodeId>> =
        std::collections::HashMap::new();
    for &id in &analysis.files_by_size {
        let n = tree.get(id);
        if n.size >= MB {
            by_key.entry((&n.name, n.size)).or_default().push(id);
        }
    }
    let mut dups: Vec<DupGroup> = by_key
        .into_iter()
        .filter(|(_, ids)| ids.len() >= 2)
        .map(|((_, size), ids)| DupGroup { size, ids })
        .collect();
    dups.sort_by_key(|g| std::cmp::Reverse(g.wasted()));

    let data = ScanData {
        tree,
        analysis,
        cleanup,
        errors: errors(),
        elapsed_secs: 41.7,
        cancelled: false,
        roots: vec![PathBuf::from("C:\\"), PathBuf::from("D:\\")],
        generation: 1,
    };
    (drives, data, dups)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_builds() {
        let (drives, data, dups) = scan_data();
        assert_eq!(drives.len(), 2);
        assert!(data.analysis.total_files > 10_000);
        assert!(
            data.tree
                .find_path(&PathBuf::from("C:\\Users\\Alex\\Videos"))
                .is_some()
        );
        assert!(!dups.is_empty());
        assert!(!data.cleanup.is_empty());
        assert!(drives[0].available > 0 && drives[0].available < drives[0].total);
    }
}
