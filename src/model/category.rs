//! Classification of files into high level categories by extension.

use egui::Color32;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
pub enum Category {
    Video,
    Audio,
    Image,
    Document,
    Archive,
    Program,
    Code,
    Build,
    Game,
    DiskImage,
    Database,
    System,
    Temporary,
    Other,
}

impl Category {
    pub const ALL: [Category; 14] = [
        Category::Video,
        Category::Audio,
        Category::Image,
        Category::Document,
        Category::Archive,
        Category::Program,
        Category::Code,
        Category::Build,
        Category::Game,
        Category::DiskImage,
        Category::Database,
        Category::System,
        Category::Temporary,
        Category::Other,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn label(self) -> &'static str {
        match self {
            Category::Video => "Videos",
            Category::Audio => "Music & Audio",
            Category::Image => "Pictures",
            Category::Document => "Documents",
            Category::Archive => "Archives",
            Category::Program => "Programs & Libraries",
            Category::Code => "Source Code",
            Category::Build => "Build Output",
            Category::Game => "Game Data",
            Category::DiskImage => "Disk Images & VMs",
            Category::Database => "Databases",
            Category::System => "System Files",
            Category::Temporary => "Temp, Logs & Cache",
            Category::Other => "Other",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Category::Video => "🎬",
            Category::Audio => "🎵",
            Category::Image => "🖼",
            Category::Document => "📄",
            Category::Archive => "📦",
            Category::Program => "⚙",
            Category::Code => "💻",
            Category::Build => "🔨",
            Category::Game => "🎮",
            Category::DiskImage => "💿",
            Category::Database => "🗄",
            Category::System => "🛠",
            Category::Temporary => "🗑",
            Category::Other => "❔",
        }
    }

    pub fn color(self) -> Color32 {
        match self {
            Category::Video => Color32::from_rgb(0xE5, 0x5B, 0x5B),
            Category::Audio => Color32::from_rgb(0xE0, 0x8A, 0x3C),
            Category::Image => Color32::from_rgb(0xD9, 0xB8, 0x3A),
            Category::Document => Color32::from_rgb(0x4F, 0x9B, 0xE8),
            Category::Archive => Color32::from_rgb(0x9C, 0x6A, 0xDE),
            Category::Program => Color32::from_rgb(0x3F, 0xB5, 0x8A),
            Category::Code => Color32::from_rgb(0x2E, 0xC4, 0xC4),
            Category::Build => Color32::from_rgb(0x5E, 0x9C, 0xA0),
            Category::Game => Color32::from_rgb(0xD6, 0x5C, 0xB0),
            Category::DiskImage => Color32::from_rgb(0x8A, 0x8F, 0xE0),
            Category::Database => Color32::from_rgb(0x6B, 0xA3, 0x4E),
            Category::System => Color32::from_rgb(0x7D, 0x86, 0x91),
            Category::Temporary => Color32::from_rgb(0xA8, 0x7B, 0x5A),
            Category::Other => Color32::from_rgb(0x9A, 0x9A, 0x9A),
        }
    }

    /// Classify a lowercase extension (without the dot).
    pub fn from_extension(ext: &str) -> Category {
        match ext {
            "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" | "m4v" | "mpg" | "mpeg"
            | "3gp" | "ts" | "m2ts" | "mts" | "vob" | "ogv" | "rm" | "rmvb" | "divx" => {
                Category::Video
            }
            "mp3" | "wav" | "flac" | "aac" | "ogg" | "oga" | "wma" | "m4a" | "opus" | "aiff"
            | "aif" | "alac" | "mid" | "midi" | "ape" | "amr" => Category::Audio,
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "tif" | "tiff" | "webp" | "heic" | "heif"
            | "raw" | "cr2" | "cr3" | "nef" | "arw" | "dng" | "orf" | "rw2" | "svg" | "ico"
            | "psd" | "xcf" | "ai" | "eps" | "avif" | "jfif" => Category::Image,
            "pdf" | "doc" | "docx" | "xls" | "xlsx" | "xlsm" | "ppt" | "pptx" | "odt" | "ods"
            | "odp" | "rtf" | "txt" | "md" | "csv" | "epub" | "mobi" | "azw3" | "pages"
            | "numbers" | "key" | "one" | "pub" | "vsdx" | "xps" | "djvu" | "tex" => {
                Category::Document
            }
            "zip" | "rar" | "7z" | "tar" | "gz" | "tgz" | "bz2" | "xz" | "zst" | "lz" | "lzma"
            | "cab" | "arj" | "z" | "tbz2" | "txz" => Category::Archive,
            "exe" | "msi" | "msix" | "appx" | "msixbundle" | "appxbundle" | "dll" | "sys"
            | "ocx" | "com" | "scr" | "so" | "dylib" | "app" | "deb" | "rpm" | "apk" | "jar"
            | "bat" | "cmd" | "ps1" | "msp" | "efi" | "drv" | "cpl" | "pyd" => {
                Category::Program
            }
            "rs" | "c" | "h" | "cpp" | "hpp" | "cc" | "cs" | "java" | "kt" | "py" | "js"
            | "jsx" | "tsx" | "go" | "rb" | "php" | "swift" | "m" | "mm" | "lua" | "pl" | "sh"
            | "html" | "htm" | "css" | "scss" | "less" | "json" | "xml" | "yaml" | "yml"
            | "toml" | "sql" | "vue" | "dart" | "scala" | "r" | "ipynb" | "sln" | "csproj"
            | "vcxproj" | "gradle" | "cmake" | "mk" | "wasm" | "map" => Category::Code,
            "pdb" | "obj" | "o" | "rlib" | "rmeta" | "class" | "pyc" | "pyo" | "lib" | "a"
            | "d" | "ilk" | "exp" | "idb" | "pch" | "ipch" | "tlog" | "lastbuildstate" | "iobj"
            | "ipdb" | "gch" | "dex" | "nupkg" | "whl" | "crate" => Category::Build,
            "pak" | "vpk" | "bsa" | "ba2" | "wad" | "upk" | "uasset" | "umap" | "forge"
            | "big" | "sav" | "esm" | "esp" | "unity3d" | "assets" | "bundle" | "pck" | "gcf"
            | "ncf" | "vdf" | "acf" | "rpf" | "cpk" | "xnb" | "pkg" | "archive" | "ff" | "ipak"
            | "xpak" | "toc" | "cas" | "sb" | "rgssad" | "oodle" => Category::Game,
            "iso" | "img" | "vhd" | "vhdx" | "vmdk" | "vdi" | "qcow2" | "ova" | "ovf" | "dmg"
            | "wim" | "esd" | "swm" | "nrg" | "bin" | "cue" | "avhdx" | "vmem" | "vmsn" => {
                Category::DiskImage
            }
            "db" | "sqlite" | "sqlite3" | "mdb" | "accdb" | "mdf" | "ldf" | "ndf" | "frm"
            | "ibd" | "edb" | "pst" | "ost" | "realm" | "kdbx" | "dbf" | "db-wal" | "db-shm" => {
                Category::Database
            }
            "tmp" | "temp" | "log" | "bak" | "old" | "dmp" | "etl" | "cache" | "crdownload"
            | "part" | "partial" | "download" | "chk" | "gid" | "~" | "swp" | "lck" => {
                Category::Temporary
            }
            "ini" | "cfg" | "dat" | "inf" | "cat" | "mui" | "manifest" | "reg" | "pf" | "evtx"
            | "msc" | "nls" | "ttf" | "otf" | "fon" | "woff" | "woff2" | "lnk" | "url"
            | "regtrans-ms" | "blf" | "cdxml" | "admx" | "adml" | "mum" | "sdb" | "tlb" => {
                Category::System
            }
            _ => Category::Other,
        }
    }

    /// Classify a file name (handles special names without extension).
    pub fn from_name(name: &str) -> Category {
        let lower = name.to_ascii_lowercase();
        match lower.as_str() {
            "pagefile.sys" | "hiberfil.sys" | "swapfile.sys" | "dumpstack.log"
            | "dumpstack.log.tmp" | "ntuser.dat" => return Category::System,
            _ => {}
        }
        match lower.rsplit_once('.') {
            Some((stem, ext)) if !stem.is_empty() => Category::from_extension(ext),
            _ => Category::Other,
        }
    }
}

/// Lowercase extension of a file name, or empty string.
pub fn extension_of(name: &str) -> String {
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && ext.len() <= 16 => ext.to_ascii_lowercase(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify() {
        assert_eq!(Category::from_name("Movie.MKV"), Category::Video);
        assert_eq!(Category::from_name("song.mp3"), Category::Audio);
        assert_eq!(Category::from_name("report.pdf"), Category::Document);
        assert_eq!(Category::from_name("setup.exe"), Category::Program);
        assert_eq!(Category::from_name("pagefile.sys"), Category::System);
        assert_eq!(Category::from_name(".gitignore"), Category::Other);
        assert_eq!(Category::from_name("README"), Category::Other);
    }

    #[test]
    fn extensions() {
        assert_eq!(extension_of("a.TXT"), "txt");
        assert_eq!(extension_of(".bashrc"), "");
        assert_eq!(extension_of("noext"), "");
    }

    #[test]
    fn index_matches_all() {
        for (i, c) in Category::ALL.iter().enumerate() {
            assert_eq!(c.index(), i);
        }
    }
}
