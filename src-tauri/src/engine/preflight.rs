//! What can be known about a torrent before agreeing to download it.
//!
//! The add flow is a review you read, not a modal you dismiss. That only works
//! if the review has something to say, so this answers the two questions worth
//! asking before 46 GB starts arriving: will it fit, and do I already have
//! some of it.

use std::path::Path;

use super::add::TorrentFile;

/// Free space on the volume containing `path`, in bytes.
///
/// Walks up to the nearest existing ancestor before asking. The save directory
/// is routinely a folder that does not exist yet — a category subfolder, a
/// freshly typed path — and the volume it *would* live on is what the question
/// is actually about.
///
/// Returns `None` when the platform will not say. A missing figure is rendered
/// as "unknown" rather than as zero, because zero free bytes is a specific and
/// alarming claim.
#[must_use]
pub fn free_bytes(path: &Path) -> Option<u64> {
    let mut probe = path;
    loop {
        if probe.exists() {
            return fs4::available_space(probe).ok();
        }
        probe = probe.parent()?;
    }
}

/// Whether each file is already sitting on disk at full length.
///
/// Length only — the bytes are not hashed. Hashing 46 GB to answer a question
/// asked before the download starts would take longer than the download, and
/// the engine verifies every piece as it arrives anyway. A same-length file
/// that turns out to be different content is caught then, and costs a re-fetch
/// of the pieces that failed rather than of the whole file.
///
/// The result is per-file and in the same order as `files`, so the caller can
/// zip it straight onto the list it already has.
#[must_use]
pub fn already_on_disk(output_folder: &Path, files: &[TorrentFile]) -> Vec<bool> {
    files
        .iter()
        .map(|file| {
            // Torrent paths are relative and always forward-slashed; joining
            // component by component keeps that correct on Windows.
            let mut candidate = output_folder.to_path_buf();
            for part in file.path.split('/') {
                candidate.push(part);
            }

            std::fs::metadata(&candidate)
                .map(|meta| meta.is_file() && meta.len() == file.length)
                .unwrap_or(false)
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn file(path: &str, length: u64) -> TorrentFile {
        TorrentFile {
            index: 0,
            path: path.to_string(),
            length,
        }
    }

    #[test]
    fn free_space_is_reported_for_a_real_directory() {
        let free = free_bytes(&std::env::temp_dir());
        assert!(
            free.is_some(),
            "the temp volume should report its free space"
        );
        assert!(free.unwrap() > 0);
    }

    #[test]
    fn free_space_walks_up_to_a_folder_that_exists() {
        // The save directory is routinely one that has not been created yet.
        let missing = std::env::temp_dir()
            .join("flume-not-created-yet")
            .join("nor-this");

        let walked = free_bytes(&missing).expect("walked up to the temp volume");
        let direct = free_bytes(&std::env::temp_dir()).expect("temp volume");

        // Compared with a tolerance, not for equality. These are two separate
        // readings of a live filesystem, and anything else on the machine can
        // allocate a block between them — which it did, and turned this into a
        // flake. What the test is actually asserting is that the walk landed on
        // the *same volume*, and a wrong volume would be off by far more than a
        // rounding of the total.
        let drift = walked.abs_diff(direct);
        assert!(
            drift < direct / 100,
            "expected the same volume: walked {walked}, direct {direct}"
        );
    }

    #[test]
    fn nothing_is_on_disk_in_an_empty_folder() {
        let dir = tempdir();
        let found = already_on_disk(&dir, &[file("a.iso", 10)]);
        assert_eq!(found, vec![false]);
    }

    #[test]
    fn a_file_of_the_right_length_counts_as_present() {
        let dir = tempdir();
        std::fs::write(dir.join("a.iso"), vec![0u8; 10]).unwrap();

        assert_eq!(already_on_disk(&dir, &[file("a.iso", 10)]), vec![true]);
    }

    #[test]
    fn a_partial_file_does_not_count() {
        // The common case this exists to catch: an interrupted download left a
        // short file behind. Calling that "already on disk" would deselect it
        // and quietly ship a truncated result.
        let dir = tempdir();
        std::fs::write(dir.join("a.iso"), vec![0u8; 4]).unwrap();

        assert_eq!(already_on_disk(&dir, &[file("a.iso", 10)]), vec![false]);
    }

    #[test]
    fn a_directory_where_a_file_should_be_does_not_count() {
        let dir = tempdir();
        std::fs::create_dir(dir.join("a.iso")).unwrap();

        assert_eq!(already_on_disk(&dir, &[file("a.iso", 10)]), vec![false]);
    }

    #[test]
    fn nested_paths_are_joined_component_by_component() {
        // Torrent paths are forward-slashed whatever the platform. Pushing the
        // whole string would create a file literally named "sub/a.iso" on
        // Windows rather than looking inside "sub".
        let dir = tempdir();
        std::fs::create_dir(dir.join("sub")).unwrap();
        std::fs::write(dir.join("sub").join("a.iso"), vec![0u8; 7]).unwrap();

        assert_eq!(already_on_disk(&dir, &[file("sub/a.iso", 7)]), vec![true]);
    }

    #[test]
    fn results_stay_in_the_order_they_were_given() {
        let dir = tempdir();
        std::fs::write(dir.join("b.iso"), vec![0u8; 2]).unwrap();

        let found = already_on_disk(
            &dir,
            &[file("a.iso", 1), file("b.iso", 2), file("c.iso", 3)],
        );
        assert_eq!(found, vec![false, true, false]);
    }

    /// A unique empty directory under the system temp dir.
    fn tempdir() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static COUNTER: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "flume-preflight-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}
