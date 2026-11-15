# Code signing

PC Scanner releases are signed with the project's own **self-signed** code signing certificate,
created with OpenSSL ([`scripts/create-signing-cert.sh`](scripts/create-signing-cert.sh)).

| | |
|---|---|
| Subject | `CN=Bilal Amjad, O=Bilal Amjad (PC Scanner)` |
| Public certificate | [`signing/pcscanner-codesign.cer`](signing/pcscanner-codesign.cer) |
| SHA-1 thumbprint | `A7AAB0826FF0EE4B5CC7BC4B78722CE255D0BC2E` |
| SHA-256 fingerprint | `A0C0D5CCF96128F8412C62988A29799617DDFB02D9CAA063BE1EE7472E3F5541` |
| Valid | 15 Nov 2026 to 14 Nov 2031 |

## What is signed

- `pcscanner.exe`, the application;
- `PCScanner-Setup-<version>-x64.exe`, the installer, and the uninstaller inside it.

Signing happens only in the [Release workflow](.github/workflows/release.yml) on GitHub-hosted
runners, for version tags built from this repository. Signatures use SHA-256 and an RFC 3161
timestamp, so they stay valid after the certificate expires.

## Why Windows still shows a warning

The certificate is not issued by a commercial certificate authority, so Windows does not trust it
by default. SmartScreen shows "Windows protected your PC" and the publisher appears as unverified.
To run it, choose **More info → Run anyway**.

The signature still lets you check that a download was built by this project and has not been
changed. If you prefer, build PC Scanner yourself from source (see the [README](README.md)).

## Verify a download

In PowerShell:

```powershell
Get-AuthenticodeSignature .\PCScanner-Setup-0.1.0-x64.exe | Format-List Status, SignerCertificate, TimeStamperCertificate
```

`Status` is `UnknownError` ("root certificate which is not trusted"); that is expected for a
self-signed certificate. Check that `SignerCertificate` shows the SHA-1 thumbprint above. You
can also compare file hashes with `SHA256SUMS.txt` from the release:

```powershell
Get-FileHash .\PCScanner-Setup-0.1.0-x64.exe -Algorithm SHA256
```

## Trusting the certificate (optional)

Only do this if you trust this project. Importing the certificate makes Windows treat files signed
with it as trusted on your computer:

```powershell
Import-Certificate -FilePath .\pcscanner-codesign.cer -CertStoreLocation Cert:\CurrentUser\TrustedPublisher
Import-Certificate -FilePath .\pcscanner-codesign.cer -CertStoreLocation Cert:\CurrentUser\Root
```

Remove it again from `certmgr.msc` (Trusted Publishers and Trusted Root Certification
Authorities). SmartScreen reputation is separate and may still warn.

## Maintainer

[Bilal Amjad](https://github.com/Thedevelop3r) owns the certificate. The private key is kept
offline and in the repository's GitHub Actions secrets only; it is never committed.
