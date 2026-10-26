//! Conservative diagnostic alarms; passing does not establish uniformity.
use crate::{trial_rng, Moments, Operation, Summary};
use rand::RngCore;
use serde::Serialize;
use std::collections::HashSet;

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Ok,
    Warning,
    Inconclusive,
    NotRun,
}
#[derive(Debug, Serialize)]
pub struct ControlCheck {
    pub case: String,
    pub diagnostic: String,
    pub expected: f64,
    pub observed: f64,
    pub standard_errors_from_target: Option<f64>,
    pub status: CheckStatus,
}
#[derive(Debug, Serialize)]
pub struct ControlHealth {
    pub status: CheckStatus,
    pub method: &'static str,
    pub checks: Vec<ControlCheck>,
    pub rng_prefix_duplicates: usize,
    pub controls_found: usize,
}

fn mean_check(case: &str, name: &str, moments: &Moments, expected: f64) -> ControlCheck {
    let se = moments.mean_variance().map(f64::sqrt);
    let delta = (moments.mean - expected).abs();
    let z = se.filter(|&x| x > 0.0).map(|se| delta / se);
    let status = if moments.count < 1000 {
        CheckStatus::Inconclusive
    } else if !moments.mean.is_finite()
        || z.is_some_and(|z| z > 6.0)
        || (se == Some(0.0) && delta > 1e-12)
    {
        CheckStatus::Warning
    } else if se.is_none() {
        CheckStatus::Inconclusive
    } else {
        CheckStatus::Ok
    };
    ControlCheck {
        case: case.into(),
        diagnostic: name.into(),
        expected,
        observed: moments.mean,
        standard_errors_from_target: z,
        status,
    }
}

pub fn control_health(summaries: &[Summary], n: usize, seed: u64, trials: u64) -> ControlHealth {
    let controls: Vec<_> = summaries
        .iter()
        .filter(|s| {
            matches!(
                s.schedule.operations.as_slice(),
                [Operation::FisherYates] | [Operation::Insertion]
            )
        })
        .collect();
    let mut checks = Vec::new();
    let mut duplicates = 0;
    let targets = [
        ("original_neighbor_adjacency_fraction", 2.0 / n as f64),
        ("mean_original_neighbor_distance", (n + 1) as f64 / 3.0),
        ("increasing_runs", (n + 1) as f64 / 2.0),
        (
            "mean_absolute_displacement",
            (n as f64 * n as f64 - 1.0) / (3.0 * n as f64),
        ),
    ];
    for control in &controls {
        let mut seen = HashSet::new();
        for trial in 0..trials.min(64) {
            let mut rng = trial_rng(seed, control.schedule.stream_id, trial);
            let prefix: [u64; 4] = std::array::from_fn(|_| rng.next_u64());
            if !seen.insert(prefix) {
                duplicates += 1;
            }
        }
        for (name, target) in targets {
            if let Some(m) = control.diagnostics.get(name) {
                checks.push(mean_check(&control.schedule.name, name, m, target));
            }
        }
    }
    let status = if duplicates > 0 || checks.iter().any(|c| c.status == CheckStatus::Warning) {
        CheckStatus::Warning
    } else if controls.is_empty() {
        CheckStatus::NotRun
    } else if checks.len() != controls.len() * 4
        || checks.iter().any(|c| c.status == CheckStatus::Inconclusive)
    {
        CheckStatus::Inconclusive
    } else {
        CheckStatus::Ok
    };
    ControlHealth { status, method: "four analytic means per single-operation uniform control; alarm beyond 6 estimated standard errors with >=1000 trials; duplicate RNG prefix check for first 64 trials. Heuristic normal-approximation guard, not a proof or calibrated familywise test.",
        checks, rng_prefix_duplicates: duplicates, controls_found: controls.len() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conservative_guard_flags_bias_but_not_every_95_percent_miss() {
        let mut m = Moments::default();
        for _ in 0..500 {
            m.add(-1.0);
            m.add(1.0);
        }
        assert_eq!(
            mean_check("control", "metric", &m, 0.0).status,
            CheckStatus::Ok
        );
        let se = m.mean_variance().unwrap().sqrt();
        assert_eq!(
            mean_check("control", "metric", &m, 2.5 * se).status,
            CheckStatus::Ok
        );
        assert_eq!(
            mean_check("control", "metric", &m, 7.0 * se).status,
            CheckStatus::Warning
        );
        let mut constant = Moments::default();
        for _ in 0..1000 {
            constant.add(1.0);
        }
        assert_eq!(
            mean_check("control", "metric", &constant, 0.0).status,
            CheckStatus::Warning
        );
        assert_eq!(
            mean_check("control", "metric", &Moments::default(), 0.0).status,
            CheckStatus::Inconclusive
        );
        assert_eq!(control_health(&[], 52, 1, 1000).status, CheckStatus::NotRun);
    }
}
