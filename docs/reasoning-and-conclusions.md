# Shuffle experiment: reasoning and current conclusions

## Question and scope

We want to understand whether five riffles plus two overhands can substitute for
seven riffles, how overhand placement affects mixing, and what changes when we
compare estimated handling effort. We also compare riffles and overhands alone
and alternating hybrids. These are simulations of specified processes, not
measurements of a person's actual shuffled decks.

The implementation is Rust for straightforward local tooling and efficient
custom simulation loops. The language-neutral [algorithm specification](../spec/algorithms.md)
and CSV/JSON outputs support independent implementations and collaboration.

## What is established and what is assumed

Fisher–Yates and fixed-input uniform insertion each generate a uniform
permutation under independent ideal random draws. They provide separate controls.
The riffle model is standard Gilbert–Shannon–Reeds (GSR): a binomial cut followed
by releases proportional to remaining packet sizes. GSR is a mathematical
reference, not a validated model of the user's riffle technique.

The overhand model draws 3–8 contiguous packets (clamped for small decks),
using uniformly chosen cut gaps conditional on packet count, then reverses
packet order while preserving card order inside each packet. This is a
hypothesis about packet transfer, not an empirically calibrated distribution.

Handling estimates use the user's reports of about 6 seconds per riffle and
4 seconds per overhand. Times add per operation. Transitions, fatigue, mistakes,
and restarts are not modeled. They describe one person's estimates, not a
population study. Program execution time is a separate quantity.

## Why several diagnostics

Large movement alone is insufficient: cutting a deck can move many cards while
preserving most of its structure. We therefore keep four separate trial means:

| Diagnostic | Meaning | Uniform expectation, N=52 |
| --- | --- | --- |
| Adj | Fraction of original neighboring pairs still adjacent, either orientation | 2/52 ≈ .038462 |
| Dist | Final distance between original neighbors | (52+1)/3 ≈ 17.666667 |
| Runs | One plus the number of descents in the final card-ID sequence | (52+1)/2 = 26.5 |
| Move | Mean absolute movement from original positions | (52²−1)/(3×52) ≈ 17.326923 |

The grid marks whether a pointwise approximate 95% mean interval is wholly above
(`+`), wholly below (`-`), or contains (`=`) the analytic target. The tests use
unrounded numbers; printed means are rounded. `=` does not prove equivalence,
and `+`/`-` do not measure practical importance. At high trial counts, very small
deviations can be statistically detectable. Multiple comparisons are unadjusted.
No diagnostic, combination of these diagnostics, or marker count is proven
total-variation distance over all permutations. There is no overall pass/fail
score or universally best sequence inferred from this table.

## RNG correction and evidence provenance

An initial implementation error sought to trial_index × 2^64 in a ChaCha word
position that has only 68 bits. Trial addresses repeated every 16 trials.
Those earlier results and confidence intervals are invalid as experimental
evidence, regardless of nominal trial count. Exact replay and permutation
invariants did not catch this defect; the strongly biased uniform controls in
the user's million-trial output exposed it.

The correction derives a fresh 32-byte child seed for each trial from a parent
schedule stream, seeking to trial_index × 8 words. All u64 trial indices fit
within the parent's word-position range. New manifests record schema 3 and RNG
protocol 2. Regression checks cover the old collision, high trial indices,
independently addressed uniform controls, and replay across parallel modes.

The corrected table below was supplied by the user after the fix. The preceding
run was explicitly reported as 1,000,000 trials per case; the corrected table is
interpreted as its rerun, but no matching manifest or full interval output was
provided for independent confirmation of trial count, seed, build, or runtime.
The agent did not execute this large experiment. Values and markers below are
transcribed from the supplied corrected table. Conclusions are descriptive;
we do not reconstruct effect-size intervals from rounded means.

For an independently auditable result, retain manifest.json, all CSVs, Cargo.lock,
compiler version, and the source revision. A proposed matching default-seed run is:

```sh
./scripts/lab --trials 1000000 --seed 20261026
```

This command is a reproduction recipe, not proof of the supplied run's seed.

## Corrected user-supplied results

| Sequence | Adj | Dist | Runs | Move | Human time, s |
| --- | ---: | ---: | ---: | ---: | ---: |
| Uniform target | .0385 | 17.667 | 26.50 | 17.33 | — |
| uniform-fisher-yates | .0384- | 17.667= | 26.50= | 17.33= | — |
| uniform-insertion | .0385= | 17.666= | 26.50= | 17.33= | — |
| 7rif | .0390+ | 17.615- | 26.30- | 17.26- | 42 |
| 5rif+2ovr-after | .0450+ | 17.226- | 25.87- | 17.18- | 38 |
| 7rif+2ovr-after | .0389+ | 17.637- | 26.34- | 17.29- | 50 |
| 5rif+2ovr-before | .0451+ | 17.045- | 25.87- | 17.18- | 38 |
| 5rif+2ovr-center | .0477+ | 16.762- | 25.87- | 17.20- | 38 |
| 5rif+2ovr-spaced | .0489+ | 16.628- | 25.89- | 17.21- | 38 |
| 4rif | .0673+ | 15.039- | 24.91- | 16.77- | 24 |
| 5rif | .0465+ | 16.903- | 25.70- | 17.05- | 30 |
| 6rif | .0405+ | 17.467- | 26.10- | 17.19- | 36 |
| 8rif | .0386+ | 17.655- | 26.40- | 17.29- | 48 |
| 9rif | .0385= | 17.668= | 26.45- | 17.31- | 54 |
| 10rif | .0385= | 17.668= | 26.47- | 17.32- | 60 |
| 1ovr | .9126+ | 2.449- | 5.50- | 24.24+ | 4 |
| 2ovr | .8338+ | 3.729- | 6.36- | 11.29- | 8 |
| 3ovr | .7624+ | 4.927- | 8.91- | 21.29+ | 12 |
| 4ovr | .6973+ | 6.039- | 9.96- | 14.29- | 16 |
| 5ovr | .6380+ | 7.066- | 11.80- | 19.48+ | 20 |
| 6ovr | .5841+ | 8.009- | 12.84- | 15.74- | 24 |
| 7ovr | .5352+ | 8.872- | 14.24- | 18.48+ | 28 |
| 8ovr | .4905+ | 9.660- | 15.18- | 16.48- | 32 |
| 9ovr | .4499+ | 10.378- | 16.29- | 17.94+ | 36 |
| 10ovr | .4128+ | 11.035- | 17.11- | 16.88- | 40 |
| 5rif+5ovr-interlaced | .0480+ | 16.961- | 26.07- | 17.37+ | 50 |
| 7rif-7ovr-interlaced | .0396+ | 17.577- | 26.42- | 17.33+ | 70 |

Interlaced means riffle then overhand, repeated five or seven times. The plus
or hyphen in those names is punctuation, not a different operation.

## Conclusions supported by these observations

**Seven riffles outperform the tested five-riffle/two-overhand substitutions on
closeness of all four displayed means to uniform targets.** Their estimated
handling times are 42 versus 38 seconds: a four-second saving, about 9.5%, for
the substitutions. After-placement adjacency is .0450 versus .0390 for 7rif,
a difference of about .006, or .6 percentage points. This does not establish
an ordering of full permutation-distribution distances.

**Overhands alone leave substantial original structure in this packet model.**
Ten overhands retain about 41.3% original-neighbor adjacency versus 3.85% under
uniform shuffling. Their estimated 40 seconds is close to seven riffles' 42,
yet adjacency, neighbor distance, and runs remain far from uniform targets.
The odd/even oscillation in movement also illustrates why movement alone can
mislead. This is not a claim about every possible human overhand technique.

**Placement changes the diagnostics, without a single best hybrid on every
metric.** Among the 5rif+2ovr variants, after-placement has neighbor distance
closest to uniform. Its adjacency mean is also slightly closer than before,
but the difference is too small to assess from rounding alone. Spaced placement
has movement closer to target than after-placement, while its adjacency and
neighbor distance are farther away. Full difference intervals are needed to
quantify uncertainty in these comparisons.

**Interlacing offers no clear general advantage for the added handling time.**
5rif+5ovr-interlaced takes an estimated 50 seconds and has worse adjacency and
neighbor distance than 7rif. The 70-second 7rif-7ovr case is closer than 7rif on
runs and movement but farther on adjacency and neighbor distance. Calling either
uniformly better would hide this tradeoff.

**Additional riffles progressively reduce the displayed departures.** The
4rif-through-10rif series approaches uniform targets, with adjacency and neighbor
distance near their targets by 9–10 riffles while runs still detects residual
structure. Seven riffles is not an exact-uniformity threshold. Agreement of
these few expectations does not establish uniformity of entire permutations.

**The corrected controls are reassuring, not a complete validation.** Both
controls closely match the analytic means. A single Fisher–Yates adjacency
interval missing its target is compatible with nominal 95% intervals and many
comparisons; a repeated pattern across independent seeds would warrant further
investigation. The controls should remain visible in future reports.

## What to do next

Preserve the corrected run artifacts and repeat selected comparisons over
independent master seeds. Review the already-produced candidate-minus-reference
intervals, rather than counting grid markers. Add separation-class diagnostics
and uncertainty for position bias. Test sensitivity to packet laws and estimated
handling times before making practical recommendations. If conclusions about
actual technique are desired, measure real cuts, release clumps, packet sizes,
errors, and handling-time variability, then validate human profiles against data.
More simulation trials reduce sampling error; they cannot repair a wrong model.


## Subsequent independent validation

[The validation follow-up](validation-followup.md) records three new fully
preserved runs with 100,000 trials per case, automatic control checks, and
fresh-checkout verification. Those runs confirm the main model-specific trends
across independent seeds; they do not retroactively supply provenance for the
user-pasted million-trial table above.
