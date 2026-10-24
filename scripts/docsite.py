#!/usr/bin/env python3
"""docsite.py -- dependency-free static site generator for the lsaca manual.

Turns the markdown documents under docs/ into a small static website that can be
published with GitHub Pages.  Only the Python 3 standard library is used: no
markdown package, no PyYAML, no network access.

Usage (paths are resolved from the repository, so the script may be run from
anywhere):

    python scripts/docsite.py                 # build docs/ into _site/
    python scripts/docsite.py --check         # validate only, never write
    python scripts/docsite.py --docs DIR --output DIR
    python scripts/docsite.py --repo-url https://github.com/owner/repo

Supported markdown (GitHub flavoured subset): ATX headings with stable
de-duplicated anchors, fenced and indented code blocks with an optional
language label, paragraphs, hard line breaks, horizontal rules, blockquotes,
ordered/unordered/nested lists, pipe tables with alignment, inline code, bold,
italic, autolinks, inline and reference-style links, and images.

Validation (always performed; --check performs only this):
  * docs/README.md must exist and every page must begin with a level-1 heading
    that provides its title; duplicate page titles are an error.
  * Every fenced code block must be closed.
  * Every relative link or image must point at an existing file, markdown page
    or anchor.  Relative links leaving docs/ are rewritten to
    https://github.com/<owner>/<repo>/blob/main/... URLs.
  * The output directory may not live inside docs/ (or be an ancestor of it),
    so a build can never delete documentation sources.

Exit codes: 0 success, 1 validation/build error, 2 command line usage error.
"""

from __future__ import annotations

import argparse
import html
import os
import posixpath
import re
import shutil
import sys
from dataclasses import dataclass, field
from pathlib import Path
from urllib.parse import unquote

SCRIPT_PATH = Path(__file__).resolve()
REPO_ROOT = SCRIPT_PATH.parent.parent
DEFAULT_DOCS = REPO_ROOT / "docs"
DEFAULT_OUTPUT = REPO_ROOT / "_site"
DEFAULT_REPO_URL = "https://github.com/menghuihuan44/lsaca"
BRANCH = "main"
STYLESHEET_REL = "_static/docsite.css"
GENERATOR = "scripts/docsite.py"
BACKTICK = "\x60"


class SiteFailure(Exception):
    """A fatal problem that prevents building the site at all."""


# ---------------------------------------------------------------------------
# small helpers
# ---------------------------------------------------------------------------

def indent_width(text):
    """Width of the leading indentation, counting a tab as four columns."""
    width = 0
    for ch in text:
        if ch == " ":
            width += 1
        elif ch == "\t":
            width += 4
        else:
            break
    return width


def strip_indent(text, width):
    """Remove at most width columns of leading indentation from text."""
    removed = 0
    index = 0
    length = len(text)
    while index < length and removed < width:
        ch = text[index]
        if ch == " ":
            removed += 1
            index += 1
        elif ch == "\t":
            removed += 4
            index += 1
        else:
            break
    return text[index:]


def is_blank(text):
    return text.strip() == ""


def slugify(text):
    """GitHub-style heading anchor: lower-case, spaces to '-', drop punctuation."""
    slug = []
    for ch in text.strip().lower():
        if ch.isalnum() or ch in "-_":
            slug.append(ch)
        elif ch.isspace():
            slug.append("-")
    result = "".join(slug)
    return result if result else "section"


def plain_text(tokens):
    """Flatten inline tokens into visible text (for slugs and image alt text)."""
    parts = []
    for token in tokens:
        kind = token[0]
        if kind in ("text", "code", "autolink"):
            parts.append(token[1])
        elif kind in ("strong", "em"):
            parts.append(plain_text(token[1]))
        elif kind in ("link", "image"):
            parts.append(plain_text(token[1]))
        elif kind == "br":
            parts.append(" ")
    return "".join(parts)


class Line:
    """A source line carrying its original 1-based line number."""

    __slots__ = ("text", "no")

    def __init__(self, text, no):
        self.text = text
        self.no = no


@dataclass
class Page:
    source: Path
    rel: Path                # relative to docs/, for example language/01-x.md
    out_rel: Path            # relative to the output root, for example language/01-x.html
    title: str = ""
    anchors: list = field(default_factory=list)
    blocks: list = field(default_factory=list)
    body: str = ""


@dataclass
class Issue:
    path: str
    line: int
    message: str


@dataclass
class Definition:
    url: str
    title: str = ""


# ---------------------------------------------------------------------------
# markdown parser
# ---------------------------------------------------------------------------

ATX_RE = re.compile(r"^ {0,3}(#{1,6})(?:[ \t]+(.*?))?[ \t]*$")
FENCE_RE = re.compile(r"^ {0,3}(\x60{3,}|~{3,})(.*)$")
FENCE_CLOSE = {
    BACKTICK: re.compile(r"^ {0,3}\x60{3,}\s*$"),
    "~": re.compile(r"^ {0,3}~{3,}\s*$"),
}
HR_RE = re.compile(r"^ {0,3}(?:(?:\*\s*){3,}|(?:-\s*){3,}|(?:_\s*){3,})$")
QUOTE_RE = re.compile(r"^ {0,3}> ?(.*)$")
LIST_RE = re.compile(
    r"^(?P<indent> {0,3})(?P<marker>[-+*]|(?P<num>\d{1,9})[.)])(?P<space>[ \t]+)(?P<content>.*)$"
)
DEF_RE = re.compile(
    r"""^ {0,3}\[([^\]]+)\]:[ \t]+(\S+)(?:[ \t]+(?:"([^"]*)"|'([^']*)'|\(([^)]*)\)))?[ \t]*$"""
)
AUTOLINK_RE = re.compile(r"<((?:https?://|mailto:)[^>\s]+)>")
TABLE_DELIM_CELL = re.compile(r"^:?-{1,}:?$")
EXTERNAL_RE = re.compile(r"^[A-Za-z][A-Za-z0-9+.-]*:")
BARE_URL_RE = re.compile(r'https?://[^\s<>"]+')
ESCAPABLE = "\\" + BACKTICK + "*_{}[]()#+-.!|>~"


def split_table_row(text):
    """Split one pipe-table row into raw cell strings (escaped pipes kept)."""
    stripped = text.strip()
    if stripped.startswith("|"):
        stripped = stripped[1:]
    if stripped.endswith("|") and not stripped.endswith("\\|"):
        stripped = stripped[:-1]
    cells = re.split(r"(?<!\\)\|", stripped)
    return [cell.replace("\\|", "|").strip() for cell in cells]


def table_alignments(text):
    """Return the alignment list for a table delimiter row, or None."""
    if "|" not in text:
        return None
    cells = split_table_row(text)
    if not cells:
        return None
    aligns = []
    for cell in cells:
        if not TABLE_DELIM_CELL.match(cell):
            return None
        left = cell.startswith(":")
        right = cell.endswith(":")
        if left and right:
            aligns.append("center")
        elif right:
            aligns.append("right")
        elif left:
            aligns.append("left")
        else:
            aligns.append(None)
    return aligns


class MarkdownParser:
    """Parse one markdown page into a block AST (links are resolved later)."""

    def __init__(self, site, page, text):
        self.site = site
        self.page = page
        self.text = text
        self.anchors = []
        self._used_anchors = set()
        self.definitions = {}

    def error(self, line_no, message):
        self.site.report(self.page.source, line_no, message)

    def parse(self):
        lines = [Line(text, index + 1) for index, text in enumerate(self.text.split("\n"))]
        while lines and is_blank(lines[-1].text):
            lines.pop()
        return self._parse_blocks(lines)

    # -- block level --------------------------------------------------------
    def _parse_blocks(self, lines):
        blocks = []
        i = 0
        count = len(lines)
        while i < count:
            raw = lines[i].text
            if is_blank(raw):
                i += 1
                continue
            indent = indent_width(raw)

            definition = DEF_RE.match(raw)
            if definition:
                label = definition.group(1).strip().lower()
                url = definition.group(2)
                title = definition.group(3) or definition.group(4) or definition.group(5) or ""
                self.definitions[label] = Definition(url, title)
                i += 1
                continue

            fence = FENCE_RE.match(raw)
            if fence:
                marker = fence.group(1)[0]
                length = len(fence.group(1))
                info = fence.group(2).strip()
                language = info.split()[0].rstrip(",") if info else ""
                start = lines[i].no
                body = []
                i += 1
                closed = False
                while i < count:
                    close = FENCE_CLOSE[marker].match(lines[i].text)
                    if close and len(close.group(0).strip()) >= length:
                        closed = True
                        i += 1
                        break
                    body.append(lines[i].text)
                    i += 1
                if not closed:
                    self.error(start, "unclosed fenced code block (missing closing fence)")
                blocks.append(("code", language, "\n".join(body), start))
                continue

            heading = ATX_RE.match(raw)
            if heading:
                level = len(heading.group(1))
                content = heading.group(2) or ""
                content = re.sub(r"[ \t]+#+[ \t]*$", "", content).strip()
                inline = self._inline(content, lines[i].no)
                anchor = self._anchor_for(plain_text(inline))
                blocks.append(("heading", level, anchor, inline, lines[i].no))
                i += 1
                continue

            if HR_RE.match(raw):
                blocks.append(("hr",))
                i += 1
                continue

            if indent <= 3 and QUOTE_RE.match(raw):
                quote_lines = []
                while i < count:
                    match = QUOTE_RE.match(lines[i].text)
                    if match:
                        quote_lines.append(Line(match.group(1), lines[i].no))
                        i += 1
                    elif is_blank(lines[i].text):
                        quote_lines.append(Line("", lines[i].no))
                        i += 1
                    else:
                        break
                while quote_lines and is_blank(quote_lines[-1].text):
                    quote_lines.pop()
                blocks.append(("quote", self._parse_blocks(quote_lines)))
                continue

            if "|" in raw and i + 1 < count:
                aligns = table_alignments(lines[i + 1].text)
                if aligns is not None:
                    headers = [self._inline(cell, lines[i].no) for cell in split_table_row(raw)]
                    if len(headers) != len(aligns):
                        self.error(lines[i].no, "table header and delimiter rows have different column counts")
                        if len(aligns) < len(headers):
                            aligns = aligns + [None] * (len(headers) - len(aligns))
                        else:
                            headers = headers[: len(aligns)]
                    rows = []
                    j = i + 2
                    while j < count and not is_blank(lines[j].text) and "|" in lines[j].text:
                        cells = split_table_row(lines[j].text)
                        if len(cells) < len(headers):
                            cells = cells + [""] * (len(headers) - len(cells))
                        rows.append([self._inline(cell, lines[j].no) for cell in cells[: len(headers)]])
                        j += 1
                    blocks.append(("table", aligns, headers, rows))
                    i = j
                    continue

            if indent <= 3 and LIST_RE.match(raw):
                block, i = self._parse_list(lines, i)
                blocks.append(block)
                continue

            if indent >= 4:
                start = lines[i].no
                body = []
                while i < count and (is_blank(lines[i].text) or indent_width(lines[i].text) >= 4):
                    body.append(strip_indent(lines[i].text, 4))
                    i += 1
                while body and is_blank(body[-1]):
                    body.pop()
                blocks.append(("code", "", "\n".join(body), start))
                continue

            start = lines[i].no
            para = [raw.strip()]
            i += 1
            while i < count:
                text = lines[i].text
                if is_blank(text):
                    break
                ind = indent_width(text)
                if ind <= 3 and (
                    ATX_RE.match(text)
                    or FENCE_RE.match(text)
                    or HR_RE.match(text)
                    or QUOTE_RE.match(text)
                    or LIST_RE.match(text)
                ):
                    break
                para.append(text[4:].strip() if ind >= 4 else text.strip())
                i += 1
            blocks.append(("paragraph", self._inline("\n".join(para), start)))
        return blocks

    def _anchor_for(self, text):
        base = slugify(text)
        candidate = base
        counter = 1
        while candidate in self._used_anchors:
            candidate = "{0}-{1}".format(base, counter)
            counter += 1
        self._used_anchors.add(candidate)
        self.anchors.append(candidate)
        return candidate

    def _parse_list(self, lines, start_index):
        first = LIST_RE.match(lines[start_index].text)
        base_indent = len(first.group("indent"))
        ordered = first.group("num") is not None
        start_number = int(first.group("num")) if ordered else 1
        items = []
        loose = False
        i = start_index
        count = len(lines)

        while i < count:
            match = LIST_RE.match(lines[i].text)
            if not match:
                break
            indent = len(match.group("indent"))
            if indent != base_indent:
                break
            if (match.group("num") is not None) != ordered:
                break
            prefix = lines[i].text[: match.start("content")].expandtabs(4)
            content_col = len(prefix)
            body = [Line(match.group("content"), lines[i].no)]
            item_had_blank = False
            i += 1
            while i < count:
                text = lines[i].text
                if is_blank(text):
                    j = i
                    while j < count and is_blank(lines[j].text):
                        j += 1
                    if j < count and indent_width(lines[j].text) > base_indent:
                        for k in range(i, j):
                            body.append(Line("", lines[k].no))
                        item_had_blank = True
                        i = j
                        continue
                    break
                ind = indent_width(text)
                nested = LIST_RE.match(text)
                if ind == base_indent and nested:
                    break
                if ind > base_indent:
                    body.append(Line(strip_indent(text, min(content_col, ind)), lines[i].no))
                    i += 1
                    continue
                if ind <= 3 and (
                    ATX_RE.match(text)
                    or FENCE_RE.match(text)
                    or HR_RE.match(text)
                    or QUOTE_RE.match(text)
                ):
                    break
                body.append(Line(text.strip(), lines[i].no))
                i += 1
            while body and is_blank(body[-1].text):
                body.pop()
            if item_had_blank:
                loose = True
            items.append(self._parse_blocks(body))
            j = i
            while j < count and is_blank(lines[j].text):
                j += 1
            if j < count:
                nxt = LIST_RE.match(lines[j].text)
                if (
                    nxt
                    and len(nxt.group("indent")) == base_indent
                    and (nxt.group("num") is not None) == ordered
                ):
                    if j != i:
                        loose = True
                    i = j
                    continue
            break
        return ("list", ordered, start_number, items, loose), i

    # -- inline level -------------------------------------------------------
    def _inline(self, text, base_line):
        tokens = []
        buffer = []

        def flush():
            if buffer:
                tokens.append(("text", "".join(buffer)))
                del buffer[:]

        i = 0
        n = len(text)
        while i < n:
            ch = text[i]
            if ch == "\\" and i + 1 < n and text[i + 1] in ESCAPABLE:
                buffer.append(text[i + 1])
                i += 2
                continue
            if ch == BACKTICK:
                run = 1
                while i + run < n and text[i + run] == BACKTICK:
                    run += 1
                close = self._find_code_close(text, i + run, run)
                if close < 0:
                    buffer.append(BACKTICK * run)
                    i += run
                    continue
                code_text = text[i + run : close]
                if (
                    len(code_text) >= 2
                    and code_text.startswith(" ")
                    and code_text.endswith(" ")
                    and code_text.strip(" ") != ""
                ):
                    code_text = code_text[1:-1]
                flush()
                tokens.append(("code", code_text))
                i = close + run
                continue
            if ch == "!" and i + 1 < n and text[i + 1] == "[":
                parsed = self._parse_link(text, i + 1)
                if parsed is not None:
                    label, url, title, end = parsed
                    line = base_line + text.count("\n", 0, i)
                    flush()
                    tokens.append(("image", self._inline(label, line), url, title, line))
                    i = end
                    continue
            if ch == "[":
                parsed = self._parse_link(text, i)
                if parsed is not None:
                    label, url, title, end = parsed
                    line = base_line + text.count("\n", 0, i)
                    flush()
                    tokens.append(("link", self._inline(label, line), url, title, line))
                    i = end
                    continue
            if text.startswith("**", i):
                close = text.find("**", i + 2)
                if close > i + 2:
                    line = base_line + text.count("\n", 0, i)
                    flush()
                    tokens.append(("strong", self._inline(text[i + 2 : close], line)))
                    i = close + 2
                    continue
            if ch == "*":
                close = text.find("*", i + 1)
                if close > i + 1:
                    line = base_line + text.count("\n", 0, i)
                    flush()
                    tokens.append(("em", self._inline(text[i + 1 : close], line)))
                    i = close + 1
                    continue
            if ch in "hH" and (i == 0 or not (text[i - 1].isalnum() or text[i - 1] in "_-/")):
                bare = BARE_URL_RE.match(text, i)
                if bare:
                    url = bare.group(0)
                    while url and url[-1] in ".,;:!?'\"*_":
                        url = url[:-1]
                    while url.endswith(")") and url.count("(") < url.count(")"):
                        url = url[:-1]
                    if url:
                        flush()
                        tokens.append(("autolink", url))
                        i += len(url)
                        continue
            if ch == "<":
                auto = AUTOLINK_RE.match(text, i)
                if auto:
                    flush()
                    tokens.append(("autolink", auto.group(1)))
                    i = auto.end()
                    continue
            if ch == "\n":
                joined = "".join(buffer)
                if joined.endswith("  "):
                    buffer[:] = list(joined[:-2])
                    flush()
                    tokens.append(("br",))
                else:
                    flush()
                    tokens.append(("text", " "))
                i += 1
                continue
            buffer.append(ch)
            i += 1
        flush()
        return tokens

    @staticmethod
    def _find_code_close(text, start, run):
        i = start
        n = len(text)
        while i < n:
            if text[i] == BACKTICK:
                j = i
                while j < n and text[j] == BACKTICK:
                    j += 1
                if j - i == run:
                    return i
                i = j
            else:
                i += 1
        return -1

    def _parse_link(self, text, open_index):
        """Parse [label](dest), [label][ref] or [label]. Returns a tuple or None."""
        n = len(text)
        i = open_index + 1
        depth = 0
        close = -1
        while i < n:
            ch = text[i]
            if ch == "\\" and i + 1 < n:
                i += 2
                continue
            if ch == "[":
                depth += 1
            elif ch == "]":
                if depth == 0:
                    close = i
                    break
                depth -= 1
            i += 1
        if close < 0:
            return None
        label = text[open_index + 1 : close]
        after = close + 1
        if after < n and text[after] == "(":
            parsed = self._parse_inline_destination(text, after + 1)
            if parsed is None:
                return None
            url, title, end = parsed
            return label, url, title, end
        if after < n and text[after] == "[":
            end = text.find("]", after + 1)
            if end < 0:
                return None
            ref = text[after + 1 : end].strip() or label
            definition = self.definitions.get(ref.strip().lower())
            if definition is None:
                return None
            return label, definition.url, definition.title, end + 1
        definition = self.definitions.get(label.strip().lower())
        if definition is not None:
            return label, definition.url, definition.title, after
        return None

    @staticmethod
    def _parse_inline_destination(text, start):
        n = len(text)
        i = start
        while i < n and text[i] in " \t\n":
            i += 1
        if i < n and text[i] == "<":
            end = text.find(">", i + 1)
            if end < 0:
                return None
            url = text[i + 1 : end]
            i = end + 1
        else:
            begin = i
            depth = 0
            while i < n:
                ch = text[i]
                if ch == "\\" and i + 1 < n:
                    i += 2
                    continue
                if ch == "(":
                    depth += 1
                elif ch == ")":
                    if depth == 0:
                        break
                    depth -= 1
                elif ch in " \t\n":
                    break
                i += 1
            url = text[begin:i]
        while i < n and text[i] in " \t\n":
            i += 1
        title = ""
        if i < n and text[i] in "\"'":
            quote = text[i]
            end = text.find(quote, i + 1)
            if end < 0:
                return None
            title = text[i + 1 : end]
            i = end + 1
            while i < n and text[i] in " \t\n":
                i += 1
        if i < n and text[i] == ")":
            return url, title, i + 1
        return None


# ---------------------------------------------------------------------------
# site model: discovery, link resolution, rendering, output
# ---------------------------------------------------------------------------

class Site:
    """One documentation site build (or validation pass)."""

    def __init__(self, docs, output, check_only, repo_root, repo_url):
        self.docs = docs.resolve()
        self.output = output.resolve()
        self.check_only = check_only
        self.repo_root = repo_root.resolve()
        self.repo_url = repo_url.rstrip("/")
        self.issues = []
        self.assets = {}
        self.pages = []
        self.page_by_source = {}
        self.page_by_rel = {}
        self.link_count = 0
        self.site_title = "lsaca"
        self.asset_count = 0

        match = re.match(r"https?://github\.com/([^/]+)/([^/]+?)(?:\.git)?/?$", self.repo_url)
        if match:
            owner, name = match.group(1), match.group(2)
            self.blob_base = "https://github.com/{0}/{1}/blob/{2}/".format(owner, name, BRANCH)
            self.tree_base = "https://github.com/{0}/{1}/tree/{2}/".format(owner, name, BRANCH)
            self.raw_base = "https://raw.githubusercontent.com/{0}/{1}/{2}/".format(owner, name, BRANCH)
        else:
            self.blob_base = self.repo_url + "/blob/" + BRANCH + "/"
            self.tree_base = self.repo_url + "/tree/" + BRANCH + "/"
            self.raw_base = self.blob_base

    # -- helpers -----------------------------------------------------------
    def display_path(self, path):
        try:
            return path.resolve().relative_to(self.repo_root).as_posix()
        except (OSError, ValueError):
            return str(path)

    def report(self, source, line_no, message):
        self.issues.append(Issue(self.display_path(source), line_no or 0, message))

    @staticmethod
    def _is_under(child, parent):
        try:
            child.relative_to(parent)
            return True
        except ValueError:
            return False

    def _href(self, target_rel, page):
        start = page.out_rel.parent.as_posix()
        return posixpath.relpath(target_rel, start=start or ".")

    @staticmethod
    def _validate_fragment(target_page, fragment, site, page, line_no, raw):
        if fragment and fragment not in target_page.anchors:
            site.report(page.source, line_no, "link target '{0}' refers to a missing anchor '#{1}'".format(raw, fragment))

    # -- discovery ---------------------------------------------------------
    def discover(self):
        if not self.docs.is_dir():
            raise SiteFailure("docs directory not found: {0}".format(self.docs))
        index_source = self.docs / "README.md"
        if not index_source.is_file():
            raise SiteFailure(
                "missing required documentation index: {0} (docs/README.md is the site home page)".format(
                    self.display_path(index_source)
                )
            )
        seen_out = {}
        for source in sorted(self.docs.rglob("*.md")):
            if not source.is_file():
                continue
            resolved = source.resolve()
            try:
                rel = resolved.relative_to(self.docs)
            except ValueError:
                continue
            if rel.name == "README.md":
                if rel.parent.as_posix() == ".":
                    out_rel = Path("index.html")
                else:
                    out_rel = rel.parent / "index.html"
            else:
                out_rel = rel.with_suffix(".html")
            page = Page(resolved, rel, out_rel)
            self.pages.append(page)
            key = out_rel.as_posix()
            if key in seen_out:
                self.report(source, 0, "output path {0} is produced by two documents".format(key))
            else:
                seen_out[key] = resolved
        for source in sorted(self.docs.rglob("*")):
            if not source.is_file():
                continue
            if source.suffix.lower() == ".md":
                continue
            try:
                rel = source.resolve().relative_to(self.docs)
            except ValueError:
                continue
            if any(part.startswith(".") for part in rel.parts):
                continue
            rel_posix = rel.as_posix()
            if rel_posix in seen_out:
                self.report(source, 0, "asset path {0} collides with a generated page".format(rel_posix))
                continue
            self.assets[rel_posix] = source.resolve()
        if STYLESHEET_REL in self.assets:
            raise SiteFailure("docs/{0} collides with the generated stylesheet path".format(STYLESHEET_REL))
        self.asset_count = len(self.assets)

    # -- parse -------------------------------------------------------------
    def parse_all(self):
        for page in self.pages:
            try:
                text = page.source.read_bytes().decode("utf-8")
            except UnicodeDecodeError as exc:
                self.report(page.source, 0, "document is not valid UTF-8: {0}".format(exc))
                continue
            except OSError as exc:
                self.report(page.source, 0, "cannot read document: {0}".format(exc))
                continue
            if text.startswith("\ufeff"):
                text = text[1:]
            parser = MarkdownParser(self, page, text)
            page.blocks = parser.parse()
            page.anchors = parser.anchors
            self.page_by_source[page.source] = page
            self.page_by_rel[page.rel.as_posix()] = page
        self.check_titles()

    def check_titles(self):
        titles = {}
        for page in self.pages:
            heading = None
            for block in page.blocks:
                if block[0] == "heading" and block[1] == 1:
                    heading = block
                    break
            if heading is None:
                self.report(page.source, 0, "page has no level-1 heading (start the page with '# Title')")
                continue
            page.title = plain_text(heading[3]).strip()
            if not page.title:
                self.report(page.source, heading[4], "page title is empty")
                continue
            key = page.title.lower()
            if key in titles:
                self.report(
                    page.source,
                    heading[4],
                    "duplicate page title '{0}' (already used by {1})".format(page.title, self.display_path(titles[key])),
                )
            else:
                titles[key] = page.source
        index_page = self.page_by_rel.get("README.md")
        if index_page is not None and index_page.title:
            self.site_title = index_page.title

    # -- links -------------------------------------------------------------
    def resolve_link(self, raw, page, line_no, is_image=False):
        target = raw.strip()
        if target == "":
            self.report(page.source, line_no, "link with an empty target")
            return None
        if "\x00" in target:
            self.report(page.source, line_no, "link target contains a NUL byte")
            return None
        if target.startswith("#"):
            fragment = unquote(target[1:])
            if fragment and fragment not in page.anchors:
                self.report(
                    page.source, line_no, "link target '{0}' refers to a missing anchor '#{1}'".format(raw, fragment)
                )
            self.link_count += 1
            return "#" + fragment
        if EXTERNAL_RE.match(target) or target.startswith("//"):
            return target
        if target.startswith("/"):
            self.report(
                page.source,
                line_no,
                "absolute link target '{0}' is not supported (use a relative path)".format(raw),
            )
            return None
        if target.startswith("?"):
            self.report(page.source, line_no, "query-only link target '{0}' is not supported".format(raw))
            return None

        path_part, _, fragment = target.partition("#")
        decoded = unquote(path_part)
        if decoded == "":
            if fragment and fragment not in page.anchors:
                self.report(
                    page.source, line_no, "link target '{0}' refers to a missing anchor '#{1}'".format(raw, fragment)
                )
            self.link_count += 1
            return ("#" + fragment) if fragment else None

        candidate = (page.source.parent / decoded).resolve()
        if self._is_under(candidate, self.docs):
            try:
                rel = candidate.relative_to(self.docs)
            except ValueError:
                self.report(page.source, line_no, "link target '{0}' does not exist".format(raw))
                return None
            if candidate.is_dir():
                candidate = candidate / "README.md"
                rel = rel / "README.md"
            target_page = self.page_by_source.get(candidate)
            if target_page is not None:
                self._validate_fragment(target_page, fragment, self, page, line_no, raw)
                self.link_count += 1
                href = self._href(target_page.out_rel.as_posix(), page)
                return href + (("#" + fragment) if fragment else "")
            if candidate.is_file() and candidate.suffix.lower() != ".md":
                rel_posix = rel.as_posix()
                self.assets[rel_posix] = candidate
                self.link_count += 1
                href = self._href(rel_posix, page)
                return href + (("#" + fragment) if fragment else "")
            self.report(page.source, line_no, "link target '{0}' does not exist".format(raw))
            return None

        if not candidate.exists():
            self.report(page.source, line_no, "link target '{0}' does not exist".format(raw))
            return None
        if not self._is_under(candidate, self.repo_root):
            self.report(
                page.source, line_no, "link target '{0}' points outside the repository".format(raw)
            )
            return None
        rel = candidate.relative_to(self.repo_root).as_posix()
        suffix = ("#" + fragment) if fragment else ""
        self.link_count += 1
        if candidate.is_dir():
            return self.tree_base + rel + suffix
        if is_image:
            return self.raw_base + rel + suffix
        return self.blob_base + rel + suffix

    # -- rendering ---------------------------------------------------------
    def render_all(self):
        for page in self.pages:
            page.body = self.render_blocks(page.blocks, page)

    def render_blocks(self, blocks, page):
        out = []
        for block in blocks:
            kind = block[0]
            if kind == "heading":
                level = min(max(block[1], 1), 6)
                out.append(
                    '<h{0} id="{1}">{2}</h{0}>'.format(
                        level, html.escape(block[2], quote=True), self.render_inline(block[3], page)
                    )
                )
            elif kind == "paragraph":
                out.append("<p>{0}</p>".format(self.render_inline(block[1], page)))
            elif kind == "code":
                out.append(self.render_code_block(block[1], block[2]))
            elif kind == "quote":
                inner = self.render_blocks(block[1], page)
                out.append("<blockquote>\n{0}\n</blockquote>".format(inner))
            elif kind == "hr":
                out.append("<hr>")
            elif kind == "list":
                out.append(self.render_list(block, page))
            elif kind == "table":
                out.append(self.render_table(block, page))
        return "\n".join(out)

    @staticmethod
    def render_code_block(language, text):
        safe = re.sub(r"[^A-Za-z0-9_+.-]", "", language or "")
        label = ""
        if language:
            label = '<span class="code-label">{0}</span>'.format(html.escape(language, quote=True))
        class_attr = ' class="language-{0}"'.format(safe) if safe else ""
        return "{0}<pre class=\"code-block\"><code{1}>{2}</code></pre>".format(
            label, class_attr, html.escape(text)
        )

    def render_list(self, block, page):
        ordered, start_number, items, loose = block[1], block[2], block[3], block[4]
        tag = "ol" if ordered else "ul"
        attrs = ' start="{0}"'.format(start_number) if ordered and start_number != 1 else ""
        parts = ["<{0}{1}>".format(tag, attrs)]
        for item_blocks in items:
            inner = self.render_list_item(item_blocks, page, not loose)
            parts.append("<li>{0}</li>".format(inner))
        parts.append("</{0}>".format(tag))
        return "\n".join(parts)

    def render_list_item(self, blocks, page, tight):
        if not blocks:
            return ""
        if tight and len(blocks) == 1 and blocks[0][0] == "paragraph":
            return self.render_inline(blocks[0][1], page)
        parts = []
        for index, block in enumerate(blocks):
            if tight and index == 0 and block[0] == "paragraph":
                parts.append(self.render_inline(block[1], page))
            else:
                parts.append(self.render_blocks([block], page))
        return "\n".join(parts)

    def render_table(self, block, page):
        aligns, headers, rows = block[1], block[2], block[3]

        def cell(tag, tokens, align):
            style = ' style="text-align:{0}"'.format(align) if align else ""
            return "<{0}{1}>{2}</{0}>".format(tag, style, self.render_inline(tokens, page))

        lines = ['<div class="table-wrap">', "<table>", "<thead>", "<tr>"]
        lines.append("".join(cell("th", tokens, aligns[i]) for i, tokens in enumerate(headers)))
        lines.append("</tr>")
        lines.append("</thead>")
        if rows:
            lines.append("<tbody>")
            for row in rows:
                lines.append("<tr>")
                lines.append("".join(cell("td", tokens, aligns[i]) for i, tokens in enumerate(row)))
                lines.append("</tr>")
            lines.append("</tbody>")
        lines.append("</table>")
        lines.append("</div>")
        return "\n".join(lines)

    def render_inline(self, tokens, page):
        out = []
        for token in tokens:
            kind = token[0]
            if kind == "text":
                out.append(html.escape(token[1], quote=False))
            elif kind == "br":
                out.append("<br>")
            elif kind == "code":
                out.append("<code>{0}</code>".format(html.escape(token[1], quote=False)))
            elif kind == "strong":
                out.append("<strong>{0}</strong>".format(self.render_inline(token[1], page)))
            elif kind == "em":
                out.append("<em>{0}</em>".format(self.render_inline(token[1], page)))
            elif kind == "link":
                out.append(self.render_link(token, page))
            elif kind == "image":
                out.append(self.render_image(token, page))
            elif kind == "autolink":
                url = token[1]
                out.append(
                    '<a href="{0}">{1}</a>'.format(html.escape(url, quote=True), html.escape(url, quote=False))
                )
        return "".join(out)

    def render_link(self, token, page):
        children, url, title, line_no = token[1], token[2], token[3], token[4]
        text = self.render_inline(children, page)
        href = self.resolve_link(url, page, line_no, False)
        if href is None:
            return text
        title_attr = ' title="{0}"'.format(html.escape(title, quote=True)) if title else ""
        return '<a href="{0}"{1}>{2}</a>'.format(html.escape(href, quote=True), title_attr, text)

    def render_image(self, token, page):
        children, url, title, line_no = token[1], token[2], token[3], token[4]
        alt = html.escape(plain_text(children), quote=True)
        src = self.resolve_link(url, page, line_no, True)
        if src is None:
            return '<span class="missing-image">[{0}]</span>'.format(alt)
        title_attr = ' title="{0}"'.format(html.escape(title, quote=True)) if title else ""
        return '<img src="{0}" alt="{1}"{2} loading="lazy">'.format(
            html.escape(src, quote=True), alt, title_attr
        )

    # -- page shell --------------------------------------------------------
    def render_sidebar(self, current):
        is_index = current.rel.as_posix() == "README.md"
        home_href = self._href("index.html", current)
        parts = ["<ul>"]
        active = ' class="active" aria-current="page"' if is_index else ""
        parts.append('<li><a href="{0}"{1}>Home</a></li>'.format(html.escape(home_href, quote=True), active))
        parts.append("</ul>")

        language = [
            page
            for page in self.pages
            if page.rel.parts[0] == "language" and re.match(r"^\d+-.*\.md$", page.rel.name)
        ]
        language.sort(key=lambda page: page.rel.as_posix())
        language_rels = set(page.rel.as_posix() for page in language)
        if language:
            parts.append('<p class="sidebar-title">Language reference</p>')
            parts.append("<ul>")
            for page in language:
                href = self._href(page.out_rel.as_posix(), current)
                active = ' class="active" aria-current="page"' if page.source == current.source else ""
                label = page.title or page.rel.stem
                parts.append(
                    '<li><a href="{0}"{1}>{2}</a></li>'.format(
                        html.escape(href, quote=True), active, html.escape(label, quote=False)
                    )
                )
            parts.append("</ul>")

        others = [
            page for page in self.pages if page.rel.as_posix() != "README.md" and page.rel.as_posix() not in language_rels
        ]
        others.sort(key=lambda page: page.rel.as_posix())
        if others:
            parts.append('<p class="sidebar-title">More pages</p>')
            parts.append("<ul>")
            for page in others:
                href = self._href(page.out_rel.as_posix(), current)
                active = ' class="active" aria-current="page"' if page.source == current.source else ""
                label = page.title or page.rel.stem
                parts.append(
                    '<li><a href="{0}"{1}>{2}</a></li>'.format(
                        html.escape(href, quote=True), active, html.escape(label, quote=False)
                    )
                )
            parts.append("</ul>")
        return "\n".join(parts)

    def render_page(self, page):
        is_index = page.rel.as_posix() == "README.md"
        home_href = self._href("index.html", page)
        css_href = self._href(STYLESHEET_REL, page)
        if is_index:
            page_title = self.site_title or "lsaca"
            back_link = '<a class="back-link" href="{0}">Source repository</a>'.format(
                html.escape(self.repo_url, quote=True)
            )
        else:
            page_title = "{0} - {1}".format(page.title, self.site_title or "lsaca")
            back_link = '<a class="back-link" href="{0}">&larr; Back to docs index</a>'.format(
                html.escape(home_href, quote=True)
            )
        lines = [
            "<!DOCTYPE html>",
            '<html lang="en">',
            "<head>",
            '<meta charset="utf-8">',
            '<meta name="viewport" content="width=device-width, initial-scale=1">',
            "<title>{0}</title>".format(html.escape(page_title, quote=False)),
            '<meta name="generator" content="{0}">'.format(html.escape(GENERATOR, quote=True)),
            '<link rel="stylesheet" href="{0}">'.format(html.escape(css_href, quote=True)),
            "</head>",
            "<body>",
            '<a class="skip-link" href="#content">Skip to content</a>',
            '<header class="site-header">',
            '<a class="site-title" href="{0}">{1}</a>'.format(
                html.escape(home_href, quote=True), html.escape(self.site_title or "lsaca", quote=False)
            ),
            '<span class="site-subtitle">Language reference manual</span>',
            "</header>",
            '<div class="layout">',
            '<nav class="sidebar" aria-label="Manual">',
            '<p class="sidebar-title">Sections</p>',
            self.render_sidebar(page),
            "</nav>",
            '<main id="content" class="content">',
            page.body,
            '<footer class="page-footer">',
            back_link,
            "<p>Generated from the markdown sources in <code>docs/</code> by <code>{0}</code>.</p>".format(
                html.escape(GENERATOR, quote=False)
            ),
            "</footer>",
            "</main>",
            "</div>",
            "</body>",
            "</html>",
            "",
        ]
        return "\n".join(lines)


    # -- output ------------------------------------------------------------
    def validate_output(self):
        out = self.output
        docs = self.docs
        if out == docs or self._is_under(out, docs):
            raise SiteFailure("output directory must not be inside docs/: {0}".format(out))
        if self._is_under(docs, out):
            raise SiteFailure("output directory must not contain docs/: {0}".format(out))
        if out == self.repo_root:
            raise SiteFailure("refusing to use the repository root as the output directory: {0}".format(out))
        if out == Path(out.anchor):
            raise SiteFailure("refusing to use the filesystem root as the output directory")
        if out.exists() and not out.is_dir():
            raise SiteFailure("output path exists and is not a directory: {0}".format(out))

    def write(self):
        try:
            if self.output.exists():
                shutil.rmtree(self.output)
            self.output.mkdir(parents=True)
            css_path = self.output / STYLESHEET_REL
            css_path.parent.mkdir(parents=True, exist_ok=True)
            css_path.write_bytes(CSS.encode("utf-8"))
            for page in sorted(self.pages, key=lambda item: item.out_rel.as_posix()):
                destination = self.output / page.out_rel
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_bytes(self.render_page(page).encode("utf-8"))
            for rel_posix in sorted(self.assets):
                destination = self.output / rel_posix
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(self.assets[rel_posix], destination)
        except OSError as exc:
            raise SiteFailure("failed to write the site: {0}".format(exc))

    def print_issues(self):
        for issue in sorted(self.issues, key=lambda item: (item.path, item.line, item.message)):
            if issue.line > 0:
                location = "{0}:{1}".format(issue.path, issue.line)
            else:
                location = issue.path
            print("docsite: error: {0}: {1}".format(location, issue.message), file=sys.stderr)
        if self.check_only:
            print(
                "docsite: {0} error(s) found during validation".format(len(self.issues)),
                file=sys.stderr,
            )
        else:
            print(
                "docsite: {0} error(s) found; no output was written".format(len(self.issues)),
                file=sys.stderr,
            )

    def build(self):
        self.discover()
        if not self.pages:
            raise SiteFailure("no markdown pages were found under docs/: {0}".format(self.docs))
        if not self.check_only:
            self.validate_output()
        self.parse_all()
        self.render_all()
        if self.issues:
            self.print_issues()
            return False
        if self.check_only:
            print(
                "docsite: checked {0} page(s), {1} asset(s), {2} internal link(s); no issues found".format(
                    len(self.pages), len(self.assets), self.link_count
                )
            )
            return True
        self.write()
        print(
            "docsite: wrote {0} page(s) and {1} asset(s) to {2}".format(
                len(self.pages), len(self.assets), self.display_path(self.output)
            )
        )
        return True


# ---------------------------------------------------------------------------
# stylesheet (kept inline so the site has no build-time dependencies)
# ---------------------------------------------------------------------------

CSS = """
:root {
  --bg: #ffffff;
  --fg: #1f2328;
  --muted: #59636e;
  --border: #d1d9e0;
  --accent: #0969da;
  --code-bg: #f6f8fa;
}

* { box-sizing: border-box; }

html { font-size: 16px; }

body {
  margin: 0;
  background: var(--bg);
  color: var(--fg);
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif;
  line-height: 1.6;
}

.skip-link {
  position: absolute;
  left: -9999px;
  top: 0;
  background: var(--fg);
  color: #ffffff;
  padding: 0.5rem 1rem;
  z-index: 10;
}

.skip-link:focus { left: 1rem; }

.site-header {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 0.75rem;
  padding: 1rem 1.5rem;
  border-bottom: 1px solid var(--border);
}

.site-title {
  font-weight: 700;
  font-size: 1.25rem;
  color: var(--fg);
  text-decoration: none;
}

.site-subtitle { color: var(--muted); font-size: 0.95rem; }

.layout {
  display: flex;
  align-items: flex-start;
  max-width: 72rem;
  margin: 0 auto;
  padding: 0 1.5rem;
}

.sidebar {
  flex: 0 0 17rem;
  position: sticky;
  top: 0;
  max-height: 100vh;
  overflow-y: auto;
  padding: 1.5rem 1rem 2rem 0;
  font-size: 0.92rem;
  border-right: 1px solid var(--border);
}

.sidebar ul { list-style: none; margin: 0 0 1rem; padding: 0; }
.sidebar li { margin: 0.15rem 0; }

.sidebar a {
  display: block;
  padding: 0.2rem 0.5rem;
  border-radius: 4px;
  color: var(--fg);
  text-decoration: none;
}

.sidebar a:hover { background: var(--code-bg); }

.sidebar a.active {
  background: #ddf4ff;
  color: var(--accent);
  font-weight: 600;
}

.sidebar-title {
  margin: 1.25rem 0 0.35rem;
  font-size: 0.75rem;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: var(--muted);
}

.content { flex: 1 1 auto; min-width: 0; padding: 1.5rem 0 3rem 2rem; }

.content h1,
.content h2,
.content h3,
.content h4,
.content h5,
.content h6 { line-height: 1.25; margin: 1.75rem 0 0.75rem; scroll-margin-top: 1rem; }

.content h1 { font-size: 2rem; border-bottom: 1px solid var(--border); padding-bottom: 0.4rem; }
.content h2 { font-size: 1.5rem; border-bottom: 1px solid var(--border); padding-bottom: 0.3rem; }
.content a { color: var(--accent); }

.content code {
  font-family: ui-monospace, SFMono-Regular, "Cascadia Code", Consolas, monospace;
  font-size: 0.9em;
  background: var(--code-bg);
  border: 1px solid var(--border);
  border-radius: 4px;
  padding: 0.1em 0.3em;
}

.code-label {
  display: inline-block;
  margin: 0.75rem 0 0;
  padding: 0.1rem 0.5rem;
  font-family: ui-monospace, Consolas, monospace;
  font-size: 0.75rem;
  color: var(--muted);
  background: var(--code-bg);
  border: 1px solid var(--border);
  border-bottom: none;
  border-radius: 4px 4px 0 0;
}

pre.code-block {
  margin: 0.5rem 0 1.25rem;
  padding: 1rem;
  overflow-x: auto;
  background: var(--code-bg);
  border: 1px solid var(--border);
  border-radius: 4px;
}

.code-label + pre.code-block { margin-top: 0; border-top-left-radius: 0; }

pre.code-block code {
  display: block;
  background: none;
  border: none;
  padding: 0;
  font-size: 0.88rem;
  line-height: 1.5;
}

blockquote {
  margin: 1rem 0;
  padding: 0.25rem 1rem;
  color: var(--muted);
  border-left: 4px solid var(--border);
}

hr { border: none; border-top: 1px solid var(--border); margin: 2rem 0; }

.table-wrap { overflow-x: auto; margin: 1rem 0 1.5rem; }

table { border-collapse: collapse; width: 100%; font-size: 0.95rem; }

th, td { border: 1px solid var(--border); padding: 0.4rem 0.75rem; text-align: left; vertical-align: top; }

thead th { background: var(--code-bg); }

.content img { max-width: 100%; height: auto; }

.missing-image { color: #cf222e; }

.page-footer {
  margin-top: 3rem;
  padding-top: 1rem;
  border-top: 1px solid var(--border);
  color: var(--muted);
  font-size: 0.88rem;
}

.page-footer p { margin: 0.5rem 0 0; }

.back-link { font-weight: 600; }

@media (max-width: 800px) {
  .layout { flex-direction: column; padding: 0 1rem; }
  .sidebar {
    position: static;
    flex: 1 1 auto;
    width: 100%;
    max-height: none;
    overflow: visible;
    border-right: none;
    border-bottom: 1px solid var(--border);
    padding: 1rem 0;
  }
  .content { padding: 1rem 0 2.5rem; }
  .site-header { padding: 0.9rem 1rem; }
}
"""


# ---------------------------------------------------------------------------
# command line entry point
# ---------------------------------------------------------------------------

def parse_args(argv):
    parser = argparse.ArgumentParser(
        prog="docsite.py",
        description="Build the lsaca documentation website from the markdown sources under docs/.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="Exit codes: 0 success, 1 validation/build error, 2 command line usage error.",
    )
    parser.add_argument(
        "--docs",
        type=Path,
        default=None,
        help="documentation source directory (default: <repo-root>/docs)",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=None,
        help="output directory (default: <repo-root>/_site)",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="validate the documentation only; do not write any output",
    )
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=None,
        help="repository root used to resolve links that leave docs/ (default: the repository containing this script)",
    )
    parser.add_argument(
        "--repo-url",
        default=DEFAULT_REPO_URL,
        help="GitHub repository URL used to rewrite links that leave docs/ (default: %(default)s)",
    )
    return parser.parse_args(argv)


def main(argv=None):
    args = parse_args(sys.argv[1:] if argv is None else argv)
    repo_root = (args.repo_root or REPO_ROOT).resolve()
    if args.docs is not None:
        docs = args.docs
    else:
        docs = repo_root / "docs"
    if args.output is not None:
        output = args.output
    else:
        output = repo_root / "_site"
    site = Site(docs, output, args.check, repo_root, args.repo_url)
    try:
        ok = site.build()
    except SiteFailure as exc:
        print("docsite: error: {0}".format(exc), file=sys.stderr)
        return 1
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
