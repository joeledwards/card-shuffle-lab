# Validation follow-up — 2026-10-26

## Completed work

Runs now preserve a sealed bundle of configuration, results, report, compiler,
lockfile, and exact compiled source. The read-only verifier checks SHA-256
inventories. Automatic conservative control-health checks are reported and can
be required with `--strict-controls`. `scripts/setup` supports an independent
checkout; `scripts/validate` performs the independent-seed experiment explicitly.

## Independent-seed experiment actually run

All 26 built-in cases ran with 52 cards and 100,000 trials per case for seeds
20261026, 20261027, and 20261028: 7.8 million trials total. RNG protocol 2, schema
4, shared case pool, 6-second riffles, and 4-second overhands were used. Full
results are preserved under `output/independent-seeds-validated/`, including
per-run reports/logs, checksummed bundles, and `summary.txt`.

The archived source SHA-256 for all three runs is:

`5ceb66720a668bcf3b20a08246aad4150a35f02b3a78d577b71a00ee15c86e73`

These runs used a working-tree build; the archived source is the record, rather
than a claim that HEAD alone reproduces it. Later bootstrap, build-metadata, and
documentation fixes do not change the numerical algorithms used for these runs.

All bundle verifications passed. The maximum absolute standardized deviation
across the eight control-mean checks in each run was:

| Seed | Maximum estimated standard errors | Conservative check |
| --- | ---: | --- |
| 20261026 | 1.358 | ok |
| 20261027 | 2.331 | ok |
| 20261028 | 1.383 | ok |

The alarm threshold is six standard errors. A 2.33-standard-error control result
can miss a pointwise 95% interval without triggering the conservative alarm.
Passing these checks is not proof of uniform permutation sampling.

Ranges below are minima/maxima of three per-seed means, not confidence intervals:

| Case | Adjacency range | Neighbor-distance range |
| --- | --- | --- |
| 7rif | 0.038845–0.039102 | 17.606382–17.622976 |
| 5rif+2ovr-after | 0.044856–0.045001 | 17.225983–17.231806 |
| 5rif+2ovr-before | 0.044945–0.045153 | 17.047238–17.051157 |
| 5rif+2ovr-center | 0.047728–0.047791 | 16.763645–16.766509 |
| 5rif+2ovr-spaced | 0.048836–0.049049 | 16.617886–16.630789 |
| 10ovr | 0.412570–0.412904 | 11.036982–11.040252 |
| 5rif+5ovr-interlaced | 0.047988–0.048036 | 16.953156–16.963848 |
| 7rif-7ovr-interlaced | 0.039621–0.039801 | 17.574849–17.578735 |

Each of the four 5rif+2ovr variants was farther from the analytic uniform target
than 7rif on all four displayed means, in every seed. For the primary after
variant, candidate-minus-7rif adjacency differences and pointwise approximate
95% intervals were:

| Seed | Difference in adjacency fraction | Approximate 95% interval |
| --- | ---: | --- |
| 20261026 | 0.0058990 | [0.0056514, 0.0061466] |
| 20261027 | 0.0060771 | [0.0058304, 0.0063237] |
| 20261028 | 0.0058227 | [0.0055761, 0.0060694] |

The observed increase is about 0.58–0.61 percentage points. All three intervals
exclude zero, but they remain unadjusted pointwise intervals. This supports
stability of this model-specific diagnostic difference, not a total-variation
ranking or a claim about actual human shuffling. The earlier conclusions about
overhand structure and interlacing tradeoffs persisted across these seeds.

## Fresh-checkout verification actually run

A source-only checkout copy without `.git`, `target`, or shared tool caches was
created under `output/fresh-checkout-validated/`. Explicit setup downloaded its
own pinned Rust, components, locked dependencies, and verified local linker.
The first attempt exposed missing `xz`; setup was corrected to use an existing
Python standard-library unpacker when needed, then tested from another clean
copy. No Python packages, virtual environments, host-global tools, or Git
operations were used. Only x86_64 GNU/Linux was tested.

The fresh copy passed all 36 isolated unit tests. From outside the checkout,
the wrapper ran all 26 cases with 8 cards and 1,000 trials in both parallel modes,
using two workers and strict controls. Both bundles verified (78 files each).
All 52 CSVs and the complete summaries/comparisons/control results matched
exactly between modes. Manifest working directories confirmed the caller's
working directory was preserved. Repeated help calls after a build did not
recompile; absent Git paths are no longer watched as always-missing inputs.

A nonfatal Zig warning about Rust's linker optimization flag remains. Build and
execution succeeded. Interactive resize/color behavior was not manually tested.

## Preservation and remaining limits

A ZIP backup of the three full seed bundles, logs, and summary is saved at
`output/independent-seeds-validated.zip` with a companion SHA-256 file. Its hash:

`04e506d69df61d2ebb88ba5411ce510f9872f97e8e2abbd51e466fffc04389e8`

These generated artifacts are intentionally ignored by Git. Copy the ZIP and
checksum to durable storage for sharing or long-term preservation. The curated
report is versionable; a pasted historical table cannot recover its original
manifest. Copy archived `source/` out of a sealed bundle before rebuilding it,
otherwise the new build files will change the bundle's inventory.

This work validates reproducibility, packaging, and selected numerical targets.
It does not calibrate human packet/cut/release distributions, prove the absence
of other defects, or replace richer distributional diagnostics. More independent
seeds and broader model sensitivity remain useful; larger trial counts alone
cannot establish realism.
