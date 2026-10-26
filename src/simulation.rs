//! Parallel generation, deterministic serial aggregation. No raw decks on disk.
use crate::{apply, trial_rng, Schedule};
use std::{io, thread};

pub const TRIAL_BATCH_SIZE: u64 = 128;

pub fn effective_workers(trials: u64, requested: usize) -> usize {
    requested.max(1).min(
        usize::try_from(trials.div_ceil(TRIAL_BATCH_SIZE))
            .unwrap_or(usize::MAX)
            .max(1),
    )
}

/// Generate bounded waves of batches concurrently. Deliver decks in chronological
/// trial order so downstream floating-point sums are independent of worker count.
/// At most workers * TRIAL_BATCH_SIZE decks are resident at once.
pub fn simulate_batches(
    n: usize,
    trials: u64,
    seed: u64,
    schedule: &Schedule,
    workers: usize,
    mut consume: impl FnMut(u64, Vec<Vec<usize>>) -> io::Result<()>,
) -> io::Result<()> {
    let workers = effective_workers(trials, workers);
    let mut next = 0;
    while next < trials {
        thread::scope(|scope| -> io::Result<()> {
            let mut handles = Vec::new();
            for _ in 0..workers {
                if next == trials {
                    break;
                }
                let first = next;
                let end = next.saturating_add(TRIAL_BATCH_SIZE).min(trials);
                next = end;
                let generate = move || {
                    (first..end)
                        .map(|trial| {
                            let mut deck: Vec<_> = (0..n).collect();
                            let mut rng = trial_rng(seed, schedule.stream_id, trial);
                            for &op in &schedule.operations {
                                apply(&mut deck, op, &mut rng);
                            }
                            deck
                        })
                        .collect::<Vec<_>>()
                };
                handles.push((
                    first,
                    thread::Builder::new()
                        .name(format!("trials-{first}"))
                        .spawn_scoped(scope, generate)?,
                ));
            }
            for (first, handle) in handles {
                let decks = handle
                    .join()
                    .map_err(|_| io::Error::other("simulation worker panicked"))?;
                consume(first, decks)?;
            }
            Ok(())
        })?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ParallelMode {
    Cases,
    Trials,
}

pub enum SimulationEvent {
    Started {
        case: usize,
    },
    Batch {
        case: usize,
        first: u64,
        decks: Vec<Vec<usize>>,
    },
}

// Zero assigned workers wins first; otherwise least-assigned wins. Starting
// the search at cursor rotates ties. Exhausted cases cannot accept more work.
fn choose_case(next: &[u64], assigned: &[usize], trials: u64, cursor: usize) -> Option<usize> {
    (0..next.len())
        .map(|offset| (cursor + offset) % next.len())
        .filter(|&case| next[case] < trials)
        .min_by_key(|&case| assigned[case])
}

/// A shared CPU budget. Cases mode dispatches batches round-robin across cases;
/// trials mode dedicates the same budget to one case at a time.
pub fn simulate_cases(
    n: usize,
    trials: u64,
    seed: u64,
    schedules: &[Schedule],
    workers: usize,
    mode: ParallelMode,
    mut consume: impl FnMut(SimulationEvent) -> io::Result<()>,
) -> io::Result<()> {
    if mode == ParallelMode::Trials {
        for (case, schedule) in schedules.iter().enumerate() {
            consume(SimulationEvent::Started { case })?;
            simulate_batches(n, trials, seed, schedule, workers, |first, decks| {
                consume(SimulationEvent::Batch { case, first, decks })
            })?;
        }
        return Ok(());
    }
    use std::collections::BTreeMap;
    use std::sync::{mpsc, Arc, Mutex};
    let total_batches = trials.div_ceil(TRIAL_BATCH_SIZE) as u128 * schedules.len() as u128;
    let workers = workers
        .max(1)
        .min(usize::try_from(total_batches).unwrap_or(usize::MAX).max(1));
    if schedules.is_empty() || trials == 0 {
        return Ok(());
    }
    thread::scope(|scope| {
        let (jobs_tx, jobs_rx) = mpsc::channel::<(usize, u64, u64)>();
        let jobs_rx = Arc::new(Mutex::new(jobs_rx));
        let (events_tx, events_rx) = mpsc::sync_channel::<Result<SimulationEvent, String>>(workers);
        let mut handles = Vec::new();
        for index in 0..workers {
            let jobs = jobs_rx.clone();
            let events = events_tx.clone();
            handles.push(
                thread::Builder::new()
                    .name(format!("shuffle-{index}"))
                    .spawn_scoped(scope, move || loop {
                        let job = match jobs.lock() {
                            Ok(receiver) => receiver.recv(),
                            Err(_) => return,
                        };
                        let Ok((case, first, end)) = job else {
                            return;
                        };
                        if events.send(Ok(SimulationEvent::Started { case })).is_err() {
                            return;
                        }
                        let generated = std::panic::catch_unwind(|| {
                            (first..end)
                                .map(|trial| {
                                    let mut deck: Vec<_> = (0..n).collect();
                                    let mut rng = trial_rng(seed, schedules[case].stream_id, trial);
                                    for &op in &schedules[case].operations {
                                        apply(&mut deck, op, &mut rng);
                                    }
                                    deck
                                })
                                .collect()
                        });
                        let event = generated
                            .map(|decks| SimulationEvent::Batch { case, first, decks })
                            .map_err(|_| "simulation worker panicked".to_string());
                        let failed = event.is_err();
                        if events.send(event).is_err() || failed {
                            return;
                        }
                    })?,
            );
        }
        drop(events_tx);
        let result = (|| -> io::Result<()> {
            let mut next = vec![0u64; schedules.len()];
            let mut expected = vec![0u64; schedules.len()];
            let mut pending: Vec<BTreeMap<u64, Vec<Vec<usize>>>> =
                (0..schedules.len()).map(|_| BTreeMap::new()).collect();
            let mut cursor = 0;
            let mut assigned = vec![0usize; schedules.len()];
            let mut outstanding = 0;
            loop {
                // Outstanding includes buffered results, bounding memory even
                // when an earlier batch is slow. No nested thread pools.
                while outstanding < workers {
                    let candidate = choose_case(&next, &assigned, trials, cursor);
                    let Some(case) = candidate else {
                        break;
                    };
                    let first = next[case];
                    next[case] = first.saturating_add(TRIAL_BATCH_SIZE).min(trials);
                    jobs_tx
                        .send((case, first, next[case]))
                        .map_err(|_| io::Error::other("worker queue closed"))?;
                    cursor = (case + 1) % schedules.len();
                    outstanding += 1;
                    assigned[case] += 1;
                }
                if outstanding == 0 {
                    break;
                }
                match events_rx
                    .recv()
                    .map_err(|_| io::Error::other("workers stopped before finishing"))?
                    .map_err(io::Error::other)?
                {
                    SimulationEvent::Started { case } => {
                        consume(SimulationEvent::Started { case })?
                    }
                    SimulationEvent::Batch { case, first, decks } => {
                        assigned[case] -= 1;
                        pending[case].insert(first, decks);
                        while let Some(decks) = pending[case].remove(&expected[case]) {
                            let first = expected[case];
                            expected[case] += decks.len() as u64;
                            consume(SimulationEvent::Batch { case, first, decks })?;
                            outstanding -= 1;
                        }
                    }
                }
            }
            Ok(())
        })();
        // Disconnect queues before joining so failures cannot leave workers
        // blocked on input or on a full progress/result channel.
        drop(jobs_tx);
        drop(events_rx);
        for handle in handles {
            if handle.join().is_err() && result.is_ok() {
                return Err(io::Error::other("worker panicked"));
            }
        }
        result
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{apply, placement_schedules, trial_rng};
    #[test]
    fn allocation_prefers_starved_then_least_assigned_with_rotating_ties() {
        assert_eq!(choose_case(&[0, 0, 0], &[2, 0, 1], 10, 0), Some(1));
        assert_eq!(choose_case(&[0, 0, 0], &[3, 1, 2], 10, 0), Some(1));
        assert_eq!(choose_case(&[0, 0, 0], &[1, 1, 1], 10, 2), Some(2));
        assert_eq!(choose_case(&[10, 0, 0], &[0, 2, 1], 10, 0), Some(2));
        assert_eq!(choose_case(&[10, 10], &[0, 0], 10, 0), None);
    }
    #[test]
    fn shared_pool_and_trial_modes_deliver_identical_ordered_decks() {
        let schedules = placement_schedules()[..3].to_vec();
        let trials = 259;
        let mut expected = vec![Vec::new(); schedules.len()];
        for (case, schedule) in schedules.iter().enumerate() {
            simulate_batches(5, trials, 9, schedule, 1, |_, decks| {
                expected[case].extend(decks);
                Ok(())
            })
            .unwrap();
        }
        for mode in [ParallelMode::Cases, ParallelMode::Trials] {
            for workers in [1, 2, 5] {
                let mut actual = vec![Vec::new(); schedules.len()];
                let mut started = vec![false; schedules.len()];
                simulate_cases(5, trials, 9, &schedules, workers, mode, |event| {
                    match event {
                        SimulationEvent::Started { case } => started[case] = true,
                        SimulationEvent::Batch { case, first, decks } => {
                            assert!(started[case]);
                            assert_eq!(first as usize, actual[case].len());
                            actual[case].extend(decks);
                        }
                    }
                    Ok(())
                })
                .unwrap();
                assert_eq!(actual, expected);
            }
        }
    }
    #[test]
    fn shared_pool_disconnects_cleanly_on_consumer_failure() {
        let schedules = placement_schedules();
        let result = simulate_cases(3, 257, 9, &schedules, 3, ParallelMode::Cases, |_| {
            Err(io::Error::other("closed"))
        });
        assert_eq!(result.unwrap_err().to_string(), "closed");
    }

    #[test]
    fn parallel_results_match_serial_trials_in_order_including_partial_batch() {
        let schedule = placement_schedules().remove(3);
        let trials = TRIAL_BATCH_SIZE * 2 + 3;
        let mut expected = Vec::new();
        for trial in 0..trials {
            let mut deck: Vec<_> = (0..5).collect();
            let mut rng = trial_rng(42, schedule.stream_id, trial);
            for &op in &schedule.operations {
                apply(&mut deck, op, &mut rng);
            }
            expected.push(deck);
        }
        for workers in [1, 2, 8] {
            let mut actual = Vec::new();
            simulate_batches(5, trials, 42, &schedule, workers, |first, batch| {
                assert_eq!(first as usize, actual.len());
                actual.extend(batch);
                Ok(())
            })
            .unwrap();
            assert_eq!(actual, expected);
        }
        assert_eq!(effective_workers(1, 100), 1);
        assert_eq!(effective_workers(trials, 100), 3);
    }
    #[test]
    fn consumer_errors_stop_the_run() {
        let schedule = placement_schedules().remove(0);
        let mut calls = 0;
        let error = simulate_batches(2, 300, 42, &schedule, 2, |_, _| {
            calls += 1;
            Err(io::Error::other("output closed"))
        })
        .unwrap_err();
        assert_eq!(calls, 1);
        assert_eq!(error.to_string(), "output closed");
    }
}
