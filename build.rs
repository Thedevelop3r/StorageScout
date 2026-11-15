//! Embeds the icon and version information into the Windows executable.

fn main() {
    println!("cargo:rerun-if-changed=assets/storagescout.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let version = env!("CARGO_PKG_VERSION");
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/storagescout.ico")
            .set("ProductName", "StorageScout")
            .set("FileDescription", "StorageScout")
            .set("InternalName", "storagescout")
            .set("OriginalFilename", "storagescout.exe")
            .set("CompanyName", "Bilal Amjad")
            .set(
                "LegalCopyright",
                "Copyright (c) 2026 Bilal Amjad. MIT License.",
            )
            .set("ProductVersion", version)
            .set("FileVersion", version);
        if let Err(e) = res.compile() {
            // Do not fail the build on machines without the Windows SDK resource compiler.
            println!("cargo:warning=could not embed Windows resources: {e}");
        }
    }
}
