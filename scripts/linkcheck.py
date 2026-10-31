#!/usr/bin/env python3
"""Check relative links and heading anchors in the repository documentation.

Covers every markdown file a reader can reach from the project front page:

  * README.md (repository root),
  * benchmarks/README.md,
  * docs/**/*.md (the documentation site sources).

The rules mirror scripts/docsite.py: relative targets resolve against the
directory of the page; a heading anchor is the GitHub-style slug of the heading
text, with -1, -2 suffixes for duplicate headings; links to directories resolve
to the README.md inside them; targets outside the repository are rejected.

Exit status: 0 when every link resolves, 1 otherwise.
"""

from __future__ import annotations

import importlib.util
import re
import sys
from pathlib import Path
from urllib.parse import unquote

REPO_ROOT = Path(__file__).resolve().parent.parent
BT = chr(96)

EXTERNAL_RE = re.compile(r"^[A-Za-z][A-Za-z0-9+.-]*:")
FENCE_OPEN_RE = re.compile(r"^ {0,3}((" + BT + r"{3,})|(~{3,}))")
LINK_RE = re.compile(r"!?\[[^\]]*\]\(([^()]*)\)")
DEFINITION_RE = re.compile(r"^ {0,3}\[([^\]]+)\]:[ \t]*(\S+)")
COMMENT_RE = re.compile(r"<!--[\s\S]*?-->")
CODE_SPAN_RE = re.compile("(" + BT + r"+)([\s\S]*?)\1")

LINK_FILES = ["README.md", "benchmarks/README.md"]
DOCS_GLOB = "docs/**/*.md"


def load_docsite():
    path = REPO_ROOT / "scripts" / "docsite.py"
    spec = importlib.util.spec_from_file_location("docsite_module", str(path))
    module = importlib.util.module_from_spec(spec)
    # dataclasses resolves the defining module through sys.modules, so the
    # module must be registered before it is executed.  Do not leave a
    # scripts/__pycache__ directory behind in the working tree.
    sys.modules[spec.name] = module
    previous = sys.dont_write_bytecode
    sys.dont_write_bytecode = True
    try:
        spec.loader.exec_module(module)
    finally:
        sys.dont_write_bytecode = previous
    return module


DOCSITE = load_docsite()


class _StubSite:
    """Swallow parser diagnostics: scripts/docsite.py --check reports them."""

    def report(self, source, line_no, message):
        pass


class _StubPage:
    def __init__(self, source):
        self.source = source


def docsite_anchors(path, text):
    """Reuse docsite.py's parser so anchors match the published site exactly."""
    parser = DOCSITE.MarkdownParser(_StubSite(), _StubPage(path), text)
    parser.parse()
    return list(parser.anchors)


def is_under(path, parent):
    try:
        path.resolve().relative_to(parent.resolve())
        return True
    except ValueError:
        return False


def strip_code_spans(line):
    return CODE_SPAN_RE.sub(lambda match: " " * len(match.group(0)), line)


def strip_comments(text):
    return COMMENT_RE.sub(lambda match: "\n" * match.group(0).count("\n"), text)


def split_lines(text):
    return text.replace("\r\n", "\n").replace("\r", "\n").split("\n")


class Page:
    def __init__(self, path):
        self.path = path
        self.anchors = []
        self.links = []

    def load(self):
        text = strip_comments(self.path.read_text(encoding="utf-8"))
        if text.startswith("\ufeff"):
            text = text[1:]
        self.anchors = docsite_anchors(self.path, text)
        lines = split_lines(text)
        fence = None
        for number, raw in enumerate(lines, start=1):
            if fence is not None:
                stripped = raw.strip()
                if stripped and stripped[0] == fence[0] and set(stripped) == {fence[0]} and len(stripped) >= len(fence):
                    fence = None
                continue
            opening = FENCE_OPEN_RE.match(raw)
            if opening:
                marker = opening.group(1)
                fence = marker[0] * len(marker)
                continue
            line = strip_code_spans(raw)
            for match in LINK_RE.finditer(line):
                destination = parse_destination(match.group(1))
                self.links.append((number, destination))
            definition = DEFINITION_RE.match(line)
            if definition:
                self.links.append((number, definition.group(2).strip()))
        return self


def parse_destination(inner):
    text = inner.strip()
    if text == "":
        return ""
    if text.startswith("<"):
        end = text.find(">")
        return text[1:end] if end != -1 else text[1:]
    return text.split()[0]


def discover_files(explicit):
    if explicit:
        return [Path(item).resolve() for item in explicit]
    files = []
    for relative in LINK_FILES:
        candidate = REPO_ROOT / relative
        if candidate.is_file():
            files.append(candidate)
    files.extend(sorted(REPO_ROOT.glob(DOCS_GLOB)))
    return files


def main(argv):
    explicit = [arg for arg in argv[1:] if not arg.startswith("-")]
    files = discover_files(explicit)
    if not files:
        print("linkcheck: no markdown files found")
        return 1

    pages = []
    for path in files:
        pages.append(Page(path).load())
    anchors_by_path = {page.path.resolve(): page.anchors for page in pages}

    def anchors_for(path):
        resolved = path.resolve()
        if resolved in anchors_by_path:
            return anchors_by_path[resolved]
        page = Page(resolved).load()
        anchors_by_path[resolved] = page.anchors
        return page.anchors

    broken = []
    internal = 0
    external = 0

    def fail(page, line, message):
        try:
            shown = page.path.relative_to(REPO_ROOT).as_posix()
        except ValueError:
            shown = str(page.path)
        broken.append("{0}:{1}: {2}".format(shown, line, message))

    for page in pages:
        for line, destination in page.links:
            if destination == "":
                fail(page, line, "link with an empty target")
                continue
            if "\x00" in destination:
                fail(page, line, "link target contains a NUL byte")
                continue
            if destination.startswith("#"):
                fragment = unquote(destination[1:])
                internal += 1
                if fragment and fragment not in page.anchors:
                    fail(page, line, "link target '{0}' refers to a missing anchor '#{1}'".format(destination, fragment))
                continue
            if EXTERNAL_RE.match(destination) or destination.startswith("//"):
                external += 1
                continue
            if destination.startswith("/"):
                fail(page, line, "absolute link target '{0}' is not supported (use a relative path)".format(destination))
                continue
            path_part, _, fragment = destination.partition("#")
            fragment = unquote(fragment)
            decoded = unquote(path_part)
            if decoded == "":
                internal += 1
                if fragment and fragment not in page.anchors:
                    fail(page, line, "link target '{0}' refers to a missing anchor '#{1}'".format(destination, fragment))
                continue
            candidate = (page.path.parent / decoded).resolve()
            if not candidate.exists():
                fail(page, line, "link target '{0}' does not exist".format(destination))
                continue
            if not is_under(candidate, REPO_ROOT):
                fail(page, line, "link target '{0}' points outside the repository".format(destination))
                continue
            internal += 1
            if fragment:
                target = candidate
                if candidate.is_dir():
                    readme = candidate / "README.md"
                    if not readme.is_file():
                        fail(page, line, "link target '{0}' refers to an anchor but the directory has no README.md".format(destination))
                        continue
                    target = readme
                if target.suffix.lower() == ".md":
                    if fragment not in anchors_for(target):
                        fail(page, line, "link target '{0}' refers to a missing anchor '#{1}'".format(destination, fragment))

    for message in broken:
        print("BROKEN " + message)
    total = internal + external
    print("linkcheck: {0} file(s), {1} link(s) checked ({2} internal, {3} external), {4} broken".format(
        len(pages), total, internal, external, len(broken)))
    if broken:
        return 1
    print("linkcheck: every relative link and heading anchor resolves")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
