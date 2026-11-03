# v2.0 official benchmark run plan

Prepared 2026-11-02 by migrator from the Phase D pre-performance baseline. This plan is
ready but NOT executed: the Lead runs the official matrix on an idle host after the
Phase D code lands and the tree is re-verified. Companion files:
benchmarks/results/v2-docs-page-template.md (page layout and publishing checklist) and
scripts/bench_report.py (section generator, incl. the three-way before/after).

## (a) Exact official command

### Preconditions

- Compiler: the Lead's final release build (newer than every edit under
  crates/lsaca/src). Do not use an intermediate or stale binary: the Phase D baseline
  showed the measured medians move a lot between builds.
- Idle host: no cargo builds, no other benchmark runs; the harness measures peak RSS and
  cold compile times on the same machine.
- Tools discoverable on PATH: gcc (MSYS2 UCRT64), rustc, node, python 3.12+.
- No pre-set LSACA_* variables; the harness builds its own child environment, drops
  LSACA_CACHE_DIR/LSACA_CC/LSACA_CFLAGS and points TEMP/TMP/TMPDIR at
  target/bench-tmp/tmp.

### Environment and run

```powershell
$repo = 'C:\Codes\lsaca - 副本 (2)'
Set-Location $repo
$env:PATH = 'C:/msys64/ucrt64/bin;' + $env:PATH
$compiler = Join-Path $repo 'target\lead-rel\release\lsaca.exe'   # substitute the final build path
python scripts/bench.py --compiler $compiler --build-mode official `
  --json "$repo\benchmarks\results\latest.json" `
  --md "$repo\benchmarks\results\latest.md"
```

Defaults of that command (do not override unless the Lead says otherwise):

- 25 programs: 22 executed (18 original + 4 alternatives) + 3 compile-only;
- 5 languages (lsaca, C, Rust, Python 3, Node.js) where each workload defines them;
- runs 7, warmups 1, compile-repeats 3, scale-runs 3, build-mode official
  (no fallback; official is the published back end), memory measurement on;
- output paths as given: latest.json + latest.md in benchmarks/results/.

Expected duration on an idle 16-thread host: about 6-9 minutes (the Phase D baseline
subset of 12 programs with 5 runs / 2 compile repeats took about 2.5 minutes on a
loaded host; the full 25-program matrix with the defaults is roughly 2.5-3x that).

### Verify the compiler identity first

```powershell
Get-Item $compiler | Select-Object Length, LastWriteTime
& $compiler version
Get-FileHash -Algorithm SHA256 $compiler
git rev-parse --short HEAD
```

Record those four values in the report identity block; they must match the JSON
toolchains.lsaca_compiler_sha256 prefix and repository_commit the harness records.

### Acceptance checks after the run

- exit code 0;
- stderr ends with the raw-results and markdown paths and has NO 'warnings:' block;
- JSON: warnings [] and oracle_verification.mismatches [];
- validation messages: every measured run matched the expected checksum;
- markdown contains the Compile time, Runtime, Peak memory tables plus the new Runtime
  by data scale, artifact sizes and independent-oracles sections;
- compare cold/cached numbers against the acceptance targets: hello-sized cold build
  0.35-0.5 s, cached build about 15 ms, compile_large cold <= 2.5x the C reference,
  every executed runtime within 1.4x of C or listed in the documented exceptions.

Any failure: do not publish. Fix the compiler/tree first (Lead decision); the harness
aborts on the first checksum mismatch, and --build-mode official fails hard instead of
silently switching to the fallback back end.

### Post-run steps (Lead or migrator)

```powershell
New-Item -ItemType Directory -Force benchmarks\results\archive\v2.0 | Out-Null
Copy-Item benchmarks\results\latest.json benchmarks\results\archive\v2.0\latest.json -Force
Copy-Item benchmarks\results\latest.md benchmarks\results\archive\v2.0\latest.md -Force
python scripts/bench_report.py --json benchmarks/results/latest.json `
  --baseline benchmarks/results/archive/v1.7/latest.json `
  --baseline2 benchmarks/results/archive/v2.0-pre-perf/latest.json `
  --out benchmarks/results/v2-report-sections.md
```

Then paste the generated sections into docs/language/21-benchmarks.md as described in
the template (docs-verifier/Lead own the page edit), and run the documentation gates:

```powershell
python scripts/check_bench21.py     # expect 0 failures
python scripts/docs_verify.py
```

`check_bench21.py` recomputes the Results tables from the new JSON: the Compile time,
Runtime and Peak memory tables must be byte-identical to latest.md, and the new tables
must not disturb the table it picks by heading keyword (see (b)).

## (b) Page-21 section draft plan

Current page: docs/language/21-benchmarks.md (read-only for me; docs-verifier/Lead
edit). Keep every existing heading and the three tables under `## Results` exactly in
their current column layout: scripts/check_bench21.py compares them byte-for-byte with
latest.md and recomputes the ratio table from the JSON. New material is appended, never
substituted.

1. `# Benchmarks` intro - update from the generated identity block: generated_at, lsaca
   version + compiler SHA-256 prefix + commit, host and same-machine wording; program
   counts (25 = 22 executed incl. 4 alternatives + 3 compile-only); the v2.0 target
   statement (within 1.4x of C or documented; cold 0.35-0.5 s; cached about 15 ms).
2. `## Workloads` - add the four alternative rows (fib_iter, fib_memo,
   sort_insertion, array_wordcount with their expected checksums) and a paragraph
   explaining they demonstrate implementation trade-offs, not racing.
3. `## What is measured` - add small/medium/large scales (medium = published default),
   independent oracles (scripts/oracles.py), functional checks
   (scripts/bench_checks.py). Keep the existing compile/runtime/peak descriptions.
4. `## Environment` - regenerate from the new JSON host + toolchains; must match
   latest.md Environment block.
5. `## Results` - keep the existing three tables, then append, in this order:
   - `### Runtime by data scale` - from '## Runtime by data scale (median ms)'
     (13 scalable workloads x small/medium/large, args, expected line, five languages);
   - `### Workload detail` - from '## Workload detail ...' (runtime, peak, executable
     bytes, cold build per workload x language; interpreted languages say n/a);
   - `### Build artefacts` - from '## Build artefacts and compile time (lsaca)'
     (all 25 programs incl. compile-only; generated C bytes/files, executable bytes,
     cold + cached medians);
   - `### Independent oracle verification` - the generated paragraph (checksum counts,
     mismatches: 0, abort-before-measure rule).
   check_bench21 safety: it finds the language tables by header[:1] == ['language']
   with fib and iterator_pipeline in the header, and resolves the compile table as the
   FIRST table whose heading contains 'Compile time'. So: never put a new table under a
   heading containing 'Compile time', and keep `### Runtime by data scale` AFTER the
   existing `### Runtime` table (the first 'Runtime' heading match must stay the old
   table). `### Workload detail`, `### Build artefacts`, `### Independent oracle
   verification` are new headings and are ignored by the checker.
6. `## Analysis` - refresh the existing narrative with v2.0 numbers, then add a final
   subsection (e.g. `### What 2.0 changed: identity, collections, standard library`,
   replacing the current `### What 2.0 changes (run pending)`) built from the
   generator's three-way block:
   - '### Runtime medians: three-way comparison (ms, lsaca)' - workload | v1.7 |
     Phase D baseline | v2.0 final | final vs v1.7 | final x C;
   - '### Compile medians: three-way comparison (lsaca build -r)' - cold/cached
     three-way;
   - '### v2.0 acceptance targets' - smallest single-file cold build (0.35-0.5 s),
     fib cached (~15 ms), compile_large cold <= 2.5x C (needs the separately measured
     gcc reference), and the within-1.4x count.
7. `## Fairness notes and caveats` - add: alternatives are deliberate trade-off
   demonstrations; scale rows share one host (no cross-machine table); the binary_trees
   by-value note (deep copies are part of the measured workload by design;
   binary_trees_refs is the reference-traversal variant), plus every remaining >1.4x
   workload with an explicit reason from the generated '### Documented exceptions and
   limitations' block; state that the Phase D baseline numbers are directional (loaded
   host) if quoted.
8. `## Reproducing` - update the command to the one in (a) and note latest.json is
   overwritten by each official run while archives keep the old ones.

Values to fill from generated blocks only: identity, Workload detail, Build artefacts,
data-scale, oracle verification, three-way before/after, exceptions list. Hand-written
numbers (intro bullets, analysis prose) must be taken from the same two JSON files.

## (c) --compile-repeats for the official run

Recommendation: keep the default `--compile-repeats 3` for the published run, because
the page states a median of three cold builds and it matches the template. `--compile-
repeats 2` is technically acceptable and schema-identical (cold_runs_s has two entries,
cold_median_s becomes a mean of two) if the run must be shorter; disclose 'two cold
builds per program' in the report. Cost difference: every program does 2 x repeats
builds (3 -> 6, 2 -> 4), so dropping to 2 saves one cold pass, roughly 20-35 s idle
over the 25-program suite (single-file cold builds ~0.9-1.0 s each, the three project
builds several seconds each) - under 10% of the run. CI keeps `--quick` (1 repeat) or
`--compile-repeats 2` only when medians are wanted.

## Notes carried from the Phase D baseline

- string_unicode was the only genuine new regression (375.0 ms / 20.0x C); root cause is
  the three full-String copies per stdlib String method call through the external
  stdlib wrappers; a generated-C prototype fix restores 284.5 -> 31.6 ms (~1.7x C).
  Phase D codegen should fix it; re-check it in the official run.
- binary_trees by-value (18.76x medium, 29.47x large) is semantics-driven and documented,
  not chased; binary_trees_refs is the comparable reference-traversal workload.
- Phase D host was loaded: cold/cached medians there are directional only.
