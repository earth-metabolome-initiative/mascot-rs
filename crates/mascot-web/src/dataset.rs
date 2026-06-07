//! Loading and summarizing dropped MGF documents.
//!
//! The web app keeps the parsed records in memory as an [`MGFVec`] (f64
//! precision) and derives a lightweight [`DatasetSummary`] for display. Parsing
//! is lenient: malformed ion blocks are skipped and counted rather than
//! aborting the whole load, because real-world MGF exports often contain a few
//! bad records.

use std::collections::BTreeMap;

use mascot_rs::prelude::{MGFVec, MascotGenericFormat};

/// Parsed MGF records held by the app, at f64 precision.
pub type Records = MGFVec;

/// A counted breakdown of one categorical metadata field.
///
/// Each entry is a `(label, count)` pair, sorted by label.
pub type Breakdown = Vec<(String, usize)>;

/// A lightweight, display-oriented summary of a loaded dataset.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DatasetSummary {
    /// Number of successfully parsed records.
    pub count: usize,
    /// Number of malformed ion blocks skipped during parsing.
    pub skipped: usize,
    /// Records carrying a SMILES annotation.
    pub with_smiles: usize,
    /// Records carrying a molecular formula.
    pub with_formula: usize,
    /// Count of records per MS level.
    pub ms_levels: Breakdown,
    /// Count of records per ion mode.
    pub ion_modes: Breakdown,
    /// Count of records per precursor charge.
    pub charges: Breakdown,
    /// Count of records per source instrument.
    pub instruments: Breakdown,
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
    while let Some(item) = iter.next() {
        if let Ok(record) = item {
            records.push(record);
        }
    }
    let skipped = iter.skipped_records();
    (MGFVec::from(records), skipped)
}

/// Increments the counter for `label` in a sorted-map accumulator.
fn bump(map: &mut BTreeMap<String, usize>, label: String) {
    *map.entry(label).or_insert(0) += 1;
}

/// Flattens a sorted-map accumulator into a [`Breakdown`].
fn flatten(map: BTreeMap<String, usize>) -> Breakdown {
    map.into_iter().collect()
}

/// Derives a [`DatasetSummary`] from parsed records.
#[must_use]
pub fn summarize(records: &Records, skipped: usize) -> DatasetSummary {
    let mut ms_levels = BTreeMap::new();
    let mut ion_modes = BTreeMap::new();
    let mut charges = BTreeMap::new();
    let mut instruments = BTreeMap::new();
    let mut with_smiles = 0;
    let mut with_formula = 0;

    for record in records.iter() {
        let metadata = record.metadata();

        let level = metadata
            .level()
            .map_or_else(|| "unknown".to_string(), |level| level.to_string());
        bump(&mut ms_levels, level);

        let ion_mode = metadata
            .ion_mode()
            .map_or("unknown", |mode| mode.as_str())
            .to_string();
        bump(&mut ion_modes, ion_mode);

        let charge = metadata
            .charge()
            .map_or_else(|| "unknown".to_string(), |charge| charge.to_string());
        bump(&mut charges, charge);

        let instrument = metadata
            .source_instrument()
            .map_or("unknown", |instrument| instrument.as_str())
            .to_string();
        bump(&mut instruments, instrument);

        if metadata.smiles().is_some() {
            with_smiles += 1;
        }
        if metadata.formula().is_some() {
            with_formula += 1;
        }
    }

    DatasetSummary {
        count: records.len(),
        skipped,
        with_smiles,
        with_formula,
        ms_levels: flatten(ms_levels),
        ion_modes: flatten(ion_modes),
        charges: flatten(charges),
        instruments: flatten(instruments),
    }
}
