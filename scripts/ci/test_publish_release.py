#!/usr/bin/env python3
"""Exercise GitHub publication using real local Git and a network-free gh stub."""

import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

import release

GH_STUB = r'''#!/usr/bin/env python3
import json, os, pathlib, shutil, sys
args = sys.argv[1:]
state = pathlib.Path(os.environ["GH_TEST_STATE"])
with (state / "calls").open("a") as log:
    log.write(json.dumps(args) + "\n")
if args[:2] == ["release", "view"]:
    if not (state / "published").exists():
        sys.exit(1)
    print(json.dumps({"isDraft": False, "isPrerelease": os.environ["GH_TEST_PRERELEASE"] == "true"}))
elif args[:2] == ["release", "create"]:
    assert "--verify-tag" in args and "--clobber" not in args
    assert ("--prerelease" in args) == (os.environ["GH_TEST_PRERELEASE"] == "true")
    (state / "assets").mkdir()
    for file in args[3:5]:
        shutil.copy(file, state / "assets")
    (state / "published").touch()
elif args[:2] == ["release", "download"]:
    destination = pathlib.Path(args[args.index("--dir") + 1])
    for file in (state / "assets").iterdir():
        shutil.copy(file, destination)
elif args[:2] == ["release", "edit"]:
    (state / "latest").write_text(args[2])
elif args[0] == "api":
    print(os.environ.get("GH_TEST_TAGS", ""))
else:
    raise AssertionError(args)
'''


class PublishTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.source = self.root / "source"
        self.source.mkdir()
        self.state = self.root / "state"
        self.state.mkdir()
        tools = self.root / "tools"
        tools.mkdir()
        (tools / "gh").write_text(GH_STUB)
        (tools / "gh").chmod(0o755)
        self.env = {**os.environ, "PATH": str(tools) + os.pathsep + os.environ["PATH"],
                    "GIT_CONFIG_GLOBAL": "/dev/null", "GIT_CONFIG_NOSYSTEM": "1", "GIT_ALLOW_PROTOCOL": "file",
                    "GH_TEST_STATE": str(self.state), "GITHUB_REPOSITORY": "example/ravel"}
        self.git("init", "--initial-branch=master")
        self.git("config", "user.name", "Release Test")
        self.git("config", "user.email", "test@example.invalid")
        (self.source / "file").write_text("fixture")
        self.git("add", ".")
        self.git("commit", "-m", "test: add release fixture")
        self.env["GITHUB_SHA"] = self.git("rev-parse", "HEAD").stdout.strip()
        self.git("init", "--bare", str(self.root / "remote.git"))
        self.git("remote", "add", "origin", str(self.root / "remote.git"))
        self.directory = self.root / "release"
        self.directory.mkdir()
        self.script = Path(__file__).with_name("publish-release.sh")

    def git(self, *args):
        return subprocess.run(["git", *args], cwd=self.source, env=self.env,
                              check=True, capture_output=True, text=True)

    def stage(self, tag):
        self.env["RELEASE_TAG"] = tag
        info = release.parse_tag(tag)
        self.env["GH_TEST_PRERELEASE"] = str(info["prerelease"]).lower()
        self.git("tag", "-a", tag, "-m", "test release")
        self.git("push", "origin", f"refs/tags/{tag}")
        archive = self.directory / info["asset"]
        archive.write_bytes(b"binary archive fixture")
        release.generate(tag, self.env["GITHUB_SHA"], archive, self.directory)

    def publish(self):
        return subprocess.run(["bash", str(self.script), str(self.directory)],
                              cwd=self.source, env=self.env, capture_output=True, text=True)

    def assert_published(self):
        result = self.publish()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_initial_release_and_identical_retry(self):
        self.stage("v1.2.3")
        self.assert_published()
        self.assert_published()
        calls = [json.loads(line) for line in (self.state / "calls").read_text().splitlines()]
        self.assertEqual(sum(call[:2] == ["release", "create"] for call in calls), 1)
        self.assertEqual((self.state / "latest").read_text(), "v1.2.3")

    def test_existing_binary_cannot_be_overwritten(self):
        self.stage("v1.2.3")
        self.assert_published()
        asset = self.state / "assets" / release.parse_tag("v1.2.3")["asset"]
        asset.write_bytes(b"different published binary")
        self.assertNotEqual(self.publish().returncode, 0)
        self.assertEqual(asset.read_bytes(), b"different published binary")

    def test_moved_tag_is_rejected_before_github_access(self):
        self.stage("v1.2.3")
        self.git("commit", "--allow-empty", "-m", "test: move tag")
        self.git("tag", "-f", "v1.2.3")
        self.git("push", "--force", "origin", "refs/tags/v1.2.3")
        self.assertNotEqual(self.publish().returncode, 0)
        self.assertFalse((self.state / "calls").exists())

    def test_prereleases_and_old_versions_do_not_become_latest(self):
        self.stage("v2.0.0-rc.1")
        self.assert_published()
        self.assertFalse((self.state / "latest").exists())

    def test_late_old_stable_release_does_not_become_latest(self):
        self.stage("v1.2.3")
        self.env["GH_TEST_TAGS"] = "v1.10.0\nv1.2.3\n"
        self.assert_published()
        self.assertFalse((self.state / "latest").exists())


if __name__ == "__main__":
    unittest.main()
