# Code signing policy

Free code signing provided by [SignPath.io](https://about.signpath.io), certificate by
[SignPath Foundation](https://signpath.org).

## What is signed

Only binaries built from the source code in this repository are signed:

- `pcscanner.exe`, the application;
- `PCScanner-Setup-<version>-x64.exe`, the Windows installer.

Both are built by the [Release workflow](.github/workflows/release.yml) on GitHub-hosted runners,
from a version tag (`v*`) in this repository. Signing is never done on a personal computer.
Third-party libraries are compiled from source into the application; no separately signed
third-party binaries are shipped.

## Team roles

| Role | Members |
|------|---------|
| Committers and reviewers | [Bilal Amjad](https://github.com/Thedevelop3r) |
| Approvers | [Bilal Amjad](https://github.com/Thedevelop3r) |

- **Committers** may change the source code without further review.
- **Reviewers** review every change from contributors outside the team (pull requests) before it
  is merged.
- **Approvers** manually approve every signing request in SignPath before a release is signed.

All team members use multi-factor authentication for GitHub and SignPath.

## Privacy policy

This program will not transfer any information to other networked systems unless specifically
requested by the user or the person installing or operating it. See the full
[privacy policy](PRIVACY.md).
