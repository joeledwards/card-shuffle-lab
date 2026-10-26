# Algorithm specification, version 1

Decks list distinct card IDs from top to bottom. A trial starts with `[0,...,N-1]`.
Library operations accept N=0 or 1; the experiment CLI requires N>=2 and at
least two trials. Every operation preserves all IDs exactly once.

## Random draw conventions

`integer(a,b)` is uniform over inclusive endpoints, with unbiased range sampling.
Each new draw is independent in the mathematical model. Bernoulli(1/2) is a
fair coin. The implementation uses rand 0.8 and ChaCha8Rng 0.3. A trial initializes
`seed_from_u64(master_seed)`, selects the schedule's explicit u64 stream ID,
and seeks to word position `trial_index * 2^64` (32-bit words). A trial must not
consume 2^64 words. Streams are intended as independent pseudorandom streams,
not a mathematical proof of independence. Adding or reordering schedules does
not change existing streams. Duplicate stream IDs are rejected. No paired
coupling is used.

Replay requires the same source, dependency lockfile, and target architecture;
this is not a cross-language bit-for-bit protocol. Archive the generated
Cargo.lock and compiler version with published runs. Timing fields and output
paths are excluded from numerical replay comparisons.

## Uniform controls

Fisher–Yates: for i=N-1 down to 1, draw j=integer(0,i), then swap deck[i],deck[j].
There are N! equally probable choice sequences, each producing a different
permutation. For N<2 there are no draws.

Insertion: start an empty output. Visit input cards in fixed order. With k cards
in output, draw j=integer(0,k) and insert the next card in slot j. Replace deck
with output. The last inserted card's position uniquely identifies its last
choice; deleting it recursively recovers all choices. Thus each permutation has
probability product(1/(k+1))=1/N!. This is not remove-and-reinsert shuffling.

## Standard GSR riffle

Draw N fair coins and let C be the number of successes, so C~Binomial(N,1/2).
Set L=deck[0:C], R=deck[C:N], output empty. While cards remain, draw
j=integer(0,|L|+|R|-1). If j<|L|, move the top of L to output; otherwise move
the top of R. Replace deck with output. Empty packets are exhausted without
invalid indexing; cuts 0 and N are allowed. Relative order within each packet
is preserved. This is a mathematical reference, not a calibrated human model.

## Overhand hypothesis

For N<2 return unchanged without draws. Draw K uniformly from
[min(3,N),min(8,N)]. Fisher–Yates shuffle the list of gaps [1,...,N-1], retain
its first K-1 entries, sort them, and append N. Consecutive boundary differences
are packet lengths. This samples each positive K-part composition uniformly
conditional on K; packet lengths are dependent.

Remove contiguous packets from the top of the source, placing each whole
packet on top of the receiving stack. Preserve order inside each packet.
Equivalently, reverse the packet list and concatenate:
`[0,1 | 2 | 3,4,5] -> [3,4,5,2,0,1]`.
One packet is identity; singleton packets reverse the deck. This is an
uncalibrated hypothesis, not an empirical human packet-size distribution.

## Schedules and timing

Apply operations in listed chronological order. Schedule JSON records `name`,
unique `stream_id`, and `operations`, whose enum values are `Riffle`, `Overhand`,
`FisherYates`, `Insertion`. Defaults include two controls, 7R, 5R followed by 2O,
and 7R followed by 2O. Custom lists can express any placement or operation count.
Random placement generation and human error/restart profiles are deferred.

Handling time is a configurable constant per riffle and overhand (initial
hypotheses: 3 and 2 seconds). Controls have no estimated handling time.
Compute wall time is measured separately. These assumptions do not establish
an equal-time comparison or describe real human performance.

## Diagnostics and uncertainty

Invert the deck to position[card]. Per trial compute mean |position[c]-c|;
mean |position[c+1]-position[c]|; fraction of those distances equal to 1;
and 1 plus the number of descents in the final deck (zero runs for empty decks).
Across independent trials use Welford moments and mean +/- 1.96 * sample SD /
sqrt(trials). These are approximate pointwise 95% normal intervals, unreliable
for tiny samples, not simultaneous intervals or evidence of uniformity.

Aggregate card-by-position counts and original-neighbor distance counts are
also retained. Pooled neighbor pairs are correlated within a trial: do not
use their count as an independent sample size. For a uniform permutation,
P(distance=d)=2(N-d)/(N(N-1)), adjacency=2/N, E(distance)=(N+1)/3.
These diagnostics are not total-variation distance or a complete randomness test.

## Placement suite and comparisons (output schema 2)

Algorithm laws above remain version 1. Output schema 2 adds `comparisons`,
`comparison_reference`, and `comparison_method`; original summary and histogram
fields remain. `--placement-comparison` selects the five original schedules plus
these distinct 5R+2O placements (R=riffle, O=overhand):

| Name suffix | Chronological operations | Stream ID |
| --- | --- | --- |
| before | O O R R R R R | 6 |
| center | R R O O R R R | 7 |
| spaced | R R O R O R R | 8 |

The original after placement remains R R R R R O O with ID 4. The center group
follows floor(5/2) riffles. The spaced variant has riffle gaps 2,1,2 around the
overhands. These are fixed placements; seeded-random and general count-based
placement generation remain future work. The placement flag conflicts with
custom schedule files; custom files can express additional placements directly.

Select the reference with `--reference NAME`. Without it, use `7R` if present,
otherwise the first schedule. A missing reference is rejected before creating
output. Compare every other schedule against that reference; no self-comparison
is emitted. Unique names and stream IDs are required. All diagnostics must have
the same keys in each summary.

For diagnostic X and candidate A versus reference B, report delta=mean(A)-mean(B)
in X's original units. With sample variances s_A² and s_B² over n_A and n_B
independent trials, the interval is:

`delta +/- 1.96 * sqrt(s_A²/n_A + s_B²/n_B)`.

This is a normal approximation, not a Welch t interval. A missing sample gives
null difference; fewer than two trials in either group gives null interval.
The CLI already requires at least two trials per schedule. Shared trial indices
do not imply paired observations: the schedules use distinct pseudorandom
streams, modeled as independent. Do not use this formula for common-random-number
coupling. The tests include unequal sample sizes and check the sign convention.

Intervals are pointwise and unadjusted for multiple diagnostics/comparisons.
Comparisons sharing a reference are correlated; do not treat them as independent
evidence. Positive delta means a larger diagnostic, not greater randomness.
An interval excluding zero is not proof of a difference in total-variation
distance. Handling-time differences are candidate minus reference under the
constant-time hypothesis, with null for comparisons involving uniform controls.
All four 5R+2O placements have identical hypothetical handling time, so their
comparison holds operation counts and assumed time fixed; comparison against
7R holds total operation count fixed, not handling time.
