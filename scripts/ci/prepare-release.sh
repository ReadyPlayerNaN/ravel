#!/usr/bin/env bash
set -euo pipefail

package=${1:?Usage: prepare-release.sh ARCH_PACKAGE OUTPUT_DIRECTORY TAG}
output=${2:?Missing output directory}
tag=${3:?Missing release tag}
: "${GITHUB_SHA:?Missing tested commit}"
: "${SOURCE_DATE_EPOCH:?Missing commit timestamp}"
repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
asset=$(python3 "$repo/scripts/ci/release.py" asset "$tag")
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$output" "$work/payload" "$work/bundle"
output=$(realpath "$output")
bsdtar -xf "$package" -C "$work/payload"
cd "$work"
install -Dm755 payload/usr/bin/ravel bundle/bin/ravel
install -Dm644 payload/usr/share/applications/global.zak.ravel.desktop \
    bundle/share/applications/global.zak.ravel.desktop
install -Dm644 payload/usr/share/doc/ravel-git/README.md bundle/share/doc/ravel/README.md
install -Dm644 payload/usr/share/licenses/ravel-git/KaTeX-LICENSE \
    bundle/share/licenses/ravel/KaTeX-LICENSE
install -Dm644 payload/usr/share/licenses/ravel-git/Highlight.js-LICENSE \
    bundle/share/licenses/ravel/Highlight.js-LICENSE

tar --sort=name --mtime="@$SOURCE_DATE_EPOCH" --owner=0 --group=0 --numeric-owner \
    -C bundle -cf - bin share | gzip -n > "$output/$asset"
python3 "$repo/scripts/ci/release.py" generate "$tag" "$GITHUB_SHA" "$output/$asset" "$output"

if [[ -d "$output/aur" ]]; then
    # No Rust compilation: check that the exact release asset, checksum and
    # generated binary recipe can produce an installable package offline.
    cd "$output/aur"
    makepkg --printsrcinfo > .SRCINFO
    mkdir "$work/check"
    cp PKGBUILD .SRCINFO "$output/$asset" "$work/check/"
    cd "$work/check"
    PKGDEST="$work" makepkg --nodeps --noconfirm
    binary_package=$(find "$work" -maxdepth 1 -name 'ravel-bin-*.pkg.tar.zst' -print -quit)
    test -n "$binary_package"
    mkdir "$work/installed"
    bsdtar -xf "$binary_package" -C "$work/installed"
    cmp "$work/bundle/bin/ravel" "$work/installed/usr/bin/ravel"
    desktop-file-validate "$work/installed/usr/share/applications/global.zak.ravel.desktop"
    test -s "$work/installed/usr/share/licenses/ravel-bin/KaTeX-LICENSE"
    test -s "$work/installed/usr/share/licenses/ravel-bin/Highlight.js-LICENSE"
fi
