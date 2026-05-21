//! Post-install manifest validation.
//!
//! The LWIW incident: a 62 MB partial archive of a ~660 MB game was
//! downloaded, marked `Installed`, and launched — its Unity engine then
//! failed because ~600 MB of asset files never landed. The download agent's
//! `validate()` was a stub that returned `Ok(true)` unconditionally, so an
//! install whose on-disk bytes did not match the manifest still transitioned
//! to `Installed`.
//!
//! This module re-derives ground truth from disk and compares it to the
//! server manifest. The status transition to `Installed` is gated behind
//! [`validate_install`] returning [`ValidationResult::Valid`].
//!
//! ## What is checked
//!
//! The Drop manifest stores per-*chunk* SHA-256 checksums (see
//! `droplet_rs::manifest::ChunkData`), computed over the *decrypted* chunk
//! payload. There are no per-file hashes. A chunk references one or more
//! `FileEntry` slices (`filename`, `start`, `length`). The downloader writes
//! decrypted bytes straight to disk, so the bytes on disk for a file region
//! are exactly the plaintext the chunk hash was taken over.
//!
//! Validation therefore proceeds per chunk:
//!   1. For every `FileEntry` in the chunk, confirm the file exists and is
//!      large enough to contain `start + length` bytes. A short/missing file
//!      is the LWIW signature and is recorded immediately.
//!   2. If all the chunk's file regions are present, re-read those exact
//!      regions in manifest order, feed them to a SHA-256 hasher, and compare
//!      the digest to `chunk_data.checksum`. A mismatch means corruption or
//!      a wrong-but-present file.

use std::collections::BTreeSet;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use droplet_rs::manifest::Manifest;
use log::{error, info, warn};
use sha2::{Digest, Sha256};

use super::download_agent::DownloadInformation;

/// One file that the manifest expects but disk does not satisfy.
#[derive(Debug, Clone)]
pub struct MissingFile {
    pub filename: String,
    /// Bytes the manifest requires the file to be at least.
    pub expected_min_size: u64,
    /// Bytes actually found on disk (`None` if the file is absent entirely).
    pub actual_size: Option<u64>,
}

/// One chunk whose on-disk bytes do not hash to the manifest checksum.
#[derive(Debug, Clone)]
pub struct MismatchedChunk {
    pub chunk_id: String,
    pub expected_checksum: String,
    pub actual_checksum: String,
}

/// Outcome of [`validate_install`].
#[derive(Debug)]
pub enum ValidationResult {
    /// Every manifest file is present at the right size and every chunk
    /// hashes correctly. Safe to mark `Installed`.
    Valid,
    /// At least one file is missing/short or one chunk is corrupt. The
    /// install MUST NOT be marked `Installed`.
    Incomplete {
        missing: Vec<MissingFile>,
        mismatched: Vec<MismatchedChunk>,
    },
}

impl ValidationResult {
    pub fn is_valid(&self) -> bool {
        matches!(self, ValidationResult::Valid)
    }

    /// Human-readable, actionable summary for the frontend / logs.
    pub fn describe(&self) -> String {
        match self {
            ValidationResult::Valid => "install validated".to_string(),
            ValidationResult::Incomplete {
                missing,
                mismatched,
            } => {
                let mut parts = Vec::new();
                if !missing.is_empty() {
                    let sample: Vec<String> = missing
                        .iter()
                        .take(5)
                        .map(|m| match m.actual_size {
                            Some(actual) => format!(
                                "{} ({} of {} bytes)",
                                m.filename, actual, m.expected_min_size
                            ),
                            None => format!("{} (missing)", m.filename),
                        })
                        .collect();
                    let suffix = if missing.len() > 5 {
                        format!(", and {} more", missing.len() - 5)
                    } else {
                        String::new()
                    };
                    parts.push(format!(
                        "{} file(s) missing or incomplete: {}{}",
                        missing.len(),
                        sample.join(", "),
                        suffix
                    ));
                }
                if !mismatched.is_empty() {
                    parts.push(format!(
                        "{} chunk(s) failed checksum verification",
                        mismatched.len()
                    ));
                }
                format!("install incomplete: {}", parts.join("; "))
            }
        }
    }
}

const VALIDATE_BUF_LEN: usize = 1024 * 1024;

/// Re-derive on-disk ground truth and compare it to the server manifest.
///
/// `install_dir` is the directory the game's files were written into (the
/// `base_path` of the download agent). `dl_info` is the server-provided
/// download information whose `manifests` carry per-chunk checksums.
///
/// This is blocking I/O — call it off the async runtime's critical path
/// (it already runs on the download manager's dedicated thread).
pub fn validate_install(dl_info: &DownloadInformation, install_dir: &Path) -> ValidationResult {
    let manifests: &std::collections::HashMap<String, Manifest> = &dl_info.manifests;

    let total_chunks: usize = manifests.values().map(|m| m.chunks.len()).sum();
    info!(
        "validating install at {}: {} manifest(s), {} chunk(s)",
        install_dir.display(),
        manifests.len(),
        total_chunks
    );

    let mut missing: Vec<MissingFile> = Vec::new();
    let mut mismatched: Vec<MismatchedChunk> = Vec::new();
    // Avoid reporting the same file once per chunk that references it.
    let mut reported_missing: BTreeSet<String> = BTreeSet::new();
    let mut checked_chunks = 0usize;

    for manifest in manifests.values() {
        for (chunk_id, chunk_data) in &manifest.chunks {
            checked_chunks += 1;

            // Step 1: every file region this chunk touches must be present
            // and long enough. A short file is the LWIW signature.
            let mut chunk_files_ok = true;
            for file in &chunk_data.files {
                let needed_end = file.start as u64 + file.length as u64;
                let path = install_dir.join(&file.filename);

                let actual_size = match std::fs::metadata(&path) {
                    Ok(meta) => Some(meta.len()),
                    Err(_) => None,
                };

                let satisfied = matches!(actual_size, Some(size) if size >= needed_end);
                if !satisfied {
                    chunk_files_ok = false;
                    if reported_missing.insert(file.filename.clone()) {
                        warn!(
                            "validation: '{}' missing or short (need >= {} bytes, found {:?})",
                            file.filename, needed_end, actual_size
                        );
                        missing.push(MissingFile {
                            filename: file.filename.clone(),
                            expected_min_size: needed_end,
                            actual_size,
                        });
                    }
                }
            }

            // If any file region is absent we cannot hash the chunk; the
            // missing-file record above already condemns the install.
            if !chunk_files_ok {
                continue;
            }

            // Step 2: re-hash the chunk's decrypted bytes from disk. The
            // downloader writes decrypted plaintext, and the manifest
            // checksum is taken over that same plaintext, so a direct read
            // (no cipher) reproduces the digest.
            match hash_chunk_from_disk(install_dir, chunk_data) {
                Ok(actual_checksum) => {
                    if actual_checksum != chunk_data.checksum {
                        error!(
                            "validation: chunk {} checksum mismatch (expected {}, got {})",
                            chunk_id, chunk_data.checksum, actual_checksum
                        );
                        mismatched.push(MismatchedChunk {
                            chunk_id: chunk_id.clone(),
                            expected_checksum: chunk_data.checksum.clone(),
                            actual_checksum,
                        });
                    }
                }
                Err(e) => {
                    // A read error after the size check passed is itself a
                    // failure — treat the chunk as corrupt rather than
                    // silently passing it.
                    error!("validation: failed to read chunk {chunk_id} from disk: {e}");
                    mismatched.push(MismatchedChunk {
                        chunk_id: chunk_id.clone(),
                        expected_checksum: chunk_data.checksum.clone(),
                        actual_checksum: format!("<unreadable: {e}>"),
                    });
                }
            }
        }
    }

    if missing.is_empty() && mismatched.is_empty() {
        info!(
            "validation passed: {checked_chunks}/{total_chunks} chunk(s) verified at {}",
            install_dir.display()
        );
        ValidationResult::Valid
    } else {
        let result = ValidationResult::Incomplete {
            missing,
            mismatched,
        };
        error!(
            "validation FAILED at {}: {}",
            install_dir.display(),
            result.describe()
        );
        result
    }
}

/// Read the file regions a chunk references, in manifest order, and return
/// the hex SHA-256 of the concatenation. Mirrors the hashing the downloader
/// performs as bytes land (`download_logic::download_game_chunk`).
fn hash_chunk_from_disk(
    install_dir: &Path,
    chunk_data: &droplet_rs::manifest::ChunkData,
) -> Result<String, std::io::Error> {
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; VALIDATE_BUF_LEN];

    for file in &chunk_data.files {
        let path = install_dir.join(&file.filename);
        let mut handle = File::open(&path)?;
        handle.seek(SeekFrom::Start(file.start as u64))?;

        let mut remaining = file.length;
        while remaining > 0 {
            let want = remaining.min(VALIDATE_BUF_LEN);
            let read = handle.read(&mut buf[0..want])?;
            if read == 0 {
                // File ended before the manifest said it should. The size
                // pre-check should have caught this; surface it loudly.
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    format!(
                        "'{}' shorter than manifest region (offset {}, len {})",
                        file.filename, file.start, file.length
                    ),
                ));
            }
            hasher.update(&buf[0..read]);
            remaining -= read;
        }
    }

    Ok(hex::encode(hasher.finalize()))
}
