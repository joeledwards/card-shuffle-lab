use card_shuffle_lab::*;
use clap::Parser;
use serde::Serialize;
use std::{collections::BTreeMap, fs, io::Write, path::PathBuf, time::Instant};

#[derive(Parser)]
#[command(version, about)]
struct Args {
    #[arg(long, default_value_t = 52, value_parser = clap::value_parser!(u32).range(2..))]
    deck_size: u32,
    #[arg(long, default_value_t = 100, value_parser = clap::value_parser!(u64).range(2..))]
    trials: u64,
    #[arg(long, default_value_t = 20261026)]
    seed: u64,
    /// Must not already exist; prevents overwriting another run.
    #[arg(long)]
    output: PathBuf,
    /// JSON array of Schedule objects; omitted selects controls and primary comparisons.
    #[arg(long)]
    schedules: Option<PathBuf>,
    /// Hypothetical constant handling seconds per riffle.
    #[arg(long, default_value_t = 3.0)]
    riffle_seconds: f64,
    /// Hypothetical constant handling seconds per overhand.
    #[arg(long, default_value_t = 2.0)]
    overhand_seconds: f64,
}

#[derive(Serialize)]
struct Summary {
    schedule: Schedule,
    diagnostics: BTreeMap<String, Moments>,
    mean_ci95_normal_approximation: BTreeMap<String, Option<[f64; 2]>>,
    estimated_handling_seconds_hypothesis: Option<f64>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if !args.riffle_seconds.is_finite() || args.riffle_seconds < 0.0
        || !args.overhand_seconds.is_finite() || args.overhand_seconds < 0.0 {
        return Err("handling times must be finite and nonnegative".into());
    }
    let schedules: Vec<Schedule> = match &args.schedules {
        Some(path) => serde_json::from_slice(&fs::read(path)?)?,
        None => default_schedules(),
    };
    if schedules.is_empty() { return Err("at least one schedule is required".into()); }
    let mut ids = std::collections::HashSet::new();
    let mut names = std::collections::HashSet::new();
    for schedule in &schedules {
        if schedule.name.is_empty() || !ids.insert(schedule.stream_id) || !names.insert(&schedule.name) {
            return Err("schedules require unique stream IDs and unique nonempty names".into());
        }
    }
    fs::create_dir(&args.output)?;
    let start = Instant::now();
    let n = args.deck_size as usize;
    let mut summaries = Vec::new();
    for schedule in schedules {
        let mut metrics: BTreeMap<String, Moments> = BTreeMap::new();
        let mut positions = vec![vec![0u64; n]; n];
        let mut neighbor_distances = vec![0u64; n];
        for trial in 0..args.trials {
            let mut deck: Vec<_> = (0..n).collect();
            let mut rng = trial_rng(args.seed, schedule.stream_id, trial);
            for &op in &schedule.operations { apply(&mut deck, op, &mut rng); }
            let d = diagnostics(&deck);
            for (name, value) in [
                ("mean_absolute_displacement", d.mean_absolute_displacement),
                ("mean_original_neighbor_distance", d.mean_original_neighbor_distance),
                ("original_neighbor_adjacency_fraction", d.original_neighbor_adjacency_fraction),
                ("increasing_runs", d.increasing_runs),
            ] { metrics.entry(name.into()).or_default().add(value); }
            let mut inverse = vec![0; n];
            for (p, &c) in deck.iter().enumerate() { positions[c][p] += 1; inverse[c] = p; }
            for pair in inverse.windows(2) { neighbor_distances[pair[0].abs_diff(pair[1])] += 1; }
        }
        let id = schedule.stream_id;
        let mut file = fs::File::create(args.output.join(format!("positions-{id}.csv")))?;
        writeln!(file, "card,position,count")?;
        for (card, row) in positions.iter().enumerate() {
            for (position, count) in row.iter().enumerate() { writeln!(file, "{card},{position},{count}")?; }
        }
        let mut file = fs::File::create(args.output.join(format!("neighbor-distances-{id}.csv")))?;
        writeln!(file, "distance,count,uniform_probability")?;
        for (distance, count) in neighbor_distances.iter().enumerate().skip(1) {
            let probability = 2.0 * (n-distance) as f64 / (n as f64 * (n-1) as f64);
            writeln!(file, "{distance},{count},{probability}")?;
        }
        let time = schedule.operations.iter().try_fold(0.0, |total, op| match op {
            Operation::Riffle => Some(total + args.riffle_seconds),
            Operation::Overhand => Some(total + args.overhand_seconds),
            _ => None,
        });
        let cis = metrics.iter().map(|(name, m)| (name.clone(), m.mean_ci95())).collect();
        summaries.push(Summary { schedule, diagnostics: metrics,
            mean_ci95_normal_approximation: cis, estimated_handling_seconds_hypothesis: time });
    }
    let manifest = serde_json::json!({
        "schema_version": SCHEMA_VERSION,
        "program_version": env!("CARGO_PKG_VERSION"),
        "seed": args.seed, "trials": args.trials, "deck_size": n,
        "rng": "rand_chacha 0.3 ChaCha8Rng; seed_from_u64; stream=schedule.stream_id; word_pos=trial*2^64",
        "replay_scope": "same source, Cargo.lock, and target architecture; not a cross-language protocol",
        "os": std::env::consts::OS, "architecture": std::env::consts::ARCH,
        "build_provenance": "Archive Cargo.lock, rustc -Vv, and source snapshot alongside results",
        "riffle_model": "standard GSR mathematical reference",
        "overhand_model": "uncalibrated uniform-cut packet hypothesis; 3..8 packets, clamped to N",
        "handling_time_model": {"status": "uncalibrated constant-time hypothesis", "riffle_seconds": args.riffle_seconds, "overhand_seconds": args.overhand_seconds},
        "uniform_targets": {"neighbor_adjacency": 2.0 / n as f64, "neighbor_distance": (n+1) as f64 / 3.0},
        "compute_seconds": start.elapsed().as_secs_f64(),
        "summaries": summaries,
    });
    fs::write(args.output.join("manifest.json"), serde_json::to_vec_pretty(&manifest)?)?;
    println!("Wrote {}", args.output.display());
    Ok(())
}
