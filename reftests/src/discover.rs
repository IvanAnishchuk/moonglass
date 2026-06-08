use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::adapters;

/// One concrete fixture under `vectors/<tag>/tests/.../<case>/`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Case {
    pub(crate) config: String,
    pub(crate) fork: String,
    pub(crate) runner: String,
    pub(crate) handler: String,
    pub(crate) suite: String,
    pub(crate) id: String,
    pub(crate) root: PathBuf,
}

impl Case {
    /// Slash-joined identifier of the form `config/fork/runner/handler/suite/case_id`.
    #[must_use]
    pub(crate) fn display_path(&self) -> String {
        format!(
            "{}/{}/{}/{}/{}/{}",
            self.config, self.fork, self.runner, self.handler, self.suite, self.id
        )
    }
}

pub(crate) fn preset_cases(tag_dir: &Path, preset: &str, fork: &str) -> anyhow::Result<Vec<Case>> {
    let root = tag_dir.join("tests").join(preset).join(fork);
    if !root.exists() {
        anyhow::bail!("no `{preset}/{fork}` tests under {}", tag_dir.display());
    }

    let mut cases = Vec::new();
    walk_runner_tree(&root, preset, fork, &mut cases)?;
    cases.sort_by_key(Case::display_path);
    Ok(cases)
}

pub(crate) fn general_cases(tag_dir: &Path) -> anyhow::Result<Vec<Case>> {
    let root = tag_dir.join("tests").join("general");
    if !root.exists() {
        anyhow::bail!("no `general` tests under {}", tag_dir.display());
    }

    let mut cases = Vec::new();
    for entry in read_subdirs(&root)? {
        let name = file_name(&entry);
        if looks_like_fork(&name) {
            walk_runner_tree(&entry, "general", &name, &mut cases)?;
        } else {
            walk_handler_tree(&entry, "general", "general", &name, &mut cases)?;
        }
    }
    cases.sort_by_key(Case::display_path);
    Ok(cases)
}

fn walk_runner_tree(
    fork_dir: &Path,
    config: &str,
    fork: &str,
    out: &mut Vec<Case>,
) -> anyhow::Result<()> {
    for runner_entry in read_subdirs(fork_dir)? {
        let runner = file_name(&runner_entry);
        walk_handler_tree(&runner_entry, config, fork, &runner, out)?;
    }
    Ok(())
}

fn walk_handler_tree(
    runner_dir: &Path,
    config: &str,
    fork: &str,
    runner: &str,
    out: &mut Vec<Case>,
) -> anyhow::Result<()> {
    for handler_entry in read_subdirs(runner_dir)? {
        let handler = file_name(&handler_entry);
        if !adapters::supports(runner, &handler) {
            continue;
        }
        for suite_entry in read_subdirs(&handler_entry)? {
            let suite = file_name(&suite_entry);
            for case_entry in read_subdirs(&suite_entry)? {
                let id = file_name(&case_entry);
                out.push(Case {
                    config: config.to_owned(),
                    fork: fork.to_owned(),
                    runner: runner.to_owned(),
                    handler: handler.clone(),
                    suite: suite.clone(),
                    id,
                    root: case_entry,
                });
            }
        }
    }
    Ok(())
}

fn read_subdirs(dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if entry.path().symlink_metadata()?.is_dir() {
            out.push(entry.path());
        }
    }
    out.sort();
    Ok(out)
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn looks_like_fork(name: &str) -> bool {
    matches!(
        name,
        "phase0"
            | "altair"
            | "bellatrix"
            | "capella"
            | "deneb"
            | "electra"
            | "fulu"
            | "gloas"
            | "eip6110"
            | "eip7732"
            | "whisk"
    )
}
