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

## CI validation

After the Fedora formatting, workspace tests, Clippy and GTK smoke checks pass,
CI builds the Arch package in an `archlinux:base-devel` container as an unprivileged
user. It verifies `.SRCINFO`, builds and tests the exact CI commit (not a moving
remote branch), and checks the installed payload and linked libraries. The
resulting `.pkg.tar.zst` files are available as workflow artifacts for seven days.
Pull requests get the same build checks.

## Automatic AUR publication

The `publish-aur` job in `.github/workflows/ci.yml` runs only after **both** the
Rust/GTK checks and Arch package build succeed, and only for pushes to `master`
in `ReadyPlayerNaN/ravel`. Pull requests, other branches, forks, failed builds,
and superseded `master` commits cannot publish. Publications are serialized and
use ordinary fast-forward Git pushes, never force pushes.

One-time setup by the repository/AUR owner:

1. Create a dedicated, passphrase-free Ed25519 SSH key for this automation, for
   example `ssh-keygen -t ed25519 -f ~/.ssh/ravel-aur-ci -N ''`. Keep the private
   key out of this repository. Add the **public** key to your AUR account. That
   account must be allowed to create `ravel-git`, or maintain/co-maintain it if it
   already exists. A dedicated key can be revoked independently of your other keys.
2. In GitHub **Settings → Environments**, create an environment named **`aur`**.
   Restrict its deployment branches to `master`. Configure:
   - Secret **`AUR_SSH_PRIVATE_KEY`**: the complete private key, including its
     header/footer and newlines.
   - Variable **`AUR_COMMIT_NAME`**: the Git author name for AUR commits.
   - Variable **`AUR_COMMIT_EMAIL`**: the Git author email for AUR commits.
     These author details will be public in the AUR repository.
3. Merge/push this configuration to `master`. A successful CI run creates the
   initial AUR submission or updates the existing recipe. If credentials were
   missing, configure them and re-run the failed job while that commit is still
   the tip of `master`.

The publish job fails clearly when required credentials are missing. The build
and test jobs never receive the AUR secret. The publisher uses a pinned AUR
Ed25519 host key (`known_hosts`), verified against the fingerprint published on
<https://aur.archlinux.org/>; a host-key rotation requires a reviewed update.
Its temporary private key is removed on exit. There is no third-party publishing
action, and no package-provided build code runs in the secret-bearing job.

Only the reviewed `PKGBUILD` and `.SRCINFO` are pushed to the AUR. Unchanged recipes
are a no-op, even when upstream code changed; the Arch binaries are GitHub Actions
artifacts, **not** uploaded to the AUR. This follows the AUR rule against version-
only commits for VCS packages.

You can test the publisher without credentials or network access:

```sh
bash scripts/ci/test-publish-aur.sh
```

This uses a temporary local bare Git repository, a dummy key, and Git's protocol
allowlist to prevent any network transport. It covers initial creation, updates,
no-op runs, rejected pushes, host-key rejection, and temporary-key cleanup.

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
