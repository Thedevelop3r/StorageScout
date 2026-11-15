//! Drive / volume enumeration.

use std::path::PathBuf;

use sysinfo::{DiskKind, Disks};

#[derive(Debug, Clone)]
pub struct DriveInfo {
    pub label: String,
    pub mount: PathBuf,
    pub file_system: String,
    pub total: u64,
    pub available: u64,
    pub removable: bool,
    pub kind: String,
}

impl DriveInfo {
    pub fn used(&self) -> u64 {
        self.total.saturating_sub(self.available)
    }

    pub fn display_name(&self) -> String {
        let mount = self.mount.to_string_lossy();
        if self.label.is_empty() { mount.into_owned() } else { format!("{} ({})", self.label, mount) }
    }
}

pub fn list_drives() -> Vec<DriveInfo> {
    let disks = match Disks::new_with_refreshed_list() {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    let mut out: Vec<DriveInfo> = (&disks)
        .into_iter()
        .filter(|d| d.total_space() > 0)
        .map(|d| DriveInfo {
            label: d.name().to_string_lossy().into_owned(),
            mount: d.mount_point().to_path_buf(),
            file_system: d.file_system().to_string_lossy().into_owned(),
            total: d.total_space(),
            available: d.available_space(),
            removable: d.is_removable(),
            kind: match d.kind() {
                DiskKind::SSD => "SSD".to_owned(),
                DiskKind::HDD => "HDD".to_owned(),
                _ => "Disk".to_owned(),
            },
        })
        .collect();
    // On Unix many pseudo mounts exist; keep only real-looking ones.
    #[cfg(unix)]
    out.retain(|d| {
        let fs = d.file_system.to_lowercase();
        !matches!(fs.as_str(), "tmpfs" | "devtmpfs" | "overlay" | "squashfs" | "proc" | "sysfs")
    });
    out.sort_by(|a, b| a.mount.cmp(&b.mount));
    out.dedup_by(|a, b| a.mount == b.mount);
    out
}
