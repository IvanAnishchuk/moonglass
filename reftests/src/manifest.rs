use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufReader, BufWriter, Write as _};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub(crate) const MANIFEST_FILENAME: &str = "manifest.json";

/// Persisted record of a fetched release.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Manifest {
    pub(crate) tag: String,
    /// Unix seconds since epoch when the release was fetched.
    pub(crate) fetched_at: u64,
    /// `asset_filename` -> hex-encoded sha256 of the downloaded archive.
    pub(crate) archive_sha256s: BTreeMap<String, String>,
}

impl Manifest {
    pub(crate) fn new(tag: String) -> anyhow::Result<Self> {
        Ok(Self {
            tag,
            fetched_at: now_unix_seconds()?,
            archive_sha256s: BTreeMap::new(),
        })
    }

    pub(crate) fn read(path: &Path) -> anyhow::Result<Self> {
        let file = File::open(path)?;
        let manifest = serde_json::from_reader(BufReader::new(file))?;
        Ok(manifest)
    }

    /// Write the manifest via a sibling `.tmp` file plus atomic rename, so a
    /// crash mid-write never leaves the live manifest truncated.
    pub(crate) fn write(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("tmp");
        {
            let file = File::create(&tmp)?;
            let mut writer = BufWriter::new(file);
            serde_json::to_writer_pretty(&mut writer, self)?;
            writer.flush()?;
        }
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
}

#[must_use]
pub(crate) fn manifest_path(tag_dir: &Path) -> PathBuf {
    tag_dir.join(MANIFEST_FILENAME)
}

fn now_unix_seconds() -> anyhow::Result<u64> {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| anyhow::anyhow!("system clock is before Unix epoch: {e}"))?
        .as_secs();
    Ok(secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_through_disk() {
        let dir = temp_dir("manifest");
        std::fs::create_dir_all(&dir).expect("mkdir");
        let path = dir.join(MANIFEST_FILENAME);

        let mut manifest = Manifest::new("v9.9.9-test".to_owned()).expect("new");
        manifest
            .archive_sha256s
            .insert("general.tar.gz".to_owned(), "abc123".to_owned());

        manifest.write(&path).expect("write");
        let read = Manifest::read(&path).expect("read");
        assert_eq!(read.tag, "v9.9.9-test");
        assert_eq!(read.archive_sha256s.len(), 1);
        assert_eq!(
            read.archive_sha256s
                .get("general.tar.gz")
                .map(String::as_str),
            Some("abc123")
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn write_is_atomic_via_tmp_then_rename() {
        let dir = temp_dir("manifest-atomic");
        std::fs::create_dir_all(&dir).expect("mkdir");
        let path = dir.join(MANIFEST_FILENAME);

        let manifest = Manifest::new("v1.2.3-atomic".to_owned()).expect("new");
        manifest.write(&path).expect("write");

        // After a successful write the sibling `.tmp` is gone.
        assert!(!path.with_extension("tmp").exists());
        assert!(path.exists());

        std::fs::remove_dir_all(&dir).ok();
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "moonglass-reftests-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ))
    }
}
