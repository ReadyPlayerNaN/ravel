#!/usr/bin/env python3
"""Validate release inputs and generate metadata without executing package code."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import sys

SEMVER = re.compile(
    r"v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)"
    r"(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?"
    r"(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?"
)


def parse_tag(tag):
    match = SEMVER.fullmatch(tag)
    if not match or len(tag) > 128:
        raise ValueError(f"Not a supported v-prefixed SemVer tag: {tag!r}")
    prerelease = match[4]
    if prerelease and any(
        part.isdigit() and len(part) > 1 and part.startswith("0")
        for part in prerelease.split(".")
    ):
        raise ValueError("Numeric prerelease identifiers cannot have leading zeroes")
    return {
        "tag": tag,
        "version": ".".join(match.group(1, 2, 3)),
        "prerelease": bool(prerelease),
        "asset": f"ravel-{tag}-linux-x86_64.tar.gz",
    }


def checksum(path):
    with Path(path).open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def generate(tag, commit, archive, output):
    info = parse_tag(tag)
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise ValueError("Release commit must be a full Git SHA")
    archive = Path(archive)
    if archive.name != info["asset"]:
        raise ValueError("Archive name does not match the release tag")
    output = Path(output)
    info.update(commit=commit, sha256=checksum(archive))
    (output / "release.json").write_text(json.dumps(info, indent=2) + "\n")
    (output / "SHA256SUMS").write_text(f"{info['sha256']}  {info['asset']}\n")
    if not info["prerelease"]:
        template = Path(__file__).resolve().parents[2] / "packaging/aur-bin/PKGBUILD.in"
        recipe = template.read_text()
        for name in ("version", "tag", "asset", "sha256"):
            recipe = recipe.replace(f"@{name.upper()}@", info[name])
        destination = output / "aur"
        destination.mkdir()
        (destination / "PKGBUILD").write_text(recipe)
    return info


def verify(tag, commit, directory):
    directory = Path(directory)
    expected = parse_tag(tag)
    expected.update(commit=commit, sha256=checksum(directory / expected["asset"]))
    if json.loads((directory / "release.json").read_text()) != expected:
        raise ValueError("Release artifact metadata does not match this tag/commit")
    if (directory / "SHA256SUMS").read_text() != f"{expected['sha256']}  {expected['asset']}\n":
        raise ValueError("Release checksum file does not match the binary")


def package_version(path):
    fields = {}
    for line in Path(path).read_text().splitlines():
        key, separator, value = line.strip().partition(" = ")
        if separator and key in ("pkgver", "pkgrel", "epoch"):
            if key in fields:
                raise ValueError(f"Duplicate {key} in .SRCINFO")
            fields[key] = value
    if "epoch" in fields:
        raise ValueError("Unexpected epoch in ravel-bin metadata; manual review required")
    version = fields.get("pkgver", "")
    info = parse_tag("v" + version)
    if info["prerelease"] or info["version"] != version:
        raise ValueError("AUR binary releases must have stable numeric versions")
    release = fields.get("pkgrel", "")
    if not re.fullmatch(r"[1-9][0-9]*", release):
        raise ValueError("Invalid package release number")
    return (*map(int, version.split(".")), int(release))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("metadata")
    asset = commands.add_parser("asset")
    asset.add_argument("tag")
    stage = commands.add_parser("generate")
    check = commands.add_parser("verify")
    for command in (stage, check):
        command.add_argument("tag")
        command.add_argument("commit")
    stage.add_argument("archive")
    stage.add_argument("directory")
    check.add_argument("directory")
    newer = commands.add_parser("newer")
    newer.add_argument("candidate")
    newer.add_argument("current")
    latest = commands.add_parser("latest")
    latest.add_argument("tag")
    args = parser.parse_args()
    if args.command == "metadata":
        info = None
        if os.environ.get("GITHUB_EVENT_NAME") == "push":
            ref = os.environ.get("GITHUB_REF", "")
            if ref.startswith("refs/tags/"):
                try:
                    info = parse_tag(ref.removeprefix("refs/tags/"))
                except ValueError:
                    pass  # Other tags still get CI, but never publication.
        lines = [f"release={'true' if info else 'false'}"]
        if info:
            lines += [f"tag={info['tag']}", f"prerelease={str(info['prerelease']).lower()}"]
        with open(os.environ["GITHUB_OUTPUT"], "a") as output:
            output.write("\n".join(lines) + "\n")
    elif args.command == "asset":
        print(parse_tag(args.tag)["asset"])
    elif args.command == "generate":
        generate(args.tag, args.commit, args.archive, args.directory)
    elif args.command == "verify":
        verify(args.tag, args.commit, args.directory)
    elif args.command == "newer":
        print(str(package_version(args.candidate) > package_version(args.current)).lower())
    elif args.command == "latest":
        info = parse_tag(args.tag)
        version = tuple(map(int, info["version"].split(".")))
        newer_exists = False
        for tag in sys.stdin.read().splitlines():
            try:
                other = parse_tag(tag)
            except ValueError:
                continue
            if not other["prerelease"] and tuple(map(int, other["version"].split("."))) > version:
                newer_exists = True
        print(str(not info["prerelease"] and not newer_exists).lower())


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, KeyError) as error:
        sys.exit(str(error))
