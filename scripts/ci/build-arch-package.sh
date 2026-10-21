#!/usr/bin/env bash
set -euo pipefail

# Run as a regular user in an Arch container with base-devel and passwordless
# sudo for pacman. The caller supplies a directory for the built package files.
output=${1:?Usage: build-arch-package.sh OUTPUT_DIRECTORY}
repo=$(git rev-parse --show-toplevel)
revision=$(git -C "$repo" rev-parse "${GITHUB_SHA:-HEAD}^{commit}")
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$output"
output=$(realpath "$output")

cd "$repo/packaging/aur"
makepkg --printsrcinfo > "$work/SRCINFO"
diff -u .SRCINFO "$work/SRCINFO"
# makepkg supplies srcdir/pkgdir and consumes the otherwise-unused metadata.
shellcheck --shell=bash --exclude=SC2034,SC2154 PKGBUILD

cp PKGBUILD "$work/PKGBUILD"
# Test precisely the CI checkout (including PR merge commits), not a moving
# remote master. Only this disposable copy gets a source override.
printf '\nsource=(%q)\n' "ravel::git+file://$repo#commit=$revision" >> "$work/PKGBUILD"
cd "$work"
export PKGDEST="$output"
makepkg --syncdeps --noconfirm --cleanbuild

# Check the install payload without installing the application into the runner.
package=$(find "$output" -maxdepth 1 -name 'ravel-git-*.pkg.tar.zst' ! -name 'ravel-git-debug-*' -print -quit)
test -n "$package"
mkdir payload
bsdtar -xf "$package" -C payload
test -x payload/usr/bin/ravel
test -s payload/usr/share/licenses/ravel-git/KaTeX-LICENSE
desktop-file-validate payload/usr/share/applications/io.github.ravel.Ravel.desktop
ldd payload/usr/bin/ravel > linked-libraries.txt
if grep -F 'not found' linked-libraries.txt; then
    echo 'Packaged binary has unresolved native libraries' >&2
    exit 1
fi
