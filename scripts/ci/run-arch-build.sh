#!/usr/bin/env bash
set -euo pipefail

: "${GITHUB_WORKSPACE:?Missing checkout directory}"
: "${GITHUB_SHA:?Missing tested commit}"
: "${RUNNER_TEMP:?Missing runner temporary directory}"

# GitHub's host runner owns private files under RUNNER_TEMP (notably event.json).
# Never chown that directory or the checkout to the container's build user.
owner=$(stat -c '%u:%g' "$RUNNER_TEMP")
output="$RUNNER_TEMP/ravel-package"
build=$(mktemp -d "$RUNNER_TEMP/ravel-build.XXXXXX")
mkdir -p "$output"
cleanup() {
    rm -rf -- "$build"
    # The host runner must be able to upload and clean up the package artifacts.
    chown -R "$owner" "$output"
}
trap cleanup EXIT

git clone --no-hardlinks -- "$GITHUB_WORKSPACE" "$build/source"
git -C "$build/source" checkout --detach "$GITHUB_SHA"
chown -R builder:builder "$build" "$output"
sudo -u builder --preserve-env=GITHUB_SHA,CARGO_BUILD_JOBS,RELEASE_TAG \
    bash -ec 'cd "$1"; exec bash scripts/ci/build-arch-package.sh "$2"' \
    _ "$build/source" "$output"
