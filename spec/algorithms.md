# Algorithm specification, version 1

Decks list distinct card IDs from top to bottom. A trial starts with `[0,...,N-1]`.
Library operations accept N=0 or 1; the experiment CLI requires N>=2 and at
least two trials. Every operation preserves all IDs exactly once.

## Random draw conventions

`integer(a,b)` is uniform over inclusive endpoints, with unbiased range sampling.
Each new draw is independent in the mathematical model. Bernoulli(1/2) is a
fair coin. RNG protocol version 2 uses rand 0.8 and ChaCha8Rng 0.3.
Initialize a parent with `seed_from_u64(master_seed)` and select the schedule's
explicit u64 stream ID. Seek to `trial_index * 8` in 32-bit words, read 32 bytes,
and initialize a fresh child with `ChaCha8Rng::from_seed(child_seed)`. All shuffle
draws for that trial use the child. For every u64 trial index the parent reads
lie below 2^67 words, within ChaCha's 68-bit position space. Distinct trials read
non-overlapping child-seed blocks. This is pseudorandom key derivation, not a
proof of independence or absence of seed collisions. Adding/reordering schedules
does not change existing streams. Duplicate stream IDs are rejected. No paired
coupling is used.

Output schema 3 records `rng_protocol_version=2`. Earlier schemas used the
invalid offset `trial_index * 2^64`, which wrapped every 16 trials. Their trial
results and confidence intervals must not be used for scientific conclusions.
The corrected protocol intentionally changes all seeded numerical outputs.

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

Handling time is a configurable constant per riffle and overhand (defaults:
6 and 4 seconds, based on user-reported personal handling estimates). Controls have no estimated handling time.
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

### Approachable CLI defaults

The CLI now selects all 26 experiment-suite sequences by default, with N=52,
100,000 trials per sequence and seed 20261026. The library's original five-sequence
`default_schedules()` remains unchanged for callers. `--placement-comparison`
is retained as a hidden compatibility option. Custom files still replace the
built-in suite. Omitting `--output` atomically reserves a fresh directory beneath
`output/`, relative to the caller's working directory; explicit paths still
refuse overwrites. This does not change the schema or random process.


### Terminal summary grid

The final report includes every sequence with mean adjacency fraction, original
neighbor distance, increasing runs, displacement, and hypothetical handling time.
A uniform-target row uses the formulas above, expected runs (N+1)/2, and expected
absolute displacement (N²-1)/(3N). Markers compare unrounded pointwise mean CIs
to those targets: `+` entirely above, `-` entirely below, `=` includes the target,
`?` interval unavailable. Missing metrics show `n/a`. These are descriptive
comparisons, without multiple-comparison correction or an overall success score.


### Parallel execution and expanded baselines

The expanded default preserves the original eight sequences and stream IDs.
It appends riffles alone for counts 4,5,6,8,9,10 (stream ID 20+count) and
overhands alone for counts 1..4 (ID 40+count). The existing 7R completes the
4..10 riffle series. All comparisons still default to reference 7R.

The execution layer supports a shared case pool and one-case-at-a-time trial
parallelism; see the dispatch specification below. Both preserve trial order
within each case and RNG addresses across worker counts and modes. Raw decks
are not persisted. Partial final batches are allowed. Errors abort the run and
may leave partial output. The manifest records mode, workers, CPUs, and batch size.

### Additional overhands and interlaced cases

The default now includes 1..10 overhands (IDs 40+count), preserving all earlier
streams and appending counts 5..10 after the original 18 cases. It then appends
`5R+5O-interlaced` (ID 61) and `7R-7O-interlaced` (ID 62). Display labels are
`5rif+5ovr-interlaced` and `7rif-7ovr-interlaced`. The plus/hyphen in those labels
is naming punctuation, not an operation: both mean `[Riffle, Overhand]` repeated
5 or 7 times, respectively. Riffle always comes first. Comparisons remain against
7rif by default; use --reference to choose another case. Counts/timing differ
between these cases, so compare their metrics alongside their modeled effort.


### Shared pool dispatch

`--parallel cases` is the default. `--parallel trials` preserves the earlier
one-case-at-a-time approach. The CLI caps `--workers` at available logical CPU
parallelism, with a fallback of one CPU, and caps workers by total available
batch jobs (per case in trials mode). Case-mode workers are persistent threads.

A batch comprises at most 128 consecutive trials in a single case. Select a
case with undispatched trials minimizing its assigned batch count (queued or
executing, not yet reported complete). Zero assigned batches has first priority.
Break ties by scanning cyclically from the case after the last dispatch. Thus
starved cases are preferred before giving extra workers to other cases.
When few cases remain, their batches can occupy multiple workers.

Dispatch at most `workers` batches not yet consumed, including buffered results.
Workers send start and completed-batch events to one coordinator. The coordinator
buffers out-of-order batches separately by case, then aggregates each case's
trials strictly in increasing index order. Slots are released as those results
are consumed. A slow earlier batch may delay redispatch, a deliberate bound on
memory rather than retaining arbitrary completed permutations. Failure closes
both queues before joining workers, preventing blocked sends/receives on exit.

The UI handles all progress centrally. Viewport filtering prioritizes all active
cases that fit; hidden cases retain state. Summaries and comparisons remain in
configuration order, regardless of completion order. No cross-case pairing or
change to statistical confidence-interval assumptions is introduced.


Default handling estimates are personal reports, not population calibration or
measured durations of these simulations. Seven riffles estimate 42 seconds;
seven overhands estimate 28 seconds; five riffles plus two overhands estimate
38 seconds. The constant model assumes additive times without transitions,
fatigue, or error penalties. The manifest records the default basis alongside
actual configured values. The grid labels handling time `Human Time s`.


### Schema 4: evidence bundles

Schema 4 retains RNG protocol 2 and adds build provenance, command arguments,
a sealed run bundle. Compile-time source bytes and
Cargo.lock are embedded and written to source/ alongside compiler version and
Git metadata. Source-content hashes are authoritative; Git metadata alone is
insufficient for uncommitted builds. The bundle includes a plain report and
SHA-256 inventory. Verification rejects changed/missing/extra files and symlinks;
checksums do not establish authenticity. A run is incomplete until sealed.


Single-operation Fisher–Yates/insertion controls are checked on all four means
against their analytic expectations. With >=1000 trials, a deviation exceeding
six estimated standard errors triggers a warning. Nonzero deviation with zero
sample variance also warns. First-64 trial RNG prefixes are checked for repeats.
Missing controls or insufficient diagnostics/trials are labeled explicitly.
The guard is a conservative heuristic under a normal approximation; it is not
an exact familywise test or proof of correct permutation sampling. Strict mode
preserves evidence then returns an unsuccessful exit unless controls are OK.
