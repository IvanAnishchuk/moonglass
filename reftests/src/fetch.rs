use std::fs::File;
use std::io::{self, BufWriter, Read, Write};
use std::path::Path;

use anyhow::Context as _;
use serde::Deserialize;

use crate::archive::{self, Limits};
use crate::manifest::{Manifest, manifest_path};

#[derive(Clone, Copy)]
pub(crate) struct RequiredArchive {
    pub(crate) name: &'static str,
    pub(crate) sha256: &'static str,
    pub(crate) compressed_bytes: u64,
    limits: Limits,
}

pub(crate) const REQUIRED_ARCHIVES: &[RequiredArchive] = &[
    RequiredArchive {
        name: "general.tar.gz",
        sha256: "b330e90553b611b8bcfdbc1b8961695ba1f87398319e9537840512df5005d361",
        compressed_bytes: 169_623_613,
        limits: Limits {
            max_entries: 22_010,
            max_unpacked_bytes: 357_480_968,
        },
    },
    RequiredArchive {
        name: "mainnet.tar.gz",
        sha256: "17c8cf98dff97272a5089beb19af74c96144ecd950365f1d5627a131c2dcec66",
        compressed_bytes: 848_042_413,
        limits: Limits {
            max_entries: 66_004,
            max_unpacked_bytes: 2_753_663_970,
        },
    },
    RequiredArchive {
        name: "minimal.tar.gz",
        sha256: "59411e3bc7b67b297cbef37fa05be6af782705b2de247b5c72d7f1aad1f40d98",
        compressed_bytes: 413_973_744,
        limits: Limits {
            max_entries: 404_827,
            max_unpacked_bytes: 705_802_528,
        },
    },
];

const REPO: &str = "ethereum/consensus-specs";
const API_BASE: &str = "https://api.github.com";
const USER_AGENT: &str = concat!("reftests/", env!("CARGO_PKG_VERSION"));

#[derive(Debug, Deserialize)]
struct Release {
    tag_name: String,
    #[serde(default)]
    assets: Vec<Asset>,
}

#[derive(Debug, Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    digest: Option<String>,
    size: u64,
}

pub(crate) fn fetch_release(tag: &str, dest_root: &Path) -> anyhow::Result<Manifest> {
    let release = resolve_release(tag)?;
    let tag_dir = dest_root.join(&release.tag_name);
    let archives_dir = tag_dir.join(".archives");
    std::fs::create_dir_all(&archives_dir)?;

    let mut manifest = Manifest::new(release.tag_name.clone())?;
    for required in REQUIRED_ARCHIVES {
        let asset = release_asset(&release, required)?;
        validate_release_asset(asset, required)?;
        let archive_path = archives_dir.join(required.name);
        let hash = ensure_archive(required, asset, &archive_path)?;
        manifest
            .archive_sha256s
            .insert(required.name.to_owned(), hash);
    }

    let live_manifest = manifest_path(&tag_dir);
    remove_path_if_exists(&live_manifest)?;

    let staging_dir = tag_dir.join(".extracting-tests");
    remove_path_if_exists(&staging_dir)?;
    std::fs::create_dir_all(&staging_dir)?;
    for required in REQUIRED_ARCHIVES {
        let archive_path = archives_dir.join(required.name);
        println!("extracting {}", required.name);
        archive::extract_tar_gz(&archive_path, &staging_dir, required.limits)?;
    }

    let staged_tests = staging_dir.join("tests");
    if !staged_tests.is_dir() {
        anyhow::bail!(
            "release {} did not extract a tests directory",
            release.tag_name
        );
    }
    if archive::contains_symlink(&staged_tests)? {
        anyhow::bail!(
            "release {} extracted symlinks under tests",
            release.tag_name
        );
    }

    let live_tests = tag_dir.join("tests");
    remove_path_if_exists(&live_tests)?;
    std::fs::rename(&staged_tests, &live_tests).with_context(|| {
        format!(
            "publish extracted tests {} -> {}",
            staged_tests.display(),
            live_tests.display()
        )
    })?;
    remove_path_if_exists(&staging_dir)?;

    manifest.write(&live_manifest)?;
    Ok(manifest)
}

fn resolve_release(tag: &str) -> anyhow::Result<Release> {
    let url = format!("{API_BASE}/repos/{REPO}/releases/tags/{tag}");
    api_get_json(&url)
}

fn release_asset<'a>(
    release: &'a Release,
    required: &RequiredArchive,
) -> anyhow::Result<&'a Asset> {
    release
        .assets
        .iter()
        .find(|asset| asset.name == required.name)
        .with_context(|| {
            format!(
                "release {} is missing asset {}",
                release.tag_name, required.name
            )
        })
}

fn validate_release_asset(asset: &Asset, required: &RequiredArchive) -> anyhow::Result<()> {
    if asset.size != required.compressed_bytes {
        anyhow::bail!(
            "release asset {name} has size {got}, want {want}",
            name = required.name,
            got = asset.size,
            want = required.compressed_bytes
        );
    }
    ensure_expected_digest(required, asset)
}

fn ensure_archive(
    required: &RequiredArchive,
    asset: &Asset,
    path: &Path,
) -> anyhow::Result<String> {
    if path.exists() {
        let hash = archive::sha256_hex(path)?;
        if hash == required.sha256 && path.metadata()?.len() == required.compressed_bytes {
            return Ok(hash);
        }
        std::fs::remove_file(path)
            .with_context(|| format!("remove stale archive {}", path.display()))?;
    }

    println!("downloading {}", required.name);
    download_to(&asset.browser_download_url, path, required.compressed_bytes)?;
    let hash = archive::sha256_hex(path)?;
    if hash != required.sha256 {
        anyhow::bail!(
            "sha256 mismatch for {name}: got {hash}, want {want}",
            name = required.name,
            want = required.sha256,
        );
    }
    Ok(hash)
}

fn ensure_expected_digest(required: &RequiredArchive, asset: &Asset) -> anyhow::Result<()> {
    let Some(expected) = asset.digest.as_deref() else {
        anyhow::bail!("release asset {} is missing digest", required.name);
    };
    let Some(hash) = expected.strip_prefix("sha256:") else {
        anyhow::bail!("unsupported digest for {}: {expected}", required.name);
    };
    if hash != required.sha256 {
        anyhow::bail!(
            "release digest mismatch for {name}: got {hash}, want {want}",
            name = required.name,
            want = required.sha256,
        );
    }
    Ok(())
}

fn remove_path_if_exists(path: &Path) -> anyhow::Result<()> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err).with_context(|| format!("inspect {}", path.display())),
    };

    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        std::fs::remove_dir_all(path).with_context(|| format!("remove {}", path.display()))?;
    } else {
        std::fs::remove_file(path).with_context(|| format!("remove {}", path.display()))?;
    }
    Ok(())
}

fn api_get_json<T: serde::de::DeserializeOwned>(url: &str) -> anyhow::Result<T> {
    let resp = ureq::get(url)
        .set("User-Agent", USER_AGENT)
        .set("Accept", "application/vnd.github+json")
        .set("X-GitHub-Api-Version", "2022-11-28")
        .call()
        .with_context(|| format!("GET {url}"))?;
    resp.into_json()
        .with_context(|| format!("decode JSON from {url}"))
}

fn download_to(url: &str, dest: &Path, expected_bytes: u64) -> anyhow::Result<()> {
    let tmp = dest.with_extension("part");
    if tmp.exists() {
        std::fs::remove_file(&tmp).with_context(|| format!("clean stale {}", tmp.display()))?;
    }
    let resp = ureq::get(url)
        .set("User-Agent", USER_AGENT)
        .set("Accept", "application/octet-stream")
        .call()
        .with_context(|| format!("GET {url}"))?;
    let mut reader = resp.into_reader();
    let mut written = 0_u64;
    {
        let mut writer = BufWriter::new(File::create(&tmp)?);
        let mut buf = vec![0_u8; 64 * 1024];
        loop {
            let read = reader.read(&mut buf)?;
            if read == 0 {
                break;
            }
            written += read as u64;
            if written > expected_bytes {
                std::fs::remove_file(&tmp).ok();
                anyhow::bail!("download from {url} exceeded {expected_bytes} bytes");
            }
            writer.write_all(&buf[..read])?;
        }
        writer.flush()?;
    }
    if written != expected_bytes {
        std::fs::remove_file(&tmp).ok();
        anyhow::bail!("download from {url} wrote {written} bytes, want {expected_bytes}");
    }
    std::fs::rename(&tmp, dest)
        .with_context(|| format!("rename {} -> {}", tmp.display(), dest.display()))?;
    Ok(())
}
