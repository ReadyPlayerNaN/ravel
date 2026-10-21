# Arch Linux / AUR package

`PKGBUILD` builds `ravel-git` from upstream `master`, with a commit-count-based
`pkgver()`. It installs the release binary, desktop entry, documentation, and
bundled KaTeX license. It conflicts with/provides `ravel`.

## Local build

On an up-to-date Arch Linux installation, install `base-devel` and `git`, then
run as a regular user:

```sh
cd packaging/aur
makepkg --syncdeps --install
```

Rust 1.92 or newer is required by the workspace. `prepare()` downloads the locked
Cargo dependencies; building and testing use `--frozen` (offline and locked).
The GTK/WebKit display-dependent tests are covered by upstream CI separately.
Extra GStreamer codecs and `jq` are optional runtime dependencies.

The project currently has **no declared license**. The metadata therefore uses
`LicenseRef-unknown`; it does not imply permission to redistribute under MIT or
any other dependency's license. A project license should be chosen by its owner
and reflected here when available. KaTeX's MIT license is installed separately.

## Maintaining metadata

After changing `PKGBUILD`, regenerate the tracked metadata:

```sh
cd packaging/aur
makepkg --printsrcinfo > .SRCINFO
```

Do not commit `pkgver`-only bumps on every upstream change: this is a VCS package,
and `makepkg` calculates the current version when building. Follow the
[AUR submission guidelines](https://wiki.archlinux.org/title/AUR_submission_guidelines).
Bump `pkgrel` when packaging changes warrant a rebuild.

A successful CI run does not pin this VCS package to the last tested commit:
users building from the AUR will get the current `master`. The CI publishing gate
controls changes to the AUR recipe, not availability of upstream Git commits.
