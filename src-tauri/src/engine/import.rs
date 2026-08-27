//! Finding torrents that already exist in another client.
//!
//! The first-run screen's most useful offer is "you already have 47 torrents
//! in Transmission, shall I take them over" — and, crucially, taking them over
//! without downloading a byte again. That second half is what makes the offer
//! safe to accept: librqbit is handed the original save path with `overwrite`
//! set, so it hashes what is on disk and keeps everything that verifies.
//!
//! Everything here is best-effort by design. Another client's config is not an
//! API: the file may be missing, half-written, from a version that spelled the
//! key differently, or absent entirely because the user moved their library.
//! Every read therefore degrades to "I could not tell" rather than failing the
//! scan, because a first-run screen that errors because some unrelated app has
//! an odd config file is worse than one that quietly offers less.
//!
//! ## What is deliberately not imported
//!
//! The design also promises categories and seeding rules. Flume has neither —
//! no category model, no per-torrent rules — so there is nowhere to put them.
//! They are not read, rather than read and dropped on the floor, and the UI
//! does not claim to bring them across.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Which client a set of torrents came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClientKind {
    /// Transmission.
    Transmission,
    /// qBittorrent.
    QBittorrent,
    /// Deluge.
    Deluge,
}

impl ClientKind {
    /// The name to show the user.
    #[must_use]
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Transmission => "Transmission",
            Self::QBittorrent => "qBittorrent",
            Self::Deluge => "Deluge",
        }
    }
}

/// Another client found on this machine, with what can be taken from it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedClient {
    /// Which client this is.
    pub kind: ClientKind,
    /// Its name, ready to show.
    pub name: String,
    /// How many `.torrent` files are sitting in its store.
    pub torrent_count: usize,
    /// Where it saves downloads, if its config could be read.
    ///
    /// `None` means the config was missing or unreadable, not that the client
    /// has no download folder. The UI says "could not read its settings"
    /// rather than implying the folder does not exist.
    pub download_dir: Option<String>,
    /// The directory its `.torrent` files live in.
    pub torrents_dir: String,
}

/// What an import actually did.
///
/// Three numbers rather than a success flag, because all three happen in a
/// normal run and they mean different things to the user: `skipped` is a
/// torrent they already had, `failed` is one this machine could not read.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportOutcome {
    /// Torrents taken over. Their files are being verified in place.
    pub added: usize,
    /// Torrents Flume already had.
    pub skipped: usize,
    /// Files that could not be read or parsed as a torrent.
    pub failed: usize,
}

/// Where a client keeps its things, relative to the user's home directory.
struct Layout {
    /// Directory holding `.torrent` files.
    torrents: &'static [&'static str],
    /// Config file to read the download directory out of.
    config: &'static [&'static str],
}

/// Per-platform layouts for one client.
///
/// Listed rather than computed because there is no pattern to compute: each
/// client picked its own directory on each platform, and a clever scheme would
/// be a guess dressed as a rule.
fn layouts(kind: ClientKind) -> &'static [Layout] {
    match kind {
        ClientKind::Transmission => &[
            // macOS
            Layout {
                torrents: &["Library", "Application Support", "Transmission", "Torrents"],
                config: &[
                    "Library",
                    "Application Support",
                    "Transmission",
                    "settings.json",
                ],
            },
            // Linux, and Windows via %APPDATA% mapping to the same relative shape
            Layout {
                torrents: &[".config", "transmission", "torrents"],
                config: &[".config", "transmission", "settings.json"],
            },
            Layout {
                torrents: &[".config", "transmission-daemon", "torrents"],
                config: &[".config", "transmission-daemon", "settings.json"],
            },
        ],
        ClientKind::QBittorrent => &[
            // macOS
            Layout {
                torrents: &["Library", "Application Support", "qBittorrent", "BT_backup"],
                config: &[
                    "Library",
                    "Application Support",
                    "qBittorrent",
                    "qBittorrent.ini",
                ],
            },
            // Linux
            Layout {
                torrents: &[".local", "share", "qBittorrent", "BT_backup"],
                config: &[".config", "qBittorrent", "qBittorrent.conf"],
            },
            // Older Linux layout
            Layout {
                torrents: &[".local", "share", "data", "qBittorrent", "BT_backup"],
                config: &[".config", "qBittorrent", "qBittorrent.conf"],
            },
        ],
        ClientKind::Deluge => &[Layout {
            torrents: &[".config", "deluge", "state"],
            config: &[".config", "deluge", "core.conf"],
        }],
    }
}

/// Joins path components onto a root.
fn under(root: &Path, parts: &[&str]) -> PathBuf {
    let mut path = root.to_path_buf();
    for part in parts {
        path.push(part);
    }
    path
}

/// Counts `.torrent` files directly inside a directory.
///
/// Not recursive. Every client here keeps a flat store, and descending would
/// risk counting a user's own folder of downloaded `.torrent` files as though
/// it were the client's library.
fn count_torrents(dir: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };

    entries
        .flatten()
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("torrent"))
        })
        .count()
}

/// Lists the `.torrent` files in a directory, sorted for a stable order.
#[must_use]
pub fn torrent_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("torrent"))
        })
        .collect();

    // Sorted so an interrupted import resumes in the same order rather than
    // whatever order the filesystem happened to hand back.
    files.sort();
    files
}

/// Pulls the download directory out of a client's config.
///
/// Three formats, none of them a stable interface:
///
/// * Transmission writes real JSON with a `download-dir` key.
/// * qBittorrent writes an INI whose key has moved between versions, so both
///   spellings are accepted.
/// * Deluge writes a JSON body after a version header, so the first `{` is
///   found before parsing.
///
/// Any failure returns `None`. A client whose config cannot be read is still
/// worth importing from — the user can pick a folder themselves.
fn read_download_dir(kind: ClientKind, config: &Path) -> Option<String> {
    let raw = std::fs::read_to_string(config).ok()?;

    match kind {
        ClientKind::Transmission => serde_json::from_str::<serde_json::Value>(&raw)
            .ok()?
            .get("download-dir")?
            .as_str()
            .map(ToString::to_string),

        ClientKind::QBittorrent => raw.lines().find_map(|line| {
            let (key, value) = line.split_once('=')?;
            let key = key.trim();
            // `Session\DefaultSavePath` is current; `Downloads\SavePath` is
            // what older versions wrote. Both appear in the wild.
            if key.eq_ignore_ascii_case(r"Session\DefaultSavePath")
                || key.eq_ignore_ascii_case(r"Downloads\SavePath")
            {
                let value = value.trim();
                (!value.is_empty()).then(|| value.to_string())
            } else {
                None
            }
        }),

        ClientKind::Deluge => {
            // `core.conf` is two JSON objects back to back: a `{"file":1,
            // "format":1}` header, then the settings. Neither `from_str` nor
            // "parse from the first brace" handles that — the first sees
            // trailing data, the second parses the header. Reading it as a
            // stream of values and taking the one that has the key is what
            // actually works, and survives the header gaining fields.
            serde_json::Deserializer::from_str(&raw)
                .into_iter::<serde_json::Value>()
                .flatten()
                .find_map(|value| {
                    value
                        .get("download_location")?
                        .as_str()
                        .map(ToString::to_string)
                })
        }
    }
}

/// Finds other clients under a home directory.
///
/// A client counts as found only if its torrent store exists *and* holds at
/// least one `.torrent`. An installed-but-empty client has nothing to offer,
/// and listing it would make the first-run screen advertise work that does not
/// exist.
///
/// `home` is a parameter rather than read from the environment so this can be
/// tested against a fixture tree instead of whatever is on the build machine.
#[must_use]
pub fn detect(home: &Path) -> Vec<DetectedClient> {
    let mut found = Vec::new();

    for kind in [
        ClientKind::Transmission,
        ClientKind::QBittorrent,
        ClientKind::Deluge,
    ] {
        for layout in layouts(kind) {
            let torrents_dir = under(home, layout.torrents);
            let count = count_torrents(&torrents_dir);
            if count == 0 {
                continue;
            }

            found.push(DetectedClient {
                kind,
                name: kind.display_name().to_string(),
                torrent_count: count,
                download_dir: read_download_dir(kind, &under(home, layout.config)),
                torrents_dir: torrents_dir.display().to_string(),
            });
            // One layout per client: a machine that has both a current and a
            // legacy directory should not have its torrents counted twice.
            break;
        }
    }

    found
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    /// A unique empty directory to build a fake home in.
    fn tempdir() -> PathBuf {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static COUNTER: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "flume-import-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Writes a file, creating parents.
    fn write(root: &Path, parts: &[&str], body: &str) {
        let path = under(root, parts);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    }

    #[test]
    fn an_empty_home_finds_nothing() {
        assert_eq!(detect(&tempdir()), Vec::new());
    }

    #[test]
    fn transmission_is_found_with_its_download_dir() {
        let home = tempdir();
        write(
            &home,
            &[
                "Library",
                "Application Support",
                "Transmission",
                "Torrents",
                "a.torrent",
            ],
            "d",
        );
        write(
            &home,
            &[
                "Library",
                "Application Support",
                "Transmission",
                "settings.json",
            ],
            r#"{"download-dir": "/Volumes/Media/Linux", "peer-port": 51413}"#,
        );

        let found = detect(&home);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].kind, ClientKind::Transmission);
        assert_eq!(found[0].torrent_count, 1);
        assert_eq!(
            found[0].download_dir.as_deref(),
            Some("/Volumes/Media/Linux")
        );
    }

    #[test]
    fn qbittorrent_reads_either_spelling_of_the_save_path() {
        for (key, expected) in [
            (r"Session\DefaultSavePath=/Volumes/New", "/Volumes/New"),
            (r"Downloads\SavePath=/Volumes/Old", "/Volumes/Old"),
        ] {
            let home = tempdir();
            write(
                &home,
                &[
                    "Library",
                    "Application Support",
                    "qBittorrent",
                    "BT_backup",
                    "a.torrent",
                ],
                "d",
            );
            write(
                &home,
                &[
                    "Library",
                    "Application Support",
                    "qBittorrent",
                    "qBittorrent.ini",
                ],
                &format!("[BitTorrent]\n{key}\nSession\\Port=6881\n"),
            );

            let found = detect(&home);
            assert_eq!(found[0].download_dir.as_deref(), Some(expected), "{key}");
        }
    }

    #[test]
    fn deluge_skips_the_version_header_before_the_json() {
        // Deluge writes a header object, then the real one. Parsing from the
        // first `{` is what makes the file readable at all.
        let home = tempdir();
        write(&home, &[".config", "deluge", "state", "a.torrent"], "d");
        write(
            &home,
            &[".config", "deluge", "core.conf"],
            "{\n  \"file\": 1,\n  \"format\": 1\n}{\n  \"download_location\": \"/srv/dl\"\n}",
        );

        let found = detect(&home);
        assert_eq!(found[0].download_dir.as_deref(), Some("/srv/dl"));
    }

    #[test]
    fn a_client_with_no_torrents_is_not_offered() {
        // An installed but empty client has nothing to import, and listing it
        // would advertise work that does not exist.
        let home = tempdir();
        std::fs::create_dir_all(under(
            &home,
            &["Library", "Application Support", "Transmission", "Torrents"],
        ))
        .unwrap();

        assert_eq!(detect(&home), Vec::new());
    }

    #[test]
    fn an_unreadable_config_still_offers_the_torrents() {
        // The user can choose a folder themselves; losing the whole client
        // because one file is missing would be the worse trade.
        let home = tempdir();
        write(
            &home,
            &[
                "Library",
                "Application Support",
                "Transmission",
                "Torrents",
                "a.torrent",
            ],
            "d",
        );

        let found = detect(&home);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].download_dir, None);
        assert_eq!(found[0].torrent_count, 1);
    }

    #[test]
    fn malformed_config_does_not_fail_the_scan() {
        let home = tempdir();
        write(
            &home,
            &[
                "Library",
                "Application Support",
                "Transmission",
                "Torrents",
                "a.torrent",
            ],
            "d",
        );
        write(
            &home,
            &[
                "Library",
                "Application Support",
                "Transmission",
                "settings.json",
            ],
            "{ this is not json",
        );

        let found = detect(&home);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].download_dir, None);
    }

    #[test]
    fn only_torrent_files_are_counted() {
        let home = tempdir();
        let store: &[&str] = &["Library", "Application Support", "Transmission", "Torrents"];
        for name in ["a.torrent", "b.TORRENT", "notes.txt", "a.torrent.resume"] {
            write(&home, &[store, &[name]].concat(), "d");
        }

        // Case-insensitive on the extension, and nothing else counts.
        assert_eq!(detect(&home)[0].torrent_count, 2);
    }

    #[test]
    fn one_client_is_reported_once_even_with_two_layouts() {
        // A machine carrying both a current and a legacy directory should not
        // have its torrents counted twice.
        let home = tempdir();
        write(
            &home,
            &[
                "Library",
                "Application Support",
                "qBittorrent",
                "BT_backup",
                "a.torrent",
            ],
            "d",
        );
        write(
            &home,
            &[".local", "share", "qBittorrent", "BT_backup", "b.torrent"],
            "d",
        );

        let found = detect(&home);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].torrent_count, 1);
    }

    #[test]
    fn every_client_present_is_reported() {
        let home = tempdir();
        write(
            &home,
            &[
                "Library",
                "Application Support",
                "Transmission",
                "Torrents",
                "a.torrent",
            ],
            "d",
        );
        write(
            &home,
            &[
                "Library",
                "Application Support",
                "qBittorrent",
                "BT_backup",
                "b.torrent",
            ],
            "d",
        );
        write(&home, &[".config", "deluge", "state", "c.torrent"], "d");

        let kinds: Vec<_> = detect(&home).into_iter().map(|c| c.kind).collect();
        assert_eq!(
            kinds,
            vec![
                ClientKind::Transmission,
                ClientKind::QBittorrent,
                ClientKind::Deluge
            ]
        );
    }

    #[test]
    fn torrent_files_are_listed_in_a_stable_order() {
        let home = tempdir();
        let store = under(&home, &["Torrents"]);
        std::fs::create_dir_all(&store).unwrap();
        for name in ["c.torrent", "a.torrent", "b.torrent", "skip.txt"] {
            std::fs::write(store.join(name), "d").unwrap();
        }

        let files: Vec<String> = torrent_files(&store)
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();

        assert_eq!(files, vec!["a.torrent", "b.torrent", "c.torrent"]);
    }

    #[test]
    fn listing_a_missing_directory_yields_nothing_rather_than_failing() {
        assert_eq!(
            torrent_files(Path::new("/nope/not/here")),
            Vec::<PathBuf>::new()
        );
    }
}
