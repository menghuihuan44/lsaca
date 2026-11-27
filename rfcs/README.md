# lsaca RFCs

This directory holds design proposals for language, library and tooling
changes that are not yet documented behaviour. The process is described in
[ROADMAP.md](../ROADMAP.md#work-process).

## Process

1. Copy the template below into `rfcs/NNNN-short-title.md`, where NNNN is the
   next free number.
2. Open a pull request with the proposal. Discussion happens on the pull
   request.
3. An accepted RFC is added to the release contract in ROADMAP.md and gets a
   milestone. A rejected RFC stays in the tree with its reason, so the decision
   is not lost.
4. An RFC that changes a shipped language rule also names the deprecation
   window required by the version policy.

RFCs are small: two or three pages of plain markdown, no tooling.

## Template

```markdown
# NNNN: Title

* Status: proposed
* Target release: v2.x (or "backlog")
* Author: name, date

## Summary

One paragraph: what changes and for whom.

## Motivation

The concrete programs or workflows that are blocked today, with examples of the
workaround that exists now.

## Design

Syntax, semantics, diagnostics, interaction with the memory model (ORC), code
generation, runtime and standard library. Include the error codes that will be
introduced or changed.

## Drawbacks

What becomes harder, slower or more complex, including compile time, runtime
cost, API surface and documentation cost.

## Alternatives

The options that were considered and why this one wins.

## Compatibility and migration

What breaks, what keeps working with a deprecation warning, and for how long.

## Unresolved questions

What the RFC does not settle.
```

## Accepted RFCs

None yet. The first candidates are the v2.3 items that need a design decision:
the HashMap entry API, and the UFCS call form (diagnostic only or implemented).
