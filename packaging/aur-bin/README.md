# Binary release package

`ravel-bin` installs a prebuilt **x86-64 Arch Linux** release binary. It provides
`ravel` and conflicts with `ravel` and `ravel-git`. Neither Cargo nor Rust is
required to install it. Shared GTK, Libadwaita, GtkSourceView, WebKitGTK and
GStreamer libraries are still required; this is not a portable/static Linux
binary. Use an up-to-date Arch installation. ARM users can build `ravel-git`.

`PKGBUILD.in` is a template, not an installable package recipe. Release preparation
fills in the version, immutable GitHub release URL and actual SHA-256 digest,
generates `.SRCINFO` with `makepkg`, and tests packaging the same archive offline.
Only that generated recipe and `.SRCINFO` go to AUR; no binaries are uploaded there.

Tags must be complete, case-sensitive `v`-prefixed SemVer, such as `v1.2.3`.
Prerelease tags such as `v1.3.0-rc.1` generate binary archives but no stable AUR
recipe. Build metadata (`v1.2.3+build.4`) is preserved in the release URL and asset
name but is not part of Arch's numeric `pkgver`.

The archive contains the executable, desktop entry, README and bundled KaTeX
license. As with `ravel-git`, the project's own license is currently undeclared
(`LicenseRef-unknown`), not inferred from dependency licenses.

Offline metadata tests:

```sh
python3 scripts/ci/test_release.py
```
