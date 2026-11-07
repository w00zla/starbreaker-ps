# 0013. entity export-batch catches a panic per job

- Status: accepted
- Date: 2026-11-06
- Scope: CLI (`cli/src/entity_batch.rs`), packshot's export workers
- Recorded: 2026-11-07, from f1e34363 (#2) and e8788f99 (#3)

## Context
[0004](0004-export-batch-and-stdin.md) left panics uncaught: a panicking job killed the process and the client
restarted it, losing the jobs in flight; catching panics per job was listed as found, not built. A long-lived
`--stdin` process holds the unpacked DataCore and the shared asset state, so every restart costs the full start-up.

## Decision
- Each job of `entity export-batch` (file list and `--stdin`) runs under `std::panic::catch_unwind`.
- A panicking job is answered like a failed job (`ok: false`, error `panic: <message>`); the process keeps running and
  takes the next job.
- After a panic the output root's reuse list is cleared, so no later job reuses files the panicked job may have left
  half written.

## Alternatives
- Keep 0004's crash and client restart: loses the start-up time and the jobs in flight for a single bad record.

## Consequences
- 0004's "panics are not caught" and "a crash loses only the job in flight" no longer hold; real crashes (abort,
  out of memory) still end the process, and the client keeps its restart handling for those.
- The client's restart limits (chunks and memory per process) can be re-evaluated, since panics no longer force a restart.

## References
f1e34363 (#2), README line on panicking jobs (e8788f99, #3), [0004](0004-export-batch-and-stdin.md).
