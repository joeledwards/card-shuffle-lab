//! Transparent reference shuffle models. See spec/algorithms.md.
use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 1;

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

/// Separate ChaCha streams for every (schedule, trial); no hash-dependent seeding.
pub fn trial_rng(seed: u64, stream_id: u64, trial: u64) -> ChaCha8Rng {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    rng.set_stream(stream_id);
    // Each trial has 2^64 32-bit words available. Trials are addressed, not iterated.
    rng.set_word_pos((trial as u128) << 64);
    rng
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
    pub fn mean_ci95(&self) -> Option<[f64; 2]> {
        if self.count < 2 {
            return None;
        }
        let half = 1.96 * (self.m2 / (self.count - 1) as f64 / self.count as f64).sqrt();
        Some([self.mean - half, self.mean + half])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
