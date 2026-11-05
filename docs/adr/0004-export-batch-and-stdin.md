# 0004. entity export-batch, then a long-lived --stdin mode

- Status: accepted
- Date: 2026-11-01
- Recorded: 2026-11-05
- Scope: cli (`entity export-batch`)

## Context

One `entity export` per item cost about 1.9 s, of which about 1.45 s was loading P4k and DataCore and about 0.4 s the item
itself. Icon runs export thousands of items. Even a batch mode still paid about 1 s per start for the central directory
and the DataCore, so small chunks were uneconomic.

## Decision

`entity export-batch <jobs.json>` loads P4k and DataCore once and works a job list (`name`, `output`, optional `worn`):
about 0.2 s per item. Class names are exact, one JSON result line is written per job, a failed `worn` export falls back to
the default geometry, and `--icon-files` leaves out animation clips and diagnostics. Files an earlier job wrote into the
same output directory are reused.

`entity export-batch --stdin` is the long-lived variant: it prints `{"ready":true}` (on error `{"error":..,"ready":false}`
and exit 1), takes one job per line and answers with the batch mode's result line. `{"end_chunk":"<dir>"}` is answered with
`{"chunk_done":"<dir>"}` and only forgets the per-directory reuse list. EOF or `{"quit":true}` ends the process. Broken
lines are answered with `{"ok":false,"error":..}`, empty lines are skipped. Panics are not caught: the process dies and the
client restarts it. Output is byte-identical to batch mode (40 items, 1038 files); about 0.09 s per item against 0.13 s.
Clients detect the mode through `export-batch --help`.

## Alternatives

- Several parallel single exports: each reloads the DataCore.
- A cache file for the unpacked DataCore (`--dcb`): saves only unpacking, superseded by the persistent process.
- Catching panics per job: found, not built.

## Consequences

The client owns restarts (packshot restarts after 40 chunks or 2 GB and sets `MALLOC_ARENA_MAX=2` on Linux, which is
outside this repository). A crash loses only the job in flight.

## References

`c0cb40ea` (export-batch), `38e26bc3` (`--stdin`).
