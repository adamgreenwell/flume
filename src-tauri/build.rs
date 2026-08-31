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

    tauri_build::build()
}
