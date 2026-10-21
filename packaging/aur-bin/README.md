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

## Publishing a release

The existing `CI` workflow handles both branches and tags. On a valid release-tag
push in `ReadyPlayerNaN/ravel`, it:

1. Checks that the tagged commit belongs to `master`.
2. Runs formatting, workspace tests, Clippy and the GTK smoke test.
3. Builds the exact commit in Arch, extracts its stripped executable and support
   files into `ravel-vVERSION-linux-x86_64.tar.gz`, and generates `SHA256SUMS`.
   Stable releases also get an offline `makepkg` test of the binary recipe.
4. Publishes the archive and checksums on GitHub Releases, using the built-in
   `GITHUB_TOKEN` with `contents: write` only in that job. Tags are checked again
   before publication; moved or deleted tags are refused.
5. For stable tags only, publishes the generated `ravel-bin` recipe to AUR with
   the existing **`aur` environment** secret and author variables. The account
   must be allowed to create/maintain `ravel-bin`, as well as `ravel-git`.

**One additional GitHub setting is required:** in **Settings → Environments →
`aur` → Deployment branches and tags**, retain the `master` branch rule and add
an allowed **tag** rule `v*`. A branch-only rule would block binary publication.
No new SSH secret or personal access token is needed. Protect release tags from
unauthorized creation, modification and deletion using GitHub rulesets.

After merging the workflow into `master`, for example:

```sh
git switch master
git pull --ff-only
git tag -a v0.1.0 -m 'Release 0.1.0'
git push origin v0.1.0
```

Only tag a version you intend to publish. Keep the workspace's Cargo version in
sync when preparing releases. Non-SemVer tags still get normal CI but do not
publish anything. Branch pushes and pull requests cannot publish binary releases.
Prerelease tags create GitHub prereleases and leave the stable AUR package alone.

## Retries and ordering

GitHub assets are never overwritten with `--clobber`. Reruns verify already
published assets byte-for-byte, and fail rather than replace a different binary.
If AUR publication fails after GitHub publication, fix the credentials/access and
**re-run the failed jobs**, retaining the original release artifact (30-day
retention). Do not move the tag or rebuild a published release to replace it;
use a new version for changed binaries.

Publication is serialized. Older releases finishing late cannot downgrade the
AUR package or replace a newer GitHub Latest release. AUR compares numeric
`major.minor.patch` and `pkgrel`; equal versions are a no-op. Build metadata alone
does not upgrade AUR users, since SemVer ignores it for precedence.

## Verification

Offline metadata and publication tests:

```sh
python3 scripts/ci/test_release.py
python3 scripts/ci/test_publish_release.py
bash scripts/ci/test-publish-bin.sh
```
