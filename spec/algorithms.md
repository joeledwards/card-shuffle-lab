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
