#!/usr/bin/env bash
set -euo pipefail

# Run as root in the Arch CI container, after creating the builder account.
repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
chmod 755 "$work"
export GITHUB_WORKSPACE="$work/checkout" RUNNER_TEMP="$work/runner-temp"
export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 GIT_ALLOW_PROTOCOL=file
mkdir -p "$GITHUB_WORKSPACE/scripts/ci" "$RUNNER_TEMP/_github_workflow"
printf '{}\n' > "$RUNNER_TEMP/_github_workflow/event.json"
# GitHub's runner UID need not match builder's UID inside the container.
runner_uid=$(( $(id -u builder) + 1 ))
chown -R "$runner_uid:$runner_uid" "$RUNNER_TEMP"
chmod 600 "$RUNNER_TEMP/_github_workflow/event.json"

# A tiny build fixture exercises ownership without compiling Rust again.
cat > "$GITHUB_WORKSPACE/scripts/ci/build-arch-package.sh" <<'BUILD'
#!/usr/bin/env bash
set -euo pipefail
test "$(id -un)" = builder
test "$(git rev-parse HEAD)" = "$GITHUB_SHA"
printf 'package\n' > "$1/ravel-git.pkg.tar.zst"
BUILD
git -C "$GITHUB_WORKSPACE" -c init.defaultBranch=master init
git -C "$GITHUB_WORKSPACE" config user.name 'CI Test'
git -C "$GITHUB_WORKSPACE" config user.email 'ci@example.invalid'
git -C "$GITHUB_WORKSPACE" add .
git -C "$GITHUB_WORKSPACE" commit -m 'test: add build fixture'
GITHUB_SHA=$(git -C "$GITHUB_WORKSPACE" rev-parse HEAD)
export GITHUB_SHA

# The source checkout advances, but the wrapper must still build GITHUB_SHA.
printf 'exit 7\n' >> "$GITHUB_WORKSPACE/scripts/ci/build-arch-package.sh"
git -C "$GITHUB_WORKSPACE" commit -am 'test: add failing build fixture'
failing_sha=$(git -C "$GITHUB_WORKSPACE" rev-parse HEAD)

before=$(stat -c '%u:%g:%a' "$RUNNER_TEMP" "$RUNNER_TEMP/_github_workflow/event.json" "$GITHUB_WORKSPACE")
bash "$repo/scripts/ci/run-arch-build.sh"
test "$(stat -c '%u:%g:%a' "$RUNNER_TEMP" "$RUNNER_TEMP/_github_workflow/event.json" "$GITHUB_WORKSPACE")" = "$before"
test "$(stat -c %u "$RUNNER_TEMP/ravel-package/ravel-git.pkg.tar.zst")" = "$runner_uid"
test "$(git -C "$GITHUB_WORKSPACE" rev-parse HEAD)" = "$failing_sha"
test -z "$(find "$RUNNER_TEMP" -maxdepth 1 -name 'ravel-build.*' -print -quit)"

# Failure must propagate, while restoring ownership and removing the build copy.
status=0
GITHUB_SHA="$failing_sha" bash "$repo/scripts/ci/run-arch-build.sh" || status=$?
test "$status" -eq 7
test "$(stat -c '%u:%g:%a' "$RUNNER_TEMP" "$RUNNER_TEMP/_github_workflow/event.json" "$GITHUB_WORKSPACE")" = "$before"
test "$(stat -c %u "$RUNNER_TEMP/ravel-package/ravel-git.pkg.tar.zst")" = "$runner_uid"
test -z "$(find "$RUNNER_TEMP" -maxdepth 1 -name 'ravel-build.*' -print -quit)"
echo 'Arch build isolation tests passed.'
