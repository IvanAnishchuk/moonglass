use std::collections::BTreeMap;

use crate::adapters::Outcome;
use crate::discover::Case;
use crate::known_failures;

#[derive(Debug, Default, Clone, Copy)]
struct Bucket {
    pass: usize,
    fail: usize,
    expected_fail: usize,
    timeout: usize,
}

impl Bucket {
    fn total(self) -> usize {
        self.pass + self.fail + self.expected_fail + self.timeout
    }
}

#[derive(Debug, Clone)]
struct Failure {
    case_path: String,
    case_root: String,
    detail: String,
}

#[derive(Debug, Clone)]
struct ExpectedFailure {
    case_path: String,
    case_root: String,
    detail: String,
    reason: &'static str,
}

#[derive(Debug, Clone)]
struct UnexpectedPass {
    case_path: String,
    reason: &'static str,
}

#[derive(Debug, Clone)]
struct TimedOut {
    case_path: String,
    case_root: String,
    detail: String,
}

#[derive(Debug, Default)]
pub(crate) struct Summary {
    buckets: BTreeMap<(String, String, String, String), Bucket>,
    failures: Vec<Failure>,
    expected_failures: Vec<ExpectedFailure>,
    unexpected_passes: Vec<UnexpectedPass>,
    timeouts: Vec<TimedOut>,
}

impl Summary {
    #[must_use]
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn record(&mut self, case: &Case, outcome: &Outcome) {
        let key = (
            case.config.clone(),
            case.fork.clone(),
            case.runner.clone(),
            case.handler.clone(),
        );
        let bucket = self.buckets.entry(key).or_default();
        let case_path = case.display_path();
        let allowlisted = known_failures::matches(&case_path);
        match outcome {
            Outcome::Pass => {
                bucket.pass += 1;
                if let Some(kf) = allowlisted {
                    self.unexpected_passes.push(UnexpectedPass {
                        case_path,
                        reason: kf.reason,
                    });
                }
            }
            Outcome::Fail(detail) => {
                let case_root = case_root_string(case);
                if let Some(kf) = allowlisted {
                    bucket.expected_fail += 1;
                    self.expected_failures.push(ExpectedFailure {
                        case_path,
                        case_root,
                        detail: detail.clone(),
                        reason: kf.reason,
                    });
                } else {
                    bucket.fail += 1;
                    self.failures.push(Failure {
                        case_path,
                        case_root,
                        detail: detail.clone(),
                    });
                }
            }
            Outcome::Timeout(detail) => {
                bucket.timeout += 1;
                self.timeouts.push(TimedOut {
                    case_path,
                    case_root: case_root_string(case),
                    detail: detail.clone(),
                });
            }
        }
    }

    /// Returns true if any unallowlisted case failed. Timeouts and
    /// allowlisted failures are reported but do not trigger a failing exit
    /// code.
    #[must_use]
    pub(crate) fn has_failures(&self) -> bool {
        !self.failures.is_empty()
    }

    #[must_use]
    pub(crate) fn totals(&self) -> Totals {
        let mut t = Totals::default();
        for b in self.buckets.values() {
            t.pass += b.pass;
            t.fail += b.fail;
            t.expected_fail += b.expected_fail;
            t.timeout += b.timeout;
        }
        t
    }

    pub(crate) fn print(&self) {
        if self.buckets.is_empty() {
            println!("no cases matched");
            return;
        }

        let mut max_key_len = 0;
        let mut rows: Vec<(String, Bucket)> = Vec::with_capacity(self.buckets.len());
        for ((config, fork, runner, handler), bucket) in &self.buckets {
            let key = format!("{config}/{fork}/{runner}/{handler}");
            max_key_len = max_key_len.max(key.len());
            rows.push((key, *bucket));
        }

        println!();
        for (key, bucket) in &rows {
            println!(
                "{key:<width$}  pass={p:<5} fail={f:<5} todo={tf:<5} timeout={to:<5} total={t}",
                key = key,
                width = max_key_len,
                p = bucket.pass,
                f = bucket.fail,
                tf = bucket.expected_fail,
                to = bucket.timeout,
                t = bucket.total(),
            );
        }

        let t = self.totals();
        println!();
        println!(
            "totals  pass={p} fail={f} todo={tf} timeout={to} total={total}",
            p = t.pass,
            f = t.fail,
            tf = t.expected_fail,
            to = t.timeout,
            total = t.total(),
        );

        if !self.timeouts.is_empty() {
            println!();
            println!("timeouts (not counted as failures):");
            for t in &self.timeouts {
                println!("  {}", t.case_path);
                println!("    path: {}", t.case_root);
                println!("    {}", t.detail);
            }
        }

        if !self.expected_failures.is_empty() {
            println!();
            println!("TODOs (allowlisted failures, not counted as failures):");
            for expected_failure in &self.expected_failures {
                println!("  {}", expected_failure.case_path);
                println!("    path: {}", expected_failure.case_root);
                println!("    reason: {}", expected_failure.reason);
                for line in expected_failure.detail.split('\n') {
                    println!("    {line}");
                }
            }
        }

        if !self.unexpected_passes.is_empty() {
            println!();
            println!(
                "unexpected passes (cases in the allowlist that passed; remove them from known_failures.rs):"
            );
            for up in &self.unexpected_passes {
                println!("  {}", up.case_path);
                println!("    reason was: {}", up.reason);
            }
        }

        if !self.failures.is_empty() {
            println!();
            println!("failures:");
            for f in &self.failures {
                println!("  {}", f.case_path);
                println!("    path: {}", f.case_root);
                for line in f.detail.split('\n') {
                    println!("    {line}");
                }
            }
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Totals {
    pub pass: usize,
    pub fail: usize,
    pub expected_fail: usize,
    pub timeout: usize,
}

impl Totals {
    #[must_use]
    pub(crate) fn total(self) -> usize {
        self.pass + self.fail + self.expected_fail + self.timeout
    }
}

fn case_root_string(case: &Case) -> String {
    case.root
        .canonicalize()
        .unwrap_or_else(|_| case.root.clone())
        .display()
        .to_string()
}
