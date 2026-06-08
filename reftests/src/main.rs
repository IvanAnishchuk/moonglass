use std::any::Any;
#[cfg(unix)]
use std::io::Write as _;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::Command;
#[cfg(unix)]
use std::process::{Output, Stdio};
use std::time::Duration;
#[cfg(unix)]
use std::time::Instant;

use anyhow::Context;

mod adapters;
mod archive;
mod compare;
mod discover;
mod fetch;
mod fixture;
mod hex;
mod known_failures;
mod manifest;
mod report;

use crate::adapters::Outcome;
use crate::manifest::{Manifest, manifest_path};
use crate::report::Summary;

const VECTORS_DIR: &str = "reftests/vectors";
const MAINNET_PRESET: &str = "mainnet";
const MINIMAL_PRESET: &str = "minimal";

/// `ethereum/consensus-specs` release moonglass targets.
///
/// Test vectors are fetched from release assets for this exact tag.
const CONSENSUS_SPECS_TAG: &str = "v1.7.0-alpha.10";

/// Fork moonglass currently targets within the consensus-specs release.
const TARGET_FORK: &str = "gloas";

const MINIMAL_TARGET_DIR: &str = "target/reftests-minimal";
const PROGRESS_INTERVAL: usize = 100;
const CASE_TIMEOUT: Duration = Duration::from_secs(10);
#[cfg(unix)]
const CASE_POLL_INTERVAL: Duration = Duration::from_millis(10);
#[cfg(unix)]
const INTERNAL_WORKER_ARG0: &str = "reftests-internal-case-worker";
#[cfg(not(unix))]
const CASE_STACK_BYTES: usize = 8 * 1024 * 1024;

#[cfg(feature = "mainnet")]
const ACTIVE_PRESET: &str = "mainnet";
#[cfg(feature = "minimal")]
const ACTIVE_PRESET: &str = "minimal";

#[cfg(not(any(feature = "mainnet", feature = "minimal")))]
compile_error!("reftests must be built with exactly one of the `mainnet` or `minimal` features");

#[cfg(all(feature = "mainnet", feature = "minimal"))]
compile_error!(
    "reftests cannot be built with both `mainnet` and `minimal` features (cargo features are additive)"
);

fn main() -> anyhow::Result<()> {
    #[cfg(unix)]
    if internal_case_worker() {
        return run_case_worker();
    }

    reject_args()?;

    if ACTIVE_PRESET == "minimal" {
        return run_minimal_only();
    }

    let mainnet = run_mainnet_with_general();
    let minimal = build_and_run_minimal();
    let mut failures = Vec::new();
    if let Err(err) = mainnet {
        failures.push(format!("mainnet: {err:#}"));
    }
    if let Err(err) = minimal {
        failures.push(format!("minimal: {err:#}"));
    }
    if !failures.is_empty() {
        for failure in &failures {
            eprintln!("{failure}");
        }
        anyhow::bail!("{} preset(s) failed", failures.len());
    }
    Ok(())
}

fn reject_args() -> anyhow::Result<()> {
    let extra: Vec<String> = std::env::args().skip(1).collect();
    if extra.is_empty() {
        return Ok(());
    }

    anyhow::bail!("reftests takes no arguments; run `target/release/reftests`")
}

fn build_and_run_minimal() -> anyhow::Result<()> {
    eprintln!();
    eprintln!("building minimal reftest runner");
    let workspace = workspace_root();
    let target = host_target()?;

    let status = Command::new("cargo")
        .current_dir(&workspace)
        .arg("build")
        .arg("--release")
        .arg("--locked")
        .arg("--manifest-path")
        .arg(workspace.join("Cargo.toml"))
        .arg("-p")
        .arg("reftests")
        .arg("--target-dir")
        .arg(minimal_target_dir())
        .arg("--target")
        .arg(&target)
        .arg("--no-default-features")
        .arg("--features")
        .arg("minimal")
        .status()
        .context("build minimal reftest runner")?;
    if !status.success() {
        anyhow::bail!("minimal reftest runner build failed");
    }

    eprintln!();
    eprintln!("running minimal reftests");
    let status = Command::new(minimal_binary(&target))
        .current_dir(&workspace)
        .status()
        .context("run minimal reftest runner")?;
    if !status.success() {
        anyhow::bail!("minimal reftests failed");
    }

    Ok(())
}

fn minimal_binary(target: &str) -> PathBuf {
    minimal_target_dir()
        .join(target)
        .join("release")
        .join(format!("reftests{}", std::env::consts::EXE_SUFFIX))
}

fn run_mainnet_with_general() -> anyhow::Result<()> {
    let tag_dir = tag_dir()?;
    let mut cases = discover::preset_cases(&tag_dir, MAINNET_PRESET, TARGET_FORK)?;
    if cases.is_empty() {
        anyhow::bail!(
            "no cases matched consensus-specs {CONSENSUS_SPECS_TAG} ({MAINNET_PRESET}/{TARGET_FORK})"
        );
    }

    let general = discover::general_cases(&tag_dir)?;
    if general.is_empty() {
        anyhow::bail!("no general cases matched consensus-specs {CONSENSUS_SPECS_TAG}");
    }
    cases.extend(general);
    cases.sort_by_key(discover::Case::display_path);

    eprintln!(
        "running {} cases for consensus-specs {} ({}/{}, plus general)",
        cases.len(),
        CONSENSUS_SPECS_TAG,
        MAINNET_PRESET,
        TARGET_FORK
    );
    run_cases(&cases, MAINNET_PRESET)
}

fn run_minimal_only() -> anyhow::Result<()> {
    let tag_dir = tag_dir()?;
    let cases = discover::preset_cases(&tag_dir, MINIMAL_PRESET, TARGET_FORK)?;
    if cases.is_empty() {
        anyhow::bail!(
            "no cases matched consensus-specs {CONSENSUS_SPECS_TAG} ({MINIMAL_PRESET}/{TARGET_FORK})"
        );
    }

    eprintln!(
        "running {} cases for consensus-specs {} ({}/{})",
        cases.len(),
        CONSENSUS_SPECS_TAG,
        MINIMAL_PRESET,
        TARGET_FORK
    );
    run_cases(&cases, MINIMAL_PRESET)
}

fn run_cases(cases: &[discover::Case], label: &str) -> anyhow::Result<()> {
    let mut summary = Summary::new();
    let total = cases.len();
    for (index, case) in cases.iter().enumerate() {
        let outcome = run_case(case);
        if (index + 1).is_multiple_of(PROGRESS_INTERVAL) || index + 1 == total {
            eprintln!("processed {}/{} cases", index + 1, total);
        }
        summary.record(case, &outcome);
    }
    summary.print();
    if summary.has_failures() {
        anyhow::bail!("{label} reftests failed");
    }
    Ok(())
}

fn run_case(case: &discover::Case) -> Outcome {
    match run_case_process(case) {
        Ok(outcome) => outcome,
        Err(err) => Outcome::Fail(format!("worker process: {err:#}")),
    }
}

#[cfg(unix)]
fn run_case_process(case: &discover::Case) -> anyhow::Result<Outcome> {
    use std::os::unix::process::CommandExt;

    let mut child = Command::new(std::env::current_exe().context("resolve current executable")?)
        .arg0(INTERNAL_WORKER_ARG0)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("spawn case worker")?;

    {
        let mut stdin = child.stdin.take().context("open worker stdin")?;
        serde_json::to_writer(&mut stdin, case).context("send case to worker")?;
        stdin.flush().context("flush worker stdin")?;
    }

    let start = Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            return decode_worker_output(&child.wait_with_output()?);
        }
        if start.elapsed() >= CASE_TIMEOUT {
            child.kill().ok();
            child.wait().ok();
            return Ok(Outcome::Timeout(format!(
                "timed out after {}s",
                CASE_TIMEOUT.as_secs()
            )));
        }
        std::thread::sleep(CASE_POLL_INTERVAL);
    }
}

#[cfg(unix)]
fn decode_worker_output(output: &Output) -> anyhow::Result<Outcome> {
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("case worker exited {}: {}", output.status, stderr.trim());
    }
    serde_json::from_slice(&output.stdout).context("decode worker outcome")
}

#[cfg(unix)]
fn internal_case_worker() -> bool {
    std::env::args().next().as_deref() == Some(INTERNAL_WORKER_ARG0)
}

#[cfg(unix)]
fn run_case_worker() -> anyhow::Result<()> {
    let case: discover::Case =
        serde_json::from_reader(std::io::stdin()).context("read worker case")?;
    let outcome = run_case_inner(&case);
    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();
    serde_json::to_writer(&mut stdout, &outcome).context("write worker outcome")?;
    stdout.flush().context("flush worker outcome")?;
    Ok(())
}

#[cfg(not(unix))]
fn run_case_process(case: &discover::Case) -> anyhow::Result<Outcome> {
    use std::sync::mpsc::{self, RecvTimeoutError};

    let (tx, rx) = mpsc::sync_channel(1);
    let case = case.clone();
    let worker = std::thread::Builder::new()
        .stack_size(CASE_STACK_BYTES)
        .spawn(move || {
            let _ = tx.send(run_case_inner(&case));
        });
    if let Err(err) = worker {
        return Ok(Outcome::Fail(format!("spawn worker thread: {err}")));
    }

    let outcome = match rx.recv_timeout(CASE_TIMEOUT) {
        Ok(outcome) => outcome,
        Err(RecvTimeoutError::Timeout) => {
            Outcome::Timeout(format!("timed out after {}s", CASE_TIMEOUT.as_secs()))
        }
        Err(RecvTimeoutError::Disconnected) => Outcome::Fail("worker thread exited".to_owned()),
    };
    Ok(outcome)
}

fn run_case_inner(case: &discover::Case) -> Outcome {
    match catch_unwind(AssertUnwindSafe(|| adapters::run(case))) {
        Ok(outcome) => outcome,
        Err(payload) => Outcome::Fail(format!("panic: {}", panic_message(&payload))),
    }
}

fn tag_dir() -> anyhow::Result<PathBuf> {
    let dest = vectors_root();
    let dir = dest.join(CONSENSUS_SPECS_TAG);
    if valid_cached_release(&dir)? {
        return Ok(dir);
    }

    let manifest = fetch::fetch_release(CONSENSUS_SPECS_TAG, &dest)?;
    let dir = dest.join(&manifest.tag);
    if !valid_cached_release(&dir)? {
        anyhow::bail!(
            "fetched {tag}, but required fixtures were not extracted",
            tag = manifest.tag,
        );
    }

    Ok(dir)
}

fn valid_cached_release(dir: &Path) -> anyhow::Result<bool> {
    let manifest_path = manifest_path(dir);
    if !manifest_path.exists() {
        return Ok(false);
    }
    let Ok(manifest) = Manifest::read(&manifest_path) else {
        return Ok(false);
    };
    if manifest.tag != CONSENSUS_SPECS_TAG || manifest.archive_sha256s.is_empty() {
        return Ok(false);
    }
    if tests_path_has_symlink(dir)? {
        return Ok(false);
    }
    if !required_fixture_roots_exist(dir) {
        return Ok(false);
    }
    for archive_info in fetch::REQUIRED_ARCHIVES {
        let Some(cached_hash) = manifest.archive_sha256s.get(archive_info.name) else {
            return Ok(false);
        };
        if cached_hash != archive_info.sha256 {
            return Ok(false);
        }

        let path = dir.join(".archives").join(archive_info.name);
        if !path.is_file() {
            return Ok(false);
        }
        if path.metadata()?.len() != archive_info.compressed_bytes {
            return Ok(false);
        }
        let got = archive::sha256_hex(&path)
            .with_context(|| format!("hash cached archive {}", path.display()))?;
        if got != archive_info.sha256 {
            return Ok(false);
        }
    }
    Ok(true)
}

fn required_fixture_roots_exist(dir: &Path) -> bool {
    dir.join("tests").join("general").is_dir()
        && dir
            .join("tests")
            .join(MAINNET_PRESET)
            .join(TARGET_FORK)
            .is_dir()
        && dir
            .join("tests")
            .join(MINIMAL_PRESET)
            .join(TARGET_FORK)
            .is_dir()
}

fn tests_path_has_symlink(dir: &Path) -> anyhow::Result<bool> {
    let tests = dir.join("tests");
    if !tests.exists() && std::fs::symlink_metadata(&tests).is_err() {
        return Ok(false);
    }
    archive::contains_symlink(&tests).with_context(|| format!("inspect {}", tests.display()))
}

fn vectors_root() -> PathBuf {
    workspace_root().join(VECTORS_DIR)
}

fn minimal_target_dir() -> PathBuf {
    workspace_root().join(MINIMAL_TARGET_DIR)
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("reftests crate lives inside workspace root")
        .to_path_buf()
}

fn host_target() -> anyhow::Result<String> {
    let output = Command::new("rustc")
        .arg("-vV")
        .output()
        .context("run rustc -vV")?;
    if !output.status.success() {
        anyhow::bail!("rustc -vV failed");
    }
    let stdout = String::from_utf8(output.stdout).context("decode rustc -vV output")?;
    stdout
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .map(str::to_owned)
        .context("rustc -vV output did not include host target")
}

fn panic_message(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&'static str>() {
        (*message).to_owned()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "non-string panic payload".to_owned()
    }
}
