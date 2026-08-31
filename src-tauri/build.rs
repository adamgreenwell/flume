//! Build script for the Flume Tauri application.
//!
//! `tauri_build::build()` generates the capability/ACL schemas and embeds the
//! application manifest, icons, and Windows resources.

fn main() {
    // `usage::sender::ENDPOINT` reads this with `option_env!`, which cargo does
    // not track as a build input on its own. Without this line, setting the
    // variable on an already-built tree changes nothing: cargo sees no reason
    // to recompile, the old `None` stays baked in, and the app launches, looks
    // healthy, and silently sends nothing. A build that appears to work is a
    // worse failure than one that does not.
    println!("cargo::rerun-if-env-changed=FLUME_USAGE_ENDPOINT");

    emit_build_identity();

    tauri_build::build()
}

/// Bakes in which commit this binary was built from.
///
/// # Why not the wall-clock time of the build
///
/// It could not stay correct. This script already emits a `rerun-if-*`
/// directive, which switches off cargo's default of re-running it whenever any
/// file in the package changes — so a `SystemTime::now()` captured here would
/// be stamped once and then go stale, and a build claiming to be from a date
/// it is not is worse than one that says nothing. A commit is keyed to files
/// cargo can watch, so it stays true.
///
/// It also identifies the thing that actually matters. Every build of Flume
/// says `1.0.0`, so the version cannot distinguish yesterday's binary from
/// today's; a commit can, and answers "does this binary have the fix?" rather
/// than "when was this compiled?".
///
/// What it deliberately does not claim is whether the working tree was clean.
/// See [`build_identity`].
///
/// Emits `unknown` rather than failing when there is no git — a source tarball
/// or a vendored build is a legitimate way to build this, and a build script
/// that refuses to run without a `.git` directory would break it.
fn emit_build_identity() {
    let identity = build_identity().unwrap_or_else(|| "unknown".to_owned());
    println!("cargo::rustc-env=FLUME_BUILD_ID={identity}");
}

/// Reads the commit date, short hash, and whether the tree was modified.
fn build_identity() -> Option<String> {
    let git_dir = git(&["rev-parse", "--absolute-git-dir"])?;

    // Watch what decides the answer, so a rebuild after a commit or a checkout
    // picks up the new value and a rebuild after neither does not pay for one.
    println!("cargo::rerun-if-changed={git_dir}/HEAD");
    if let Some(head) = std::fs::read_to_string(format!("{git_dir}/HEAD"))
        .ok()
        .and_then(|head| head.strip_prefix("ref: ").map(|r| r.trim().to_owned()))
    {
        println!("cargo::rerun-if-changed={git_dir}/{head}");
    }

    let date = git(&["log", "-1", "--format=%cd", "--date=short"])?;
    let hash = git(&["rev-parse", "--short", "HEAD"])?;

    // Deliberately no "dirty tree" marker, though it was written and removed.
    // Editing a source file recompiles the crate but does not re-run this
    // script -- nothing it watches changed -- so the flag would report
    // whatever was true the last time a commit or a checkout happened to
    // trigger a run. It would say "clean" for a build made from edited
    // sources, which is the case it exists to catch, and it would say
    // "modified" for a build made after those edits were reverted. Only the
    // parts keyed to files cargo watches can be kept true, so only those are
    // reported.
    Some(format!("{date} ({hash})"))
}

/// Runs a git command, returning its trimmed stdout on success.
fn git(args: &[&str]) -> Option<String> {
    let output = std::process::Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8(output.stdout).ok()?.trim().to_owned())
}
