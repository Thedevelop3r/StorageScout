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
            Category::Audio => "Music and audio",
            Category::Image => "Pictures",
            Category::Document => "Documents",
            Category::Archive => "Archives",
            Category::Program => "Programs and libraries",
            Category::Code => "Source code",
            Category::Build => "Build output",
            Category::Game => "Game data",
            Category::DiskImage => "Disk images and VMs",
            Category::Database => "Databases",
            Category::System => "System files",
            Category::Temporary => "Temp, logs and cache",
            Category::Other => "Other",
        }
    }

    /// Phosphor icon glyph.
    pub fn icon(self) -> &'static str {
        use crate::ui::icons as i;
        match self {
            Category::Video => i::FILM_STRIP,
            Category::Audio => i::MUSIC_NOTE,
            Category::Image => i::IMAGE,
            Category::Document => i::FILE_TEXT,
            Category::Archive => i::FILE_ZIP,
            Category::Program => i::APP_WINDOW,
            Category::Code => i::CODE,
            Category::Build => i::HAMMER,
            Category::Game => i::GAME_CONTROLLER,
            Category::DiskImage => i::DISC,
            Category::Database => i::DATABASE,
            Category::System => i::GEAR,
            Category::Temporary => i::BROOM,
            Category::Other => i::QUESTION,
        }
    }

    pub fn color(self) -> Color32 {
        match self {
            Category::Video => Color32::from_rgb(0xFF, 0x6B, 0x81),
            Category::Audio => Color32::from_rgb(0x5E, 0xE6, 0xFF),
            Category::Image => Color32::from_rgb(0x4F, 0xD1, 0xC5),
            Category::Document => Color32::from_rgb(0x60, 0xA5, 0xFA),
            Category::Archive => Color32::from_rgb(0xFF, 0xB4, 0x54),
            Category::Program => Color32::from_rgb(0x34, 0xD3, 0x99),
            Category::Code => Color32::from_rgb(0xA3, 0xE6, 0x35),
            Category::Build => Color32::from_rgb(0xF4, 0x72, 0xB6),
            Category::Game => Color32::from_rgb(0xC7, 0x7D, 0xFF),
            Category::DiskImage => Color32::from_rgb(0x7C, 0x8C, 0xFF),
            Category::Database => Color32::from_rgb(0xFA, 0xCC, 0x15),
            Category::System => Color32::from_rgb(0x8C, 0x96, 0xA8),
            Category::Temporary => Color32::from_rgb(0xD4, 0xA3, 0x73),
            Category::Other => Color32::from_rgb(0x5B, 0x65, 0x78),
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
            | "bat" | "cmd" | "ps1" | "msp" | "efi" | "drv" | "cpl" | "pyd" => Category::Program,
            "rs" | "c" | "h" | "cpp" | "hpp" | "cc" | "cs" | "java" | "kt" | "py" | "js"
            | "jsx" | "tsx" | "go" | "rb" | "php" | "swift" | "m" | "mm" | "lua" | "pl" | "sh"
            | "html" | "htm" | "css" | "scss" | "less" | "json" | "xml" | "yaml" | "yml"
            | "toml" | "sql" | "vue" | "dart" | "scala" | "r" | "ipynb" | "sln" | "csproj"
            | "vcxproj" | "gradle" | "cmake" | "mk" | "wasm" | "map" => Category::Code,
            "pdb" | "obj" | "o" | "rlib" | "rmeta" | "class" | "pyc" | "pyo" | "lib" | "a"
            | "d" | "ilk" | "exp" | "idb" | "pch" | "ipch" | "tlog" | "lastbuildstate" | "iobj"
            | "ipdb" | "gch" | "dex" | "nupkg" | "whl" | "crate" => Category::Build,
            "pak" | "vpk" | "bsa" | "ba2" | "wad" | "upk" | "uasset" | "umap" | "forge" | "big"
            | "sav" | "esm" | "esp" | "unity3d" | "assets" | "bundle" | "pck" | "gcf" | "ncf"
            | "vdf" | "acf" | "rpf" | "cpk" | "xnb" | "pkg" | "archive" | "ff" | "ipak"
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
