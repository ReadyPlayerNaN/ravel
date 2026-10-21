#!/usr/bin/env bash
set -euo pipefail

: "${AUR_SSH_PRIVATE_KEY:?Set the aur environment secret AUR_SSH_PRIVATE_KEY}"
: "${AUR_COMMIT_NAME:?Set the aur environment variable AUR_COMMIT_NAME}"
: "${AUR_COMMIT_EMAIL:?Set the aur environment variable AUR_COMMIT_EMAIL}"

repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
recipe="$repo/packaging/aur"
grep -qx 'pkgbase = ravel-git' "$recipe/.SRCINFO"
grep -qx 'pkgname = ravel-git' "$recipe/.SRCINFO"

# Pin the host key rather than trusting unauthenticated ssh-keyscan output.
fingerprint=$(ssh-keygen -lf "$recipe/known_hosts" | awk '{print $2}')
if [[ "$fingerprint" != 'SHA256:RFzBCUItH9LZS0cKB5UE6ceAYhBD5C8GeOBip8Z11+4' ]]; then
    echo 'Unexpected AUR host key; verify it against https://aur.archlinux.org/' >&2
    exit 1
fi

umask 077
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
printf '%s\n' "$AUR_SSH_PRIVATE_KEY" > "$work/key"
unset AUR_SSH_PRIVATE_KEY
printf -v GIT_SSH_COMMAND 'ssh -i %q -o UserKnownHostsFile=%q -o StrictHostKeyChecking=yes -o BatchMode=yes -o IdentitiesOnly=yes -o IdentityAgent=none' \
    "$work/key" "$recipe/known_hosts"
export GIT_SSH_COMMAND GIT_TERMINAL_PROMPT=0

# Cloning an empty repository is the normal initial AUR submission procedure.
git -c init.defaultBranch=master clone -- ssh://aur@aur.archlinux.org/ravel-git.git "$work/aur"
cd "$work/aur"
git checkout -B master
git config user.name "$AUR_COMMIT_NAME"
git config user.email "$AUR_COMMIT_EMAIL"
cp --remove-destination -- "$recipe/PKGBUILD" "$recipe/.SRCINFO" .
git add -- PKGBUILD .SRCINFO
if git diff --cached --quiet; then
    echo 'AUR recipe is unchanged; nothing to publish.'
    exit 0
fi

# AUR forbids version-only churn for VCS packages. Copy the reviewed metadata
# unchanged; pkgver() determines the source version when a user builds it.
git -c commit.gpgsign=false commit -m 'chore: update ravel-git package recipe'
git push origin HEAD:refs/heads/master
