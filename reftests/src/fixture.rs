use std::path::Path;

use anyhow::Context;
use serde::Deserialize;

const META_FILENAME: &str = "meta.yaml";

/// Optional fields parsed from a case's `meta.yaml`. Only fields actively
/// consumed by adapters live here. Add new ones explicitly as new runners get
/// wired (notably `bls_setting` for any runner that exercises signature-disabled
/// fixtures).
#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct Meta {
    pub(crate) blocks_count: Option<u64>,
}

/// Decompress a `.ssz_snappy` reference-test file (raw snappy block format) into raw SSZ bytes.
pub(crate) fn read_snappy_file(path: &Path) -> anyhow::Result<Vec<u8>> {
    let compressed = std::fs::read(path).with_context(|| format!("open {}", path.display()))?;
    let mut decoder = snap::raw::Decoder::new();
    let out = decoder
        .decompress_vec(&compressed)
        .with_context(|| format!("snappy decode {}", path.display()))?;
    Ok(out)
}

/// Decompress an SSZ-snappy file and decode it into a moonglass container.
pub(crate) fn decode_ssz_snappy<T>(path: &Path) -> anyhow::Result<T>
where
    T: ssz_rs::Deserialize,
{
    let bytes = read_snappy_file(path)?;
    T::deserialize(&bytes).with_context(|| format!("ssz decode {}", path.display()))
}

/// Read `meta.yaml` from a case directory if present.
pub(crate) fn read_meta(case_dir: &Path) -> anyhow::Result<Meta> {
    let path = case_dir.join(META_FILENAME);
    if !path.exists() {
        return Ok(Meta::default());
    }
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let meta: Meta =
        serde_yaml::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
    Ok(meta)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meta_parses_blocks_count() {
        let m: Meta = serde_yaml::from_str("{blocks_count: 4}").expect("parse");
        assert_eq!(m.blocks_count, Some(4));
    }

    #[test]
    fn meta_default_when_field_absent() {
        let m: Meta = serde_yaml::from_str("{other_field: 99}").expect("parse");
        assert_eq!(m.blocks_count, None);
    }

    #[test]
    fn read_meta_returns_default_when_file_absent() {
        let nowhere = std::path::Path::new("/nonexistent-moonglass-reftests-meta-dir");
        let m = read_meta(nowhere).expect("read");
        assert!(m.blocks_count.is_none());
    }
}
