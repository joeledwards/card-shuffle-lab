//! Read-only analysis of independently seeded, checksum-verified run bundles.
use crate::{artifacts, ScheduleComparison, Summary, SCHEMA_VERSION};
use std::fmt::Write as _;
use std::{collections::HashSet, fs, io, path::PathBuf};

pub fn summarize(paths: &[PathBuf]) -> Result<String, Box<dyn std::error::Error>> {
    if paths.len() < 2 {
        return Err("provide at least two independent run bundles".into());
    }
    let mut runs = Vec::new();
    let mut seeds = HashSet::new();
    for path in paths {
        artifacts::verify(path)?;
        let run: serde_json::Value =
            serde_json::from_slice(&fs::read(path.join("manifest.json"))?)?;
        if run["schema_version"] != SCHEMA_VERSION || run["rng_protocol_version"] != 2 {
            return Err(
                "unsupported schema or RNG protocol; legacy RNG results cannot be combined".into(),
            );
        }
        let seed = run["seed"].as_u64().ok_or("missing seed")?;
        if !seeds.insert(seed) {
            return Err("repeated master seeds are not independent reruns".into());
        }
        if run["control_health"]["status"] != "ok" {
            return Err("control health is not OK in every run".into());
        }
        if let Some(first) = runs.first() {
            let first: &serde_json::Value = first;
            for key in [
                "deck_size",
                "trials",
                "handling_time_model",
                "comparison_reference",
            ] {
                if first[key] != run[key] {
                    return Err(format!("incompatible runs: {key} differs").into());
                }
            }
            if first["build_provenance"]["source_sha256"]
                != run["build_provenance"]["source_sha256"]
            {
                return Err(
                    "source snapshots differ; analyze model/code changes separately".into(),
                );
            }
        }
        runs.push(run);
    }
    let summaries: Vec<Vec<Summary>> = runs
        .iter()
        .map(|r| serde_json::from_value(r["summaries"].clone()))
        .collect::<Result<_, _>>()?;
    let baseline_schedules =
        serde_json::to_value(summaries[0].iter().map(|s| &s.schedule).collect::<Vec<_>>())?;
    for run in summaries.iter().skip(1) {
        if serde_json::to_value(run.iter().map(|s| &s.schedule).collect::<Vec<_>>())?
            != baseline_schedules
        {
            return Err("shuffle sequence definitions or order differ".into());
        }
    }
    let comparisons: Vec<Vec<ScheduleComparison>> = runs
        .iter()
        .map(|r| serde_json::from_value(r["comparisons"].clone()))
        .collect::<Result<_, _>>()?;
    let mut text = String::new();
    writeln!(
        text,
        "Independent-seed validation: {} verified bundles",
        paths.len()
    )?;
    writeln!(
        text,
        "Trials per case per seed: {}; deck size: {}",
        runs[0]["trials"], runs[0]["deck_size"]
    )?;
    writeln!(
        text,
        "Seeds: {}",
        runs.iter()
            .map(|r| r["seed"].to_string())
            .collect::<Vec<_>>()
            .join(", ")
    )?;
    writeln!(
        text,
        "Source SHA-256: {}",
        runs[0]["build_provenance"]["source_sha256"]
    )?;
    writeln!(
        text,
        "All conservative control checks OK. Passing is not proof of uniformity."
    )?;
    writeln!(
        text,
        "\nRanges of per-run means (not confidence intervals):"
    )?;
    for (index, summary) in summaries[0].iter().enumerate() {
        writeln!(text, "{}", summary.schedule.name)?;
        for name in summary.diagnostics.keys() {
            let values: Vec<_> = summaries
                .iter()
                .map(|run| run[index].diagnostics.get(name).map(|m| m.mean))
                .collect::<Option<_>>()
                .ok_or("diagnostic sets differ")?;
            let lo = values.iter().copied().fold(f64::INFINITY, f64::min);
            let hi = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            writeln!(text, "  {name}: {lo:.7} .. {hi:.7}")?;
        }
    }
    writeln!(text, "\nReference difference interval signs, in seed order (+ above zero, - below, = contains zero, ? unavailable):")?;
    for (index, comparison) in comparisons[0].iter().enumerate() {
        writeln!(
            text,
            "{} minus {}",
            comparison.candidate, comparison.reference
        )?;
        for name in comparison.diagnostics.keys() {
            let mut signs = String::new();
            for run in &comparisons {
                let current = run.get(index).ok_or("comparison sets differ")?;
                if current.candidate != comparison.candidate
                    || current.reference != comparison.reference
                {
                    return Err("comparison sets differ".into());
                }
                let ci = current
                    .diagnostics
                    .get(name)
                    .ok_or("comparison diagnostics differ")?
                    .ci95_normal_approximation;
                signs.push(match ci {
                    Some([lo, _]) if lo > 0.0 => '+',
                    Some([_, hi]) if hi < 0.0 => '-',
                    Some(_) => '=',
                    None => '?',
                });
            }
            writeln!(text, "  {name}: {signs}")?;
        }
    }
    writeln!(text, "\nIntervals are unadjusted pointwise normal approximations. Signs describe raw metric differences, not randomness rankings.\nIndependent seeds test sampling variability, not the realism of human models.")?;
    Ok(text)
}

// Kept here to avoid silently treating a missing bundle as an empty analysis.
pub fn write_summary(
    paths: &[PathBuf],
    writer: &mut impl io::Write,
) -> Result<(), Box<dyn std::error::Error>> {
    writer.write_all(summarize(paths)?.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(path: &std::path::Path, seed: u64) {
        fs::create_dir(path).unwrap();
        let manifest = serde_json::json!({"schema_version":SCHEMA_VERSION, "rng_protocol_version":2,
            "seed":seed,"deck_size":52,"trials":1000,"handling_time_model":{"riffle_seconds":6},
            "comparison_reference":"control","control_health":{"status":"ok"},
            "build_provenance":{"source_sha256":"test-fixture"}, "summaries":[], "comparisons":[]});
        fs::write(
            path.join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        fs::write(path.join("provenance.json"), b"{}").unwrap();
        artifacts::seal(path).unwrap();
    }
    #[test]
    fn independent_bundle_analysis_checks_seeds_config_and_integrity() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!(
                "validation-test-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        fs::create_dir_all(&root).unwrap();
        let a = root.join("a");
        let b = root.join("b");
        fixture(&a, 10);
        fixture(&b, 11);
        assert!(summarize(&[a.clone(), b.clone()])
            .unwrap()
            .contains("2 verified bundles"));
        assert!(summarize(&[a.clone(), a.clone()])
            .unwrap_err()
            .to_string()
            .contains("repeated master seeds"));
        let mut run: serde_json::Value =
            serde_json::from_slice(&fs::read(b.join("manifest.json")).unwrap()).unwrap();
        run["deck_size"] = 8.into();
        fs::write(b.join("manifest.json"), serde_json::to_vec(&run).unwrap()).unwrap();
        assert!(summarize(&[a.clone(), b.clone()])
            .unwrap_err()
            .to_string()
            .contains("checksum mismatch"));
        artifacts::seal(&b).unwrap();
        assert!(summarize(&[a, b])
            .unwrap_err()
            .to_string()
            .contains("deck_size differs"));
        fs::remove_dir_all(root).unwrap();
    }
}
