# Initial validation — 2026-10-26

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
