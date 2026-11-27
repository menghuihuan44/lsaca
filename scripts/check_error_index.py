#!/usr/bin/env python3
"""Check that docs/language/31-error-codes.md indexes every diagnostic code.

The compiler attaches stable codes such as E0308 to diagnostics.  This
script scans every Rust source file under crates/lsaca/src for quoted
"E"-plus-four-or-five-digit literals, parses the Code and Meaning columns
of the tables in docs/language/31-error-codes.md, and compares the two
sets.

It exits with status 1 and a message naming every offending code when:

* a code is raised in the compiler but missing from the page;
* a code is documented on the page but never raised in the compiler;
* a documented row has an empty Meaning cell;
* the same code is documented more than once.

It also rejects a table row whose Code cell is not a valid code.

Usage (from the repository root):

    python scripts/check_error_index.py
    python scripts/check_error_index.py --root /path/to/checkout
    python scripts/check_error_index.py --json target/error-index.json

--root DIR checks another checkout; the default is the repository root
inferred from this script's location.  --json PATH writes a
machine-readable report with the documented codes, the raised codes and every
mismatch; the report is written before the script exits, including on failure.

Only the Python 3 standard library is used.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

PAGE_RELATIVE = Path("docs/language/31-error-codes.md")
SOURCES_RELATIVE = Path("crates/lsaca/src")

# A quoted code literal in a Rust source file: "E0308" or "E12345".
SOURCE_CODE_RE = re.compile(r'"(E[0-9]{4,5})"')
# A code cell in the markdown page, after surrounding code ticks are removed.
PAGE_CODE_RE = re.compile(r"E[0-9]{4,5}")
SEPARATOR_RE = re.compile(r":?-{3,}:?")


def collect_compiler_codes(root: Path) -> dict[str, list[str]]:
    """Return every quoted code literal and the source locations raising it."""
    source_root = root / SOURCES_RELATIVE
    raised: dict[str, set[str]] = {}
    for path in sorted(source_root.rglob("*.rs")):
        relative = path.relative_to(root).as_posix()
        try:
            text = path.read_text(encoding="utf-8")
        except OSError as error:
            print(f"error index: cannot read {relative}: {error}", file=sys.stderr)
            raise SystemExit(1)
        for line_number, line in enumerate(text.splitlines(), 1):
            for match in SOURCE_CODE_RE.finditer(line):
                raised.setdefault(match.group(1), set()).add(
                    f"{relative}:{line_number}"
                )
    return {code: sorted(locations) for code, locations in raised.items()}


def split_cells(line: str) -> list[str]:
    """Split one markdown table row on unescaped pipes."""
    text = line.strip()
    if text.startswith("|"):
        text = text[1:]
    if text.endswith("|") and not text.endswith(r"\|"):
        text = text[:-1]
    return [cell.replace(r"\|", "|").strip() for cell in re.split(r"(?<!\\)\|", text)]


def normalize_header(cell: str) -> str:
    return cell.strip().strip("\u0060*_ ").casefold()


def is_separator(cells: list[str]) -> bool:
    meaningful = [cell for cell in cells if cell]
    return bool(meaningful) and all(
        SEPARATOR_RE.fullmatch(cell.strip()) for cell in meaningful
    )


def parse_page(page: Path) -> tuple[list[str], dict[str, str], list[str], list[str]]:
    """Parse the page tables.

    Returns the documented codes in page order, the Meaning text per code, the
    duplicated codes, and descriptions of malformed Code cells.
    """
    documented: list[str] = []
    meanings: dict[str, str] = {}
    duplicates: list[str] = []
    invalid: list[str] = []
    header: list[str] | None = None

    try:
        lines = page.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        print(f"error index: cannot read {page}: {error}", file=sys.stderr)
        raise SystemExit(1)

    for line_number, line in enumerate(lines, 1):
        if not line.lstrip().startswith("|"):
            header = None
            continue
        cells = split_cells(line)
        if is_separator(cells):
            continue
        normalized = [normalize_header(cell) for cell in cells]
        if "code" in normalized and "meaning" in normalized:
            header = normalized
            continue
        if header is None:
            continue
        code_index = header.index("code")
        meaning_index = header.index("meaning")
        if len(cells) <= max(code_index, meaning_index):
            continue
        code_cell = cells[code_index].strip().strip("\u0060").strip()
        meaning = cells[meaning_index].strip()
        if not code_cell and not meaning:
            continue
        if not PAGE_CODE_RE.fullmatch(code_cell):
            invalid.append(f"line {line_number}: {code_cell or '<empty>'!r}")
            continue
        if code_cell in meanings:
            duplicates.append(code_cell)
            continue
        documented.append(code_cell)
        meanings[code_cell] = meaning
    return documented, meanings, duplicates, invalid


def write_json_report(path: Path, report: dict[str, object]) -> bool:
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(
            json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
    except OSError as error:
        print(f"error index: cannot write JSON report to {path}: {error}", file=sys.stderr)
        return False
    return True


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Check the diagnostic-code index against the compiler sources."
    )
    parser.add_argument(
        "--root",
        type=Path,
        default=Path(__file__).resolve().parent.parent,
        help="repository root to check (default: inferred from this script)",
    )
    parser.add_argument(
        "--json",
        type=Path,
        default=None,
        help="write a machine-readable report of the codes to this path",
    )
    args = parser.parse_args()

    root = args.root.resolve()
    if not root.is_dir():
        print(f"error index: repository root not found: {root}", file=sys.stderr)
        return 1
    page = root / PAGE_RELATIVE
    source_root = root / SOURCES_RELATIVE
    if not page.is_file():
        print(f"error index: page not found: {page}", file=sys.stderr)
        return 1
    if not source_root.is_dir():
        print(f"error index: compiler source directory not found: {source_root}", file=sys.stderr)
        return 1

    raised = collect_compiler_codes(root)
    documented, meanings, duplicates, invalid = parse_page(page)

    documented_set = set(documented)
    raised_set = set(raised)
    missing_from_page = sorted(raised_set - documented_set)
    unknown_to_compiler = sorted(documented_set - raised_set)
    empty_meaning = sorted(
        code for code in documented_set if not meanings.get(code, "").strip()
    )
    duplicate_codes = sorted(set(duplicates))

    problems = bool(
        missing_from_page or unknown_to_compiler or empty_meaning or duplicate_codes or invalid
    )
    report: dict[str, object] = {
        "codes": sorted(documented_set),
        "documented": sorted(documented_set),
        "raised_in_compiler": sorted(raised_set),
        "missing_from_page": missing_from_page,
        "documented_but_not_raised": unknown_to_compiler,
        "empty_meaning": empty_meaning,
        "duplicate_rows": duplicate_codes,
        "invalid_code_cells": invalid,
        "ok": not problems,
    }
    if args.json is not None and not write_json_report(args.json, report):
        return 1

    if problems:
        print("error index: the diagnostic code index is out of date", file=sys.stderr)
        if missing_from_page:
            print(
                f"  missing from {PAGE_RELATIVE.as_posix()} "
                f"({len(missing_from_page)}): {', '.join(missing_from_page)}",
                file=sys.stderr,
            )
            for code in missing_from_page:
                locations = raised[code]
                shown = ", ".join(locations[:3])
                if len(locations) > 3:
                    shown += ", ..."
                print(f"    {code}: raised at {shown}", file=sys.stderr)
        if unknown_to_compiler:
            print(
                f"  documented but never raised in the compiler "
                f"({len(unknown_to_compiler)}): {', '.join(unknown_to_compiler)}",
                file=sys.stderr,
            )
        if empty_meaning:
            print(
                f"  rows with an empty Meaning ({len(empty_meaning)}): "
                f"{', '.join(empty_meaning)}",
                file=sys.stderr,
            )
        if duplicate_codes:
            print(
                f"  duplicate Code rows ({len(duplicate_codes)}): "
                f"{', '.join(duplicate_codes)}",
                file=sys.stderr,
            )
        for problem in invalid:
            print(f"  invalid Code cell: {problem}", file=sys.stderr)
        return 1

    print(
        f"error index: {len(documented_set)} codes documented and "
        "all raised in the compiler"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
