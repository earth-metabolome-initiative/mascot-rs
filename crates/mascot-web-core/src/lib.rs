//! Shared, platform-agnostic core for the mascot-web app and its Web Worker.
//!
//! Holds the heavy graph-building logic (MGF parsing, FLASH neighbour search,
//! Louvain and Leiden community detection, and the ForceAtlas2 layout) plus the
//! worker message types, so the same code runs on the main thread and inside the
//! dedicated worker without any UI dependencies.

pub mod layout;
pub mod message;
pub mod similarity;

use mascot_rs::prelude::{MGFVec, MascotGenericFormat};

/// Parses an MGF document, skipping malformed ion blocks.
///
/// Returns the collected records together with the number of malformed blocks
/// skipped.
#[must_use]
pub fn parse_mgf(text: &str) -> (MGFVec, usize) {
    let mut iter = MGFVec::iter_from_str(text).skipping_invalid_records();
    let mut records: Vec<MascotGenericFormat> = Vec::new();
    for record in iter.by_ref().flatten() {
        records.push(record);
    }
    let skipped = iter.skipped_records();
    (MGFVec::from(records), skipped)
}
