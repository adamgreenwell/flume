//! Build script for the Flume Tauri application.
//!
//! `tauri_build::build()` generates the capability/ACL schemas and embeds the
//! application manifest, icons, and Windows resources.

fn main() {
    tauri_build::build()
}
