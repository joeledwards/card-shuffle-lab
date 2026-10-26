# Card shuffle lab

Rust prototype for comparing seven GSR riffles with five GSR riffles followed
by two overhands, plus independent Fisher–Yates and uniform insertion controls.
The language-neutral [specification](spec/algorithms.md) is the source of model
semantics. Rust is the initial implementation choice; portable specifications
and CSV/JSON outputs support future Julia or Python collaboration.

## Explicit local tooling

The Cargo wrapper never installs or bootstraps tooling.
Rust, Cargo, their caches, and future toolchain files belong under the workspace's
`.tools/rust/`. No Python environment is needed.

Developer-only setup, from this repository directory, after obtaining a trusted
rustup-init executable (network access and dependency downloads are explicit):

```sh
workspace_dir=$(CDPATH= cd ../.. && pwd)
export CARGO_HOME="$workspace_dir/.tools/rust/cargo"
export RUSTUP_HOME="$workspace_dir/.tools/rust/rustup"
# Run your verified installer; do not use a host-global installation:
/path/to/rustup-init --no-modify-path --profile minimal --default-toolchain stable -y
./scripts/cargo-local generate-lockfile
./scripts/cargo-local test --lib --locked
```

Cargo.lock is now generated and should be retained in version control. Use
`--locked` for tests and experiments to prevent dependency resolution changes. Dependency resolution and builds
require downloads on first use. The wrapper derives paths from its own location
and preserves the caller's working directory; run the following from this repo,
or supply Cargo's --manifest-path when calling it from elsewhere.

### Linker in minimal Linux environments

This workspace has no system `cc`. A checksum-verified Zig 0.17.0 distribution
is installed under `.tools/rust/linker/zig-x86_64-linux-0.17.0`, with
`version.txt` containing `0.17.0`. `scripts/cargo-local` selects
`scripts/linker-local` when no `cc` exists. Both Zig cache directories remain
under `.tools/rust/linker/`; no host package installation is needed. The wrapper
currently supports this fallback for x86_64 GNU/Linux only.

For a new environment, explicitly download the official archive and verify it
before extracting (commands from the repository directory):

```sh
workspace_dir=$(CDPATH= cd ../.. && pwd)
linker_dir="$workspace_dir/.tools/rust/linker"
mkdir -p "$linker_dir"
curl --fail --location --proto '=https' --tlsv1.2 \
  https://ziglang.org/download/0.17.0/zig-x86_64-linux-0.17.0.tar.xz \
  -o "$linker_dir/zig.tar.xz"
(cd "$linker_dir" && printf '%s  zig.tar.xz\n' \
  1cbe9df9f27e6b78d14ccbca43b6703a404ef79ef1c463de901d7f088d4e2026 | sha256sum -c -)
tar -xJf "$linker_dir/zig.tar.xz" -C "$linker_dir"
printf '%s\n' 0.17.0 > "$linker_dir/version.txt"
```

Stop if download or verification fails. Rustfmt and Clippy are optional Rust
components; install explicitly using the workspace-local rustup with CARGO_HOME
and RUSTUP_HOME set as above. See [validation results](spec/validation.md) for
the installed compiler and completed checks.

## Experiments (commands to run explicitly)

```sh
./scripts/cargo-local run --release --locked -- --deck-size 52 --trials 100 --seed 20261026 --output output/smoke-001
```

The output directory must not already exist. Results contain a versioned
`manifest.json` with exact schedules, seed, parameters, pointwise confidence
intervals and hypothetical handling times, plus aggregated position and neighbor
distance CSVs. No giant raw permutation file is needed. Numerical results replay
with the same source, lockfile, target, seed, and configuration; compute timings
vary. Archive the source snapshot, Cargo.lock, and `rustc -Vv` with shared results.
An interrupted run may leave partial files; use a new directory for the next run.

Use `--schedules schedules.json` for other placements, for example:

```json
[
  {"name":"2O-before-5R", "stream_id":10,
   "operations":["Overhand","Overhand","Riffle","Riffle","Riffle","Riffle","Riffle"]}
]
```

Use `--riffle-seconds` and `--overhand-seconds` to vary handling-time hypotheses.
Compare all diagnostics and their uncertainty against controls; neither uniform
control is inherently more random. A CI on each schedule mean is not a CI on
the difference between schedules. No experimental results or performance claims
are asserted until experiments are explicitly run and reviewed.

## Scope and next steps

GSR, the uniform controls, and uniform-pair analytic targets have mathematical
justification. Overhand packet sizes and handling times are uncalibrated
hypotheses. No diagnostic here measures proven total-variation distance.

Initial implementation includes algorithm unit tests, permutation/replay checks,
deterministic packet examples, streaming trial moments, and aggregate CSVs.
Still to implement: human cut/clump/error profiles with restart costs; generated
placement families; displacement and multiple-separation histograms; positional
uncertainty; effect-size comparisons; plotting and reports; richer time models;
and empirical control validation. Large simulations, benchmarks, and the full
experimental protocol require separate authorization. Unit tests alone do not
validate the human models or demonstrate statistical agreement of controls.
