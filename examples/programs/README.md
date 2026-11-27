# Real programs: json, markdown, lisp and wordstats

`examples/programs/` contains four small but complete lsaca programs. Each one is
deterministic: with no arguments it runs an embedded input and prints a fixed result; with
one argument (a file path) it runs the same code over that file instead. Each program has a
single-file C17 counterpart that implements the same algorithms and prints byte-identical
stdout, so the pair is useful as a cross-language smoke test as well as a language showcase.

| program | lsaca source | lines | C counterpart | lines |
| --- | --- | ---: | --- | ---: |
| JSON parser and canonical serializer | [json.ls](json.ls) | 792 | [json.c](json.c) | 1138 |
| Markdown (subset) to HTML converter | [markdown.ls](markdown.ls) | 366 | [markdown.c](markdown.c) | 544 |
| Tiny Lisp interpreter | [lisp.ls](lisp.ls) | 729 | [lisp.c](lisp.c) | 1344 |
| Word statistics report | [wordstats.ls](wordstats.ls) | 177 | [wordstats.c](wordstats.c) | 334 |

The C counterparts are plain C17 with no dependencies (link `-lm`; on Windows the programs
switch stdout to binary mode so `\n` is not translated to `\r\n`).

## Running

```console
$ lsaca run examples/programs/json.ls
$ lsaca run examples/programs/json.ls -- examples/programs/data/sample.json
$ lsaca run examples/programs/markdown.ls
$ lsaca run examples/programs/markdown.ls -- examples/programs/data/sample.md
$ lsaca run examples/programs/lisp.ls
$ lsaca run examples/programs/lisp.ls -- examples/programs/data/sample.lisp
$ lsaca run examples/programs/wordstats.ls
$ lsaca run examples/programs/wordstats.ls -- examples/programs/data/sample.txt
```

Build and run the C counterparts:

```console
$ gcc -std=c17 -O2 -Wall -Wextra -Werror examples/programs/json.c -lm -o json_c
$ gcc -std=c17 -O2 -Wall -Wextra -Werror examples/programs/markdown.c -lm -o markdown_c
$ gcc -std=c17 -O2 -Wall -Wextra -Werror examples/programs/lisp.c -lm -o lisp_c
$ gcc -std=c17 -O2 -Wall -Wextra -Werror examples/programs/wordstats.c -o wordstats_c
$ ./json_c
$ ./json_c examples/programs/data/sample.json
```

## json.ls

A JSON tokenizer, parser, validator, canonicalizer and pretty printer, driven by eight
embedded documents plus an optional file argument. It reports per-document statistics and
error positions, sorts object keys for canonical output, and renders numbers in the same
stable form every time.

Expected stdout with no arguments (1340 bytes):

```text
json: parse, validate and canonicalize JSON
documents: 8
[1] parsed
  canonical: {"active":true,"escaped":"line\nbreak\ttab \"quoted\" back\\slash","final":null,"meta":{"empty":[],"nothing":{},"tags":["lsaca","json","v2.2"]},"name":"lsaca","scores":[1,2.5,3.75],"unicode":"café","version":2.2}
  stats: nodes=18 objects=3 arrays=3 strings=6 numbers=4 bools=1 nulls=1 depth=4
  pretty:
{
  "active": true,
  "escaped": "line\nbreak\ttab \"quoted\" back\\slash",
  "final": null,
  "meta": {
    "empty": [],
    "nothing": {},
    "tags": [
      "lsaca",
      "json",
      "v2.2"
    ]
  },
  "name": "lsaca",
  "scores": [
    1,
    2.5,
    3.75
  ],
  "unicode": "café",
  "version": 2.2
}
[2] parsed
  canonical: [1,2,3]
  stats: nodes=4 objects=0 arrays=1 strings=0 numbers=3 bools=0 nulls=0 depth=2
[3] parsed
  canonical: {"a":2,"z":1}
  stats: nodes=3 objects=1 arrays=0 strings=0 numbers=2 bools=0 nulls=0 depth=2
[4] parsed
  canonical: "plain string"
  stats: nodes=1 objects=0 arrays=0 strings=1 numbers=0 bools=0 nulls=0 depth=1
[5] rejected: duplicate object key at line 1 column 16
[6] rejected: trailing comma in array at line 1 column 7
[7] rejected: unicode escapes above U+00FF are not supported in this build at line 1 column 16
[8] rejected: unterminated string at line 1 column 18
summary: 4 accepted, 4 rejected
```

With `examples/programs/data/sample.json` (1528 bytes):

```console
$ lsaca run examples/programs/json.ls -- examples/programs/data/sample.json | tail -n 8
$ ./json_c examples/programs/data/sample.json | tail -n 8
```

Both print the same per-document report; the file-mode output starts with the same banner and
embedded-document report, then appends the `sample.json` entry. A missing file prints
`file document: read error: No such file or directory (os error 2)` without failing.

## markdown.ls

A line-based Markdown subset converter. It supports ATX headings, paragraphs, unordered
lists, blockquotes, fenced code blocks, inline code, bold and italic emphasis, links, and
backslash escapes, and it HTML-escapes text along the way. The embedded document and
`data/sample.md` exercise every construct.

Expected stdout with no arguments (1084 bytes):

```text
markdown: convert a Markdown subset to HTML
input lines: 25
output bytes: 941
blocks: headings=2 paragraphs=3 lists=1 items=3 quotes=1 code=1
<h1>Markdown demo</h1>
<p>markdown.ls converts a small Markdown subset to HTML. It handles <strong>bold</strong> text, <em>italic</em> text, <code>inline code</code> and links such as <a href="https://example.com/lsaca?lang=lsaca&amp;tab=docs">the lsaca repository</a>.</p>
<p>Escapes keep markup literal: *not italic*, `not code`, # no heading.</p>
<h2>Lists, quotes and code</h2>
<ul>
<li>first item</li>
<li>second item with <strong>bold</strong> and <em>emphasis</em></li>
<li>third item with <code>x = 1</code> and a <a href="https://example.com/list">link</a></li>
</ul>
<blockquote>
<p>quoted line one quoted line two with <strong>bold</strong> and <code>code</code> and a <a href="https://example.com/quote">link</a></p>
</blockquote>
<pre><code>fn main() {
    println!(&quot;hello from a fenced block &lt;tag&gt; &amp; more&quot;);
}</code></pre>
<p>Trailing paragraph with unicode: café, naïve, and 5 &lt; 6 &amp; 7 &gt; 6.</p>

```

The `blocks: ...` line summarizes what the converter saw; the rest is the generated HTML.
File mode appends `file: <path>` and the same byte/block report for the file.

## lisp.ls

A tiny Lisp interpreter: tokenizer, reader, evaluator with lexical environments, special
forms (`quote`, `if`, `define`, `set!`, `let`, `lambda`, `begin`, `print`) and builtins for
arithmetic, comparison and lists. The embedded script exercises recursion, closures, lists,
quotes, `let`, `set!` and predicates; two parse-error demos show the reader's diagnostics.

Expected stdout with no arguments (351 bytes):

```text
lisp: a small Lisp interpreter
embedded script lines: 28
42
144
720
42
(1 2 3)
1
(2 3)
(0 1 2)
3
"less"
12
(a b c)
sym
43
1
1
true
true
true
true
true
false
"done"
expressions evaluated: 183
values printed: 23
parse error demo: expected ')' but reached end of input at line 1 column 7
parse error demo: unterminated string literal at line 1 column 14
```

With `data/sample.lisp` the tail of the output is (464 bytes total):

```text
42
12
"file mode ok"
4
true
true
expressions evaluated: 137
values printed: 8

```

## wordstats.ls

A word-frequency report over embedded text or a file. It tokenizes runs of ASCII letters and
digits, lowercases them, and prints the line and byte counts, the number of words, unique
words and total word bytes, the average and longest word, the five most frequent words
(count descending, then lexicographic) and a length histogram. The report order never depends
on hash-map slot order, so the output is deterministic.

Expected stdout with no arguments (261 bytes):

```text
word statistics
lines: 6
long lines: 4
bytes: 273
words: 51
unique words: 34
average word length: 4
longest word: iterators (9)
top words:
  the 7
  dog 4
  fox 3
  lazy 3
  a 2
histogram:
  1-2: 6
  3-4: 28
  5-6: 9
  7-8: 6
  9-10: 2
  11+: 0
word bytes: 212
```

File mode appends `file: <path>` and the same report for `data/sample.txt`.

## Data fixtures

| file | bytes | used by |
| --- | ---: | --- |
| [data/sample.json](data/sample.json) | 175 | `json.ls -- data/sample.json` |
| [data/sample.md](data/sample.md) | 819 | `markdown.ls -- data/sample.md` |
| [data/sample.lisp](data/sample.lisp) | 504 | `lisp.ls -- data/sample.lisp` |
| [data/sample.txt](data/sample.txt) | 262 | `wordstats.ls -- data/sample.txt` |

## Verification

Every mode below was run through both implementations and compared byte for byte
(`fc /b` on Windows, `cmp` elsewhere); stdout, exit code and stderr match (stderr is empty).

| program | invocation | stdout bytes | SHA-256 |
| --- | --- | ---: | --- |
| json | no arguments | 1340 | `7489c158da84d90ddf3e163c93d9afd5fddf4e56bf62a10d3213bef0fcd48250` |
| json | `data/sample.json` | 1528 | `1bfdb0243fe03a1a9b3eac3008c6f1d568b585b7daa10e78f2dd4edb950ce938` |
| markdown | no arguments | 1084 | `16a902a6bc3ebdde1abaa29db2f135c148fd893264410bb39cf56c4f4437c9db` |
| markdown | `data/sample.md` | 2261 | `228c63c5e255b612ffc0abab60c8f91e2481a60d4c88dce2ee4e23e06d8faf73` |
| lisp | no arguments | 351 | `0cd802d14a61707147f8cd5d46ca4c64fcb10b2318a4d2571253ebee2084b20f` |
| lisp | `data/sample.lisp` | 464 | `730d5070bf812ff171474aaca7c5df98d0612d4a5eb8e6d2094684a6a927e2cb` |
| wordstats | no arguments | 261 | `7cfa47b26a1f600707ab234ad468809af8a6f8da1306a4cd6a29c00e27021d93` |
| wordstats | `data/sample.txt` | 527 | `498b74868e29dd39859dd3792be0e2e71c73522c2e61cb37619f4ce8654db8a3` |

Hashes are SHA-256 of the CRLF-normalized stdout. The `Real example programs` CI job
re-checks all eight invocations against both implementations with
`scripts/real_programs.py`.

Notes:

* The JSON program decodes `\uXXXX` escapes only up to U+00FF (the same restriction as the
  lsaca program); code points above that are copied through unchanged.
* The Lisp programs resolve recursive definitions by merging the caller's environment into a
  closure's captured environment. This keeps the embedded `fact` example working in both
  implementations without changing observable behavior for lexical closures.
