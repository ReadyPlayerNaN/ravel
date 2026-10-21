#!/usr/bin/env python3
"""Offline tests for release tag policy, checksums and AUR version ordering."""

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

import release


class ReleaseTests(unittest.TestCase):
    def test_semver_tags(self):
        for tag in ("v0.1.0", "v12.3.45", "v1.0.0-rc.1", "v1.0.0+build.7", "v1.0.0-rc.1+build.7"):
            with self.subTest(tag=tag):
                self.assertEqual(release.parse_tag(tag)["tag"], tag)
        self.assertTrue(release.parse_tag("v1.0.0-rc.1")["prerelease"])
        self.assertFalse(release.parse_tag("v1.0.0+build.7")["prerelease"])

    def test_non_semver_and_unsafe_tags_are_rejected(self):
        for tag in ("1.0.0", "v1.0", "v01.0.0", "v1.0.0-01", "v1.0.0-rc..1", "v1.0.0+", "v1.0.0\n", "v1.0.0/../../x", "v1.0.0';echo x", "v" + "1" * 129):
            with self.subTest(tag=tag):
                with self.assertRaises(ValueError):
                    release.parse_tag(tag)

    def test_only_tag_pushes_emit_release_outputs(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "outputs"
            for event, ref, enabled in (
                ("push", "refs/tags/v1.2.3", True),
                ("push", "refs/tags/v1.2.3-rc.1", True),
                ("push", "refs/tags/v1.2", False),
                ("push", "refs/heads/v1.2.3", False),
                ("pull_request", "refs/tags/v1.2.3", False),
            ):
                with self.subTest(event=event, ref=ref):
                    output.write_text("")
                    subprocess.run([sys.executable, release.__file__, "metadata"], check=True,
                                   env={**os.environ, "GITHUB_EVENT_NAME": event, "GITHUB_REF": ref, "GITHUB_OUTPUT": str(output)})
                    self.assertIn(f"release={str(enabled).lower()}\n", output.read_text())

    def test_generated_recipe_pins_asset_and_checksum(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            tag, commit = "v1.2.3+build.4", "a" * 40
            archive = output / release.parse_tag(tag)["asset"]
            archive.write_bytes(b"test release asset")
            info = release.generate(tag, commit, archive, output)
            recipe = (output / "aur/PKGBUILD").read_text()
            self.assertIn("pkgver=1.2.3\n", recipe)
            self.assertIn(f"releases/download/{tag}/{archive.name}", recipe)
            self.assertIn(info["sha256"], recipe)
            self.assertNotIn("SKIP", recipe)
            self.assertNotIn("cargo", recipe)
            self.assertNotIn("@VERSION@", recipe)
            release.verify(tag, commit, output)
            archive.write_bytes(b"tampered release asset")
            with self.assertRaises(ValueError):
                release.verify(tag, commit, output)

    def test_mismatched_commit_and_checksum_file_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            archive = output / release.parse_tag("v1.0.0")["asset"]
            archive.write_bytes(b"asset")
            release.generate("v1.0.0", "a" * 40, archive, output)
            with self.assertRaises(ValueError):
                release.verify("v1.0.0", "b" * 40, output)
            (output / "SHA256SUMS").write_text("wrong checksum\n")
            with self.assertRaises(ValueError):
                release.verify("v1.0.0", "a" * 40, output)

    def test_prerelease_has_no_stable_aur_recipe(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            tag = "v2.0.0-beta.1"
            archive = output / release.parse_tag(tag)["asset"]
            archive.write_bytes(b"prerelease")
            release.generate(tag, "a" * 40, archive, output)
            self.assertFalse((output / "aur").exists())
            self.assertTrue(json.loads((output / "release.json").read_text())["prerelease"])

    def test_aur_version_comparison_is_numeric_and_fail_closed(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / ".SRCINFO"
            path.write_text("pkgver = 1.10.0\npkgrel = 1\n")
            self.assertGreater(release.package_version(path), (1, 9, 9, 1))
            for text in ("pkgver = 1.0.0-rc.1\npkgrel = 1", "pkgver = 1.0.0\npkgrel = 1\nepoch = 1", "pkgver = 1.0.0\npkgver = 9.0.0\npkgrel = 1"):
                path.write_text(text)
                with self.assertRaises(ValueError):
                    release.package_version(path)

    def test_old_tags_do_not_become_latest(self):
        for tag, expected in (("v1.9.0", "false"), ("v1.10.0", "true"), ("v2.0.0-rc.1", "false")):
            result = subprocess.run([sys.executable, release.__file__, "latest", tag],
                                    input="v1.10.0\nv2.0.0-rc.2\nnot-semver\n", text=True,
                                    capture_output=True, check=True)
            self.assertEqual(result.stdout.strip(), expected)


if __name__ == "__main__":
    unittest.main()
