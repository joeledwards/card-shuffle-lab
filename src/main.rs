mod ui;
use card_shuffle_lab::*;
use clap::Parser;
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::PathBuf,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Parser)]
#[command(version, about)]
struct Args {
    #[arg(long, default_value_t = 52, value_parser = clap::value_parser!(u32).range(2..))]
    deck_size: u32,
    #[arg(long, default_value_t = 100000, value_parser = clap::value_parser!(u64).range(2..))]
    trials: u64,
    #[arg(long, default_value_t = 20261026)]
    seed: u64,
    /// Worker threads; omitted uses available CPU parallelism.
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..))]
    workers: Option<u32>,
    /// Cases share a worker pool (default); trials dedicates the pool to one case at a time.
    #[arg(long, value_enum, default_value_t = ParallelMode::Cases)]
    parallel: ParallelMode,
    /// Results directory; omitted creates a fresh directory under output/.
    #[arg(long)]
    output: Option<PathBuf>,
    /// JSON array of shuffle sequences; omitted runs all 26 built-in sequences.
    #[arg(long)]
    schedules: Option<PathBuf>,
    /// Compatibility flag: placement comparisons are now included by default.
    #[arg(long, hide = true, conflicts_with = "schedules")]
    placement_comparison: bool,
    /// Compare other shuffle sequences against this name (default: 7rif, or first custom sequence).
    #[arg(long)]
    reference: Option<String>,
    /// Estimated handling seconds per riffle (default based on user-reported timing).
    #[arg(long, default_value_t = 6.0)]
    riffle_seconds: f64,
    /// Estimated handling seconds per overhand (default based on user-reported timing).
    #[arg(long, default_value_t = 4.0)]
    overhand_seconds: f64,
    /// Verify a previously saved run bundle without simulating.
    #[arg(long)]
    verify_run: Option<PathBuf>,
    /// Exit with an error after saving if controls warn, are absent, or lack enough trials.
    #[arg(long)]
    strict_controls: bool,
}

/// Atomically reserve a new directory, including on simultaneous invocations.
fn create_output(explicit: Option<&std::path::Path>) -> std::io::Result<PathBuf> {
    if let Some(path) = explicit {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        fs::create_dir(path)?;
        return Ok(path.to_path_buf());
    }
    let root = PathBuf::from("output");
    fs::create_dir_all(&root)?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    for attempt in 0..1000 {
        let path = root.join(format!("run-{timestamp}-{}-{attempt}", std::process::id()));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "could not reserve a fresh run directory",
    ))
}

struct CaseStats {
    metrics: BTreeMap<String, Moments>,
    positions: Vec<Vec<u64>>,
    neighbor_distances: Vec<u64>,
}
impl CaseStats {
    fn new(n: usize) -> Self {
        Self {
            metrics: BTreeMap::new(),
            positions: vec![vec![0; n]; n],
            neighbor_distances: vec![0; n],
        }
    }
    fn add(&mut self, deck: Vec<usize>) {
        let d = diagnostics(&deck);
        for (name, value) in [
            ("mean_absolute_displacement", d.mean_absolute_displacement),
            (
                "mean_original_neighbor_distance",
                d.mean_original_neighbor_distance,
            ),
            (
                "original_neighbor_adjacency_fraction",
                d.original_neighbor_adjacency_fraction,
            ),
            ("increasing_runs", d.increasing_runs),
        ] {
            self.metrics.entry(name.into()).or_default().add(value);
        }
        let mut inverse = vec![0; deck.len()];
        for (p, &c) in deck.iter().enumerate() {
            self.positions[c][p] += 1;
            inverse[c] = p;
        }
        for pair in inverse.windows(2) {
            self.neighbor_distances[pair[0].abs_diff(pair[1])] += 1;
        }
    }
}

fn resolve_reference(schedules: &[Schedule], requested: &str) -> Result<String, String> {
    schedules
        .iter()
        .find(|s| s.name == requested)
        .or_else(|| {
            schedules
                .iter()
                .find(|s| ui::case_name(&s.name) == requested)
        })
        .map(|s| s.name.clone())
        .ok_or_else(|| format!("reference shuffle sequence {requested:?} is missing"))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if let Some(path) = &args.verify_run {
        let count = artifacts::verify(path)?;
        println!("Verified {count} artifact checksums in {}", path.display());
        return Ok(());
    }
    if !args.riffle_seconds.is_finite()
        || args.riffle_seconds < 0.0
        || !args.overhand_seconds.is_finite()
        || args.overhand_seconds < 0.0
    {
        return Err("handling times must be finite and nonnegative".into());
    }
    let schedules: Vec<Schedule> = match &args.schedules {
        Some(path) => serde_json::from_slice(&fs::read(path)?)?,
        None => experiment_schedules(),
    };
    if schedules.is_empty() {
        return Err("at least one shuffle sequence is required".into());
    }
    let mut ids = std::collections::HashSet::new();
    let mut names = std::collections::HashSet::new();
    for schedule in &schedules {
        if schedule.name.is_empty()
            || !ids.insert(schedule.stream_id)
            || !names.insert(&schedule.name)
        {
            return Err(
                "shuffle sequences require unique stream IDs and unique nonempty names".into(),
            );
        }
    }
    let requested_reference = args.reference.clone().unwrap_or_else(|| {
        if schedules.iter().any(|s| s.name == "7R") {
            "7R".into()
        } else {
            schedules[0].name.clone()
        }
    });
    let reference = resolve_reference(&schedules, &requested_reference)?;
    let reference_label = ui::case_name(&reference);
    let output = create_output(args.output.as_deref())?;
    fs::write(
        output.join("run-status.json"),
        b"{\"status\":\"incomplete\"}\n",
    )?;
    eprintln!(
        "Preserving build source and compiler metadata in {}",
        output.display()
    );
    let provenance = artifacts::preserve_build(&output)?;
    let start = Instant::now();
    let n = args.deck_size as usize;
    let available_cpus = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1);
    let cpu_limit = args
        .workers
        .map(|w| w as usize)
        .unwrap_or(available_cpus)
        .min(available_cpus)
        .max(1);
    let job_count = args.trials.div_ceil(TRIAL_BATCH_SIZE) as u128
        * if args.parallel == ParallelMode::Cases {
            schedules.len() as u128
        } else {
            1
        };
    let workers = cpu_limit.min(usize::try_from(job_count).unwrap_or(usize::MAX).max(1));
    let mut progress = ui::Progress::stderr();
    let sequence_count = schedules.len();
    progress.set_cases(schedules.iter().map(|s| s.name.clone()), args.trials);
    progress.update(&[
        format!(
            "Card Shuffle Lab — {n} cards; {} trials per sequence",
            args.trials
        ),
        format!(
            "Seed {}; {sequence_count} sequences; reference {reference_label}",
            args.seed
        ),
        format!("Workers {workers}; available CPUs {available_cpus}; batch {TRIAL_BATCH_SIZE}"),
        "Plan: 1/3 simulate, 2/3 compare, 3/3 save and report".into(),
        "GSR reference; overhands hypothetical; handling times estimated".into(),
        format!("Output: {}", output.display()),
    ])?;
    let mut stats: Vec<_> = schedules.iter().map(|_| CaseStats::new(n)).collect();
    let mut completed = vec![0u64; sequence_count];
    let mut started = vec![false; sequence_count];
    let mut finished = vec![false; sequence_count];
    let mut last_feedback = Instant::now();
    let feedback_stride = args.trials.div_ceil(20);
    simulate_cases(
        n,
        args.trials,
        args.seed,
        &schedules,
        workers,
        args.parallel,
        |event| {
            match event {
                SimulationEvent::Started { case } => {
                    if !started[case] {
                        started[case] = true;
                        progress.case(case, 0, false);
                        progress.update(&[
                            format!(
                                "Run phase 1/3: simulate; elapsed {:.1}s",
                                start.elapsed().as_secs_f64()
                            ),
                            format!(
                                "Case {}/{sequence_count}: {} started",
                                case + 1,
                                ui::case_name(&schedules[case].name)
                            ),
                            format!(
                                "Parallel mode {:?}; shared worker budget {workers}",
                                args.parallel
                            ),
                            format!(
                                "Overall trials {}/{}",
                                completed.iter().map(|&n| n as u128).sum::<u128>(),
                                sequence_count as u128 * args.trials as u128
                            ),
                        ])?;
                    }
                }
                SimulationEvent::Batch { case, first, decks } => {
                    let batch_len = decks.len() as u64;
                    for deck in decks {
                        stats[case].add(deck);
                    }
                    completed[case] = first + batch_len;
                    progress.case(case, completed[case], false);
                    let mut context = vec![
                        format!(
                            "Run phase 1/3: simulate; elapsed {:.1}s",
                            start.elapsed().as_secs_f64()
                        ),
                        format!(
                            "Case {}/{sequence_count}: {}",
                            case + 1,
                            ui::case_name(&schedules[case].name)
                        ),
                        "Case step 1/3: simulate trials and aggregate metrics".into(),
                        format!(
                            "Trials {}/{}; {}/{} cases complete",
                            completed[case],
                            args.trials,
                            finished.iter().filter(|&&x| x).count(),
                            sequence_count
                        ),
                        format!(
                            "Overall trials {}/{}; workers {workers}",
                            completed.iter().map(|&n| n as u128).sum::<u128>(),
                            sequence_count as u128 * args.trials as u128
                        ),
                    ];
                    if completed[case] / feedback_stride > first / feedback_stride
                        || completed[case] == args.trials
                        || last_feedback.elapsed() >= Duration::from_secs(1)
                    {
                        progress.update(&context)?;
                        last_feedback = Instant::now();
                    }
                    if completed[case] == args.trials {
                        context[2] = "Case step 2/3: save card-position counts".into();
                        progress.update(&context)?;
                        let id = schedules[case].stream_id;
                        let mut file =
                            fs::File::create(output.join(format!("positions-{id}.csv")))?;
                        writeln!(file, "card,position,count")?;
                        for (card, row) in stats[case].positions.iter().enumerate() {
                            for (position, count) in row.iter().enumerate() {
                                writeln!(file, "{card},{position},{count}")?;
                            }
                        }
                        drop(file);
                        context[2] = "Case step 3/3: save original-neighbor distance counts".into();
                        progress.update(&context)?;
                        let mut file =
                            fs::File::create(output.join(format!("neighbor-distances-{id}.csv")))?;
                        writeln!(file, "distance,count,uniform_probability")?;
                        for (distance, count) in
                            stats[case].neighbor_distances.iter().enumerate().skip(1)
                        {
                            let probability =
                                2.0 * (n - distance) as f64 / (n as f64 * (n - 1) as f64);
                            writeln!(file, "{distance},{count},{probability}")?;
                        }
                        drop(file);
                        finished[case] = true;
                        progress.case(case, args.trials, true);
                        context[2] = "Case step 3/3 complete: both CSV files saved".into();
                        context[3] = format!(
                            "{}/{} cases complete",
                            finished.iter().filter(|&&x| x).count(),
                            sequence_count
                        );
                        progress.update(&context)?;
                    }
                }
            }
            Ok(())
        },
    )?;
    let summaries: Vec<_> = schedules
        .into_iter()
        .zip(stats)
        .map(|(schedule, stats)| {
            let time = schedule
                .operations
                .iter()
                .try_fold(0.0, |total, op| match op {
                    Operation::Riffle => Some(total + args.riffle_seconds),
                    Operation::Overhand => Some(total + args.overhand_seconds),
                    _ => None,
                });
            let cis = stats
                .metrics
                .iter()
                .map(|(name, m)| (name.clone(), m.mean_ci95()))
                .collect();
            Summary {
                schedule,
                diagnostics: stats.metrics,
                mean_ci95_normal_approximation: cis,
                estimated_handling_seconds_hypothesis: time,
            }
        })
        .collect();
    progress.update(&[
        "Run phase 2/3: compare independent trial means".into(),
        format!("Shuffle sequences {sequence_count}/{sequence_count} complete"),
        format!("Reference: {reference_label}; computing differences and intervals"),
    ])?;
    let comparisons = compare_summaries(&summaries, &reference)?;
    let health = control_health(&summaries, n, args.seed, args.trials);
    let control_message = match health.status {
        CheckStatus::Ok => {
            "CONTROL CHECKS OK: no conservative alarm; this does not prove uniformity."
        }
        CheckStatus::Warning => {
            "CONTROL WARNING: suspicious uniform controls; investigate before interpreting results."
        }
        CheckStatus::Inconclusive => {
            "CONTROL CHECKS INCONCLUSIVE: insufficient trials or diagnostics (need >=1000)."
        }
        CheckStatus::NotRun => {
            "CONTROL CHECKS NOT RUN: no single-operation uniform controls in this configuration."
        }
    };
    eprintln!("{control_message}");
    progress.update(&[
        "Run phase 3/3: save manifest and print results".into(),
        format!(
            "Comparisons {}/{} complete",
            comparisons.len(),
            comparisons.len()
        ),
        format!("Writing {}", output.join("manifest.json").display()),
    ])?;
    let manifest = serde_json::json!({
        "schema_version": SCHEMA_VERSION,
        "program_version": env!("CARGO_PKG_VERSION"),
        "arguments": std::env::args().collect::<Vec<_>>(),
        "working_directory": std::env::current_dir()?.display().to_string(),
        "seed": args.seed, "trials": args.trials, "deck_size": n,
        "execution": {"workers": workers, "available_cpus": available_cpus, "batch_size": TRIAL_BATCH_SIZE, "aggregation": "serial trial order per case", "parallel_mode": args.parallel},
        "rng_protocol_version": 2,
        "rng": "v2: ChaCha8 parent seed_from_u64(master); stream=schedule.stream_id; word_pos=trial*8; read 32-byte child seed; child ChaCha8Rng::from_seed",
        "replay_scope": "same source, Cargo.lock, and target architecture; not a cross-language protocol",
        "os": std::env::consts::OS, "architecture": std::env::consts::ARCH,
        "build_provenance": provenance,
        "control_health": health,
        "riffle_model": "standard GSR mathematical reference",
        "overhand_model": "uncalibrated uniform-cut packet hypothesis; 3..8 packets, clamped to N",
        "handling_time_model": {"status": "estimated constant-time model; not population-calibrated", "default_basis": "user reports about 6 seconds per riffle and 4 per overhand", "riffle_seconds": args.riffle_seconds, "overhand_seconds": args.overhand_seconds},
        "uniform_targets": {"neighbor_adjacency": 2.0 / n as f64, "neighbor_distance": (n+1) as f64 / 3.0},
        "compute_seconds": start.elapsed().as_secs_f64(),
        "summaries": summaries,
        "comparison_method": "candidate minus reference; independent trial means; pointwise approximate 95% normal intervals; no multiplicity correction",
        "comparison_reference": reference,
        "comparisons": comparisons,
    });
    fs::write(
        output.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    let mut report = Vec::new();
    writeln!(report, "{control_message}")?;
    ui::report(&mut report, &summaries, &comparisons, n, false)?;
    fs::write(output.join("report.txt"), &report)?;
    artifacts::seal(&output)?;
    progress.update(&[
        "Run phase 3/3 complete: all files saved".into(),
        format!(
            "Shuffle sequences {sequence_count}/{sequence_count}; elapsed {:.2}s",
            start.elapsed().as_secs_f64()
        ),
        format!("Results directory: {}", output.display()),
    ])?;
    let mut stdout = std::io::stdout().lock();
    writeln!(
        stdout,
        "\nSaved self-contained bundle (source, compiler, manifest, CSVs, report, checksums) in {}",
        output.display()
    )?;
    writeln!(stdout, "{control_message}")?;
    ui::report(&mut stdout, &summaries, &comparisons, n, ui::stdout_color())?;
    if args.strict_controls && health.status != CheckStatus::Ok {
        return Err(
            "strict control checks did not pass; results were preserved for investigation".into(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_arguments_select_a_small_reproducible_run() {
        let args = Args::try_parse_from(["lab"]).unwrap();
        assert_eq!(args.deck_size, 52);
        assert_eq!(args.trials, 100000);
        assert_eq!(args.riffle_seconds, 6.0);
        assert_eq!(args.overhand_seconds, 4.0);
        assert_eq!(args.seed, 20261026);
        assert!(args.output.is_none());
        assert!(args.schedules.is_none());
        assert_eq!(args.parallel, ParallelMode::Cases);
    }

    #[test]
    fn references_accept_friendly_and_original_names() {
        let sequences = experiment_schedules();
        assert_eq!(resolve_reference(&sequences, "7rif").unwrap(), "7R");
        assert_eq!(resolve_reference(&sequences, "7R").unwrap(), "7R");
        assert_eq!(
            resolve_reference(&sequences, "5rif+2ovr-after").unwrap(),
            "5R+2O-after"
        );
        assert!(resolve_reference(&sequences, "unknown").is_err());
    }

    #[test]
    fn workers_override_must_be_positive() {
        assert_eq!(
            Args::try_parse_from(["lab", "--parallel", "trials"])
                .unwrap()
                .parallel,
            ParallelMode::Trials
        );
        assert_eq!(
            Args::try_parse_from(["lab", "--workers", "2"])
                .unwrap()
                .workers,
            Some(2)
        );
        assert!(Args::try_parse_from(["lab", "--workers", "0"]).is_err());
    }

    #[test]
    fn placement_option_parsing() {
        let args = Args::try_parse_from([
            "lab",
            "--output",
            "result",
            "--placement-comparison",
            "--reference",
            "5R+2O-after",
        ])
        .unwrap();
        assert!(args.placement_comparison);
        assert_eq!(args.reference.as_deref(), Some("5R+2O-after"));
        assert!(Args::try_parse_from([
            "lab",
            "--output",
            "result",
            "--placement-comparison",
            "--schedules",
            "custom.json"
        ])
        .is_err());
    }
}
