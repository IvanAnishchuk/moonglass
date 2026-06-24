//! Download and publish pinned consensus-spec test-vector releases.
//!
//! The runner treats upstream vectors as immutable inputs. Every selected
//! archive is checked against its expected GitHub asset size, GitHub-reported
//! sha256 digest, locally computed sha256, and extraction limits before the
//! extracted `tests/` tree is published into the local cache.

use std::fs::File;
use std::io::{self, BufWriter, Read, Write};
use std::path::Path;

use serde::Deserialize;

use crate::error::FetchError;

use super::FixtureSet;
use super::archive::{self, Limits};
use super::manifest::{Manifest, manifest_path};

/// Fetch operation result.
pub(super) type Result<T> = std::result::Result<T, FetchError>;

#[derive(Clone, Copy)]
pub(super) struct RequiredArchive {
    /// GitHub release asset filename.
    pub(super) name: &'static str,
    /// Expected hex-encoded sha256 digest.
    pub(super) sha256: &'static str,
    /// Expected compressed asset size in bytes.
    pub(super) compressed_bytes: u64,
    limits: Limits,
}

const GENERAL_ARCHIVE: RequiredArchive = RequiredArchive {
    name: "general.tar.gz",
    sha256: "b330e90553b611b8bcfdbc1b8961695ba1f87398319e9537840512df5005d361",
    compressed_bytes: 169_623_613,
    limits: Limits {
        max_entries: 22_010,
        max_unpacked_bytes: 357_480_968,
    },
};

const MAINNET_ARCHIVE: RequiredArchive = RequiredArchive {
    name: "mainnet.tar.gz",
    sha256: "956cc05f9bb2e745ecd04b60fb2bb91679c80ede82e81b489b5d47a9d65eb66b",
    compressed_bytes: 851_476_438,
    limits: Limits {
        max_entries: 66_309,
        max_unpacked_bytes: 2_763_608_206,
    },
};

const MINIMAL_ARCHIVE: RequiredArchive = RequiredArchive {
    name: "minimal.tar.gz",
    sha256: "8ab52feb780e034dde188143db709adcfb03abcee8beb03f1d502fb521baff4a",
    compressed_bytes: 412_296_024,
    limits: Limits {
        max_entries: 425_672,
        max_unpacked_bytes: 722_124_429,
    },
};

const GENERAL_ARCHIVES: &[RequiredArchive] = &[GENERAL_ARCHIVE];
const MAINNET_ARCHIVES: &[RequiredArchive] = &[MAINNET_ARCHIVE];
const MINIMAL_ARCHIVES: &[RequiredArchive] = &[MINIMAL_ARCHIVE];

pub(super) const fn required_archives(fixtures: FixtureSet) -> &'static [RequiredArchive] {
    match fixtures {
        FixtureSet::General => GENERAL_ARCHIVES,
        FixtureSet::Mainnet => MAINNET_ARCHIVES,
        FixtureSet::Minimal => MINIMAL_ARCHIVES,
    }
}

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

/// Fetch, verify, extract, and publish the release identified by `tag`.
pub(super) fn fetch_release(tag: &str, tag_dir: &Path, fixtures: FixtureSet) -> Result<Manifest> {
    let release = resolve_release(tag)?;
    let archives_dir = tag_dir.join(".archives");
    std::fs::create_dir_all(&archives_dir).map_err(|source| FetchError::Io {
        action: "create directory",
        path: archives_dir.clone(),
        source,
    })?;

    let mut manifest = Manifest::new(release.tag_name.clone())?;
    for required in required_archives(fixtures) {
        let asset = release_asset(&release, required)?;
        validate_release_asset(asset, required)?;
        let archive_path = archives_dir.join(required.name);
        let hash = ensure_archive(required, asset, &archive_path)?;
        manifest
            .archive_sha256s
            .insert(required.name.to_owned(), hash);
    }

    let live_manifest = manifest_path(tag_dir);
    remove_path_if_exists(&live_manifest)?;

    let staging_dir = tag_dir.join(".extracting-tests");
    remove_path_if_exists(&staging_dir)?;
    std::fs::create_dir_all(&staging_dir).map_err(|source| FetchError::Io {
        action: "create directory",
        path: staging_dir.clone(),
        source,
    })?;
    for required in required_archives(fixtures) {
        let archive_path = archives_dir.join(required.name);
        eprintln!("extracting {}", required.name);
        archive::extract_tar_gz(&archive_path, &staging_dir, required.limits)?;
    }

    let staged_tests = staging_dir.join("tests");
    if !staged_tests.is_dir() {
        return Err(FetchError::MissingTestsDirectory {
            tag: release.tag_name,
        });
    }
    if archive::contains_symlink(&staged_tests)? {
        return Err(FetchError::SymlinkedTestsDirectory {
            tag: release.tag_name,
        });
    }

    let live_tests = tag_dir.join("tests");
    remove_path_if_exists(&live_tests)?;
    std::fs::rename(&staged_tests, &live_tests).map_err(|source| FetchError::Io {
        action: "publish extracted tests",
        path: live_tests.clone(),
        source,
    })?;
    remove_path_if_exists(&staging_dir)?;

    manifest.write(&live_manifest)?;
    Ok(manifest)
}

fn resolve_release(tag: &str) -> Result<Release> {
    let url = format!("{API_BASE}/repos/{REPO}/releases/tags/{tag}");
    api_get_json(&url)
}

fn release_asset<'a>(release: &'a Release, required: &RequiredArchive) -> Result<&'a Asset> {
    release
        .assets
        .iter()
        .find(|asset| asset.name == required.name)
        .ok_or_else(|| FetchError::MissingAsset {
            tag: release.tag_name.clone(),
            asset: required.name,
        })
}

fn validate_release_asset(asset: &Asset, required: &RequiredArchive) -> Result<()> {
    if asset.size != required.compressed_bytes {
        return Err(FetchError::AssetSize {
            asset: required.name,
            got: asset.size,
            want: required.compressed_bytes,
        });
    }
    ensure_expected_digest(required, asset)
}

fn ensure_archive(required: &RequiredArchive, asset: &Asset, path: &Path) -> Result<String> {
    let cached_metadata = match path.metadata() {
        Ok(metadata) => Some(metadata),
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => {
            return Err(FetchError::Io {
                action: "inspect",
                path: path.to_path_buf(),
                source: e,
            });
        }
    };

    if let Some(metadata) = cached_metadata {
        let hash = archive::sha256_hex(path)?;
        if hash == required.sha256 && metadata.len() == required.compressed_bytes {
            return Ok(hash);
        }
        std::fs::remove_file(path).map_err(|source| FetchError::Io {
            action: "remove stale archive",
            path: path.to_path_buf(),
            source,
        })?;
    }

    eprintln!("downloading {}", required.name);
    download_to(&asset.browser_download_url, path, required.compressed_bytes)?;
    let hash = archive::sha256_hex(path)?;
    if hash != required.sha256 {
        return Err(FetchError::ArchiveDigestMismatch {
            asset: required.name,
            got: hash,
            want: required.sha256,
        });
    }
    Ok(hash)
}

fn ensure_expected_digest(required: &RequiredArchive, asset: &Asset) -> Result<()> {
    let Some(expected) = asset.digest.as_deref() else {
        return Err(FetchError::MissingDigest {
            asset: required.name,
        });
    };
    let Some(hash) = expected.strip_prefix("sha256:") else {
        return Err(FetchError::UnsupportedDigest {
            asset: required.name,
            digest: expected.to_owned(),
        });
    };
    if hash != required.sha256 {
        return Err(FetchError::ReleaseDigestMismatch {
            asset: required.name,
            got: hash.to_owned(),
            want: required.sha256,
        });
    }
    Ok(())
}

fn remove_path_if_exists(path: &Path) -> Result<()> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => {
            return Err(FetchError::Io {
                action: "inspect",
                path: path.to_path_buf(),
                source: err,
            });
        }
    };

    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        std::fs::remove_dir_all(path).map_err(|source| FetchError::Io {
            action: "remove",
            path: path.to_path_buf(),
            source,
        })?;
    } else {
        std::fs::remove_file(path).map_err(|source| FetchError::Io {
            action: "remove",
            path: path.to_path_buf(),
            source,
        })?;
    }
    Ok(())
}

fn api_get_json<T: serde::de::DeserializeOwned>(url: &str) -> Result<T> {
    let resp = ureq::get(url)
        .set("User-Agent", USER_AGENT)
        .set("Accept", "application/vnd.github+json")
        .set("X-GitHub-Api-Version", "2022-11-28")
        .call()
        .map_err(|source| FetchError::Http {
            url: url.to_owned(),
            source: Box::new(source),
        })?;
    resp.into_json().map_err(|source| FetchError::Json {
        url: url.to_owned(),
        source,
    })
}

fn download_to(url: &str, dest: &Path, expected_bytes: u64) -> Result<()> {
    let tmp = dest.with_extension("part");
    match std::fs::remove_file(&tmp) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => {
            return Err(FetchError::Io {
                action: "clean stale",
                path: tmp.clone(),
                source: e,
            });
        }
    }
    let resp = ureq::get(url)
        .set("User-Agent", USER_AGENT)
        .set("Accept", "application/octet-stream")
        .call()
        .map_err(|source| FetchError::Http {
            url: url.to_owned(),
            source: Box::new(source),
        })?;
    let mut reader = resp.into_reader();
    let mut written = 0_u64;
    {
        let mut writer = BufWriter::new(File::create(&tmp).map_err(|source| FetchError::Io {
            action: "create",
            path: tmp.clone(),
            source,
        })?);
        let mut buf = vec![0_u8; 64 * 1024];
        loop {
            let read = reader.read(&mut buf).map_err(|source| FetchError::Io {
                action: "read download",
                path: tmp.clone(),
                source,
            })?;
            if read == 0 {
                break;
            }
            written += read as u64;
            if written > expected_bytes {
                std::fs::remove_file(&tmp).ok();
                return Err(FetchError::DownloadTooLarge {
                    url: url.to_owned(),
                    expected_bytes,
                });
            }
            writer
                .write_all(&buf[..read])
                .map_err(|source| FetchError::Io {
                    action: "write download",
                    path: tmp.clone(),
                    source,
                })?;
        }
        writer.flush().map_err(|source| FetchError::Io {
            action: "flush download",
            path: tmp.clone(),
            source,
        })?;
    }
    if written != expected_bytes {
        std::fs::remove_file(&tmp).ok();
        return Err(FetchError::DownloadWrongSize {
            url: url.to_owned(),
            written,
            expected_bytes,
        });
    }
    std::fs::rename(&tmp, dest).map_err(|source| FetchError::Io {
        action: "rename download",
        path: dest.to_path_buf(),
        source,
    })?;
    Ok(())
}
