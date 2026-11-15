# Privacy Policy

_Last updated: 15 November 2026_

PC Scanner is a desktop application that runs entirely on your computer.

**This program will not transfer any information to other networked systems unless specifically
requested by the user or the person installing or operating it.**

## What PC Scanner reads

To show you what is using disk space, PC Scanner reads on your own computer:

- the names, sizes and modification dates of files and folders on the drives or folders you
  choose to scan;
- the list of installed programs from the Windows registry (name, version, publisher, install
  date, install location, size and uninstall command);
- the size and free space of your drives.

When you use **Duplicates**, it reads the contents of files of the same size to compare them.

## What PC Scanner does with it

- Everything is kept in memory while the program is running, and discarded when you close it.
- Nothing about your files, programs or computer is uploaded, shared or sent anywhere. PC Scanner
  has no telemetry, no analytics, no crash reporting, no advertising and no accounts.
- PC Scanner writes exports (CSV or JSON) only to the location you pick, and keeps its own
  preferences in `%APPDATA%\PC Scanner\settings.json`: theme, accent colour, whether to check for
  updates, when it last checked and which version you chose to skip. Nothing else is stored there.
- Files are only moved to the Recycle Bin or deleted after you confirm it in a dialog.
- Opening a file or folder hands it to the program Windows has registered for it (for example
  File Explorer). Running an uninstaller starts the uninstall command the program registered
  with Windows.

## Update checks

The update check is the only time PC Scanner connects to the internet, and it only happens if you
allow it:

- The first time PC Scanner starts, it asks whether it may check for updates. Until you answer
  **Check for updates**, no network request is made. You can turn this off at any time in
  **Settings**, or check by hand with **Check now**.
- When allowed, PC Scanner asks GitHub (`api.github.com`) for the latest release of
  `Thedevelop3r/pcscanner`, at most once a day when it starts. The request contains only the
  program name and version in its user agent (`PC-Scanner-Updater/<version>`). No file names,
  scan results, programs, account, machine or user details are sent.
- If you choose **Install and restart**, the installer and `SHA256SUMS.txt` are downloaded from
  the release on GitHub. The installer is used only if its SHA-256 matches and it is signed with
  the project's certificate (see [SIGNING.md](SIGNING.md)); otherwise it is deleted.
- Like any website, GitHub sees your IP address when PC Scanner contacts it. GitHub's handling of
  that is covered by the
  [GitHub Privacy Statement](https://docs.github.com/site-policy/privacy-policies/github-general-privacy-statement).
  In demo mode (`--demo`) PC Scanner makes no network requests at all.
- The Microsoft Store version never checks for updates itself and makes no network requests;
  the Microsoft Store keeps it up to date.

## Third-party components and services

PC Scanner is built from open source Rust libraries that are compiled into the program. Apart from
the HTTPS client used for the update check described above, none of them sends data over the
network. The full list is in [`Cargo.lock`](Cargo.lock).

Releases are signed with the project's own certificate when they are built on GitHub (see
[SIGNING.md](SIGNING.md)). Signing does not involve your computer or your data.

Downloading PC Scanner from GitHub is subject to the
[GitHub Privacy Statement](https://docs.github.com/site-policy/privacy-policies/github-general-privacy-statement).

## Contact

Questions about privacy: open an issue at <https://github.com/Thedevelop3r/pcscanner/issues>.
