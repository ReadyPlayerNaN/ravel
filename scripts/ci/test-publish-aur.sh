#!/usr/bin/env bash
set -euo pipefail

repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
fixture="$work/fixture"
remote="$work/aur.git"
mkdir -p "$fixture/scripts/ci" "$fixture/packaging/aur" "$work/scratch"
cp "$repo/scripts/ci/publish-aur.sh" "$fixture/scripts/ci/"
cp "$repo/packaging/aur/"{PKGBUILD,.SRCINFO,known_hosts} "$fixture/packaging/aur/"
publisher="$fixture/scripts/ci/publish-aur.sh"
cd "$work"

# Never contact the AUR or use the developer's Git/SSH configuration. Rewriting
# the fixed production URL exercises the real publisher against a local remote.
export GIT_CONFIG_GLOBAL="$work/gitconfig" GIT_CONFIG_NOSYSTEM=1
export GIT_ALLOW_PROTOCOL=file TMPDIR="$work/scratch"
export AUR_SSH_PRIVATE_KEY='not-a-real-key'
export AUR_COMMIT_NAME='Publisher Test' AUR_COMMIT_EMAIL='publisher@example.invalid'
git -c init.defaultBranch=master init --bare "$remote"
git config --global "url.file://$remote.insteadOf" ssh://aur@aur.archlinux.org/ravel-git.git

if env -u AUR_SSH_PRIVATE_KEY bash "$publisher" > "$work/missing-key.log" 2>&1; then
    echo 'Publication unexpectedly accepted missing credentials' >&2
    exit 1
fi

# Initial submission to an empty AUR repository.
bash "$publisher"
first=$(git --git-dir="$remote" rev-parse master)
git --git-dir="$remote" show master:PKGBUILD > "$work/published-pkgbuild"
git --git-dir="$remote" show master:.SRCINFO > "$work/published-srcinfo"
cmp "$fixture/packaging/aur/PKGBUILD" "$work/published-pkgbuild"
cmp "$fixture/packaging/aur/.SRCINFO" "$work/published-srcinfo"
test "$(git --git-dir="$remote" ls-tree --name-only master | wc -l)" -eq 2
test -z "$(find "$work/scratch" -mindepth 1 -print -quit)"

# Code-only upstream changes must not produce AUR version-bump commits.
bash "$publisher"
test "$(git --git-dir="$remote" rev-parse master)" = "$first"

# A reviewed recipe change becomes a normal fast-forward update.
printf '\n# Updated packaging fixture\n' >> "$fixture/packaging/aur/PKGBUILD"
bash "$publisher"
second=$(git --git-dir="$remote" rev-parse master)
test "$second" != "$first"
git --git-dir="$remote" merge-base --is-ancestor "$first" "$second"
git --git-dir="$remote" show master:PKGBUILD > "$work/published-pkgbuild"
cmp "$fixture/packaging/aur/PKGBUILD" "$work/published-pkgbuild"

# A push failure must fail the job, without leaving a private key behind.
printf '#!/bin/sh\nexit 1\n' > "$remote/hooks/pre-receive"
chmod +x "$remote/hooks/pre-receive"
printf '\n# Rejected update fixture\n' >> "$fixture/packaging/aur/PKGBUILD"
if bash "$publisher" > "$work/rejected-push.log" 2>&1; then
    echo 'Publication unexpectedly succeeded despite a rejected push' >&2
    exit 1
fi
test "$(git --git-dir="$remote" rev-parse master)" = "$second"
test -z "$(find "$work/scratch" -mindepth 1 -print -quit)"

# Do not silently trust a replacement host key.
printf 'invalid host key\n' > "$fixture/packaging/aur/known_hosts"
if bash "$publisher" > "$work/host-key.log" 2>&1; then
    echo 'Publication unexpectedly accepted an invalid host key' >&2
    exit 1
fi

echo 'AUR publisher tests passed (local Git remote only).'
