//! Cloud-save storage quota: the numbers, and the warning that has to come
//! before an upload rather than after it.
//!
//! The server has enforced a per-user cap since the feature shipped and the
//! client never once read it. The only way to discover the cap existed was to
//! finish a session, have the upload rejected, and read a 413's text in a
//! modal — after the bytes had already been pushed across the wire and after
//! the moment the user could have done anything about it.
//!
//! Everything here except [`fetch_quota`] is pure so it can be tested. The
//! byte formatter deliberately mirrors `formatBytes` in the server's
//! `internal/cloudsaves/quota.ts`: the client's "1.20 GiB" and the server's
//! rejection text have to be the same number written the same way, or the two
//! read as two different problems.

use std::collections::HashMap;

use crate::error::RemoteAccessError;
use crate::requests::{RemoteRequest, generate_url, remote_request};

use super::{CloudSaveQuota, LocalSaveFile, list_cloud_saves};

/// Read the signed-in user's cloud-save usage and cap.
pub async fn fetch_quota() -> Result<CloudSaveQuota, RemoteAccessError> {
    let url = generate_url(&["/api/v1/client/saves/quota"], &[])?;
    remote_request(RemoteRequest::get(url)).await
}

/// How much of an upload the quota has room for, and what has to be left out.
pub struct QuotaPlan<'a> {
    /// The files to send, in the order the server will evaluate them.
    pub fits: Vec<&'a LocalSaveFile>,
    /// Filenames the cap has no room for.
    pub skipped: Vec<String>,
    /// What to tell the user about `skipped`, or `None` when nothing was cut.
    pub message: Option<String>,
}

impl<'a> QuotaPlan<'a> {
    /// Everything the caller handed over, untouched. Used when the numbers
    /// could not be read at all.
    fn allow_all(files: &[&'a LocalSaveFile]) -> Self {
        QuotaPlan {
            fits: files.to_vec(),
            skipped: Vec::new(),
            message: None,
        }
    }

    /// There was no room for any of it, so there is nothing to send.
    pub fn nothing_fits(&self) -> bool {
        self.fits.is_empty() && !self.skipped.is_empty()
    }
}

/// Check a set of files against the user's quota BEFORE uploading them, and
/// keep the ones that fit.
///
/// The server enforces the cap itself and returns a 413, so the point of this
/// is timing. A rejection after the upload is a notification that the backup
/// did not happen; a refusal before it is something the user can act on while
/// their progress is still the only copy.
///
/// It refuses per file, not per batch, because that is what the server does:
/// `bulk-upload.post.ts` walks the saves against a running total and stores
/// every one that still fits, pushing the rest into `errors[]`. An
/// all-or-nothing pre-flight in front of that turned a session where five
/// small saves would have been backed up and one large one rejected into a
/// session where nothing was backed up at all, which is the data exposure the
/// check was added to prevent.
///
/// Deliberately approximate, in the direction that under-warns:
///
///   * It credits the caller with the size of any cloud row holding the same
///     filename, because an upload upserts on `(gameId, userId, filename)`.
///     For a shared PC save that row can belong to another account, which the
///     listing does not say, so the credit is occasionally too generous.
///   * Anything it cannot read (quota lookup down, listing down) it treats as
///     "go ahead". Being unable to reach the server is the upload's own
///     failure to report, with its own message. Blocking here would replace a
///     clear "could not reach your server" with a quota complaint about
///     numbers that were never read.
///
/// The server's exact check is still behind all of it. A pre-flight that cries
/// wolf is worse than one that occasionally defers to the real check.
pub async fn preflight_quota<'a>(game_id: &str, files: &[&'a LocalSaveFile]) -> QuotaPlan<'a> {
    if files.is_empty() {
        return QuotaPlan::allow_all(files);
    }
    let Ok(quota) = fetch_quota().await else {
        return QuotaPlan::allow_all(files);
    };

    let replacing: HashMap<String, u64> = match list_cloud_saves(game_id).await {
        Ok(cloud) => cloud
            .iter()
            .map(|c| (c.filename.clone(), c.size.max(0) as u64))
            .collect(),
        Err(_) => HashMap::new(),
    };

    plan_within_quota(files, &replacing, quota.used_bytes, quota.limit_bytes)
}

/// Walk `files` against a running total and keep the ones that still fit.
///
/// Input order is preserved on purpose: the server evaluates the batch in the
/// order it arrives, so keeping the same order is what makes this pre-flight
/// predict the same verdicts rather than a different subset of them.
pub fn plan_within_quota<'a>(
    files: &[&'a LocalSaveFile],
    replacing: &HashMap<String, u64>,
    used_bytes: u64,
    limit_bytes: u64,
) -> QuotaPlan<'a> {
    let mut running = used_bytes;
    let mut fits: Vec<&LocalSaveFile> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();

    for file in files {
        let credit = replacing.get(&file.filename).copied().unwrap_or(0);
        let projected = projected_usage(running, credit, file.size);
        if projected > limit_bytes {
            skipped.push(file.filename.clone());
            continue;
        }
        running = projected;
        fits.push(file);
    }

    let message = if skipped.is_empty() {
        None
    } else if fits.is_empty() {
        let credit: u64 = files
            .iter()
            .map(|f| replacing.get(&f.filename).copied().unwrap_or(0))
            .sum();
        let incoming: u64 = files.iter().map(|f| f.size).sum();
        quota_warning(
            projected_usage(used_bytes, credit, incoming),
            limit_bytes,
        )
    } else {
        Some(partial_quota_warning(&skipped, limit_bytes))
    };

    QuotaPlan {
        fits,
        skipped,
        message,
    }
}

/// Format a byte count the way the server's quota errors do.
pub fn format_bytes(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * KIB;
    const GIB: u64 = 1024 * MIB;
    if bytes < KIB {
        return format!("{bytes} B");
    }
    if bytes < MIB {
        return format!("{:.1} KiB", bytes as f64 / KIB as f64);
    }
    if bytes < GIB {
        return format!("{:.1} MiB", bytes as f64 / MIB as f64);
    }
    format!("{:.2} GiB", bytes as f64 / GIB as f64)
}

/// What the user would be storing after an upload.
///
/// `replacing_bytes` is the size of the cloud rows this upload overwrites.
/// An upload is an upsert keyed on `(gameId, userId, filename)`, so re-backing
/// up a save that grew by a kilobyte costs a kilobyte, not the whole file.
/// Ignoring that is how a pre-flight check turns into a wall of warnings for
/// people who are nowhere near their cap.
pub fn projected_usage(used_bytes: u64, replacing_bytes: u64, incoming_bytes: u64) -> u64 {
    used_bytes.saturating_sub(replacing_bytes) + incoming_bytes
}

/// The message to show INSTEAD of starting an upload that cannot fit, or
/// `None` when there is room.
///
/// A cap of zero is treated as "no room", not as "unlimited": the server's
/// default is 1 GiB and a zero here means an administrator set it to zero.
pub fn quota_warning(projected_bytes: u64, limit_bytes: u64) -> Option<String> {
    if projected_bytes <= limit_bytes {
        return None;
    }
    Some(format!(
        "Backing this up would put you at {} of your {} of cloud save space, so Drop did not \
         start the upload. Your progress is still on this PC. Delete saves you no longer need \
         from a game's Cloud Saves panel, or ask whoever runs your Drop server for more space.",
        format_bytes(projected_bytes),
        format_bytes(limit_bytes)
    ))
}

/// The message for a backup that mostly worked: some saves went up, these did
/// not fit. Names the files, because "1 save did not fit" gives someone nothing
/// to decide with when a game has twenty of them.
fn partial_quota_warning(skipped: &[String], limit_bytes: u64) -> String {
    const NAMES_SHOWN: usize = 3;
    let shown: Vec<&str> = skipped
        .iter()
        .take(NAMES_SHOWN)
        .map(|s| s.as_str())
        .collect();
    let mut names = shown.join(", ");
    if skipped.len() > shown.len() {
        names.push_str(&format!(" and {} more", skipped.len() - shown.len()));
    }
    format!(
        "{} of this game's saves did not fit in your {} of cloud save space: {}. Everything else \
         was backed up, and {} still on this PC. Delete saves you no longer need from a game's \
         Cloud Saves panel, or ask whoever runs your Drop server for more space.",
        skipped.len(),
        format_bytes(limit_bytes),
        names,
        if skipped.len() == 1 { "it is" } else { "they are" },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The formatter has to agree with the server's, because both numbers can
    /// end up on screen in the same session describing the same bytes.
    #[test]
    fn bytes_are_formatted_like_the_servers_quota_errors() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.0 KiB");
        assert_eq!(format_bytes(1536), "1.5 KiB");
        assert_eq!(format_bytes(5 * 1024 * 1024), "5.0 MiB");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.00 GiB");
        assert_eq!(format_bytes(1_288_490_188), "1.20 GiB");
    }

    /// Re-uploading a save the cloud already holds costs the difference, not
    /// the whole file. Counting it twice would warn users who have plenty of
    /// room and train them to ignore the warning.
    #[test]
    fn an_overwrite_only_costs_what_it_adds() {
        let used = 100;
        assert_eq!(projected_usage(used, 40, 45), 105);
        assert_eq!(projected_usage(used, 40, 40), 100);
        assert_eq!(projected_usage(used, 0, 25), 125);
        // An administrative quota cut can leave `used` below what we think we
        // are replacing; saturating keeps that from wrapping to ~18 exabytes
        // and reporting a full disk to someone with an empty one.
        assert_eq!(projected_usage(10, 40, 5), 5);
    }

    #[test]
    fn a_fitting_upload_produces_no_warning() {
        assert!(quota_warning(0, 1024).is_none());
        assert!(quota_warning(1024, 1024).is_none(), "exactly full still fits");
    }

    #[test]
    fn an_overflowing_upload_names_the_cause_and_an_action() {
        let msg = quota_warning(2 * 1024 * 1024 * 1024, 1024 * 1024 * 1024)
            .expect("2 GiB into a 1 GiB cap must warn");
        assert!(msg.contains("2.00 GiB"));
        assert!(msg.contains("1.00 GiB"));
        assert!(msg.contains("Delete saves"), "no action named: {msg}");
        assert!(!msg.contains('\u{2014}'), "em-dash in {msg:?}");
    }

    /// Zero is a cap an administrator set, not an absence of one.
    #[test]
    fn a_zero_cap_leaves_no_room() {
        assert!(quota_warning(1, 0).is_some());
    }

    fn save(filename: &str, size: u64) -> LocalSaveFile {
        LocalSaveFile {
            filename: filename.to_string(),
            path: std::path::PathBuf::from(filename),
            save_type: "pc".to_string(),
            size,
            modified_at: 0,
            data_hash: String::new(),
        }
    }

    /// The whole point of the per-file walk: one save too big for the
    /// remaining space must not take the other five down with it. This is the
    /// case the old all-or-nothing pre-flight turned into zero backups.
    #[test]
    fn one_oversized_save_does_not_block_the_rest() {
        let files = [
            save("huge.sav", 900),
            save("a.sav", 10),
            save("b.sav", 10),
            save("c.sav", 10),
        ];
        let refs: Vec<&LocalSaveFile> = files.iter().collect();
        let plan = plan_within_quota(&refs, &HashMap::new(), 950, 1000);

        assert_eq!(
            plan.fits.iter().map(|f| f.filename.as_str()).collect::<Vec<_>>(),
            vec!["a.sav", "b.sav", "c.sav"]
        );
        assert_eq!(plan.skipped, vec!["huge.sav".to_string()]);
        assert!(!plan.nothing_fits());
        let message = plan.message.expect("a skipped file has to be reported");
        assert!(message.contains("huge.sav"), "no filename in {message:?}");
        assert!(message.contains("Delete saves"), "no action in {message:?}");
        assert!(!message.contains('\u{2014}'), "em-dash in {message:?}");
    }

    /// A running total, not one projection per file: three saves that each fit
    /// on their own but not together must not all be waved through.
    #[test]
    fn the_running_total_carries_between_files() {
        let files = [save("a.sav", 60), save("b.sav", 60), save("c.sav", 60)];
        let refs: Vec<&LocalSaveFile> = files.iter().collect();
        let plan = plan_within_quota(&refs, &HashMap::new(), 0, 100);

        assert_eq!(plan.fits.len(), 1);
        assert_eq!(plan.skipped, vec!["b.sav".to_string(), "c.sav".to_string()]);
    }

    /// Replacing a cloud row costs the difference, per file, the same way the
    /// server's `existingByFilename` credit works.
    #[test]
    fn an_overwrite_is_credited_per_file() {
        let files = [save("a.sav", 50)];
        let refs: Vec<&LocalSaveFile> = files.iter().collect();
        let replacing = HashMap::from([("a.sav".to_string(), 45u64)]);
        // Without the credit this is 150 against a 110 cap and gets refused.
        let plan = plan_within_quota(&refs, &replacing, 100, 110);

        assert_eq!(plan.fits.len(), 1, "only 5 bytes are actually being added");
        assert!(plan.message.is_none());
    }

    /// When nothing fits there is no upload to start, and the message is the
    /// whole-batch one rather than a list of every file.
    #[test]
    fn a_batch_with_no_room_at_all_is_refused_outright() {
        let files = [save("a.sav", 500), save("b.sav", 500)];
        let refs: Vec<&LocalSaveFile> = files.iter().collect();
        let plan = plan_within_quota(&refs, &HashMap::new(), 900, 1000);

        assert!(plan.fits.is_empty());
        assert!(plan.nothing_fits());
        let message = plan.message.expect("a refusal has to say why");
        assert!(message.contains("did not start the upload"), "{message:?}");
    }

    #[test]
    fn a_batch_that_fits_is_left_alone() {
        let files = [save("a.sav", 10), save("b.sav", 10)];
        let refs: Vec<&LocalSaveFile> = files.iter().collect();
        let plan = plan_within_quota(&refs, &HashMap::new(), 0, 1000);

        assert_eq!(plan.fits.len(), 2);
        assert!(plan.skipped.is_empty());
        assert!(plan.message.is_none());
        assert!(!plan.nothing_fits());
    }

    /// A long list gets three names and a count, not twenty names.
    #[test]
    fn the_partial_message_caps_how_many_files_it_names() {
        let skipped: Vec<String> = (0..7).map(|i| format!("save{i}.sav")).collect();
        let message = partial_quota_warning(&skipped, 1024);
        assert!(message.contains("save0.sav"));
        assert!(message.contains("and 4 more"), "{message:?}");
        assert!(!message.contains("save5.sav"), "{message:?}");
    }
}
