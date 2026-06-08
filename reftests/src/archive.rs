use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};
use tar::EntryType;

use crate::hex;

#[derive(Clone, Copy)]
pub(crate) struct Limits {
    pub(crate) max_entries: u64,
    pub(crate) max_unpacked_bytes: u64,
}

/// Extract a gzipped tar archive into `dest`.
pub(crate) fn extract_tar_gz(archive: &Path, dest: &Path, limits: Limits) -> anyhow::Result<()> {
    std::fs::create_dir_all(dest)?;
    let file = File::open(archive)?;
    let gz = GzDecoder::new(BufReader::new(file));
    let mut tar = tar::Archive::new(gz);
    tar.set_preserve_permissions(false);
    tar.set_overwrite(true);

    let mut entries = 0_u64;
    let mut unpacked_bytes = 0_u64;
    for entry in tar.entries()? {
        let mut entry = entry?;
        entries += 1;
        if entries > limits.max_entries {
            anyhow::bail!(
                "archive {} has too many entries: over {}",
                archive.display(),
                limits.max_entries
            );
        }

        let entry_type = entry.header().entry_type();
        match entry_type {
            EntryType::Regular | EntryType::Directory => {}
            _ => anyhow::bail!(
                "archive {} contains unsupported entry type {:?}",
                archive.display(),
                entry_type
            ),
        }

        unpacked_bytes = unpacked_bytes
            .checked_add(entry.size())
            .ok_or_else(|| anyhow::anyhow!("archive {} size overflow", archive.display()))?;
        if unpacked_bytes > limits.max_unpacked_bytes {
            anyhow::bail!(
                "archive {} unpacks over {} bytes",
                archive.display(),
                limits.max_unpacked_bytes
            );
        }

        if !entry.unpack_in(dest)? {
            anyhow::bail!(
                "archive {} contains path outside extraction root",
                archive.display()
            );
        }
    }
    Ok(())
}

pub(crate) fn contains_symlink(path: &Path) -> anyhow::Result<bool> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Ok(true);
    }
    if !metadata.is_dir() {
        return Ok(false);
    }

    for entry in std::fs::read_dir(path)? {
        if contains_symlink(&entry?.path())? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Compute the hex-encoded sha256 of a file.
pub(crate) fn sha256_hex(path: &Path) -> anyhow::Result<String> {
    let mut file = BufReader::new(File::open(path)?);
    let mut hasher = Sha256::new();
    let mut buf = vec![0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buf)?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    let digest = hasher.finalize();
    Ok(hex::encode(&digest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_hex_matches_known_vector() {
        // sha256("abc") is one of the canonical NIST vectors.
        let tmp = temp_path("sha256");
        std::fs::write(&tmp, b"abc").expect("write");
        let got = sha256_hex(&tmp).expect("hash");
        std::fs::remove_file(&tmp).ok();
        assert_eq!(
            got,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    fn temp_path(name: &str) -> std::path::PathBuf {
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
