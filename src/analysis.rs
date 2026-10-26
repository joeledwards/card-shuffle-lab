//! Comparisons use independent schedule streams, not paired trial differences.
use crate::{Moments, Schedule};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

#[derive(Serialize, Deserialize)]
pub struct Summary {
    pub schedule: Schedule,
    pub diagnostics: BTreeMap<String, Moments>,
    pub mean_ci95_normal_approximation: BTreeMap<String, Option<[f64; 2]>>,
    pub estimated_handling_seconds_hypothesis: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MeanDifference {
    pub candidate_trials: u64,
    pub reference_trials: u64,
    /// Raw units of the diagnostic; positive means candidate has a larger mean.
    pub candidate_minus_reference: Option<f64>,
    pub ci95_normal_approximation: Option<[f64; 2]>,
}

impl MeanDifference {
    pub fn independent(candidate: &Moments, reference: &Moments) -> Self {
        let difference = if candidate.count == 0 || reference.count == 0 {
            None
        } else {
            Some(candidate.mean - reference.mean)
        };
        let ci = candidate
            .mean_variance()
            .zip(reference.mean_variance())
            .zip(difference)
            .map(|((a, b), mean)| {
                let half = 1.96 * (a + b).sqrt();
                [mean - half, mean + half]
            });
        Self {
            candidate_trials: candidate.count,
            reference_trials: reference.count,
            candidate_minus_reference: difference,
            ci95_normal_approximation: ci,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ScheduleComparison {
    pub candidate: String,
    pub candidate_stream_id: u64,
    pub reference: String,
    pub reference_stream_id: u64,
    pub diagnostics: BTreeMap<String, MeanDifference>,
    pub estimated_handling_seconds_candidate_minus_reference_hypothesis: Option<f64>,
}

/// Pointwise approximate confidence intervals, without multiplicity correction.
/// Checks reject duplicate streams, because the independent-sample variance
/// formula must not silently be used on schedules sharing an RNG stream.
pub fn compare_summaries(
    summaries: &[Summary],
    reference: &str,
) -> Result<Vec<ScheduleComparison>, String> {
    let mut ids = HashSet::new();
    let mut names = HashSet::new();
    for s in summaries {
        if !ids.insert(s.schedule.stream_id) || !names.insert(s.schedule.name.as_str()) {
            return Err(
                "comparison requires unique schedule names and independent stream IDs".into(),
            );
        }
    }
    let baseline = summaries
        .iter()
        .find(|s| s.schedule.name == reference)
        .ok_or_else(|| format!("reference schedule {reference:?} is missing"))?;
    let mut comparisons = Vec::new();
    for candidate in summaries {
        if candidate.schedule.name == reference {
            continue;
        }
        if !candidate.diagnostics.keys().eq(baseline.diagnostics.keys()) {
            return Err("comparison requires matching diagnostic sets".into());
        }
        let diagnostics = candidate
            .diagnostics
            .iter()
            .map(|(name, moments)| {
                (
                    name.clone(),
                    MeanDifference::independent(moments, &baseline.diagnostics[name]),
                )
            })
            .collect();
        comparisons.push(ScheduleComparison {
            candidate: candidate.schedule.name.clone(),
            candidate_stream_id: candidate.schedule.stream_id,
            reference: reference.into(),
            reference_stream_id: baseline.schedule.stream_id,
            diagnostics,
            estimated_handling_seconds_candidate_minus_reference_hypothesis: candidate
                .estimated_handling_seconds_hypothesis
                .zip(baseline.estimated_handling_seconds_hypothesis)
                .map(|(a, b)| a - b),
        });
    }
    Ok(comparisons)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn moments(values: &[f64]) -> Moments {
        let mut m = Moments::default();
        for &v in values {
            m.add(v);
        }
        m
    }
    fn summary(name: &str, id: u64, values: &[f64]) -> Summary {
        Summary {
            schedule: Schedule {
                name: name.into(),
                stream_id: id,
                operations: vec![],
            },
            diagnostics: BTreeMap::from([("metric".into(), moments(values))]),
            mean_ci95_normal_approximation: BTreeMap::new(),
            estimated_handling_seconds_hypothesis: Some(id as f64),
        }
    }
    #[test]
    fn independent_difference_handles_unequal_sample_sizes_and_sign() {
        // Sample variances: 2 and 4. Variance of difference: 2/2 + 4/3.
        let a = moments(&[1.0, 3.0]);
        let b = moments(&[2.0, 4.0, 6.0]);
        let diff = MeanDifference::independent(&a, &b);
        assert_eq!(diff.candidate_minus_reference, Some(-2.0));
        assert_eq!((diff.candidate_trials, diff.reference_trials), (2, 3));
        let half = 1.96 * (1.0_f64 + 4.0 / 3.0).sqrt();
        let ci = diff.ci95_normal_approximation.unwrap();
        assert!((ci[0] - (-2.0 - half)).abs() < 1e-12);
        assert!((ci[1] - (-2.0 + half)).abs() < 1e-12);
        let reverse = MeanDifference::independent(&b, &a)
            .ci95_normal_approximation
            .unwrap();
        assert!((reverse[0] + ci[1]).abs() < 1e-12);
    }
    #[test]
    fn missing_samples_and_constant_data() {
        let empty = moments(&[]);
        let single = moments(&[2.0]);
        let constant = moments(&[3.0, 3.0]);
        assert!(MeanDifference::independent(&empty, &constant)
            .candidate_minus_reference
            .is_none());
        let single_diff = MeanDifference::independent(&single, &constant);
        assert_eq!(single_diff.candidate_minus_reference, Some(-1.0));
        assert!(single_diff.ci95_normal_approximation.is_none());
        assert_eq!(
            MeanDifference::independent(&constant, &constant).ci95_normal_approximation,
            Some([0.0, 0.0])
        );
    }
    #[test]
    fn comparisons_identify_reference_and_reject_invalid_inputs() {
        let mut summaries = vec![
            summary("baseline", 1, &[1.0, 3.0]),
            summary("candidate", 2, &[4.0, 6.0]),
        ];
        let comparisons = compare_summaries(&summaries, "baseline").unwrap();
        assert_eq!(comparisons.len(), 1);
        let c = &comparisons[0];
        assert_eq!(c.candidate, "candidate");
        assert_eq!(c.reference, "baseline");
        assert_eq!(c.diagnostics["metric"].candidate_minus_reference, Some(3.0));
        assert_eq!(
            c.estimated_handling_seconds_candidate_minus_reference_hypothesis,
            Some(1.0)
        );
        assert!(compare_summaries(&summaries, "absent").is_err());
        summaries[1].schedule.stream_id = 1;
        assert!(compare_summaries(&summaries, "baseline").is_err());
        summaries[1].schedule.stream_id = 2;
        summaries[1].diagnostics.clear();
        assert!(compare_summaries(&summaries, "baseline").is_err());
        summaries[1].schedule.name = "baseline".into();
        assert!(compare_summaries(&summaries, "baseline").is_err());
    }
}
