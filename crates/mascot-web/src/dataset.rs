//! Loading and summarizing dropped MGF documents.
//!
//! The web app keeps the parsed records in memory as an [`MGFVec`] (f64
//! precision) and derives a lightweight [`DatasetSummary`] for display. Parsing
//! is lenient: malformed ion blocks are skipped and counted rather than
//! aborting the whole load, because real-world MGF exports often contain a few
//! bad records.

use std::collections::HashSet;

use mascot_rs::prelude::{MGFVec, MascotGenericFormat, Spectrum, SpectrumFloat, SpectrumSplash};

/// Parsed MGF records held by the app, at f64 precision.
pub type Records = MGFVec;

/// A lightweight, display-oriented summary of a loaded dataset.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DatasetSummary {
    /// Number of successfully parsed records.
    pub count: usize,
    /// Number of malformed ion blocks skipped during parsing.
    pub skipped: usize,
    /// Records whose SPLASH is shared by an earlier record (likely duplicates).
    pub duplicate_splash: usize,
    /// Records whose precursor m/z is shared by an earlier record.
    pub shared_pepmass: usize,
}

/// The current state of the single dataset the app works with.
pub enum DatasetState {
    /// No file has been dropped yet.
    Empty,
    /// A file is being read and parsed.
    Loading {
        /// Name of the file being loaded.
        name: String,
    },
    /// A file has been parsed successfully.
    Loaded {
        /// Name of the loaded file.
        name: String,
        /// The parsed records.
        records: Records,
        /// A summary derived from the records.
        summary: DatasetSummary,
    },
    /// Loading or parsing failed.
    Failed {
        /// Name of the file that failed to load.
        name: String,
        /// A human-readable error description.
        error: String,
    },
}

impl DatasetState {
    /// Returns the parsed records when the dataset is loaded.
    #[must_use]
    pub const fn records(&self) -> Option<&Records> {
        match self {
            Self::Loaded { records, .. } => Some(records),
            _ => None,
        }
    }
}

/// Parses an MGF document, skipping malformed records.
///
/// Returns the collected records together with the number of malformed ion
/// blocks that were skipped.
#[must_use]
pub fn parse_mgf(text: &str) -> (Records, usize) {
    let mut iter = MGFVec::iter_from_str(text).skipping_invalid_records();
    let mut records: Vec<MascotGenericFormat> = Vec::new();
    for record in iter.by_ref().flatten() {
        records.push(record);
    }
    let skipped = iter.skipped_records();
    (MGFVec::from(records), skipped)
}

/// A stable key for a precursor m/z, rounded to 4 decimals.
fn pepmass_key(record: &MascotGenericFormat) -> i64 {
    (record.precursor_mz().to_f64() * 10_000.0).round() as i64
}

/// The SPLASH of a record, preferring the value carried in the MGF metadata and
/// otherwise computing it from the peaks.
fn splash_key(record: &MascotGenericFormat) -> Option<String> {
    record
        .metadata()
        .splash()
        .map(ToString::to_string)
        .or_else(|| record.splash().ok())
}

/// Derives a [`DatasetSummary`] from parsed records, including duplicate checks
/// on SPLASH and precursor m/z (pepmass).
#[must_use]
pub fn summarize(records: &Records, skipped: usize) -> DatasetSummary {
    let mut seen_splash: HashSet<String> = HashSet::new();
    let mut seen_pepmass: HashSet<i64> = HashSet::new();
    let mut duplicate_splash = 0;
    let mut shared_pepmass = 0;

    for record in records.iter() {
        if let Some(splash) = splash_key(record) {
            if !seen_splash.insert(splash) {
                duplicate_splash += 1;
            }
        }
        if !seen_pepmass.insert(pepmass_key(record)) {
            shared_pepmass += 1;
        }
    }

    DatasetSummary {
        count: records.len(),
        skipped,
        duplicate_splash,
        shared_pepmass,
    }
}
