//! Self-update from GitHub Releases.
//!
//! 1. [`check`] asks the GitHub API for the latest published release of this repository.
//! 2. [`download_and_verify`] downloads the installer and `SHA256SUMS.txt` from that release,
//!    checks the SHA-256 and then the Authenticode signature: the file must not have been
//!    changed since it was signed, and the signing certificate must be the project's own
//!    (pinned by thumbprint below). Anything else is refused and deleted.
//! 3. [`launch_installer`] runs the verified installer silently; it closes PC Scanner,
//!    installs the new version over the old one and starts it again.
//!
//! The only network requests PC Scanner makes are the ones in this file, all to GitHub.
//! They send no personal data (see PRIVACY.md).

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::time::Duration;

use serde::Deserialize;
use sha2::{Digest, Sha256};

/// GitHub repository the releases come from.
pub const REPO: &str = "Thedevelop3r/pcscanner";
/// SHA-1 thumbprint of the project's code signing certificate
/// (`signing/pcscanner-codesign.cer`, see SIGNING.md).
pub const THUMBPRINT: &str = "A7AAB0826FF0EE4B5CC7BC4B78722CE255D0BC2E";
/// The version of this program.
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");
/// Inno Setup AppId (installer/pcscanner.iss), used to find the installed copy.
#[cfg(windows)]
const APP_ID: &str = "{2A3DA3C9-B524-428F-86AE-1FB1E103E09F}";

/// Largest installer that will be downloaded.
const MAX_INSTALLER: u64 = 512 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct Asset {
    pub name: String,
    pub url: String,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub struct Release {
    /// "0.3.0"
    pub version: String,
    /// Release notes (GitHub Markdown).
    pub notes: String,
    /// Release page on GitHub.
    pub page: String,
    pub published: Option<chrono::DateTime<chrono::Utc>>,
    pub installer: Asset,
    pub sums: Asset,
}

/// A downloaded installer that passed every check.
#[derive(Debug, Clone)]
pub struct Verified {
    pub path: PathBuf,
    pub signer: String,
    pub sha256: String,
}

/// Download progress shared with the UI thread.
#[derive(Default)]
pub struct Progress {
    pub done: AtomicU64,
    pub total: AtomicU64,
    /// One of the `STAGE_*` values.
    pub stage: AtomicU8,
    pub cancel: AtomicBool,
}

pub const STAGE_DOWNLOAD: u8 = 0;
pub const STAGE_HASH: u8 = 1;
pub const STAGE_SIGNATURE: u8 = 2;

#[derive(Deserialize)]
struct ApiRelease {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    body: Option<String>,
    html_url: String,
    published_at: Option<String>,
    #[serde(default)]
    assets: Vec<ApiAsset>,
}

#[derive(Deserialize)]
struct ApiAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

/// "v1.2.3" or "1.2.3" to `[1, 2, 3]`. Pre-release versions ("1.2.3-beta") are not accepted.
pub fn parse_version(s: &str) -> Option<[u64; 3]> {
    let s = s.trim().trim_start_matches(['v', 'V']);
    let mut parts = s.split('.');
    let mut out = [0u64; 3];
    for slot in &mut out {
        *slot = parts.next()?.parse().ok()?;
    }
    if parts.next().is_some() {
        return None;
    }
    Some(out)
}

/// True if `version` is newer than the running program.
pub fn is_newer(version: &str) -> bool {
    match (parse_version(version), parse_version(CURRENT)) {
        (Some(new), Some(cur)) => new > cur,
        _ => false,
    }
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_recv_response(Some(Duration::from_secs(30)))
        .timeout_recv_body(Some(Duration::from_secs(60)))
        // A generic name only: no user, machine or install details are sent.
        .user_agent(format!("PC-Scanner-Updater/{CURRENT}"))
        .build()
        .into()
}

fn download_prefix() -> String {
    format!("https://github.com/{REPO}/releases/download/")
}

/// Ask GitHub for the latest release. `Ok(None)` means this is the newest version.
pub fn check() -> Result<Option<Release>, String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let mut resp = match agent()
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .call()
    {
        Ok(r) => r,
        // No published release yet.
        Err(ureq::Error::StatusCode(404)) => return Ok(None),
        Err(e) => return Err(format!("Could not reach GitHub: {e}")),
    };
    let text = resp
        .body_mut()
        .with_config()
        .limit(4 * 1024 * 1024)
        .read_to_string()
        .map_err(|e| format!("Could not read GitHub's answer: {e}"))?;
    let api: ApiRelease =
        serde_json::from_str(&text).map_err(|e| format!("Unexpected answer from GitHub: {e}"))?;
    release_from_api(api)
}

fn release_from_api(api: ApiRelease) -> Result<Option<Release>, String> {
    if api.draft || api.prerelease {
        return Ok(None);
    }
    let Some(v) = parse_version(&api.tag_name) else {
        return Ok(None);
    };
    let version = format!("{}.{}.{}", v[0], v[1], v[2]);
    if !is_newer(&version) {
        return Ok(None);
    }
    let prefix = download_prefix();
    let find = |name: &str| -> Result<Asset, String> {
        let a = api
            .assets
            .iter()
            .find(|a| a.name == name)
            .ok_or_else(|| format!("The {version} release has no {name}."))?;
        if !a.browser_download_url.starts_with(&prefix) {
            return Err(format!(
                "{name} is not hosted on the project's GitHub releases."
            ));
        }
        Ok(Asset {
            name: a.name.clone(),
            url: a.browser_download_url.clone(),
            size: a.size,
        })
    };
    Ok(Some(Release {
        installer: find(&format!("PCScanner-Setup-{version}-x64.exe"))?,
        sums: find("SHA256SUMS.txt")?,
        notes: api.body.unwrap_or_default(),
        page: api.html_url,
        published: api
            .published_at
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
            .map(|d| d.with_timezone(&chrono::Utc)),
        version,
    }))
}

/// Find the expected SHA-256 of `name` in a `SHA256SUMS.txt` file.
fn expected_hash(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let mut it = line.split_whitespace();
        let hash = it.next()?;
        let file = it.next()?.trim_start_matches('*');
        (file == name && hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit()))
            .then(|| hash.to_ascii_lowercase())
    })
}

/// Folder the installer is downloaded to.
pub fn download_dir() -> PathBuf {
    std::env::temp_dir().join("PC Scanner update")
}

/// Download the installer, check its SHA-256 and signature. On any failure the file is deleted.
pub fn download_and_verify(release: &Release, progress: &Progress) -> Result<Verified, String> {
    let prefix = download_prefix();
    if !release.installer.url.starts_with(&prefix) || !release.sums.url.starts_with(&prefix) {
        return Err("The download is not hosted on the project's GitHub releases.".into());
    }
    let agent = agent();

    // Checksums first: it is tiny and tells us what to expect.
    let sums = agent
        .get(&release.sums.url)
        .call()
        .map_err(|e| format!("Could not download SHA256SUMS.txt: {e}"))?
        .body_mut()
        .with_config()
        .limit(1024 * 1024)
        .read_to_string()
        .map_err(|e| format!("Could not read SHA256SUMS.txt: {e}"))?;
    let expected = expected_hash(&sums, &release.installer.name)
        .ok_or("SHA256SUMS.txt does not list the installer.")?;

    let dir = download_dir();
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
    let path = dir.join(&release.installer.name);
    let result = download_to(&agent, release, &path, progress).and_then(|actual| {
        progress.stage.store(STAGE_HASH, Ordering::Relaxed);
        if actual != expected {
            return Err(format!(
                "The download does not match SHA256SUMS.txt (expected {expected}, got {actual})."
            ));
        }
        progress.stage.store(STAGE_SIGNATURE, Ordering::Relaxed);
        let signer = verify_signature(&path)?;
        Ok(Verified {
            path: path.clone(),
            signer,
            sha256: actual,
        })
    });
    if result.is_err() {
        let _ = std::fs::remove_file(&path);
    }
    result
}

/// Stream the installer to `path`, returning its SHA-256 (lowercase hex).
fn download_to(
    agent: &ureq::Agent,
    release: &Release,
    path: &Path,
    progress: &Progress,
) -> Result<String, String> {
    progress.stage.store(STAGE_DOWNLOAD, Ordering::Relaxed);
    progress.done.store(0, Ordering::Relaxed);
    progress
        .total
        .store(release.installer.size, Ordering::Relaxed);
    let mut resp = agent
        .get(&release.installer.url)
        .call()
        .map_err(|e| format!("Could not download the installer: {e}"))?;
    let mut reader = resp.body_mut().with_config().limit(MAX_INSTALLER).reader();
    let mut file =
        std::fs::File::create(path).map_err(|e| format!("Could not save the installer: {e}"))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut done = 0u64;
    loop {
        if progress.cancel.load(Ordering::Relaxed) {
            return Err("Download cancelled.".into());
        }
        let n = reader
            .read(&mut buf)
            .map_err(|e| format!("The download failed: {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        file.write_all(&buf[..n])
            .map_err(|e| format!("Could not save the installer: {e}"))?;
        done += n as u64;
        progress.done.store(done, Ordering::Relaxed);
    }
    file.flush().map_err(|e| e.to_string())?;
    drop(file);
    if release.installer.size != 0 && done != release.installer.size {
        return Err(format!(
            "The download is incomplete ({done} of {} bytes).",
            release.installer.size
        ));
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

/// Run the verified installer silently. It closes PC Scanner, installs over the current
/// version (keeping the install folder and the settings) and starts the new version.
pub fn launch_installer(path: &Path) -> Result<(), String> {
    std::process::Command::new(path)
        .args([
            "/VERYSILENT",
            "/SUPPRESSMSGBOXES",
            "/NORESTART",
            "/CLOSEAPPLICATIONS",
            // PC Scanner ignores Restart Manager's polite close request (tested: setup then
            // stops with exit code 5), so let setup end any copy that is still running.
            // This copy quits itself right after starting setup.
            "/FORCECLOSEAPPLICATIONS",
            "/RELAUNCH=1",
        ])
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Could not start the installer: {e}"))
}

/// True when this program runs from the folder the installer put it in. Portable copies
/// cannot be updated by the installer; they get a link to the release page instead.
#[cfg(windows)]
pub fn is_installed_copy() -> bool {
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .and_then(|p| p.canonicalize().ok())
    else {
        return false;
    };
    let key = format!(r"Software\Microsoft\Windows\CurrentVersion\Uninstall\{APP_ID}_is1");
    [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE]
        .into_iter()
        .any(|hive| {
            RegKey::predef(hive)
                .open_subkey(&key)
                .and_then(|k| k.get_value::<String, _>("InstallLocation"))
                .ok()
                .and_then(|loc| PathBuf::from(loc).canonicalize().ok())
                .is_some_and(|loc| loc == exe_dir)
        })
}

#[cfg(not(windows))]
pub fn is_installed_copy() -> bool {
    false
}

/// Check the Authenticode signature of `path`: the file must be unchanged since signing and
/// signed by the certificate with [`THUMBPRINT`]. Returns the signer's name.
///
/// The certificate is self-signed, so Windows reports its root as "not trusted" unless the
/// user imported it. That one result is accepted because the certificate itself is pinned;
/// every other failure (changed file, bad or missing signature, distrusted certificate) is not.
#[cfg(windows)]
pub fn verify_signature(path: &Path) -> Result<String, String> {
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::null_mut;
    use windows_sys::Win32::Security::Cryptography::{
        CERT_NAME_SIMPLE_DISPLAY_TYPE, CERT_SHA1_HASH_PROP_ID, CertGetCertificateContextProperty,
        CertGetNameStringW,
    };
    use windows_sys::Win32::Security::WinTrust::{
        WINTRUST_ACTION_GENERIC_VERIFY_V2, WINTRUST_DATA, WINTRUST_DATA_0, WINTRUST_FILE_INFO,
        WTD_CACHE_ONLY_URL_RETRIEVAL, WTD_CHOICE_FILE, WTD_REVOKE_NONE, WTD_STATEACTION_CLOSE,
        WTD_STATEACTION_VERIFY, WTD_UI_NONE, WTHelperGetProvCertFromChain,
        WTHelperGetProvSignerFromChain, WTHelperProvDataFromStateData, WinVerifyTrust,
    };

    const CERT_E_UNTRUSTEDROOT: i32 = 0x800B0109_u32 as i32;
    const TRUST_E_NOSIGNATURE: i32 = 0x800B0100_u32 as i32;
    const TRUST_E_BAD_DIGEST: i32 = 0x80096010_u32 as i32;

    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut file = WINTRUST_FILE_INFO {
        cbStruct: size_of::<WINTRUST_FILE_INFO>() as u32,
        pcwszFilePath: wide.as_ptr(),
        hFile: null_mut(),
        pgKnownSubject: null_mut(),
    };
    // SAFETY: WINTRUST_DATA is a plain C struct; all-zero is a valid starting value.
    let mut data: WINTRUST_DATA = unsafe { std::mem::zeroed() };
    data.cbStruct = size_of::<WINTRUST_DATA>() as u32;
    data.dwUIChoice = WTD_UI_NONE;
    data.fdwRevocationChecks = WTD_REVOKE_NONE;
    data.dwUnionChoice = WTD_CHOICE_FILE;
    data.Anonymous = WINTRUST_DATA_0 { pFile: &mut file };
    data.dwStateAction = WTD_STATEACTION_VERIFY;
    // Never go online from here: the check must not depend on (or leak to) other servers.
    data.dwProvFlags = WTD_CACHE_ONLY_URL_RETRIEVAL;
    let mut action = WINTRUST_ACTION_GENERIC_VERIFY_V2;

    // SAFETY: `data`, `file`, `wide` and `action` outlive both calls; the state is closed below.
    let status = unsafe { WinVerifyTrust(null_mut(), &mut action, (&raw mut data).cast()) };
    let result = (|| {
        match status {
            0 | CERT_E_UNTRUSTEDROOT => {}
            TRUST_E_NOSIGNATURE => return Err("The installer is not signed.".to_string()),
            TRUST_E_BAD_DIGEST => {
                return Err("The installer was changed after it was signed.".to_string());
            }
            other => {
                return Err(format!(
                    "The installer's signature is not valid (error 0x{:08X}).",
                    other as u32
                ));
            }
        }
        // SAFETY: the state data is valid until WTD_STATEACTION_CLOSE; each pointer is checked.
        unsafe {
            let prov = WTHelperProvDataFromStateData(data.hWVTStateData);
            if prov.is_null() {
                return Err("Could not read the installer's signature.".into());
            }
            let signer = WTHelperGetProvSignerFromChain(prov, 0, 0, 0);
            if signer.is_null() {
                return Err("Could not read the installer's signer.".into());
            }
            let cert = WTHelperGetProvCertFromChain(signer, 0);
            if cert.is_null() || (*cert).pCert.is_null() {
                return Err("Could not read the signing certificate.".into());
            }
            let ctx = (*cert).pCert;
            let mut hash = [0u8; 20];
            let mut len = hash.len() as u32;
            if CertGetCertificateContextProperty(
                ctx,
                CERT_SHA1_HASH_PROP_ID,
                hash.as_mut_ptr().cast(),
                &mut len,
            ) == 0
            {
                return Err("Could not read the signing certificate.".into());
            }
            let thumb: String = hash[..len as usize]
                .iter()
                .map(|b| format!("{b:02X}"))
                .collect();
            if thumb != THUMBPRINT {
                return Err(format!(
                    "The installer is signed with a different certificate ({thumb})."
                ));
            }
            let mut name = [0u16; 256];
            let n = CertGetNameStringW(
                ctx,
                CERT_NAME_SIMPLE_DISPLAY_TYPE,
                0,
                std::ptr::null(),
                name.as_mut_ptr(),
                name.len() as u32,
            );
            Ok(String::from_utf16_lossy(
                &name[..(n as usize).saturating_sub(1)],
            ))
        }
    })();
    data.dwStateAction = WTD_STATEACTION_CLOSE;
    // SAFETY: closes the state opened above.
    unsafe { WinVerifyTrust(null_mut(), &mut action, (&raw mut data).cast()) };
    result
}

#[cfg(not(windows))]
pub fn verify_signature(_path: &Path) -> Result<String, String> {
    Err("Signature checks are only available on Windows.".into())
}

/// Release notes reduced to plain lines for display: headings, bullets and text, with
/// Markdown links and emphasis removed. Returns `(is_heading, is_bullet, text)`.
pub fn notes_lines(notes: &str) -> Vec<(bool, bool, String)> {
    let mut out = Vec::new();
    for raw in notes.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("<!--") {
            continue;
        }
        let (heading, bullet, text) = if let Some(t) = line.strip_prefix('#') {
            (true, false, t.trim_start_matches('#').trim())
        } else if let Some(t) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
            (false, true, t)
        } else {
            (false, false, line)
        };
        out.push((heading, bullet, plain_markdown(text)));
    }
    out
}

fn plain_markdown(s: &str) -> String {
    // [text](url) -> text
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find('[') {
        let Some(mid) = rest[start..].find("](") else {
            break;
        };
        let Some(end) = rest[start + mid..].find(')') else {
            break;
        };
        out.push_str(&rest[..start]);
        out.push_str(&rest[start + 1..start + mid]);
        rest = &rest[start + mid + end + 1..];
    }
    out.push_str(rest);
    out.replace("**", "").replace('`', "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert_eq!(parse_version("v1.2.3"), Some([1, 2, 3]));
        assert_eq!(parse_version("0.10.0"), Some([0, 10, 0]));
        assert_eq!(parse_version("1.2"), None);
        assert_eq!(parse_version("1.2.3-beta"), None);
        assert!(is_newer("999.0.0"));
        assert!(!is_newer("0.0.1"));
        assert!(!is_newer(CURRENT));
    }

    #[test]
    fn checksums() {
        let h = "a".repeat(64);
        let sums = format!(
            "{h}  PCScanner-Setup-9.0.0-x64.exe\r\n{}  other.zip\n",
            "b".repeat(64)
        );
        assert_eq!(
            expected_hash(&sums, "PCScanner-Setup-9.0.0-x64.exe"),
            Some(h)
        );
        assert_eq!(expected_hash(&sums, "missing.exe"), None);
        assert_eq!(expected_hash("xyz  a.exe", "a.exe"), None);
    }

    fn api(tag: &str, url_base: &str) -> ApiRelease {
        let v = tag.trim_start_matches('v');
        ApiRelease {
            tag_name: tag.into(),
            draft: false,
            prerelease: false,
            body: Some("notes".into()),
            html_url: "https://github.com/x".into(),
            published_at: Some("2026-11-15T10:00:00Z".into()),
            assets: vec![
                ApiAsset {
                    name: format!("PCScanner-Setup-{v}-x64.exe"),
                    browser_download_url: format!("{url_base}{tag}/PCScanner-Setup-{v}-x64.exe"),
                    size: 10,
                },
                ApiAsset {
                    name: "SHA256SUMS.txt".into(),
                    browser_download_url: format!("{url_base}{tag}/SHA256SUMS.txt"),
                    size: 1,
                },
            ],
        }
    }

    #[test]
    fn releases() {
        let ok = release_from_api(api("v999.0.0", &download_prefix()))
            .unwrap()
            .unwrap();
        assert_eq!(ok.version, "999.0.0");
        assert!(ok.published.is_some());
        // Same or older version: nothing to do.
        assert!(
            release_from_api(api(&format!("v{CURRENT}"), &download_prefix()))
                .unwrap()
                .is_none()
        );
        // Assets hosted elsewhere are refused.
        assert!(release_from_api(api("v999.0.0", "https://example.com/")).is_err());
        // Drafts and pre-releases are ignored.
        let mut draft = api("v999.0.0", &download_prefix());
        draft.draft = true;
        assert!(release_from_api(draft).unwrap().is_none());
    }

    #[test]
    fn notes() {
        let lines =
            notes_lines("## What's new\n* **Faster** scans by [@x](https://x)\n\ntext `code`");
        assert_eq!(lines[0], (true, false, "What's new".into()));
        assert_eq!(lines[1], (false, true, "Faster scans by @x".into()));
        assert_eq!(lines[2], (false, false, "text code".into()));
    }

    /// Asks the real GitHub API (network). Run with `cargo test -- --ignored live_check`.
    #[test]
    #[ignore]
    fn live_check() {
        println!("{:?}", check().map(|r| r.map(|r| r.version)));
    }

    /// Verifies a real signed file when `PCSCANNER_SIGNED_FILE` points at one.
    #[test]
    #[ignore]
    fn signature_of_release_file() {
        let path = std::env::var("PCSCANNER_SIGNED_FILE").expect("set PCSCANNER_SIGNED_FILE");
        println!("{:?}", verify_signature(Path::new(&path)));
    }
}
