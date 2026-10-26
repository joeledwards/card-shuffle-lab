# Card shuffle lab

Rust prototype for comparing seven GSR riffles with five GSR riffles followed
by two overhands, plus independent Fisher–Yates and uniform insertion controls.
The language-neutral [specification](spec/algorithms.md) is the source of model
semantics. Rust is the initial implementation choice; portable specifications
and CSV/JSON outputs support future Julia or Python collaboration.

## RNG correction: rerun earlier results

Earlier output schemas 1 and 2 accidentally repeated trial RNG streams every
16 trials. Increasing the trial count did not supply independent samples, and
reported confidence intervals were invalid. Discard scientific conclusions from
those runs. The corrected RNG protocol derives a separate child seed per trial;
corrected manifests identify output schema 3 or later and `rng_protocol_version: 2`. Rebuild
and rerun with `./scripts/lab` (or your prior `--trials` override). Seeded results
will change. The historical validation table is retained but marked invalid.

## Reasoning and conclusions

[Reasoning and current conclusions](docs/reasoning-and-conclusions.md) records
model choices, diagnostic interpretation, the RNG correction, and the corrected
user-supplied results. It separates observed model behavior from claims about
human shuffling and lists remaining validation work. The
[validation follow-up](docs/validation-followup.md) records the completed
three-seed experiment and fresh-checkout checks.

## Run it

Once local tooling and dependencies are set up, from this repository run:

```sh
./scripts/lab
```

This runs all 26 built-in shuffle sequences with 52 cards, 100,000 trials each,
and seed 20261026. It shows live progress, prints metrics and comparisons against
7R, and automatically creates a fresh `output/run-...` directory. Repeating the
command reproduces numerical results without overwriting the previous run.
Output paths are relative to your current directory. The wrapper preserves it.

For a shorter run, change just the trial count:

```sh
./scripts/lab --trials 1000
```

`./scripts/lab --help` lists optional overrides. The wrapper uses a locked,
offline release build and never downloads dependencies or tooling. The first
build may take time; Cargo prints compilation progress. Plain `cargo run` also
needs no application arguments when your shell has the equivalent Rust setup.

### Parallel execution and coverage

The default includes two uniform controls, all four 5rif+2ovr placements,
7rif+2ovr-after, riffles alone from 4rif through 10rif, overhands alone from
1ovr through 10ovr, and two alternating cases: 5rif+5ovr-interlaced and
7rif-7ovr-interlaced. Each interlaced case starts with a riffle and alternates
riffle, overhand until both counts are reached (10 and 14 total operations).
That is 26 distinct sequences, 2,600,000 trials at the default count. Their
metrics and reference differences appear in the same summaries and final grid.
These comparisons have different operation counts and hypothetical handling
times; they are not automatically equal-effort comparisons.

Available logical CPU parallelism determines the shared worker budget. `--workers`
can lower that budget; requests above the available CPUs are capped. No case
creates a nested pool. Choose either approach:

```sh
./scripts/lab                         # Default: cases share the worker pool
./scripts/lab --parallel trials       # One case at a time, parallel trials
./scripts/lab --workers 2             # At most two simulation workers
./scripts/lab --workers 1             # Serial generation
```

In the default `--parallel cases` mode, each available dispatch slot goes first
to a case with no assigned worker; otherwise it goes to the case with the fewest
assigned workers. Ties rotate. Finished or fully dispatched cases are skipped.
A worker receives a 128-trial batch, then becomes available for another case or
another batch in the same case. Spare workers therefore help the remaining
cases as others finish. Reassignment happens at batch boundaries, not mid-trial.

Metrics aggregate in original trial order within each case, giving identical
numerical results across modes and worker counts. Timings and execution metadata
can differ. Memory is bounded by `workers * 128` temporarily generated decks,
plus a position matrix and histograms for every case. Outstanding batches include
results awaiting earlier trials: a slow earlier batch can temporarily hold up
redispatch, keeping buffering bounded. There is no throughput benchmark claim.

Progress updates are processed centrally; workers never write to the terminal.
The dashboard shows concurrent cases, while summaries and the final grid retain
the configured case order. No new dependency or automatic download is needed.

### Shuffle labels

Terminal labels use `rif` for riffles and `ovr` for overhands: `7rif`,
`2ovr`, or `5rif+2ovr-after`. `--reference` accepts these labels as well as
older identifiers such as `7R`. Existing JSON identifiers and seeds are retained
for compatibility; custom names are displayed as supplied.

## Setup from a fresh checkout

```sh
./scripts/setup
./scripts/lab
```

`setup` is the explicit network step: it installs Rust 1.99.1 with formatting and
lint tools, fetches dependencies from Cargo.lock, and installs a checksum-verified
local Zig linker if no system `cc` exists on x86_64 Linux. It never runs an
experiment. Prerequisites are a POSIX shell, curl, tar, and sha256sum (or shasum).
Unpacking the optional linker uses xz, with an existing Python 3 standard-library
unpacker as fallback; no Python packages or virtual environment are installed. Bootstrap supports Linux x86_64/aarch64 and macOS
x86_64/arm64; the no-cc linker fallback is currently x86_64 Linux only. Other
platforms need explicit local toolchain setup. Only x86_64 Linux has been tested.

Toolchains and caches live under `.tools/rust/`: in this managed workspace they
use the workspace directory, while an independent checkout uses its own directory.
`SHUFFLE_TOOLS_DIR=/absolute/path/to/.tools/rust` overrides that location. Keep
that override inside the active workspace. No host-global installation, shell
profile modification, or automatic download occurs in everyday wrappers.
The wrappers derive their paths from their own location and preserve your
working directory. Cargo build output remains in the checkout's `target/`.

Run isolated checks explicitly:

```sh
./scripts/cargo-local test --lib --bin card-shuffle-lab --locked --offline
./scripts/cargo-local fmt --check
./scripts/cargo-local clippy --all-targets --locked --offline -- -D warnings
```

The Cargo wrapper uses `rustup run 1.99.1` without automatic installation. If
tooling or dependencies are missing, rerun `scripts/setup` explicitly.

## Preserved results and automatic controls

Every new schema-4 run creates a self-contained bundle containing:

- `manifest.json`: configuration, RNG protocol 2, comparisons, control checks,
  and build provenance.
- CSV aggregates and a plain `report.txt` matching the terminal analysis.
- `source/`: the exact build-time source, lockfile, specs, docs, and executable
  scripts; `provenance.json`: compiler, source hashes, executable hash, and
  supplementary Git metadata.
- `run-status.json` and `checksums.json`: completion state and SHA-256 integrity
  checks for all saved files. No raw permutations are stored.

Verify before sharing or analyzing an archive:

```sh
./scripts/lab --verify-run output/run-YOUR-RUN
```

Verification rejects changed, missing, additional, or symlinked files. Checksums
are not signatures; they detect changes, not authenticity. Keep personal notes
outside a sealed bundle. Copy archived `source/` out before rebuilding; building
inside the bundle adds files and invalidates its inventory. Source hashes describe compiled inputs even if the
working tree has changed since compilation. An interrupted run remains incomplete
or lacks a valid checksum inventory. Earlier user-provided results cannot be
retroactively recovered from a pasted table; retain the original run files if
available, or generate a new preserved run.

Uniform controls are checked automatically against four analytic expectations.
An alarm requires a departure beyond six estimated standard errors with at least
1,000 trials, or a repeated RNG prefix among the first 64 addressed trials.
Smaller runs are labeled inconclusive; custom suites without single-operation
uniform controls are labeled not checked. One missed 95% interval is not an
alarm. This conservative normal-approximation guard is not a proof, an exact
familywise test, or a replacement for tests of richer distributions.
`--strict-controls` saves results then exits unsuccessfully unless checks are OK.

## Repeat over independent seeds

```sh
./scripts/validate
```

This explicitly runs all 26 cases with 100,000 trials each for seeds 20261026,
20261027, and 20261028: 7.8 million total trials. It saves three complete bundles,
per-run terminal/progress logs, and a cross-seed summary. Each stage reports its
seed and log path. `SHUFFLE_VALIDATION_TRIALS=1000 ./scripts/validate` is a shorter
check. Passing an output directory as the script's argument chooses its location;
it must not already exist.

The read-only summary command can also analyze existing compatible bundles:

```sh
./scripts/lab --summarize-runs output/RUN-A output/RUN-B output/RUN-C
```

It verifies checksums, distinct seeds, OK controls, source identity, trial counts,
models, and shuffle sequence definitions before reporting ranges of per-seed
means and reference-difference interval signs. Ranges are not confidence
intervals. This tests stability over random seeds, not realism of human models.

## Experiments (commands to run explicitly)

```sh
./scripts/cargo-local run --release --locked -- --deck-size 52 --trials 100 --seed 20261026 --output output/smoke-001
```

The output directory must not already exist. Results contain a versioned
`manifest.json` with exact schedules, seed, parameters, pointwise confidence
intervals and hypothetical handling times, plus aggregated position and neighbor
distance CSVs. No giant raw permutation file is needed. Numerical results replay
with the same source, lockfile, target, seed, and configuration; compute timings
vary. Archive the source snapshot, Cargo.lock, and `rustc -Vv` with shared results.
An interrupted run may leave partial files; use a new directory for the next run.

Use `--schedules schedules.json` for other placements, for example:

```json
[
  {"name":"2O-before-5R", "stream_id":10,
   "operations":["Overhand","Overhand","Riffle","Riffle","Riffle","Riffle","Riffle"]}
]
```

Default estimated handling times are **6 seconds per riffle** and **4 per
overhand**, based on your personal estimates: 7rif is 42 seconds, 7ovr is
28 seconds, and 5rif+2ovr is 38 seconds. The `Human Time s` column sums these times
for one shuffle sequence, not all simulation trials. These are personal estimates,
not population-calibrated human timings. Use `--riffle-seconds` and
`--overhand-seconds` to supply different estimates.
Compare all diagnostics and their uncertainty against controls; neither uniform
control is inherently more random. The manifest also reports differences from a reference schedule with independent-sample
confidence intervals; these differ from intervals on individual schedule means. No experimental results or performance claims
are asserted until experiments are explicitly run and reviewed.

## Progress and terminal results

The summary-grid headings and matching names in the key use distinct colors
when stdout is a terminal. Redirected reports remain plain text. Set `NO_COLOR=1`
to disable color; `TERM=dumb` also disables it. Colors identify columns, not
better or worse results.

Feedback is verbose by default. In an interactive ANSI terminal with `TERM` set
(and not `dumb`), stderr shows a dashboard that redraws in place. It uses ordinary
terminal output without raw keyboard mode, a hidden cursor, or an alternate
screen. The dashboard adapts to terminal size and rechecks dimensions during updates.
When stderr is redirected or terminal support is absent, it emits chronological
progress lines with no terminal escape codes.

Each case has a persistent progress bar: empty while waiting, filling as its
trials are aggregated, and retained full after both CSVs are saved. `W` means
waiting, `R` running (including saving), and `D` done. Case numbers identify
abbreviated names in compact layouts. Cases run concurrently by default;
`--parallel trials` instead advances one case at a time.

The default 26 cases use side-by-side columns at 80x24; taller terminals
can show one case per row. Shorter terminals use additional columns when possible. If that still cannot fit every case, active cases are prioritized,
with visible/active/completed counts. Hidden waiting
and finished cases retain their state; expanding the terminal restores them.
Extremely small terminals prioritize the active bar over context; some labels
must be clipped. The inline display reserves a terminal row and avoids wrapping.

Size is queried from the controlling terminal using `stty size`, with
`COLUMNS`/`LINES` and then 80x24 as fallbacks. Dimension checks happen at most
four times a second on progress updates, not via a background event loop.
ASCII labels in the live dashboard keep column widths predictable; full names
remain in the report and manifest.

Illustrative bars (not captured experimental output):

```text
01 uniform-fisher-yates [####################] 100% D
02 uniform-insertion    [##########          ]  50% R
03 7R                   [                    ]   0% W
```

The three run phases are simulation, comparison, and saving/reporting. Each
sequence has three steps: simulate/aggregate, save position counts, save neighbor
distances. Trial feedback occurs roughly every 5% or after a second of trial
work, whichever comes first, checked between batches. A single expensive batch
or file write can take longer without a refresh. The dashboard reports actual
elapsed time, not a guessed completion deadline.

Stdout receives a permanent report with diagnostic means, approximate pointwise
95% confidence intervals, theoretical uniform targets, handling-time hypotheses,
and candidate-minus-reference comparisons. A final summary grid puts all sequences
next to uniform targets, with interval-direction markers and hypothetical handling
times. `+`/`-` mean the entire interval is above/below the target; `=` means it
includes the target, not that uniformity is established. It explains the diagnostics and
model limitations, then lists the results directory. Keep both streams if you
want the progress history as well as the report:

```sh
# Proposed command, not an additional experiment already run:
./scripts/cargo-local run --release --locked --offline -- \
  --placement-comparison --trials 100 --seed 20261026 \
  --output output/terminal-001 > report.txt 2> progress.log
```

JSON field names and the `--schedules` custom-file option retain their existing
names for compatibility; user-facing descriptions call them shuffle sequences.
The no-argument command runs the default experiment; use `--trials` to adjust its size.

## Compare overhand placements

The default suite includes before, center, after, and spaced variants of 5R+2O.
It keeps existing case stream IDs unchanged within RNG protocol 2. Choose a
reference to compare against all other schedules:

```sh
# Proposed command; this placement experiment has not been run.
./scripts/cargo-local run --release --locked --offline -- \
  --placement-comparison --reference 5rif+2ovr-after \
  --deck-size 52 --trials 100 --seed 20261026 --output output/placement-001
```

Without `--reference`, comparisons use `7R` when present, otherwise the first
schedule. Custom `--schedules` files support the same reference option but cannot
be combined with `--placement-comparison`.

The current schema 4 retains `comparisons` to the manifest, including candidate/reference
names and stream IDs, per-diagnostic candidate-minus-reference mean differences,
approximate pointwise 95% confidence intervals, and hypothetical handling-time
differences. Positive differences mean larger diagnostics, not better mixing.
After all cases complete, the detailed summaries and differences print once,
followed by the final summary grid. The intervals assume independent schedule streams, use normal approximations,
and have no multiple-comparison correction. They do not establish uniformity.

## Scope and next steps

GSR, the uniform controls, and uniform-pair analytic targets have mathematical
justification. Overhand packet sizes and handling times are uncalibrated
hypotheses. No diagnostic here measures proven total-variation distance.

Initial implementation includes algorithm unit tests, permutation/replay checks,
deterministic packet examples, streaming trial moments, and aggregate CSVs.
Still to implement: human cut/clump/error profiles with restart costs; generalized/random
placement families; displacement and multiple-separation histograms; positional
uncertainty; multiple-comparison analysis; plotting and reports; richer time models;
and empirical control validation. Large simulations, benchmarks, and the full
experimental protocol require separate authorization. Unit tests alone do not
validate the human models or demonstrate statistical agreement of controls.
