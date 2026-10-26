# Initial validation — 2026-10-26

**RNG defect discovered after these initial checks:** the original trial offset
wrapped every 16 trials. All historical smoke statistics below and their
confidence intervals are invalid as experimental evidence, even though replay
and histogram accounting checks passed. The user's million-trial table was
affected too. Corrected runs use schema 3 / RNG protocol 2. No corrected full
experiment has been run by the agent.


Environment: x86_64 GNU/Linux; rustc 1.99.1 (58cf8f9c3, LLVM 23.1.1).
Rustup installer downloaded from static.rust-lang.org and checked against its
published SHA-256. Zig 0.17.0 supplies a local C linker; archive checked against
the official download index SHA-256. All tooling/caches live in `.tools/rust/`.
Dependencies are recorded in Cargo.lock. No Git state was changed.

## Checks actually run

- `cargo fmt --check`: passed.
- `cargo clippy --all-targets --locked -- -D warnings`: passed.
- `cargo test --lib --locked`: seven tests passed.
- Release CLI: five default schedules, N=52, 100 trials each, seed=20261026.
- Same CLI repeated with the same seed/configuration in a different directory:
  every CSV matched byte for byte; manifests matched after excluding compute time.
- All position matrix row/column totals were 100; each neighbor histogram
  contained 5,100 observations (51 original neighbor pairs per trial).

Commands used `scripts/cargo-local`. Results are ignored files under
`output/smoke-001` and `output/smoke-001-replay`. The first CLI attempt found that
an absent parent output directory caused failure; the CLI now creates parent
directories while still refusing to overwrite an existing run directory.
Release linking emitted a nonfatal Zig warning about Rust's deprecated linker
optimization setting; the binary built and executed successfully.

Small-deck probability tests use fixed seeds and broad regression bounds:
both uniform controls cover all six permutations of three cards with roughly
equal frequencies; a two-card GSR riffle leaves identity with probability 3/4.
These are finite sampling checks, not proofs of probability laws. An exhaustive
ordered-position test separately verifies the analytic pair-distance counts.

## Smoke observations

Means and pointwise approximate 95% confidence intervals over 100 trials:

| Schedule | Original-neighbor adjacency fraction | Mean original-neighbor distance | Increasing runs |
| --- | --- | --- | --- |
| Fisher–Yates | .0363 [.0306, .0420] | 17.522 [17.169, 17.874] | 26.29 [25.916, 26.664] |
| Uniform insertion | .0422 [.0379, .0464] | 17.511 [17.244, 17.777] | 26.88 [26.519, 27.241] |
| 7R | .0265 [.0216, .0313] | 17.820 [17.591, 18.049] | 26.90 [26.524, 27.276] |
| 5R+2O-after | .0631 [.0549, .0714] | 17.303 [16.990, 17.616] | 25.69 [25.176, 26.204] |
| 7R+2O-after | .0400 [.0345, .0455] | 17.763 [17.543, 17.983] | 24.98 [24.520, 25.440] |

Uniform targets: adjacency 2/52 ≈ .03846, neighbor distance 53/3 ≈ 17.6667,
expected increasing runs (52+1)/2 = 26.5. Both controls' neighbor diagnostic
intervals include their targets. Some intervals for other diagnostics/schedules
do not; these are unadjusted pointwise intervals across multiple diagnostics.
This smoke run does not establish agreement of whole permutation distributions
or rank the schedules by overall randomness. Too few neighbors remaining
adjacent is also a deviation from uniform, not automatically better mixing.

The hybrid has more surviving neighbor adjacency in this sample. Its estimated
19 seconds versus 21 for 7R comes solely from the hypothetical constants of
3 seconds/riffle and 2 seconds/overhand. No human calibration, equal-time study,
large experiment, or benchmark was performed. Effect-size intervals, sensitivity
studies, and additional independent seeds remain future analysis.

## Placement/comparison implementation validation

After adding output schema 2 and the optional placement suite:

- `cargo test --lib --bin card-shuffle-lab --locked --offline`: 12 tests passed.
- `cargo fmt --check` and Clippy with `--all-targets --locked --offline -- -D warnings`: passed.
- New tests cover fixed schedule counts/placement, preservation of baseline IDs
  and sequences, unequal-size independent mean differences, sign reversal,
  missing/constant samples, reference selection in comparisons, invalid summary
  sets, and CLI option parsing/conflicts.

No new simulation experiment or end-to-end execution was run for these changes.
The earlier smoke observations describe the previous schema 1 implementation.
The README placement command is proposed, not an experimental result.

## Progress and terminal report validation

- Isolated library and binary unit tests: 15 passed (11 library, 4 binary).
- Formatting and Clippy with warnings denied: passed, using locked offline builds.
- New unit checks cover dashboard cursor redraw sequences, plain redirected
  output, sanitizing control characters, and a report built from synthetic
  summaries with known means, intervals, targets, and model limitations.

No simulation or manual interactive terminal session was run for this UI change.
The README dashboard is an illustrative layout, not captured simulation output.
Progress checks happen between trials and file-writing steps; there is no
background heartbeat during a single long-running trial or write.

### No-argument defaults

All 16 isolated unit tests passed, including CLI parsing with no arguments;
formatting and Clippy passed. The new `scripts/lab` wrapper passed shell syntax
validation. No simulation was run to validate this change. Automatic output
reservation is implemented with atomic directory creation and collision retries.

### 1,000-trial default and final summary grid

All 17 isolated unit tests passed; formatting and Clippy with warnings denied
also passed. Default parsing now checks 1,000 trials per sequence. A synthetic
grid test checks above/below/target-containing interval markers, missing metrics,
and hypothetical handling seconds. No simulation was run for these changes.

### Parallel execution and additional baselines

- 21 isolated unit tests passed; formatting and warnings-denied Clippy passed.
- Parallel generation with 1, 2, and 8 requested workers matched explicit serial
  generation trial-for-trial, including a partial final batch (259 trials,
  five cards). Tests also verify consumer errors stop delivery and worker-count
  validation rejects zero.
- The expanded suite test verifies 18 unique names/streams, coverage of 4..10
  riffles and 1..4 overhands, and unchanged original eight sequences.
- Default parsing checks 10,000 trials. No larger experiment, end-to-end replay,
  or performance benchmark was run. Parallel speedup remains unmeasured.

### Persistent per-case bars and viewport layout

- 23 isolated unit tests passed; formatting and Clippy with warnings denied passed.
- Synthetic UI checks verify waiting/partial/completed bars, retained state across
  case transitions, side-by-side layouts, active-case visibility in restricted
  viewports, restoration after expansion, and line/row bounds down to 1x1.
- Report tests check that details, comparisons, and limitations precede the final
  grid. All cases remain in the report regardless of viewport filtering.
- No simulation or manual interactive session was run. Resize layouts are tested
  with injected dimensions; real terminal size detection uses stty with fallbacks.

### Clear shuffle labels

Terminal labels now use lowercase `rif` and `ovr`. All 25 isolated unit tests,
formatting, and Clippy passed. Tests cover built-in label conversion, unchanged
custom names, progress-bar labels, and reference lookup using both friendly and
legacy identifiers. No simulation was run for this display change.

### Ten overhands and alternating hybrids

26 isolated unit tests passed. Tests verify 26 unique cases/stream IDs, all
1..10 overhand counts and stable IDs, exact riffle-then-overhand alternation in
the five-pair and seven-pair cases, and the corresponding rif/ovr display labels.
Formatting and Clippy passed. No simulation was run for this expansion.

### Shared case pool and trial-only mode

30 isolated unit tests passed; formatting and warnings-denied Clippy passed.
Tests cover starvation priority, least-assigned-worker selection, rotating ties,
skipping fully dispatched cases, per-case order and identical generated decks
across both modes with 1/2/5 workers, partial batches, clean consumer-error
shutdown, CLI mode parsing, and prioritization of multiple active dashboard cases.
No full experiment, end-to-end run, manual terminal session, or speed benchmark
was run. Per-case deck identity and preserved aggregation order establish the
replay mechanism; whole-file replay across the new modes has not been exercised.

### Colored headings and matching key

All 13 binary unit tests passed; formatting and Clippy passed. The new test
checks matching header/key color codes, identical visible table alignment after
removing ANSI codes, and color policy for redirected output, NO_COLOR, and dumb
terminals. No simulation or manual interactive color inspection was run.

### RNG protocol correction

33 isolated unit tests passed; formatting and Clippy passed. New tests check
unique stream prefixes across 1,024 trials and near u64::MAX, including the old
16-trial collision, and verify four small-deck uniform targets with independently
addressed trials (fixed seed, broad six-standard-error regression bounds).
Replay across modes/workers remains covered. These are isolated tests, not a
corrected 52-card experiment. No corrected full experiment was run.
