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
- Nothing is uploaded, shared or sent anywhere. PC Scanner has no network code, no telemetry,
  no analytics, no crash reporting, no advertising and no accounts.
- PC Scanner only writes files when you ask it to: CSV or JSON exports are saved to the location
  you pick.
- Files are only moved to the Recycle Bin or deleted after you confirm it in a dialog.
- Opening a file or folder hands it to the program Windows has registered for it (for example
  File Explorer). Running an uninstaller starts the uninstall command the program registered
  with Windows.

## Third-party components and services

PC Scanner is built from open source Rust libraries that are compiled into the program. None of
them sends data over the network as used by PC Scanner. The full list is in
[`Cargo.lock`](Cargo.lock).

Releases are signed with the project's own certificate when they are built on GitHub (see
[SIGNING.md](SIGNING.md)). Signing does not involve your computer or your data.

Downloading PC Scanner from GitHub is subject to the
[GitHub Privacy Statement](https://docs.github.com/site-policy/privacy-policies/github-general-privacy-statement).

## Contact

Questions about privacy: open an issue at <https://github.com/Thedevelop3r/pcscanner/issues>.
