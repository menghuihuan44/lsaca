#!/usr/bin/env python3
"""Check the documentation tree for version and reachability drift.

The workspace version in Cargo.toml ([workspace.package].version) is the single
source of truth.  This checker verifies the release statements that must repeat
it, scans the prose for claims whose target version has already shipped, checks
the language-chapter numbering, and proves that every language and standard
library page can be reached from docs/README.md within two relative links.

Usage:
    python scripts/docs_drift.py [--root PATH] [--json PATH]

Exit status: 0 when the tree is clean, 1 when drift is found (each finding is
printed as a "DRIFT ERROR: ..." line).  --json writes a machine-readable report.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from collections import deque
from pathlib import Path

VERSION_RE = r"\d+\.\d+\.\d+"
SHORT_VERSION_RE = r"\d+\.\d+(?:\.\d+)?"

# Claims like "planned for v2.4" are only wrong once the target has shipped.
FUTURE_CLAIM_PATTERNS = (
    re.compile(
        r"\b(?:planned|scheduled|deferred)\s+(?:for|to|until)\s+"
        r"(?:version\s+)?v?(" + SHORT_VERSION_RE + r")\b",
        re.IGNORECASE,
    ),
    re.compile(
        r"\b(?:will be added|expected)\s+in\s+(?:version\s+)?v?("
        + SHORT_VERSION_RE
        + r")\b",
        re.IGNORECASE,
    ),
)

# Current-state claims must name the version that is current right now.
CURRENT_CLAIM_PATTERN = re.compile(
    r"\b(?:not implemented in|status in|as of version)\s+v?(" + VERSION_RE + r")\b",
    re.IGNORECASE,
)

README_RELEASE_RE = re.compile(r"The current release is \*\*(" + VERSION_RE + r")\*\*")
DOCS_README_RE = re.compile(
    r"reference for the language as of version (" + VERSION_RE + r")"
)
CHANGELOG_SECTION_RE = re.compile(r"^## \[(" + VERSION_RE + r")\]", re.MULTILINE)

LINK_RE = re.compile(r"!?\[[^\]]*\]\(\s*<?([^)\s>]+)>?(?:\s+[\"'][^)]*[\"'])?\s*\)")
REFERENCE_LINK_RE = re.compile(r"^\[[^\]]+\]:\s*<?([^\s>]+)>?", re.MULTILINE)


def parse_version(text):
    parts = [int(part) for part in text.split(".")]
    while len(parts) < 3:
        parts.append(0)
    return tuple(parts[:3])


def workspace_version(root, errors):
    manifest = root / "Cargo.toml"
    if not manifest.is_file():
        errors.append(
            {
                "check": "version-source",
                "file": "Cargo.toml",
                "line": None,
                "message": "Cargo.toml is missing; cannot read [workspace.package] version",
            }
        )
        return None
    text = manifest.read_text(encoding="utf-8")
    match = re.search(
        r"(?ms)^\[workspace\.package\]\s*$(.*?)(?=^\[|\Z)", text
    )
    if not match:
        errors.append(
            {
                "check": "version-source",
                "file": "Cargo.toml",
                "line": None,
                "message": "no [workspace.package] table found",
            }
        )
        return None
    version_match = re.search(
        r'^version\s*=\s*"(' + VERSION_RE + r')"', match.group(1), re.MULTILINE
    )
    if not version_match:
        errors.append(
            {
                "check": "version-source",
                "file": "Cargo.toml",
                "line": None,
                "message": "no version key in [workspace.package]",
            }
        )
        return None
    return version_match.group(1)


def line_of(text, offset):
    return text.count("\n", 0, offset) + 1


def add_error(errors, check, path, line, message):
    errors.append(
        {
            "check": check,
            "file": str(path),
            "line": line,
            "message": message,
        }
    )


def check_release_statements(root, version, errors):
    readme = root / "README.md"
    if not readme.is_file():
        add_error(errors, "readme-release", "README.md", None, "README.md is missing")
    else:
        text = readme.read_text(encoding="utf-8")
        matches = list(README_RELEASE_RE.finditer(text))
        if len(matches) != 1:
            add_error(
                errors,
                "readme-release",
                "README.md",
                line_of(text, matches[0].start()) if matches else None,
                "expected exactly one 'The current release is **X.Y.Z**' statement, "
                "found %d" % len(matches),
            )
        elif matches[0].group(1) != version:
            add_error(
                errors,
                "readme-release",
                "README.md",
                line_of(text, matches[0].start()),
                "README.md states release %s but Cargo.toml says %s"
                % (matches[0].group(1), version),
            )

    docs_readme = root / "docs" / "README.md"
    if not docs_readme.is_file():
        add_error(
            errors, "docs-readme-version", "docs/README.md", None, "docs/README.md is missing"
        )
    else:
        text = docs_readme.read_text(encoding="utf-8")
        match = DOCS_README_RE.search(text)
        if not match:
            add_error(
                errors,
                "docs-readme-version",
                "docs/README.md",
                None,
                "missing 'reference for the language as of version X.Y.Z' statement",
            )
        elif match.group(1) != version:
            add_error(
                errors,
                "docs-readme-version",
                "docs/README.md",
                line_of(text, match.start()),
                "docs/README.md documents version %s but Cargo.toml says %s"
                % (match.group(1), version),
            )

    changelog = root / "CHANGELOG.md"
    if not changelog.is_file():
        add_error(errors, "changelog", "CHANGELOG.md", None, "CHANGELOG.md is missing")
    else:
        text = changelog.read_text(encoding="utf-8")
        match = CHANGELOG_SECTION_RE.search(text)
        if not match:
            add_error(
                errors,
                "changelog",
                "CHANGELOG.md",
                None,
                "no '## [X.Y.Z]' release section found",
            )
        elif match.group(1) != version:
            add_error(
                errors,
                "changelog",
                "CHANGELOG.md",
                line_of(text, match.start()),
                "first release section is %s but Cargo.toml says %s"
                % (match.group(1), version),
            )


def scan_prose(root, version, errors):
    current = parse_version(version)
    files = []
    for name in ("README.md", "ROADMAP.md"):
        path = root / name
        if path.is_file():
            files.append(path)
    for directory in (root / "docs", root / "rfcs"):
        if directory.is_dir():
            files.extend(sorted(directory.rglob("*.md")))
    seen = set()
    scanned = 0
    for path in files:
        resolved = path.resolve()
        if resolved in seen:
            continue
        seen.add(resolved)
        text = path.read_text(encoding="utf-8")
        scanned += 1
        rel = path.relative_to(root).as_posix()
        for pattern in FUTURE_CLAIM_PATTERNS:
            for match in pattern.finditer(text):
                target = parse_version(match.group(1))
                if target <= current:
                    add_error(
                        errors,
                        "stale-future-claim",
                        rel,
                        line_of(text, match.start()),
                        "%r targets %s, which has already shipped (current %s)"
                        % (match.group(0).strip(), match.group(1), version),
                    )
        for match in CURRENT_CLAIM_PATTERN.finditer(text):
            if match.group(1) != version:
                add_error(
                    errors,
                    "stale-current-claim",
                    rel,
                    line_of(text, match.start()),
                    "%r names %s, but the current version is %s"
                    % (match.group(0).strip(), match.group(1), version),
                )
    return scanned


def check_numbering(root, errors):
    language = root / "docs" / "language"
    if not language.is_dir():
        add_error(
            errors,
            "language-numbering",
            "docs/language",
            None,
            "docs/language directory is missing",
        )
        return
    numbers = {}
    for path in sorted(language.glob("*.md")):
        match = re.match(r"^(\d+)-", path.name)
        if not match:
            # Unnumbered pages such as deviations.md and limitations.md are
            # supplementary material, not part of the numbered chapter sequence.
            continue
        number = int(match.group(1))
        if number in numbers:
            add_error(
                errors,
                "language-numbering",
                path.relative_to(root).as_posix(),
                None,
                "chapter number %02d is also used by %s" % (number, numbers[number]),
            )
        else:
            numbers[number] = path.name
    if not numbers:
        add_error(
            errors,
            "language-numbering",
            "docs/language",
            None,
            "no numbered language chapters found",
        )
        return
    if 0 in numbers:
        add_error(
            errors,
            "language-numbering",
            "docs/language",
            None,
            "chapter number 00 is not allowed; numbering starts at 01",
        )
    highest = max(numbers)
    for number in range(1, highest + 1):
        if number not in numbers:
            add_error(
                errors,
                "language-numbering",
                "docs/language",
                None,
                "missing chapter number %02d" % number,
            )


def markdown_links(path):
    text = path.read_text(encoding="utf-8")
    for match in LINK_RE.finditer(text):
        yield match.group(1)
    for match in REFERENCE_LINK_RE.finditer(text):
        yield match.group(1)


def check_reachability(root, errors):
    docs = root / "docs"
    start = docs / "README.md"
    if not start.is_file():
        add_error(
            errors,
            "reachability",
            "docs/README.md",
            None,
            "docs/README.md is missing; cannot start the link walk",
        )
        return
    required = []
    language = docs / "language"
    if language.is_dir():
        required.extend(sorted(language.glob("*.md")))
    stdlib = docs / "stdlib"
    if stdlib.is_dir():
        required.extend(sorted(stdlib.rglob("*.md")))

    depths = {start.resolve(): 0}
    queue = deque([(start.resolve(), 0)])
    while queue:
        path, depth = queue.popleft()
        if depth >= 2:
            continue
        if not path.is_file():
            continue
        for raw in markdown_links(path):
            target = raw.split("#", 1)[0].split("?", 1)[0].strip()
            if not target or target.startswith("/") or "://" in target:
                continue
            if target.startswith("mailto:"):
                continue
            from urllib.parse import unquote

            target = unquote(target)
            candidate = (path.parent / target).resolve()
            try:
                candidate.relative_to(root.resolve())
            except ValueError:
                continue
            if not candidate.is_file():
                continue
            if candidate.suffix.lower() != ".md":
                continue
            if candidate not in depths:
                depths[candidate] = depth + 1
                queue.append((candidate, depth + 1))

    for path in required:
        resolved = path.resolve()
        rel = path.relative_to(root).as_posix()
        if resolved not in depths:
            add_error(
                errors,
                "reachability",
                rel,
                None,
                "not reachable from docs/README.md within two relative links",
            )


def write_json(path, payload):
    destination = Path(path)
    if destination.parent and not destination.parent.exists():
        destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )


def main(argv=None):
    parser = argparse.ArgumentParser(
        description="Check lsaca documentation for version and link drift."
    )
    parser.add_argument(
        "--root",
        default=str(Path(__file__).resolve().parent.parent),
        help="repository root (default: the checkout containing this script)",
    )
    parser.add_argument(
        "--json", metavar="PATH", help="write a machine-readable report to PATH"
    )
    args = parser.parse_args(argv)

    root = Path(args.root).resolve()
    errors = []
    version = workspace_version(root, errors)
    scanned = 0
    if version:
        check_release_statements(root, version, errors)
        scanned = scan_prose(root, version, errors)
        check_numbering(root, errors)
        check_reachability(root, errors)

    payload = {
        "root": str(root),
        "version": version,
        "files_scanned": scanned,
        "ok": not errors,
        "errors": errors,
    }
    if args.json:
        write_json(args.json, payload)

    if errors:
        for error in errors:
            location = error["file"]
            if error["line"] is not None:
                location += ":%d" % error["line"]
            print("DRIFT ERROR: %s: %s" % (location, error["message"]))
        print("docs drift: FAILED (%d error(s))" % len(errors))
        return 1

    print(
        "docs drift: OK (version %s, %d documentation files scanned)" % (version, scanned)
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
