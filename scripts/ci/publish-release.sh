#!/usr/bin/env bash
set -euo pipefail

: "${RELEASE_TAG:?Missing release tag}"
: "${GITHUB_SHA:?Missing tested commit}"
: "${GITHUB_REPOSITORY:?Missing GitHub repository}"
directory=$(realpath "${1:?Usage: publish-release.sh RELEASE_DIRECTORY}")
repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
python3 "$repo/scripts/ci/release.py" verify "$RELEASE_TAG" "$GITHUB_SHA" "$directory"
asset=$(python3 "$repo/scripts/ci/release.py" asset "$RELEASE_TAG")
prerelease=$(jq -r .prerelease "$directory/release.json")

# Refuse moved/deleted tags, even if their earlier CI run passed.
git fetch --no-tags origin "refs/tags/$RELEASE_TAG"
test "$(git rev-parse 'FETCH_HEAD^{commit}')" = "$GITHUB_SHA"

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
if ! gh release view "$RELEASE_TAG" --repo "$GITHUB_REPOSITORY" --json isDraft,isPrerelease > "$work/state.json"; then
    flags=()
    if [[ "$prerelease" == true ]]; then
        flags+=(--prerelease)
    fi
    gh release create "$RELEASE_TAG" "$directory/$asset" "$directory/SHA256SUMS" \
        --repo "$GITHUB_REPOSITORY" --verify-tag --target "$GITHUB_SHA" \
        --title "$RELEASE_TAG" --latest=false "${flags[@]}" \
        --notes 'Prebuilt x86-64 Arch Linux binary. Requires system GTK4, Libadwaita, GtkSourceView, WebKitGTK and GStreamer libraries. SHA256SUMS is included; stable releases are also packaged as ravel-bin in AUR.'
    gh release view "$RELEASE_TAG" --repo "$GITHUB_REPOSITORY" --json isDraft,isPrerelease > "$work/state.json"
fi

# Reruns may finish an interrupted AUR publication, but cannot silently replace
# previously published binaries. A differing rebuild requires a new release tag.
test "$(jq -r .isDraft "$work/state.json")" = false
test "$(jq -r .isPrerelease "$work/state.json")" = "$prerelease"
gh release download "$RELEASE_TAG" --repo "$GITHUB_REPOSITORY" \
    --pattern "$asset" --pattern SHA256SUMS --dir "$work"
cmp "$directory/$asset" "$work/$asset"
cmp "$directory/SHA256SUMS" "$work/SHA256SUMS"

# Finishing an older release later must not steal the Latest badge.
gh api --paginate "repos/$GITHUB_REPOSITORY/releases" \
    --jq '.[] | select(.draft == false and .prerelease == false) | .tag_name' > "$work/tags"
latest=$(python3 "$repo/scripts/ci/release.py" latest "$RELEASE_TAG" < "$work/tags")
if [[ "$latest" == true ]]; then
    gh release edit "$RELEASE_TAG" --repo "$GITHUB_REPOSITORY" --latest
fi
