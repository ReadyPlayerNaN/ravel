#!/usr/bin/env bash
set -euo pipefail

repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/repo/scripts/ci" "$work/repo/packaging/aur" "$work/recipe"
cp "$repo/scripts/ci/"{publish-aur.sh,release.py} "$work/repo/scripts/ci/"
cp "$repo/packaging/aur/known_hosts" "$work/repo/packaging/aur/"
publisher="$work/repo/scripts/ci/publish-aur.sh"
cd "$work"
export GIT_CONFIG_GLOBAL="$work/gitconfig" GIT_CONFIG_NOSYSTEM=1 GIT_ALLOW_PROTOCOL=file
export AUR_SSH_PRIVATE_KEY='not-a-real-key'
export AUR_COMMIT_NAME='Publisher Test' AUR_COMMIT_EMAIL='publisher@example.invalid'
git -c init.defaultBranch=master init --bare "$work/aur.git"
git config --global "url.file://$work/aur.git.insteadOf" ssh://aur@aur.archlinux.org/ravel-bin.git

recipe() {
    printf 'pkgname=ravel-bin\npkgver=%s\npkgrel=1\n' "$1" > "$work/recipe/PKGBUILD"
    printf 'pkgbase = ravel-bin\n\tpkgver = %s\n\tpkgrel = 1\npkgname = ravel-bin\n' "$1" > "$work/recipe/.SRCINFO"
}

recipe 1.9.0
bash "$publisher" ravel-bin "$work/recipe"
first=$(git --git-dir="$work/aur.git" rev-parse master)
recipe 1.10.0
bash "$publisher" ravel-bin "$work/recipe"
second=$(git --git-dir="$work/aur.git" rev-parse master)
test "$first" != "$second"
git --git-dir="$work/aur.git" merge-base --is-ancestor "$first" "$second"

# Late-finishing old releases and retries cannot downgrade/replace binaries.
for version in 1.9.1 1.10.0; do
    recipe "$version"
    bash "$publisher" ravel-bin "$work/recipe"
    test "$(git --git-dir="$work/aur.git" rev-parse master)" = "$second"
done

recipe 2.0.0-rc.1
if bash "$publisher" ravel-bin "$work/recipe" > "$work/prerelease.log" 2>&1; then
    echo 'Publisher accepted a prerelease for the stable AUR package' >&2
    exit 1
fi
if bash "$publisher" unrelated-package "$work/recipe" > "$work/name.log" 2>&1; then
    echo 'Publisher accepted an unexpected package name' >&2
    exit 1
fi
test "$(git --git-dir="$work/aur.git" rev-parse master)" = "$second"
echo 'Binary AUR publisher tests passed (local Git remote only).'
