#!/usr/bin/env python3
"""Check STATUS.md rows against test and documentation evidence.

STATUS.md is the hand-maintained completion matrix. This gate parses its three
tables and enforces:

* every row state is complete, partial, missing, or deliberate;
* every partial or missing row names a target release or boundary instead of -;
* every complete row has an entry in docs/status-traceability.md with at least
  one test reference and one documentation reference;
* every test reference points at an existing repository file and its symbol
  occurs in that file;
* every documentation reference points at an existing docs page and its
  GitHub-style heading anchor exists; and
* traceability rows name a real STATUS row in the right section.

The traceability file is data, not code. Its format is documented at the top of
docs/status-traceability.md: one section per STATUS section and a Markdown table
with Row | Tests | Documentation. Tests are semicolon-separated path[::symbol]
references; documentation is semicolon-separated docs/...md[#anchor] refs.

Negative cases this check rejects (also documented here for maintainers):

* an unknown state such as done or finished;
* a partial or missing row with - in the Target column;
* a complete row with no traceability entry, an empty Tests or Documentation
  cell, or a reference to a missing file, symbol, or heading anchor;
* a traceability entry for an unknown row, a row in the wrong section, or a
  duplicate traceability entry.

Usage:
    python scripts/check_status.py
    python scripts/check_status.py --root PATH --json target/status.json
    python scripts/check_status.py --traceability docs/status-traceability.md

Exit status: 0 when every enforced rule passes, 1 otherwise. On success the
output is one line:
    status: 47 rows (complete N, partial N, missing N, deliberate N);
    traced N/N complete rows; docs N files
On failure each finding is printed as a STATUS ERROR line with the file and
line when known.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

SECTIONS = (
    "Language and semantics",
    "Standard library",
    "Tooling and delivery",
)
STATES = ("complete", "partial", "missing", "deliberate")
TARGETLESS = {"", "-", "--", "—", "n/a", "N/A"}
DEFAULT_TRACEABILITY = "docs/status-traceability.md"
HEADING_RE = re.compile(r"^(#{1,6})\s+(.*?)\s*$")
SEPARATOR_RE = re.compile(r"^:?-{3,}:?$")


def add_error(errors, check, file, line, message):
    errors.append(
        {
            "check": check,
            "file": str(file),
            "line": line,
            "message": message,
        }
    )


def read_text(path, errors, check):
    try:
        return path.read_text(encoding="utf-8")
    except FileNotFoundError:
        add_error(errors, check, path, None, "file is missing")
    except OSError as error:
        add_error(errors, check, path, None, "cannot read file: %s" % error)
    except UnicodeError as error:
        add_error(errors, check, path, None, "file is not valid UTF-8: %s" % error)
    return None


def table_cells(line):
    stripped = line.strip()
    if not stripped.startswith("|"):
        return None
    return [cell.strip() for cell in stripped.strip("|").split("|")]


def is_table_separator(cells):
    return bool(cells) and all(SEPARATOR_RE.match(cell) for cell in cells)


def parse_status(root, errors):
    """Return STATUS.md rows as dictionaries, collecting structural errors."""
    path = root / "STATUS.md"
    text = read_text(path, errors, "status-read")
    if text is None:
        return []

    rows = []
    section_headings = {section: 0 for section in SECTIONS}
    seen_rows = {}
    current = None

    for number, line in enumerate(text.splitlines(), 1):
        heading = re.match(r"^##\s+(.+?)\s*$", line)
        if heading:
            title = heading.group(1)
            if title in SECTIONS:
                section_headings[title] += 1
                current = title
            else:
                current = None
            continue

        if current is None or not line.lstrip().startswith("|"):
            continue
        cells = table_cells(line)
        if cells is None:
            continue
        if len(cells) != 4:
            add_error(
                errors,
                "status-table",
                path,
                number,
                "expected 4 columns Area | State | Gap or note | Target, found %d"
                % len(cells),
            )
            continue

        area, raw_state, gap, target = cells
        if area in ("Area", "Module", "Row") or is_table_separator(cells):
            continue
        if not area:
            add_error(errors, "status-table", path, number, "row has an empty Area")
            continue
        if area in seen_rows:
            add_error(
                errors,
                "status-table",
                path,
                number,
                "duplicate STATUS row %r (first seen on line %s)"
                % (area, seen_rows[area]),
            )
        seen_rows[area] = number

        state = raw_state.lower()
        if state not in STATES:
            add_error(
                errors,
                "status-state",
                path,
                number,
                "row %r: state %r is not one of %s"
                % (area, raw_state, ", ".join(STATES)),
            )
        if state in ("partial", "missing") and target in TARGETLESS:
            add_error(
                errors,
                "status-target",
                path,
                number,
                "row %r is %s but has no target release; name the version or "
                "boundary that closes the gap" % (area, state),
            )

        rows.append(
            {
                "section": current,
                "area": area,
                "state": state,
                "raw_state": raw_state,
                "gap": gap,
                "target": target,
                "line": number,
            }
        )

    for section in SECTIONS:
        if section_headings[section] == 0:
            add_error(
                errors,
                "status-section",
                path,
                None,
                "missing section %r" % section,
            )
        elif not any(row["section"] == section for row in rows):
            add_error(
                errors,
                "status-section",
                path,
                None,
                "section %r has no data rows" % section,
            )
        if section_headings[section] > 1:
            add_error(
                errors,
                "status-section",
                path,
                None,
                "section %r appears more than once" % section,
            )
    return rows


def parse_traceability(root, trace_path, errors):
    """Return {row name: entry} from the traceability markdown file."""
    full_path = root / trace_path
    text = read_text(full_path, errors, "traceability-read")
    if text is None:
        return {}

    entries = {}
    section_headings = set()
    current = None

    for number, line in enumerate(text.splitlines(), 1):
        heading = re.match(r"^##\s+(.+?)\s*$", line)
        if heading:
            title = heading.group(1)
            current = title if title in SECTIONS else None
            if current is not None:
                section_headings.add(current)
            continue

        if current is None or not line.lstrip().startswith("|"):
            continue
        cells = table_cells(line)
        if cells is None:
            continue
        if len(cells) != 3:
            add_error(
                errors,
                "traceability-table",
                full_path,
                number,
                "expected 3 columns Row | Tests | Documentation, found %d"
                % len(cells),
            )
            continue

        area, tests, docs = cells
        if area in ("Row", "Area") or is_table_separator(cells):
            continue
        if not area:
            add_error(
                errors, "traceability-table", full_path, number, "row has an empty Row"
            )
            continue
        if area in entries:
            add_error(
                errors,
                "traceability-table",
                full_path,
                number,
                "duplicate traceability entry for %r" % area,
            )
            continue
        entries[area] = {
            "section": current,
            "tests": tests,
            "docs": docs,
            "line": number,
        }

    for section in SECTIONS:
        if section not in section_headings:
            add_error(
                errors,
                "traceability-section",
                full_path,
                None,
                "missing traceability section %r" % section,
            )
    return entries


def slugify(text):
    """GitHub heading slug, matching the rules used by docs_verify.py."""
    text = text.strip().lower()
    text = re.sub(r"[^\w\s-]", "", text, flags=re.UNICODE)
    text = re.sub(r"\s", "-", text)
    return text.strip("-")


def heading_slugs(path, text):
    # Scan every ATX heading instead of tracking fences: several historical docs
    # pages have console blocks with an unbalanced closing fence, and a strict
    # fence tracker would hide real prose headings. This mirrors the verifier
    # used when the traceability data was assembled.
    anchors = set()
    seen = {}
    for line in text.splitlines():
        match = HEADING_RE.match(line)
        if not match:
            continue
        base = slugify(match.group(2))
        if not base:
            continue
        count = seen.get(base, 0)
        seen[base] = count + 1
        anchors.add(base if count == 0 else "%s-%d" % (base, count))
    return anchors


def split_refs(cell):
    return [ref.strip() for ref in cell.split(";") if ref.strip()]


def check_test_ref(root, ref, area, errors, stats):
    stats["test_refs"] += 1
    if "::" in ref:
        path_text, symbol = ref.split("::", 1)
    else:
        path_text, symbol = ref, None
    path_text = path_text.strip()
    if not path_text:
        add_error(
            errors,
            "test-ref",
            "STATUS.md",
            None,
            "row %r: empty test reference %r" % (area, ref),
        )
        return False

    file_path = Path(path_text)
    if file_path.is_absolute() or ".." in file_path.parts:
        add_error(
            errors,
            "test-ref",
            "STATUS.md",
            None,
            "row %r: test reference %r must be repository-relative"
            % (area, ref),
        )
        return False
    full_path = root / file_path
    if not full_path.is_file():
        add_error(
            errors,
            "test-ref",
            "STATUS.md",
            None,
            "row %r: missing test file %r" % (area, path_text),
        )
        return False

    text = read_text(full_path, errors, "test-ref")
    if text is None:
        return False

    if symbol is not None and symbol.strip():
        symbol = symbol.strip()
        if symbol.startswith("mod "):
            name = symbol[4:].strip()
            if not name:
                add_error(
                    errors,
                    "test-ref",
                    "STATUS.md",
                    None,
                    "row %r: malformed module reference %r" % (area, ref),
                )
                return False
            pattern = r"\bmod\s+" + re.escape(name) + r"\b"
        else:
            pattern = r"\b" + re.escape(symbol) + r"\b"
        if not re.search(pattern, text):
            add_error(
                errors,
                "test-ref",
                "STATUS.md",
                None,
                "row %r: symbol %r does not occur in %s"
                % (area, symbol, path_text),
            )
            return False
    return True


def check_doc_ref(root, ref, area, errors, stats, heading_cache):
    stats["doc_refs"] += 1
    path_text, separator, anchor = ref.partition("#")
    path_text = path_text.strip()
    anchor = anchor.strip()
    if not path_text:
        add_error(
            errors,
            "doc-ref",
            "STATUS.md",
            None,
            "row %r: empty documentation reference %r" % (area, ref),
        )
        return False
    if not path_text.startswith("docs/") or not path_text.endswith(".md"):
        add_error(
            errors,
            "doc-ref",
            "STATUS.md",
            None,
            "row %r: documentation reference must be docs/...md[#anchor]: %r"
            % (area, ref),
        )
        return False
    if separator and not anchor:
        add_error(
            errors,
            "doc-ref",
            "STATUS.md",
            None,
            "row %r: documentation reference has an empty anchor: %r"
            % (area, ref),
        )
        return False

    file_path = Path(path_text)
    if file_path.is_absolute() or ".." in file_path.parts:
        add_error(
            errors,
            "doc-ref",
            "STATUS.md",
            None,
            "row %r: documentation reference %r must be repository-relative"
            % (area, ref),
        )
        return False
    full_path = root / file_path
    if not full_path.is_file():
        add_error(
            errors,
            "doc-ref",
            "STATUS.md",
            None,
            "row %r: missing documentation file %r" % (area, path_text),
        )
        return False

    if not anchor:
        return True

    if path_text not in heading_cache:
        text = read_text(full_path, errors, "doc-ref")
        heading_cache[path_text] = heading_slugs(full_path, text) if text else set()
    anchors = heading_cache[path_text]
    if anchor not in anchors:
        near = sorted(item for item in anchors if anchor.split("-")[0] in item)[:5]
        hint = "; near: %s" % near if near else ""
        add_error(
            errors,
            "doc-ref",
            "STATUS.md",
            None,
            "row %r: missing heading anchor #%s in %s%s"
            % (area, anchor, path_text, hint),
        )
        return False
    return True


def validate_evidence(root, rows, entries, errors):
    stats = {"test_refs": 0, "doc_refs": 0, "traced_complete": 0}
    heading_cache = {}
    row_by_area = {row["area"]: row for row in rows}
    entry_valid = {}

    for area, entry in entries.items():
        errors_before = len(errors)
        row = row_by_area.get(area)
        if row is None:
            add_error(
                errors,
                "traceability-row",
                "docs/status-traceability.md",
                entry["line"],
                "traceability row %r does not match any STATUS.md row" % area,
            )
        elif entry["section"] != row["section"]:
            add_error(
                errors,
                "traceability-section",
                "docs/status-traceability.md",
                entry["line"],
                "traceability row %r is in section %r but STATUS.md puts it in %r"
                % (area, entry["section"], row["section"]),
            )

        tests = split_refs(entry["tests"])
        docs = split_refs(entry["docs"])
        if not tests:
            add_error(
                errors,
                "traceability-evidence",
                "docs/status-traceability.md",
                entry["line"],
                "row %r has no test references" % area,
            )
        if not docs:
            add_error(
                errors,
                "traceability-evidence",
                "docs/status-traceability.md",
                entry["line"],
                "row %r has no documentation references" % area,
            )
        for ref in tests:
            check_test_ref(root, ref, area, errors, stats)
        for ref in docs:
            check_doc_ref(root, ref, area, errors, stats, heading_cache)
        entry_valid[area] = len(errors) == errors_before

    for row in rows:
        if row["state"] != "complete":
            continue
        area = row["area"]
        entry = entries.get(area)
        if entry is None:
            add_error(
                errors,
                "complete-row",
                "STATUS.md",
                row["line"],
                "complete row %r has no evidence; add it to %s"
                % (area, DEFAULT_TRACEABILITY),
            )
            continue
        if not split_refs(entry["tests"]) or not split_refs(entry["docs"]):
            continue
        if entry_valid.get(area):
            stats["traced_complete"] += 1
        else:
            add_error(
                errors,
                "complete-row",
                "STATUS.md",
                row["line"],
                "complete row %r has invalid evidence; see the errors above"
                % area,
            )
    return stats


def count_docs_files(root, errors):
    docs = root / "docs"
    if not docs.is_dir():
        add_error(errors, "docs-count", docs, None, "docs directory is missing")
        return 0
    return sum(1 for path in docs.rglob("*.md") if path.is_file())


def count_states(rows):
    counts = {state: 0 for state in STATES}
    for row in rows:
        if row["state"] in counts:
            counts[row["state"]] += 1
    return counts


def summary_line(rows, stats, docs_files):
    counts = count_states(rows)
    return (
        "status: %d rows (complete %d, partial %d, missing %d, deliberate %d); "
        "traced %d/%d complete rows; docs %d files"
        % (
            len(rows),
            counts["complete"],
            counts["partial"],
            counts["missing"],
            counts["deliberate"],
            stats["traced_complete"],
            counts["complete"],
            docs_files,
        )
    )


def write_json(path, report):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(report, indent=2, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )


def main(argv=None):
    parser = argparse.ArgumentParser(
        description="Check STATUS.md against test and documentation evidence."
    )
    parser.add_argument(
        "--root",
        default=str(Path(__file__).resolve().parents[1]),
        help="repository root (default: the checkout containing this script)",
    )
    parser.add_argument(
        "--traceability",
        default=DEFAULT_TRACEABILITY,
        help="traceability markdown file, relative to the root (default: %(default)s)",
    )
    parser.add_argument("--json", help="write a machine-readable report to PATH")
    args = parser.parse_args(argv)

    root = Path(args.root).resolve()
    trace_path = Path(args.traceability)
    if trace_path.is_absolute():
        try:
            trace_path = trace_path.relative_to(root)
        except ValueError:
            pass
    errors = []

    rows = parse_status(root, errors)
    entries = parse_traceability(root, trace_path, errors)
    stats = validate_evidence(root, rows, entries, errors)
    docs_files = count_docs_files(root, errors)

    summary = summary_line(rows, stats, docs_files)
    counts = count_states(rows)
    report = {
        "ok": not errors,
        "summary": summary,
        "status": {
            "file": "STATUS.md",
            "rows": rows,
            "counts": counts,
        },
        "traceability": {
            "file": str(trace_path),
            "entries": len(entries),
            "test_refs": stats["test_refs"],
            "doc_refs": stats["doc_refs"],
        },
        "docs_files": docs_files,
        "errors": errors,
    }

    if args.json:
        json_path = Path(args.json)
        try:
            write_json(json_path, report)
        except OSError as error:
            add_error(
                errors,
                "json-output",
                json_path,
                None,
                "cannot write JSON report: %s" % error,
            )
            report["ok"] = False
            report["errors"] = errors

    if errors:
        for item in errors:
            location = item["file"]
            if item["line"] is not None:
                location = "%s:%s" % (location, item["line"])
            print("STATUS ERROR: %s: %s" % (location, item["message"]))
        print("status check: FAILED (%d error(s))" % len(errors))
        return 1

    print(summary)
    return 0


if __name__ == "__main__":
    sys.exit(main())
