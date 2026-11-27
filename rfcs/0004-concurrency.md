# 0004: Concurrency stays out of 2.x; v3.0 decides the memory model

* Status: accepted
* Target release: v3.0 (decision recorded now)
* Author: lsaca maintainers, 2026-11-27

## Summary

lsaca 2.x is single-threaded by contract. v3.0 is the first release allowed to
change the memory model, and it will decide concurrency as one package: an
explicit thread API, `Send`/`Sync`-equivalent capabilities, an atomic or
explicitly thread-safe reference-counted handle, and a cycle collector that is
correct across threads. Until then no language or library feature may assume
threads, and no `Rc` operation may become atomic.

## Motivation

The ORC memory model is the language's defining feature: reference counting
plus a Bacon-Rajan trial-deletion cycle collector, with copy-on-write for
strings and value semantics for aggregates. Reference counts and the collector
are plain `size_t` counters in the C runtime; making them atomic is a runtime
cost paid by every program, including the many single-threaded ones. Making
concurrency opt-in instead requires the type system to know which values may
cross a thread boundary -- that is exactly the kind of type-system change the
version policy reserves for a major release.

Starting concurrency before references in aggregates (v2.6) are finished would
also freeze two interacting designs at once: a borrowed value that may be sent
between threads is a lifetime problem the 2.x borrow checker deliberately does
not solve.

## Design (decision records and constraints for v3.0)

### What 2.x guarantees

1. Every generated program runs one thread. The runtime does not create
   threads, and the C ABI has no locking.
2. `Rc<T>` refcounts are non-atomic; sharing an `Rc` across a thread boundary is
   not expressible because there is no thread API.
3. The cycle collector runs on the program thread; `ls_collect_cycles` is not
   reentrant across threads.
4. Blocking system calls (`io`, `net`, `process`) block the program thread.
   Async I/O, green threads and event loops are not on any roadmap.

### What v3.0 must decide together

1. **Handle split.** Keep `Rc<T>` non-atomic and single-threaded, and add an
   explicit `Arc<T>` with atomic counts for shared ownership? Or make the
   existing `Rc<T>` atomic and pay the cost everywhere? The first is the
   working assumption: concurrency is opt-in and visible in the type, matching
   the language's preference for visible costs.
2. **Capabilities.** A `Send`/`Sync`-shaped capability is needed for `thread::spawn`
   closures and captured values. It must be inferred structurally from the
   contained types (a struct is not `Send` if it stores an `Rc`), which means
   the property interacts with generics and trait objects.
3. **Cycle collector.** Per-thread ORC heaps are the simple answer; the hard
   question is a cycle that crosses threads (a thread-safe `Arc` back-edge to a
   per-thread `Rc`). Options: forbid cross-thread cycles through the capability
   system, or move to a globally synchronized collector. The first keeps the
   single-threaded fast path; the second is a fallback only if a real program
   needs it.
4. **Panics and joins.** A panic in a spawned thread must either abort the
   process or poison the join; the v3.0 design must pick one and keep the exit
   code (101) meaningful.
5. **Data-race detection.** CI gains a ThreadSanitizer build for a small
   concurrency suite; the existing Address/UB sanitizer jobs stay.
6. **Runtime ABI.** The C runtime grows an atomic refcount path and a thread
   abstraction; the generated C must not include OS headers directly until the
   runtime provides the wrapper.

### Non-decisions

* No `async`/`await`, no coroutines, no thread-local storage, no work-stealing
  runtime during 2.x.
* No `Mutex`/`RwLock` before `Arc` and capabilities exist.
* The standard library may expose blocking I/O only; a future async layer would
  be a separate RFC after v3.0.

## Drawbacks

* lsaca is not a systems-concurrency language in 2.x; some real workloads
  (servers, parallel pipelines) need multiple processes instead.
* Deferring means the collector design must be revisited in v3.0 with a much
  larger corpus of existing programs.

## Alternatives

1. **Atomic `Rc` in 2.x.** Simple type-system story (all values are shareable),
   but adds an uncontended atomic on every retain/release and fixes the
   collector cost before the capability system exists. Rejected.
2. **Concurrency in v2.5 together with unsafe/FFI.** Would let users write
   their own threads on raw pointers before the type system can describe them;
   rejected as the unsafe release is for C interop, not for bypassing missing
   safe abstractions.
3. **Single-threaded forever.** Rejected: servers and parallel data processing
   are a reasonable long-term target, so the decision is made now even though
   the implementation is later.

## Compatibility and migration

2.x programs keep compiling unchanged in v3.0 as long as they do not spawn
threads; `Rc<T>` semantics stay single-threaded even if `Arc<T>` appears. The
only breaking change allowed in v3.0 is the memory-model change itself, with
the deprecation window required by the version policy and a dedicated
migration guide.

## Unresolved questions

* Whether capability inference can be implemented for trait objects in the
  same release; a conservative rule (dyn values are never `Send`) may be needed
  initially.
* Whether the ORC collector must support cross-thread cycles or whether the
  capability system can forbid them soundly.
* Whether `Arc<T>` cycles are collected at all, or documented as a leak.
