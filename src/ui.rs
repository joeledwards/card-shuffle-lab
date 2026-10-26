use card_shuffle_lab::{ScheduleComparison, Summary};
use std::io::{self, IsTerminal, Write};
use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
struct Viewport {
    columns: usize,
    rows: usize,
}

fn terminal_size() -> Viewport {
    // stty queries the controlling terminal even if stdin is redirected. No
    // dependency downloads; environment/fixed defaults support other platforms.
    if let Ok(tty) = std::fs::File::open("/dev/tty") {
        if let Ok(output) = std::process::Command::new("stty")
            .arg("size")
            .stdin(tty)
            .stderr(std::process::Stdio::null())
            .output()
        {
            let text = String::from_utf8_lossy(&output.stdout);
            let values: Vec<usize> = text
                .split_whitespace()
                .filter_map(|x| x.parse().ok())
                .collect();
            if output.status.success() && values.len() == 2 && values.iter().all(|&n| n > 0) {
                return Viewport {
                    rows: values[0],
                    columns: values[1],
                };
            }
        }
    }
    let dimension = |name, default| {
        std::env::var(name)
            .ok()
            .and_then(|x| x.parse::<usize>().ok())
            .filter(|&n| n > 0)
            .unwrap_or(default)
    };
    Viewport {
        columns: dimension("COLUMNS", 80),
        rows: dimension("LINES", 24),
    }
}

struct CaseProgress {
    name: String,
    total: u64,
    completed: u64,
    started: bool,
    finished: bool,
}

/// A bounded inline dashboard. Waiting/completed cases stay in the model even
/// when the viewport can display only a subset. No raw mode or hidden cursor.
pub struct Progress<W: Write> {
    writer: W,
    interactive: bool,
    previous_lines: usize,
    cases: Vec<CaseProgress>,
    viewport: Viewport,
    live_dimensions: bool,
    last_resize_check: Instant,
}
impl Progress<io::Stderr> {
    pub fn stderr() -> Self {
        let writer = io::stderr();
        let interactive =
            writer.is_terminal() && std::env::var("TERM").is_ok_and(|term| term != "dumb");
        let mut progress = Self::new(writer, interactive);
        if interactive {
            progress.viewport = terminal_size();
            progress.live_dimensions = true;
        }
        progress
    }
}
impl<W: Write> Progress<W> {
    fn new(writer: W, interactive: bool) -> Self {
        Self {
            writer,
            interactive,
            previous_lines: 0,
            cases: Vec::new(),
            viewport: Viewport {
                columns: 80,
                rows: 24,
            },
            live_dimensions: false,
            last_resize_check: Instant::now(),
        }
    }
    pub fn set_cases(&mut self, names: impl IntoIterator<Item = String>, trials: u64) {
        self.cases = names
            .into_iter()
            .map(|name| CaseProgress {
                name: case_name(&name),
                total: trials,
                completed: 0,
                started: false,
                finished: false,
            })
            .collect();
    }
    pub fn case(&mut self, index: usize, completed: u64, finished: bool) {
        let case = &mut self.cases[index];
        case.completed = completed.min(case.total);
        case.started = true;
        case.finished = finished;
    }
    pub fn update(&mut self, lines: &[String]) -> io::Result<()> {
        if self.interactive {
            if self.live_dimensions
                && self.last_resize_check.elapsed() >= Duration::from_millis(250)
            {
                self.viewport = terminal_size();
                self.last_resize_check = Instant::now();
            }
            let rendered = self.layout(lines);
            if self.previous_lines > 0 {
                write!(
                    self.writer,
                    "\x1b[{}A\x1b[J",
                    self.previous_lines
                        .min(self.viewport.rows.saturating_sub(1))
                        .max(1)
                )?;
            }
            for line in &rendered {
                writeln!(self.writer, "{line}")?;
            }
            self.previous_lines = rendered.len();
        } else {
            writeln!(
                self.writer,
                "{}",
                lines
                    .iter()
                    .map(|s| terminal_text(s))
                    .collect::<Vec<_>>()
                    .join(" | ")
            )?;
        }
        self.writer.flush()
    }
    fn layout(&self, context: &[String]) -> Vec<String> {
        let width = self.viewport.columns.saturating_sub(1).max(1);
        let height = self.viewport.rows.saturating_sub(1).max(1);
        let clip = |s: &str| {
            s.chars()
                .filter(|c| !c.is_control())
                .map(|c| if c.is_ascii() { c } else { '?' })
                .take(width)
                .collect::<String>()
        };
        if self.cases.is_empty() {
            return context.iter().take(height).map(|s| clip(s)).collect();
        }
        let header = (height / 3)
            .min(5)
            .min(context.len())
            .min(height.saturating_sub(1));
        let mut lines: Vec<_> = context
            .iter()
            .take(header.saturating_sub(1))
            .map(|s| clip(s))
            .collect();
        if header > 0 {
            if let Some(last) = context.last() {
                lines.push(clip(&format!("{last} | W waiting R running D done")));
            }
        }
        let bar_rows = height - lines.len();
        let columns = self.cases.len().div_ceil(bar_rows).min((width / 26).max(1));
        let hidden = self.cases.len() > bar_rows * columns;
        let display_rows = if hidden && bar_rows > 1 {
            bar_rows - 1
        } else {
            bar_rows
        };
        let capacity = display_rows * columns;
        let active: Vec<_> = self
            .cases
            .iter()
            .enumerate()
            .filter_map(|(i, c)| (c.started && !c.finished).then_some(i))
            .collect();
        let mut selected: Vec<_> = if hidden {
            active
                .iter()
                .copied()
                .chain((0..self.cases.len()).filter(|i| !active.contains(i)))
                .take(capacity)
                .collect()
        } else {
            (0..self.cases.len()).collect()
        };
        selected.sort_unstable();
        let cell_width = (width.saturating_sub((columns - 1) * 2)) / columns;
        for row in selected.chunks(columns) {
            let cells: Vec<_> = row.iter().map(|&i| self.case_bar(i, cell_width)).collect();
            lines.push(clip(&cells.join("  ")));
        }
        if hidden && bar_rows > 1 {
            let done = self.cases.iter().filter(|c| c.finished).count();
            lines.push(clip(&format!(
                "Showing cases {}/{}; {} active; {done} done",
                selected.len(),
                self.cases.len(),
                active.len()
            )));
        }
        lines
    }
    fn case_bar(&self, index: usize, width: usize) -> String {
        let case = &self.cases[index];
        let percent = if case.finished {
            100
        } else if case.total > 0 {
            (case.completed as u128 * 100 / case.total as u128) as usize
        } else {
            0
        };
        let state = if case.finished {
            'D'
        } else if case.started {
            'R'
        } else {
            'W'
        };
        let label: String = case
            .name
            .chars()
            .filter(|c| !c.is_control())
            .map(|c| if c.is_ascii() { c } else { '?' })
            .take((width / 3).clamp(1, 22))
            .collect();
        let prefix = format!("{:02} {label} ", index + 1);
        let bar_width = width.saturating_sub(prefix.len() + 10).max(1);
        let filled = if case.finished {
            bar_width
        } else {
            bar_width * percent / 100
        };
        let bar = format!(
            "{prefix}[{}{}] {percent:>3}% {state}",
            "#".repeat(filled),
            " ".repeat(bar_width - filled)
        );
        bar.chars().take(width).collect()
    }
}

/// Friendly labels for built-in cases; machine identifiers remain stable.
pub fn case_name(name: &str) -> String {
    match name {
        "4R" | "5R" | "6R" | "7R" | "8R" | "9R" | "10R" | "1O" | "2O" | "3O" | "4O" | "5O"
        | "6O" | "7O" | "8O" | "9O" | "10O" | "5R+5O-interlaced" | "7R-7O-interlaced"
        | "5R+2O-after" | "5R+2O-before" | "5R+2O-center" | "5R+2O-spaced" | "7R+2O-after" => {
            name.replace('R', "rif").replace('O', "ovr")
        }
        _ => name.into(),
    }
}

// Sequence names originate in user JSON. Do not let control characters alter
// terminal state or inject fake lines. Limit width to avoid dashboard wrapping.
fn terminal_text(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).take(72).collect()
}

const GRID_HEADINGS: [(&str, u8); 6] = [
    ("Sequence", 97),
    ("Adj", 96),
    ("Dist", 94),
    ("Runs", 93),
    ("Move", 95),
    ("Human Time s", 92),
];

pub fn stdout_color() -> bool {
    color_enabled(
        io::stdout().is_terminal(),
        std::env::var_os("NO_COLOR").is_some(),
        std::env::var("TERM").ok().as_deref(),
    )
}
fn color_enabled(terminal: bool, no_color: bool, term: Option<&str>) -> bool {
    terminal && !no_color && term != Some("dumb")
}
fn heading_label(index: usize, color: bool) -> String {
    let (label, code) = GRID_HEADINGS[index];
    if color {
        format!("\x1b[1;{code}m{label}\x1b[0m")
    } else {
        label.into()
    }
}

pub fn report<W: Write>(
    writer: &mut W,
    summaries: &[Summary],
    comparisons: &[ScheduleComparison],
    n: usize,
    color: bool,
) -> io::Result<()> {
    writeln!(
        writer,
        "\nRESULTS — means with approximate pointwise 95% confidence intervals"
    )?;
    writeln!(writer, "Uniform targets: adjacency {:.5}; neighbor distance {:.5}; increasing runs {:.5}; displacement {:.5}",
        2.0/n as f64, (n+1) as f64/3.0, (n+1) as f64/2.0, (n as f64*n as f64-1.0)/(3.0*n as f64))?;
    writeln!(writer, "Adjacency counts original neighboring cards still next to each other.\nNeighbor distance measures their final separation. Runs count ascending sections.\nDisplacement measures how far cards moved from their initial positions.")?;
    for summary in summaries {
        writeln!(
            writer,
            "\n{}",
            terminal_text(&case_name(&summary.schedule.name))
        )?;
        for (name, moments) in &summary.diagnostics {
            let ci = interval(moments.mean_ci95());
            writeln!(writer, "  {name}: {:.5} {ci}", moments.mean)?;
        }
        match summary.estimated_handling_seconds_hypothesis {
            Some(time) => writeln!(writer, "  Estimated handling time: {time:.2}s")?,
            None => writeln!(writer, "  Handling time: not modeled for uniform controls")?,
        }
    }
    writeln!(
        writer,
        "\nDIFFERENCES — candidate minus reference, in each metric's original units"
    )?;
    for comparison in comparisons {
        writeln!(
            writer,
            "\n{} minus {}",
            terminal_text(&case_name(&comparison.candidate)),
            terminal_text(&case_name(&comparison.reference))
        )?;
        for (name, difference) in &comparison.diagnostics {
            let value = difference
                .candidate_minus_reference
                .map(|x| format!("{x:+.5}"))
                .unwrap_or_else(|| "unavailable".into());
            writeln!(
                writer,
                "  {name}: {value} {}",
                interval(difference.ci95_normal_approximation)
            )?;
        }
    }
    writeln!(writer, "\nLarger or smaller is not automatically more random; compare with uniform targets.\nIntervals assume independent trials and are not adjusted for multiple comparisons.\nOverhand and handling-time models are uncalibrated hypotheses.\nThese diagnostics do not measure total-variation distance.")?;
    summary_grid(writer, summaries, n, color)
}
/// End-of-report overview. Markers compare intervals to mathematical targets,
/// not schedules to an invented overall randomness score.
fn summary_grid<W: Write>(
    writer: &mut W,
    summaries: &[Summary],
    n: usize,
    color: bool,
) -> io::Result<()> {
    let metrics = [
        ("original_neighbor_adjacency_fraction", 2.0 / n as f64, 4),
        ("mean_original_neighbor_distance", (n + 1) as f64 / 3.0, 3),
        ("increasing_runs", (n + 1) as f64 / 2.0, 2),
        (
            "mean_absolute_displacement",
            (n as f64 * n as f64 - 1.0) / (3.0 * n as f64),
            2,
        ),
    ];
    let widths = [22, 8, 8, 7, 7, 12];
    let rule = format!(
        "+{}+",
        widths
            .iter()
            .map(|w| "-".repeat(w + 2))
            .collect::<Vec<_>>()
            .join("+")
    );
    let row = |writer: &mut W, cells: &[String], header: bool| -> io::Result<()> {
        write!(writer, "|")?;
        for (index, (cell, width)) in cells.iter().zip(widths).enumerate() {
            // Pad visible text before adding escapes so column widths stay fixed.
            if header && color {
                let code = GRID_HEADINGS[index].1;
                write!(writer, " \x1b[1;{code}m{cell:<width$}\x1b[0m |")?;
            } else {
                write!(writer, " {cell:<width$} |")?;
            }
        }
        writeln!(writer)
    };
    writeln!(
        writer,
        "\nSUMMARY GRID — means compared with uniform targets"
    )?;
    writeln!(writer, "{rule}")?;
    row(
        writer,
        &GRID_HEADINGS
            .iter()
            .map(|(label, _)| label.to_string())
            .collect::<Vec<_>>(),
        true,
    )?;
    writeln!(writer, "{rule}")?;
    let mut targets = vec!["Uniform target".into()];
    targets.extend(
        metrics
            .iter()
            .map(|(_, value, precision)| format!("{value:.precision$}")),
    );
    targets.push("—".into());
    row(writer, &targets, false)?;
    writeln!(writer, "{rule}")?;
    for summary in summaries {
        let name: String = terminal_text(&case_name(&summary.schedule.name))
            .chars()
            .take(22)
            .collect();
        let mut cells = vec![name];
        for (metric, target, precision) in metrics {
            let cell = summary
                .diagnostics
                .get(metric)
                .filter(|m| m.count > 0)
                .map(|m| {
                    let mark = match m.mean_ci95() {
                        Some([lo, _]) if lo > target => '+',
                        Some([_, hi]) if hi < target => '-',
                        Some(_) => '=',
                        None => '?',
                    };
                    format!("{:.precision$}{mark}", m.mean)
                })
                .unwrap_or_else(|| "n/a".into());
            cells.push(cell);
        }
        cells.push(
            summary
                .estimated_handling_seconds_hypothesis
                .map(|t| format!("{t:.1}"))
                .unwrap_or_else(|| "—".into()),
        );
        row(writer, &cells, false)?;
    }
    writeln!(writer, "{rule}")?;
    writeln!(
        writer,
        "{}: shuffle operations applied in chronological order.",
        heading_label(0, color)
    )?;
    writeln!(
        writer,
        "{}: original-neighbor adjacency fraction; {}: mean neighbor distance.",
        heading_label(1, color),
        heading_label(2, color)
    )?;
    writeln!(
        writer,
        "{}: ascending sections; {}: mean absolute displacement.",
        heading_label(3, color),
        heading_label(4, color)
    )?;
    writeln!(
        writer,
        "{}: estimated human handling seconds, not measured compute time.",
        heading_label(5, color)
    )?;
    writeln!(writer, "+ / -: the entire approximate 95% interval is above / below the target.\n=: the interval includes the target; this does not establish equivalence.\n?: too few trials for an interval; n/a: metric unavailable.\nRead across all diagnostics; there is no overall winner or pass/fail score.")
}

fn interval(ci: Option<[f64; 2]>) -> String {
    ci.map(|[lo, hi]| format!("[{lo:.5}, {hi:.5}]"))
        .unwrap_or_else(|| "[interval unavailable]".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grid_colors_match_the_key_without_changing_alignment() {
        let mut colored = Vec::new();
        let mut plain = Vec::new();
        summary_grid(&mut colored, &[], 52, true).unwrap();
        summary_grid(&mut plain, &[], 52, false).unwrap();
        let mut text = String::from_utf8(colored).unwrap();
        for (label, code) in GRID_HEADINGS {
            let escape = format!("\x1b[1;{code}m");
            assert_eq!(text.matches(&escape).count(), 2, "{label}");
            text = text.replace(&escape, "");
        }
        text = text.replace("\x1b[0m", "");
        assert_eq!(text, String::from_utf8(plain).unwrap());
        assert!(color_enabled(true, false, Some("xterm")));
        assert!(!color_enabled(false, false, Some("xterm")));
        assert!(!color_enabled(true, true, Some("xterm")));
        assert!(!color_enabled(true, false, Some("dumb")));
    }

    #[test]
    fn friendly_case_labels_preserve_custom_names() {
        assert_eq!(case_name("5R+2O-after"), "5rif+2ovr-after");
        assert_eq!(case_name("7R"), "7rif");
        assert_eq!(case_name("2O"), "2ovr");
        assert_eq!(case_name("4O"), "4ovr");
        assert_eq!(case_name("10O"), "10ovr");
        assert_eq!(case_name("5R+5O-interlaced"), "5rif+5ovr-interlaced");
        assert_eq!(case_name("7R-7O-interlaced"), "7rif-7ovr-interlaced");
        assert_eq!(case_name("Original Run"), "Original Run");
        let mut progress = cases();
        progress.set_cases(["2O".into()], 100);
        assert!(progress.case_bar(0, 50).contains("2ovr"));
    }

    #[test]
    fn redirected_progress_has_no_terminal_controls_and_flushes_each_update() {
        let mut progress = Progress::new(Vec::new(), false);
        progress
            .update(&["Run 1/3".into(), "Trials 5/10".into()])
            .unwrap();
        progress.update(&["Run 2/3".into()]).unwrap();
        assert_eq!(
            String::from_utf8(progress.writer).unwrap(),
            "Run 1/3 | Trials 5/10\nRun 2/3\n"
        );
    }
    #[test]
    fn report_contains_means_intervals_targets_and_model_limits() {
        use card_shuffle_lab::{compare_summaries, Moments, Schedule};
        use std::collections::BTreeMap;
        let summaries: Vec<_> = [("reference", 1, 1.0), ("candidate", 2, 3.0)]
            .into_iter()
            .map(|(name, stream_id, value)| {
                let mut m = Moments::default();
                m.add(value);
                m.add(value);
                Summary {
                    schedule: Schedule {
                        name: name.into(),
                        stream_id,
                        operations: vec![],
                    },
                    diagnostics: BTreeMap::from([("test_metric".into(), m)]),
                    mean_ci95_normal_approximation: BTreeMap::new(),
                    estimated_handling_seconds_hypothesis: None,
                }
            })
            .collect();
        let comparisons = compare_summaries(&summaries, "reference").unwrap();
        let mut output = Vec::new();
        report(&mut output, &summaries, &comparisons, 52, false).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("adjacency 0.03846"));
        assert!(text.contains("test_metric: 3.00000 [3.00000, 3.00000]"));
        assert!(text.contains("candidate minus reference"));
        assert!(text.contains("test_metric: +2.00000 [2.00000, 2.00000]"));
        assert!(text.contains("uncalibrated hypotheses"));
        assert!(text.contains("not adjusted for multiple comparisons"));
        assert!(text.find("DIFFERENCES").unwrap() < text.find("SUMMARY GRID").unwrap());
        assert!(text.find("uncalibrated hypotheses").unwrap() < text.find("SUMMARY GRID").unwrap());
    }

    #[test]
    fn grid_marks_interval_direction_and_missing_metrics() {
        use card_shuffle_lab::{Moments, Schedule};
        use std::collections::BTreeMap;
        let summaries: Vec<_> = [("below", 0.0), ("at-target", 1.0), ("above", 3.0)]
            .into_iter()
            .enumerate()
            .map(|(id, (name, value))| {
                let mut m = Moments::default();
                m.add(value);
                m.add(value);
                Summary {
                    schedule: Schedule {
                        name: name.into(),
                        stream_id: id as u64,
                        operations: vec![],
                    },
                    diagnostics: BTreeMap::from([(
                        "original_neighbor_adjacency_fraction".into(),
                        m,
                    )]),
                    mean_ci95_normal_approximation: BTreeMap::new(),
                    estimated_handling_seconds_hypothesis: Some(19.0),
                }
            })
            .collect();
        let mut output = Vec::new();
        summary_grid(&mut output, &summaries, 2, false).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("Uniform target"));
        assert!(text.contains("0.0000-"));
        assert!(text.contains("1.0000="));
        assert!(text.contains("3.0000+"));
        assert!(text.contains("n/a"));
        assert!(text.contains("19.0"));
        assert!(text.contains("does not establish equivalence"));
    }

    fn cases() -> Progress<Vec<u8>> {
        let mut progress = Progress::new(Vec::new(), true);
        progress.set_cases((1..=18).map(|i| format!("case-{i:02}")), 100);
        progress
    }
    #[test]
    fn bars_persist_waiting_running_and_completed_states() {
        let mut progress = cases();
        let empty = progress.case_bar(0, 50);
        assert!(empty.contains("  0% W"));
        assert!(!empty.contains('#'));
        progress.case(0, 50, false);
        let running = progress.case_bar(0, 50);
        assert!(running.contains(" 50% R"));
        assert!(running.contains('#'));
        progress.case(0, 100, true);
        progress.case(1, 25, false);
        let done = progress.case_bar(0, 50);
        assert!(done.contains("100% D"));
        let bar = done.split('[').nth(1).unwrap().split(']').next().unwrap();
        assert!(bar.chars().all(|c| c == '#'));
        let layout = progress.layout(&["Run 1/3".into()]).join("\n");
        assert!(layout.contains("case-01"));
        assert!(layout.contains("case-02"));
        assert!(layout.contains("case-18"));
    }
    #[test]
    fn viewport_layout_fits_and_keeps_active_case_visible() {
        let mut progress = cases();
        for index in 0..12 {
            progress.case(index, 100, true);
        }
        progress.case(12, 35, false);
        let context = vec![
            "Run 1/3".into(),
            "Sequence 13/18".into(),
            "Step 1/3".into(),
            "Trials 35/100".into(),
            "Overall".into(),
        ];
        for (columns, rows) in [(80, 24), (80, 10), (40, 8), (24, 5), (10, 3), (1, 1)] {
            progress.viewport = Viewport { columns, rows };
            let layout = progress.layout(&context);
            assert!(layout.len() <= rows.saturating_sub(1).max(1));
            assert!(layout
                .iter()
                .all(|s| s.len() <= columns.saturating_sub(1).max(1)));
            if columns >= 24 {
                assert!(layout.iter().any(|s| s.contains("case-13")), "{layout:?}");
            }
        }
        progress.viewport = Viewport {
            columns: 80,
            rows: 10,
        };
        assert!(progress
            .layout(&context)
            .iter()
            .any(|s| s.contains("case-01") && s.contains("case-02")));
        progress.viewport = Viewport {
            columns: 40,
            rows: 8,
        };
        assert!(progress
            .layout(&context)
            .iter()
            .any(|s| s.contains("Showing cases")));
        // Expanding again restores every case and its retained state.
        progress.viewport = Viewport {
            columns: 80,
            rows: 24,
        };
        assert_eq!(
            progress
                .layout(&context)
                .iter()
                .filter(|s| s.contains("case-"))
                .count(),
            18
        );
    }

    #[test]
    fn restricted_view_prioritizes_multiple_active_cases() {
        let mut progress = cases();
        progress.viewport = Viewport {
            columns: 40,
            rows: 8,
        };
        progress.case(12, 50, false);
        progress.case(16, 50, false);
        let lines = progress.layout(&["Run".into(), "Phase".into()]);
        assert!(lines.iter().any(|s| s.contains("case-13")));
        assert!(lines.iter().any(|s| s.contains("case-17")));
    }

    #[test]
    fn dashboard_redraws_only_its_previous_lines() {
        let mut progress = Progress::new(Vec::new(), true);
        progress
            .update(&["Run 1/3".into(), "Trials 0/10".into()])
            .unwrap();
        progress
            .update(&["Run 1/3".into(), "Trials 10/10".into()])
            .unwrap();
        assert_eq!(
            String::from_utf8(progress.writer).unwrap(),
            "Run 1/3\nTrials 0/10\n\x1b[2A\x1b[JRun 1/3\nTrials 10/10\n"
        );
        assert_eq!(terminal_text("bad\x1b\nname"), "badname");
    }
}
