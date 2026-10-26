//! Transparent reference shuffle models. See spec/algorithms.md.
use rand::SeedableRng;
use rand::{Rng, RngCore};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

pub mod artifacts;
mod control_checks;
pub use control_checks::{control_health, CheckStatus, ControlHealth};
mod analysis;
mod simulation;
pub use analysis::{compare_summaries, MeanDifference, ScheduleComparison, Summary};
pub use simulation::{
    effective_workers, simulate_batches, simulate_cases, ParallelMode, SimulationEvent,
    TRIAL_BATCH_SIZE,
};

pub const SCHEMA_VERSION: u32 = 4;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Operation {
    Riffle,
    Overhand,
    FisherYates,
    Insertion,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Schedule {
    pub name: String,
    /// Stable stream ID, independent of the schedule's position in a list.
    pub stream_id: u64,
    pub operations: Vec<Operation>,
}

pub fn default_schedules() -> Vec<Schedule> {
    use Operation::*;
    vec![
        Schedule {
            name: "uniform-fisher-yates".into(),
            stream_id: 1,
            operations: vec![FisherYates],
        },
        Schedule {
            name: "uniform-insertion".into(),
            stream_id: 2,
            operations: vec![Insertion],
        },
        Schedule {
            name: "7R".into(),
            stream_id: 3,
            operations: vec![Riffle; 7],
        },
        Schedule {
            name: "5R+2O-after".into(),
            stream_id: 4,
            operations: vec![Riffle, Riffle, Riffle, Riffle, Riffle, Overhand, Overhand],
        },
        Schedule {
            name: "7R+2O-after".into(),
            stream_id: 5,
            operations: vec![
                Riffle, Riffle, Riffle, Riffle, Riffle, Riffle, Riffle, Overhand, Overhand,
            ],
        },
    ]
}

/// Primary schedules plus three fixed placements of the same five riffles and
/// two overhands. Existing IDs and operations remain unchanged for replay.
pub fn placement_schedules() -> Vec<Schedule> {
    use Operation::{Overhand as O, Riffle as R};
    let mut schedules = default_schedules();
    for (name, stream_id, operations) in [
        ("5R+2O-before", 6, vec![O, O, R, R, R, R, R]),
        ("5R+2O-center", 7, vec![R, R, O, O, R, R, R]),
        ("5R+2O-spaced", 8, vec![R, R, O, R, O, R, R]),
    ] {
        schedules.push(Schedule {
            name: name.into(),
            stream_id,
            operations,
        });
    }
    schedules
}

/// Expanded experiment: preserve the placement suite, then include riffle-only
/// and overhand-only baselines. Existing stream IDs are never renumbered.
pub fn experiment_schedules() -> Vec<Schedule> {
    let mut schedules = placement_schedules();
    for count in 4..=10 {
        if count == 7 {
            continue;
        }
        schedules.push(Schedule {
            name: format!("{count}R"),
            stream_id: 20 + count as u64,
            operations: vec![Operation::Riffle; count],
        });
    }
    for count in 1..=10 {
        schedules.push(Schedule {
            name: format!("{count}O"),
            stream_id: 40 + count as u64,
            operations: vec![Operation::Overhand; count],
        });
    }
    for (count, name, stream_id) in [(5, "5R+5O-interlaced", 61), (7, "7R-7O-interlaced", 62)] {
        schedules.push(Schedule {
            name: name.into(),
            stream_id,
            operations: (0..count)
                .flat_map(|_| [Operation::Riffle, Operation::Overhand])
                .collect(),
        });
    }
    schedules
}

/// Separate ChaCha streams for every (schedule, trial); no hash-dependent seeding.
pub fn trial_rng(seed: u64, stream_id: u64, trial: u64) -> ChaCha8Rng {
    let mut parent = ChaCha8Rng::seed_from_u64(seed);
    parent.set_stream(stream_id);
    // A 32-byte child seed occupies eight words. Even trial=u64::MAX
    // stays below 2^67 words, inside ChaCha's 68-bit position space.
    parent.set_word_pos(u128::from(trial) * 8);
    let mut child_seed = [0u8; 32];
    parent.fill_bytes(&mut child_seed);
    ChaCha8Rng::from_seed(child_seed)
}

pub fn fisher_yates<R: Rng + ?Sized>(deck: &mut [usize], rng: &mut R) {
    for i in (1..deck.len()).rev() {
        let j = rng.gen_range(0..=i);
        deck.swap(i, j);
    }
}

pub fn insertion<R: Rng + ?Sized>(deck: &mut Vec<usize>, rng: &mut R) {
    let mut output = Vec::with_capacity(deck.len());
    for &card in deck.iter() {
        let slot = rng.gen_range(0..=output.len());
        output.insert(slot, card);
    }
    *deck = output;
}

pub fn riffle<R: Rng + ?Sized>(deck: &mut Vec<usize>, rng: &mut R) {
    let n = deck.len();
    let cut = (0..n).filter(|_| rng.gen_bool(0.5)).count();
    let (mut left, mut right) = (0, cut);
    let mut output = Vec::with_capacity(n);
    while output.len() < n {
        let remaining = n - output.len();
        if rng.gen_range(0..remaining) < cut - left {
            output.push(deck[left]);
            left += 1;
        } else {
            output.push(deck[right]);
            right += 1;
        }
    }
    *deck = output;
}

/// Reverse contiguous packet order without reversing cards within packets.
pub fn transfer_packets(deck: &[usize], lengths: &[usize]) -> Result<Vec<usize>, String> {
    if lengths.contains(&0)
        || lengths.iter().try_fold(0usize, |a, &b| a.checked_add(b)) != Some(deck.len())
    {
        return Err("packet lengths must be positive and sum to deck size".into());
    }
    let mut packets = Vec::new();
    let mut offset = 0;
    for &length in lengths {
        packets.push(&deck[offset..offset + length]);
        offset += length;
    }
    Ok(packets.into_iter().rev().flatten().copied().collect())
}

pub fn overhand<R: Rng + ?Sized>(deck: &mut Vec<usize>, rng: &mut R) {
    let n = deck.len();
    if n < 2 {
        return;
    }
    let k = rng.gen_range(3.min(n)..=8.min(n));
    let mut boundaries: Vec<usize> = (1..n).collect();
    fisher_yates(&mut boundaries, rng);
    boundaries.truncate(k - 1);
    boundaries.sort_unstable();
    boundaries.push(n);
    let mut start = 0;
    let lengths: Vec<_> = boundaries
        .into_iter()
        .map(|end| {
            let length = end - start;
            start = end;
            length
        })
        .collect();
    *deck = transfer_packets(deck, &lengths).expect("generated valid partition");
}

pub fn apply<R: Rng + ?Sized>(deck: &mut Vec<usize>, operation: Operation, rng: &mut R) {
    match operation {
        Operation::Riffle => riffle(deck, rng),
        Operation::Overhand => overhand(deck, rng),
        Operation::FisherYates => fisher_yates(deck, rng),
        Operation::Insertion => insertion(deck, rng),
    }
}

#[derive(Debug, Serialize)]
pub struct Diagnostics {
    pub mean_absolute_displacement: f64,
    pub mean_original_neighbor_distance: f64,
    pub original_neighbor_adjacency_fraction: f64,
    pub increasing_runs: f64,
}

pub fn diagnostics(deck: &[usize]) -> Diagnostics {
    let n = deck.len();
    let mut positions = vec![0; n];
    for (position, &card) in deck.iter().enumerate() {
        positions[card] = position;
    }
    let distances: Vec<_> = positions.windows(2).map(|w| w[0].abs_diff(w[1])).collect();
    let pairs = distances.len().max(1) as f64;
    Diagnostics {
        mean_absolute_displacement: positions
            .iter()
            .enumerate()
            .map(|(c, &p)| c.abs_diff(p) as f64)
            .sum::<f64>()
            / n.max(1) as f64,
        mean_original_neighbor_distance: distances.iter().sum::<usize>() as f64 / pairs,
        original_neighbor_adjacency_fraction: distances.iter().filter(|&&d| d == 1).count() as f64
            / pairs,
        increasing_runs: if n == 0 {
            0.0
        } else {
            1.0 + deck.windows(2).filter(|w| w[0] > w[1]).count() as f64
        },
    }
}

#[derive(Default, Debug, Serialize)]
pub struct Moments {
    pub count: u64,
    pub mean: f64,
    m2: f64,
}
impl Moments {
    pub fn add(&mut self, value: f64) {
        self.count += 1;
        let delta = value - self.mean;
        self.mean += delta / self.count as f64;
        self.m2 += delta * (value - self.mean);
    }
    /// Estimated sampling variance of the mean across independent trials.
    pub fn mean_variance(&self) -> Option<f64> {
        if self.count < 2 {
            return None;
        }
        Some(self.m2 / (self.count - 1) as f64 / self.count as f64)
    }
    pub fn mean_ci95(&self) -> Option<[f64; 2]> {
        let half = 1.96 * self.mean_variance()?.sqrt();
        Some([self.mean - half, self.mean + half])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn placement_family_has_fixed_counts_unique_ids_and_stable_baselines() {
        let schedules = placement_schedules();
        let defaults = default_schedules();
        assert_eq!(
            serde_json::to_value(&schedules[..5]).unwrap(),
            serde_json::to_value(defaults).unwrap()
        );
        let ids: std::collections::HashSet<_> = schedules.iter().map(|s| s.stream_id).collect();
        assert_eq!(ids.len(), schedules.len());
        let hybrids: Vec<_> = schedules
            .iter()
            .filter(|s| s.name.starts_with("5R+2O"))
            .collect();
        assert_eq!(hybrids.len(), 4);
        for s in &hybrids {
            assert_eq!(
                s.operations
                    .iter()
                    .filter(|&&op| op == Operation::Riffle)
                    .count(),
                5
            );
            assert_eq!(
                s.operations
                    .iter()
                    .filter(|&&op| op == Operation::Overhand)
                    .count(),
                2
            );
        }
        for (i, a) in hybrids.iter().enumerate() {
            for b in hybrids.iter().skip(i + 1) {
                assert_ne!(a.operations, b.operations);
            }
        }
        assert_eq!(schedules[5].operations[..2], [Operation::Overhand; 2]);
        assert_eq!(schedules[6].operations[2..4], [Operation::Overhand; 2]);
        assert_eq!(
            schedules[7].operations,
            vec![
                Operation::Riffle,
                Operation::Riffle,
                Operation::Overhand,
                Operation::Riffle,
                Operation::Overhand,
                Operation::Riffle,
                Operation::Riffle
            ]
        );
    }

    #[test]
    fn expanded_experiment_preserves_existing_streams_and_covers_baselines() {
        let schedules = experiment_schedules();
        assert_eq!(schedules.len(), 26);
        assert_eq!(
            serde_json::to_value(&schedules[..8]).unwrap(),
            serde_json::to_value(placement_schedules()).unwrap()
        );
        let ids: std::collections::HashSet<_> = schedules.iter().map(|s| s.stream_id).collect();
        let names: std::collections::HashSet<_> = schedules.iter().map(|s| &s.name).collect();
        assert_eq!(ids.len(), 26);
        assert_eq!(names.len(), 26);
        for count in 4..=10 {
            let sequence = schedules
                .iter()
                .find(|s| s.name == format!("{count}R"))
                .unwrap();
            assert_eq!(sequence.operations, vec![Operation::Riffle; count]);
        }
        for count in 1..=10 {
            let sequence = schedules
                .iter()
                .find(|s| s.name == format!("{count}O"))
                .unwrap();
            assert_eq!(sequence.operations, vec![Operation::Overhand; count]);
        }
    }

    #[test]
    fn interlaced_sequences_alternate_riffle_then_overhand() {
        let sequences = experiment_schedules();
        for (name, count, id) in [("5R+5O-interlaced", 5, 61), ("7R-7O-interlaced", 7, 62)] {
            let sequence = sequences.iter().find(|s| s.name == name).unwrap();
            assert_eq!(sequence.stream_id, id);
            assert_eq!(sequence.operations.len(), count * 2);
            for pair in sequence.operations.as_chunks::<2>().0 {
                assert_eq!(*pair, [Operation::Riffle, Operation::Overhand]);
            }
        }
        for count in 1..=10 {
            let sequence = sequences
                .iter()
                .find(|s| s.name == format!("{count}O"))
                .unwrap();
            assert_eq!(sequence.stream_id, 40 + count as u64);
        }
    }

    #[test]
    fn packet_direction() {
        assert_eq!(
            transfer_packets(&[0, 1, 2, 3, 4, 5], &[2, 1, 3]).unwrap(),
            vec![3, 4, 5, 2, 0, 1]
        );
        assert!(transfer_packets(&[0, 1], &[0, 2]).is_err());
        assert!(transfer_packets(&[0, 1], &[1]).is_err());
        assert_eq!(transfer_packets(&[0, 1], &[2]).unwrap(), vec![0, 1]);
        assert_eq!(transfer_packets(&[], &[]).unwrap(), Vec::<usize>::new());
    }
    #[test]
    fn permutations_and_replay() {
        for n in [0, 1, 2, 5, 52] {
            for op in [
                Operation::Riffle,
                Operation::Overhand,
                Operation::FisherYates,
                Operation::Insertion,
            ] {
                for seed in 0..12 {
                    let original: Vec<_> = (0..n).collect();
                    let mut a = original.clone();
                    let mut b = original.clone();
                    apply(&mut a, op, &mut trial_rng(seed, 3, 2));
                    apply(&mut b, op, &mut trial_rng(seed, 3, 2));
                    assert_eq!(a, b);
                    a.sort_unstable();
                    assert_eq!(a, original);
                }
            }
        }
    }
    #[test]
    fn known_metrics_and_moments() {
        let d = diagnostics(&[0, 1, 2, 3]);
        assert_eq!(d.mean_absolute_displacement, 0.0);
        assert_eq!(d.mean_original_neighbor_distance, 1.0);
        assert_eq!(d.original_neighbor_adjacency_fraction, 1.0);
        assert_eq!(d.increasing_runs, 1.0);
        let mut m = Moments::default();
        assert_eq!(m.mean_ci95(), None);
        m.add(1.0);
        m.add(3.0);
        assert_eq!(m.mean, 2.0);
        let ci = m.mean_ci95().unwrap();
        assert!((ci[1] - 3.96).abs() < 1e-12);
    }
    #[test]
    fn small_deck_control_distributions() {
        // A fixed seed makes these regression checks deterministic. Broad bounds
        // detect gross probability errors; they do not prove uniformity.
        for op in [Operation::FisherYates, Operation::Insertion] {
            let mut counts = std::collections::BTreeMap::new();
            let mut rng = trial_rng(8217, 42, 0);
            for _ in 0..6000 {
                let mut deck = vec![0, 1, 2];
                apply(&mut deck, op, &mut rng);
                *counts.entry(deck).or_insert(0usize) += 1;
            }
            assert_eq!(counts.len(), 6);
            for count in counts.values() {
                assert!((800..=1200).contains(count), "{op:?}: {counts:?}");
            }
        }
    }

    #[test]
    fn two_card_gsr_probability() {
        // Empty/full cuts contribute 1/2 to identity, and the middle cut
        // contributes another 1/4. Identity probability is therefore 3/4.
        let mut rng = trial_rng(8217, 43, 0);
        let mut identity = 0;
        for _ in 0..4000 {
            let mut deck = vec![0, 1];
            riffle(&mut deck, &mut rng);
            identity += usize::from(deck == [0, 1]);
        }
        assert!((2800..=3200).contains(&identity), "{identity}");
    }

    #[test]
    fn independently_addressed_controls_match_small_deck_targets() {
        for op in [Operation::FisherYates, Operation::Insertion] {
            let mut stats: [Moments; 4] = std::array::from_fn(|_| Moments::default());
            for trial in 0..2048 {
                let mut deck: Vec<_> = (0..5).collect();
                apply(&mut deck, op, &mut trial_rng(20261026, 99, trial));
                let d = diagnostics(&deck);
                for (m, value) in stats.iter_mut().zip([
                    d.original_neighbor_adjacency_fraction,
                    d.mean_original_neighbor_distance,
                    d.increasing_runs,
                    d.mean_absolute_displacement,
                ]) {
                    m.add(value);
                }
            }
            // Broad deterministic regression bound; not a proof of uniformity.
            for (m, target) in stats.iter().zip([0.4, 2.0, 3.0, 1.6]) {
                assert!(
                    (m.mean - target).abs() < 6.0 * m.mean_variance().unwrap().sqrt(),
                    "{op:?}: {m:?} target {target}"
                );
            }
        }
    }

    #[test]
    fn trial_streams_do_not_repeat_after_sixteen_trials() {
        let mut seen = std::collections::HashSet::new();
        for trial in (0..1024).chain([u64::MAX - 16, u64::MAX]) {
            let mut rng = trial_rng(20261026, 1, trial);
            let words: Vec<_> = (0..8).map(|_| rng.next_u64()).collect();
            assert!(seen.insert(words), "repeated stream at trial {trial}");
        }
    }

    #[test]
    fn trial_streams_are_addressable() {
        use rand::RngCore;
        let mut a = trial_rng(9, 3, 4);
        let mut b = trial_rng(9, 3, 4);
        let expected: Vec<_> = (0..8).map(|_| a.next_u64()).collect();
        let mut other = trial_rng(9, 4, 4);
        let different: Vec<_> = (0..8).map(|_| other.next_u64()).collect();
        assert_ne!(expected, different);
        let actual: Vec<_> = (0..8).map(|_| b.next_u64()).collect();
        assert_eq!(expected, actual);
        let mut next = trial_rng(9, 3, 5);
        assert_ne!(
            expected,
            (0..8).map(|_| next.next_u64()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn exhaustive_uniform_pair_targets() {
        // Every ordered placement of two distinct cards is equally likely.
        for n in 2usize..9 {
            let mut counts = vec![0; n];
            for a in 0..n {
                for b in 0..n {
                    if a != b {
                        counts[a.abs_diff(b)] += 1;
                    }
                }
            }
            for (d, &count) in counts.iter().enumerate().skip(1) {
                assert_eq!(count, 2 * (n - d));
            }
        }
    }
}
