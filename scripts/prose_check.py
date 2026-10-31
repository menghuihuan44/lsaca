#!/usr/bin/env python3
"""Probe documentation claims against the compiler.

Each case ties a claim written in the docs to a small program (or a CLI command)
and pins the behaviour the docs promise.  The script checks that the claim text
still appears in the documentation (so a doc rewrite that drops the claim is
noticed) and that the compiler still behaves the way the claim says.

V16-FLIP
--------
The expectations in this file target the v1.6 contract (the char type, UTF-8
String with character APIs, full-Unicode case mapping and trimming,
character-boundary substring panics, and a Swiss-Table HashMap).  They were
rewritten while the v1.6 documentation was drafted, before the v1.6 compiler
existed; against a v1.5 binary some cases fail on purpose during the migration
window.  When the compiler changes again, edit the doc claims and the case data
here together.

Exit status: 0 when every claim and probe passes, 1 otherwise.
"""

from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
BT = chr(96)

DEFAULT_WORK = REPO_ROOT / "target" / "docs-tmp" / "prose"
DEFAULT_TIMEOUT = 120

# V16-FLIP: True while the case data describes the 1.6 contract.
V16_CONTRACT = True

SOURCES = {}

SOURCES["version"] = ""

# Byte-oriented API: len counts bytes and substring takes byte offsets.  The
# char_count method counts Unicode scalar values instead.
SOURCES["date_parts"] = r"""fn main() {
    let text = String::from("héllo");
    println!("{} {} {}", text.len(), "hello".len(), text.char_count());
    println!("{}", text.substring(1, 3));
    println!("{}", text.to_uppercase());
}
"""

SOURCES["find_trim"] = r"""fn main() {
    let text = String::from("héllo");
    match text.find("l") {
        Some(index) => println!("find {}", index),
        None => println!("find none"),
    }

    let spaced = String::from("  hi  ");
    let nbsp = String::from("\u{00A0}hi\u{00A0}");
    println!("{} {}", spaced.trim().len(), nbsp.trim().len());
}
"""

SOURCES["case_mapping"] = r"""fn main() {
    println!("{}", String::from("straße").to_uppercase());
    println!("{}", String::from("ÉCOLE").to_lowercase());
}
"""

SOURCES["width_chars"] = r"""fn main() {
    println!("[{:^6}]", "你好");
    println!("[{:>8}]", "héllo");
}
"""

SOURCES["precision_chars"] = r"""fn main() {
    println!("[{:.2}]", "你好世界");
}
"""

SOURCES["debug_quotes"] = r"""fn main() {
    println!("{:?} {}", "hi", "hi");
    println!("{:?}", Some("x"));
}
"""

SOURCES["debug_escaping"] = r"""fn main() {
    println!("{:?}", String::from("café"));
    println!("{:?}", String::from("a\tb"));
    println!("{:?}", '\n');
}
"""

SOURCES["substring_panic"] = r"""fn main() {
    let text = String::from("abc");
    println!("{}", text.substring(10, 2));
}
"""

SOURCES["substring_boundary_panic"] = r"""fn main() {
    let text = String::from("héllo");
    println!("{}", text.substring(1, 1));
}
"""

SOURCES["char_literal"] = r"""fn main() {
    let c = 'a';
    println!("{} {:?}", c, c);
}
"""

SOURCES["char_api"] = r"""fn main() {
    let mut text = String::from("héllo");
    println!("{} {} {:?}", text.len(), text.char_count(), text.chars().len());
    println!("{} {}", text.is_char_boundary(1), text.is_char_boundary(2));
    println!("{:?} {:?}", text.char_at(1), text.char_at(9));
    text.push('!');
    println!("{} {:?}", text, text.pop());
    println!("{:?}", String::from_char('ñ'));
}
"""

SOURCES["hashmap_insert"] = r"""fn main() {
    let mut ages: HashMap<String, i64> = HashMap::new();
    let first = ages.insert(String::from("ana"), 30);
    let second = ages.insert(String::from("ana"), 31);
    println!("{:?} {:?}", first, second);
    println!("{:?}", ages.get(String::from("ana")));
}
"""

SOURCES["hashmap_keys"] = r"""fn main() {
    let mut scores: HashMap<String, i64> = HashMap::new();
    scores.insert(String::from("a"), 1);

    let keys = scores.keys();
    let values = scores.values();
    println!("{:?}", keys);
    println!("{:?}", values);
}
"""

SOURCES["hashmap_equality"] = r"""fn main() {
    let mut first: HashMap<i64, i64> = HashMap::new();
    first.insert(1, 10);
    first.insert(2, 20);

    let mut second: HashMap<i64, i64> = HashMap::new();
    second.insert(2, 20);
    second.insert(1, 10);

    println!("{}", first == second);
}
"""

SOURCES["hashmap_format"] = r"""fn main() {
    let mut map: HashMap<i64, i64> = HashMap::new();
    map.insert(1, 10);
    println!("{}", map);
}
"""

SOURCES["hashmap_char_keys"] = r"""fn main() {
    let mut letters: HashMap<char, i64> = HashMap::new();
    letters.insert('a', 1);
    letters.insert('é', 2);

    println!("{:?} {:?}", letters.get('a'), letters.get('\u{61}'));
    println!("{}", letters.contains_key('é'));
    println!("{}", letters.len());
}
"""

SOURCES["hashmap_bad_key"] = r"""fn main() {
    let mut map: HashMap<Vec<i64>, i64> = HashMap::new();
    map.insert(vec![1], 2);
}
"""

SOURCES["hashmap_for_loop"] = r"""fn main() {
    let mut scores: HashMap<i64, i64> = HashMap::new();
    scores.insert(1, 2);
    for entry in scores {
        println!("{}", entry);
    }
}
"""

CASES = [
    {
        "name": "release-version",
        "kind": "version",
        "docs": [("docs/README.md", [
            "This documentation set is the reference for the language as of",
            "Every runnable example on these pages has been executed",
        ])],
    },
    {
        "name": "help-text",
        "kind": "help",
        "docs": [("docs/language/01-getting-started.md", [
            "The complete help text of version",
            "lsaca <COMMAND> [OPTIONS]",
            "EXIT CODES:",
        ])],
    },
    {
        "name": "char-literal-and-type",
        "kind": "run",
        "source": "char_literal",
        "docs": [
            ("docs/language/03-lexical-structure.md", [
                "A character literal is written in single quotes",
            ]),
            ("docs/language/04-types.md", [
                "Unicode scalar value",
            ]),
            ("docs/language/16-roadmap.md", [
                "Characters are scalars; strings are UTF-8",
            ]),
        ],
        "stdout": ["a 'a'"],
    },
    {
        "name": "string-byte-lengths",
        "kind": "run",
        "source": "date_parts",
        "docs": [("docs/language/15-standard-library.md", [
            "length in bytes, not characters",
            "panics unless both boundaries are character boundaries",
        ])],
        "stdout": ["6 5 5", "él", "HÉLLO"],
    },
    {
        "name": "string-character-methods",
        "kind": "run",
        "source": "char_api",
        "docs": [("docs/language/15-standard-library.md", [
            "number of Unicode scalar values",
            "String::from_char",
        ])],
        "stdout": [
            "6 5 5",
            "true false",
            "Some('é') None",
            "héllo! Some('!')",
            '"ñ"',
        ],
    },
    {
        "name": "string-find-offset-and-trim",
        "kind": "run",
        "source": "find_trim",
        "docs": [("docs/language/15-standard-library.md", [
            "byte index of the first occurrence",
            "strips leading/trailing Unicode whitespace",
        ])],
        "stdout": ["find 3", "2 2"],
    },
    {
        "name": "unicode-case-mapping",
        "kind": "run",
        "source": "case_mapping",
        "docs": [("docs/language/15-standard-library.md", [
            "Unicode default uppercasing",
            "Unicode default lowercasing",
        ])],
        "stdout": ["STRASSE", "école"],
    },
    {
        "name": "format-width-counts-characters",
        "kind": "run",
        "source": "width_chars",
        "docs": [("docs/language/15-standard-library.md", [
            "Width counts characters, not bytes",
        ])],
        "stdout": ["[  你好  ]", "[   héllo]"],
    },
    {
        "name": "format-precision-counts-characters",
        "kind": "run",
        "source": "precision_chars",
        "docs": [("docs/language/15-standard-library.md", [
            "counted in characters",
        ])],
        "stdout": ["[你好]"],
    },
    {
        "name": "debug-quotes-strings",
        "kind": "run",
        "source": "debug_quotes",
        "docs": [("docs/language/15-standard-library.md", [
            "prints strings with quotes and escapes",
        ])],
        "stdout": ['"hi" hi', 'Some("x")'],
    },
    {
        "name": "debug-escapes-nonprintable",
        "kind": "run",
        "source": "debug_escaping",
        "docs": [("docs/language/15-standard-library.md", [
            "follows Rust's escape_debug",
        ])],
        "stdout": ['"café"', '"a\\tb"', "'\\n'"],
    },
    {
        "name": "substring-panics-when-out-of-range",
        "kind": "run",
        "source": "substring_panic",
        "docs": [("docs/language/15-standard-library.md", [
            "panics unless both boundaries are character boundaries",
        ])],
        "exit": 101,
        "stderr_contains": ["byte index 12 is out of bounds of"],
    },
    {
        "name": "substring-panics-inside-character",
        "kind": "run",
        "source": "substring_boundary_panic",
        "docs": [("docs/language/15-standard-library.md", [
            "panics unless both boundaries are character boundaries",
        ])],
        "exit": 101,
        "stderr_contains": ["is not a char boundary"],
    },
    {
        "name": "hashmap-insert-returns-previous",
        "kind": "run",
        "source": "hashmap_insert",
        "docs": [("docs/language/20-hashmap.md", [
            "moves both arguments into the map; returns the previous value for the key",
        ])],
        "stdout": ["None Some(30)", "Some(31)"],
    },
    {
        "name": "hashmap-keys-slot-order",
        "kind": "run",
        "source": "hashmap_keys",
        "docs": [("docs/language/20-hashmap.md", [
            "internal slot order",
            "not insertion order",
        ])],
        "stdout": ['["a"]', "[1]"],
    },
    {
        "name": "hashmap-content-equality",
        "kind": "run",
        "source": "hashmap_equality",
        "docs": [("docs/language/20-hashmap.md", [
            "insertion order does not matter",
        ])],
        "stdout": ["true"],
    },
    {
        "name": "hashmap-format-slot-order",
        "kind": "run",
        "source": "hashmap_format",
        "docs": [("docs/language/20-hashmap.md", [
            "print the entries in slot order in the form",
        ])],
        "stdout": ["{1: 10}"],
    },
    {
        "name": "hashmap-char-keys",
        "kind": "run",
        "source": "hashmap_char_keys",
        "docs": [("docs/language/20-hashmap.md", [
            "A char is a Copy scalar and a valid key type",
        ])],
        "stdout": ["Some(1) Some(1)", "true", "2"],
    },
    {
        "name": "hashmap-key-type-diagnostic",
        "kind": "check",
        "source": "hashmap_bad_key",
        "docs": [("docs/language/20-hashmap.md", [
            "The key type must be i64, u64, bool, String or char",
        ])],
        "stderr_contains": [
            "error[E0277]: HashMap keys must be one of i64, u64, bool, char or String, found Vec<i64>",
        ],
    },
    {
        "name": "hashmap-for-loop-rejection",
        "kind": "check",
        "source": "hashmap_for_loop",
        "docs": [("docs/language/20-hashmap.md", [
            "cannot be iterated with for in 1.6.0",
        ])],
        "stderr_contains": ["cannot iterate over a value of type HashMap<i64, i64>"],
    },
]


def normalize(text):
    lines = text.replace("\r\n", "\n").replace("\r", "\n").split("\n")
    lines = [line.rstrip() for line in lines]
    while lines and lines[0] == "":
        lines.pop(0)
    while lines and lines[-1] == "":
        lines.pop()
    return "\n".join(lines)


def collapse(text):
    text = text.replace(BT, "")
    text = text.replace("**", "").replace("__", "")
    return re.sub(r"\s+", " ", text).strip()


def find_binary(explicit):
    candidates = []
    if explicit:
        candidates.append(Path(explicit))
    if os.environ.get("LSACA_BIN"):
        candidates.append(Path(os.environ["LSACA_BIN"]))
    candidates.extend([
        REPO_ROOT / "target" / "v16" / "bin" / "lsaca.exe",
        REPO_ROOT / "target" / "release" / "lsaca.exe",
        REPO_ROOT / "target" / "debug" / "lsaca.exe",
    ])
    for candidate in candidates:
        if candidate.is_file():
            return candidate.resolve()
    return None


def probe_env(work):
    env = os.environ.copy()
    temp = REPO_ROOT / "target" / "docs-tmp" / "tmp"
    temp.mkdir(parents=True, exist_ok=True)
    for name in ("TEMP", "TMP", "TMPDIR"):
        env[name] = str(temp)
    env["LSACA_CACHE_DIR"] = str(work / ".cache")
    return env


def run_command(binary, arguments, work, timeout):
    try:
        completed = subprocess.run(
            [str(binary)] + arguments,
            capture_output=True,
            text=True,
            errors="replace",
            timeout=timeout,
            env=probe_env(work),
        )
    except subprocess.TimeoutExpired:
        return None
    return completed


def main(argv):
    parser = argparse.ArgumentParser(description="Probe documentation claims against the compiler.")
    parser.add_argument("--bin", help="path to the lsaca executable")
    parser.add_argument("--work", help="scratch directory", default=str(DEFAULT_WORK))
    parser.add_argument("--timeout", type=int, default=DEFAULT_TIMEOUT)
    parser.add_argument("--verbose", action="store_true")
    options = parser.parse_args(argv[1:])

    binary = find_binary(options.bin)
    if binary is None:
        print("prose_check: no lsaca executable found; pass --bin")
        return 1

    work = Path(options.work).resolve()
    work.mkdir(parents=True, exist_ok=True)

    doc_texts = {}

    def doc_text(relative):
        if relative not in doc_texts:
            path = REPO_ROOT / relative
            if not path.is_file():
                doc_texts[relative] = None
            else:
                doc_texts[relative] = collapse(path.read_text(encoding="utf-8"))
        return doc_texts[relative]

    readme = collapse((REPO_ROOT / "docs" / "README.md").read_text(encoding="utf-8"))
    # collapse() strips markdown emphasis, so match the plain wording.
    version_match = re.search(r"reference for the language as of version ([0-9]+\.[0-9]+\.[0-9]+)", readme)
    version = version_match.group(1) if version_match else None

    failures = []
    claims = 0
    probes = 0

    def fail(name, message):
        failures.append("{0}: {1}".format(name, message))

    for index, case in enumerate(CASES, start=1):
        name = case["name"]
        case_failures = []
        for relative, fragments in case.get("docs", []):
            text = doc_text(relative)
            if text is None:
                case_failures.append("missing documentation file " + relative)
                continue
            for fragment in fragments:
                claims += 1
                if collapse(fragment) not in text:
                    case_failures.append("claim no longer present in {0}: {1}".format(relative, fragment))

        kind = case["kind"]
        probes += 1
        completed = None
        if kind == "version":
            if version is None:
                case_failures.append("cannot read the release version from docs/README.md")
            else:
                completed = run_command(binary, ["--version"], work, options.timeout)
                if completed is None:
                    case_failures.append("compiler timed out")
                else:
                    actual = normalize(completed.stdout)
                    expected = "lsaca " + version
                    if actual != expected:
                        case_failures.append("--version printed {0!r}, expected {1!r}".format(actual, expected))
        elif kind == "help":
            completed = run_command(binary, ["help"], work, options.timeout)
            if completed is None:
                case_failures.append("compiler timed out")
            else:
                output = normalize(completed.stdout)
                required = ["lsaca <COMMAND> [OPTIONS]", "EXIT CODES"]
                for needle in required:
                    if needle not in output:
                        case_failures.append("help output is missing " + repr(needle))
                if version is not None and version not in output:
                    case_failures.append("help output does not mention version " + version)
        else:
            source = SOURCES.get(case.get("source", ""))
            if source is None:
                case_failures.append("case has no probe source")
            else:
                source_path = work / "probe-{0:02d}.ls".format(index)
                source_path.write_text(source, encoding="utf-8")
                command = "check" if kind == "check" else "run"
                completed = run_command(binary, [command, str(source_path)], work, options.timeout)
                if completed is None:
                    case_failures.append("compiler timed out")
                else:
                    expected_exit = 0
                    if kind == "check":
                        expected_exit = 1
                    elif "exit" in case:
                        expected_exit = case["exit"]
                    if completed.returncode != expected_exit:
                        case_failures.append("exit code {0}, expected {1}".format(completed.returncode, expected_exit))
                    if "stdout" in case:
                        actual_lines = normalize(completed.stdout).split("\n") if normalize(completed.stdout) else []
                        expected_lines = case["stdout"]
                        if actual_lines != expected_lines:
                            case_failures.append("stdout {0!r}, expected {1!r}".format(actual_lines, expected_lines))
                    if "stderr_contains" in case:
                        combined = completed.stderr + completed.stdout
                        for needle in case["stderr_contains"]:
                            if needle not in combined:
                                case_failures.append("diagnostics are missing " + repr(needle))
                    if options.verbose:
                        print("  " + name + " -> exit " + str(completed.returncode))

        if case_failures:
            fail(name, "; ".join(case_failures))
            print("FAIL " + name)
        else:
            print("PASS " + name)

    print("prose_check: {0} case(s), {1} claim(s) checked, {2} failure(s)".format(len(CASES), claims, len(failures)))
    if failures:
        for message in failures:
            print("  " + message)
        return 1
    print("prose_check: every probed documentation claim still matches the compiler")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
