use alloc::boxed::Box;
use alloc::string::String;

use mass_spectrometry::prelude::{GenericSpectrumMutationError, SplashError};
use thiserror::Error;

/// Crate-wide result type.
pub type Result<T> = core::result::Result<T, MascotError>;

/// Errors returned while parsing and validating MGF documents.
#[derive(Debug, Error)]
pub enum MascotError {
    /// A source or target file could not be accessed.
    #[cfg(feature = "std")]
    #[error("could not access MGF file \"{path}\": {source}")]
    Io {
        /// Path that could not be accessed.
        path: String,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// A line-oriented input stream could not be read.
    #[cfg(feature = "std")]
    #[error("could not read MGF input stream: {source}")]
    InputIo {
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// A line-oriented output stream could not be written.
    #[cfg(feature = "std")]
    #[error("could not write MGF output stream: {source}")]
    OutputIo {
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// A specific input line could not be parsed or built.
    #[error("could not process MGF input line {line_number} \"{line}\": {source}")]
    InputLine {
        /// One-based input line number.
        line_number: usize,
        /// Original input line.
        line: String,
        /// Underlying parse or validation error.
        #[source]
        source: Box<Self>,
    },
    /// A remote MGF library could not be downloaded.
    #[cfg(feature = "std")]
    #[error("could not download MGF library from \"{url}\": {source}")]
    Download {
        /// URL that could not be downloaded.
        url: String,
        /// Underlying HTTP error.
        #[source]
        source: Box<ureq::Error>,
    },
    /// A Zenodo operation failed while retrieving a dataset.
    #[cfg(feature = "std")]
    #[error("Zenodo {operation} failed: {source}")]
    Zenodo {
        /// Operation being attempted.
        operation: String,
        /// Underlying Zenodo error.
        #[source]
        source: Box<zenodo_rs::ZenodoError>,
    },
    /// A `GeMS-A10` part number is outside the known published range.
    #[error("GeMS-A10 part {part} is out of range; valid parts are 0..{part_count}")]
    InvalidGemsA10Part {
        /// Requested part number.
        part: u8,
        /// Number of published MGF parts.
        part_count: u8,
    },
    /// A builder is missing a required field.
    #[error("could not build {builder}: {field} is missing")]
    MissingField {
        /// Builder or object being created.
        builder: &'static str,
        /// Missing field name.
        field: &'static str,
    },
    /// A line could not be parsed into a supported value.
    #[error("could not parse {field} from line \"{line}\"")]
    ParseField {
        /// Field being parsed.
        field: &'static str,
        /// Original input line.
        line: String,
    },
    /// A `SMILES` metadata line could not be parsed.
    #[error("could not parse SMILES from line \"{line}\": {error}")]
    InvalidSmiles {
        /// Original input line.
        line: String,
        /// Underlying SMILES parser error.
        error: smiles_parser::prelude::SmilesErrorWithSpan,
    },
    /// A `FORMULA` metadata line could not be parsed.
    #[error("could not parse molecular formula from line \"{line}\": {error}")]
    InvalidFormula {
        /// Original input line.
        line: String,
        /// Underlying molecular formula parser error.
        error: molecular_formulas::errors::ParserError,
    },
    /// A parsed molecular formula mixture could not be merged for validation.
    #[error(
        "FORMULA/SMILES validation could not merge mixture components for the {formula_source} formula {formula}: {source}"
    )]
    FormulaMixtureMerge {
        /// Side of the comparison whose mixtures could not be merged.
        formula_source: &'static str,
        /// Formula whose mixture components could not be merged.
        formula: String,
        /// Underlying molecular formula count error.
        source: molecular_formulas::errors::CountError,
    },
    /// An isotope-insensitive atom-count vector could not be computed.
    #[error(
        "FORMULA/SMILES validation could not compute the isotope-insensitive atom-count vector for the {formula_source} formula {formula}: {source}"
    )]
    FormulaAtomCountVector {
        /// Side of the comparison whose atom-count vector could not be computed.
        formula_source: &'static str,
        /// Formula whose atom-count vector could not be computed.
        formula: String,
        /// Underlying molecular formula count error.
        source: molecular_formulas::errors::CountError,
    },
    /// Parsed `FORMULA` metadata does not match the molecular formula from `SMILES`.
    #[error(
        "FORMULA/SMILES validation failed: the MGF FORMULA header is {formula}, which merges to {merged_formula}; the SMILES-derived formula is {smiles_formula}, which merges to {merged_smiles_formula}; their isotope-insensitive atom-count vectors are different"
    )]
    FormulaSmilesMismatch {
        /// Formula reported by the MGF header.
        formula: String,
        /// Header formula after merging mixture components.
        merged_formula: String,
        /// Formula calculated from the parsed SMILES.
        smiles_formula: String,
        /// SMILES-derived formula after merging mixture components.
        merged_smiles_formula: String,
    },
    /// A line provided a non-finite floating-point value.
    #[error("line \"{line}\" contains a non-finite {field}")]
    NonFiniteField {
        /// Field being parsed.
        field: &'static str,
        /// Original input line.
        line: String,
    },
    /// A line provided a zero or negative value for a strictly positive field.
    #[error("line \"{line}\" contains a zero or negative {field}; it must be strictly positive")]
    NonPositiveField {
        /// Field being parsed.
        field: &'static str,
        /// Original input line.
        line: String,
    },
    /// A line provided a value that cannot be stored in the requested precision.
    #[error(
        "line \"{line}\" contains a {field} that cannot be represented in the selected precision"
    )]
    UnrepresentablePrecisionField {
        /// Field being parsed.
        field: &'static str,
        /// Original input line.
        line: String,
    },
    /// A field appeared more than once with a different value.
    #[error("{field} was already encountered and is now different in line \"{line}\"")]
    ConflictingField {
        /// Field name.
        field: &'static str,
        /// Original input line.
        line: String,
    },
    /// Charge and ion-mode metadata point to different polarities.
    #[error(
        "charge {charge} is incompatible with ion mode {ion_mode}; positive charges require positive ion mode and negative charges require negative ion mode"
    )]
    ChargeIonModeMismatch {
        /// Charge value.
        charge: i8,
        /// Ion-mode value.
        ion_mode: &'static str,
    },
    /// The adduct-derived charge disagrees with explicit charge metadata.
    #[error(
        "ADDUCT {adduct} implies charge {adduct_charge}, but explicit CHARGE metadata reports {charge}"
    )]
    AdductChargeMismatch {
        /// Adduct header value.
        adduct: String,
        /// Charge derived from the adduct.
        adduct_charge: i8,
        /// Explicit charge metadata value.
        charge: i8,
    },
    /// The adduct-derived ion mode disagrees with explicit ion-mode metadata.
    #[error(
        "ADDUCT {adduct} implies {adduct_ion_mode} ion mode, but explicit IONMODE metadata reports {ion_mode}"
    )]
    AdductIonModeMismatch {
        /// Adduct header value.
        adduct: String,
        /// Ion mode derived from the adduct.
        adduct_ion_mode: &'static str,
        /// Explicit ion-mode metadata value.
        ion_mode: &'static str,
    },
    /// A line is not supported by the current parser.
    #[error("{parser} does not support line \"{line}\"")]
    UnsupportedLine {
        /// Parser name.
        parser: &'static str,
        /// Original input line.
        line: String,
    },
    /// A structured MGF record field was inserted through metadata alone.
    #[error(
        "{field} belongs to the owning MGF record and cannot be inserted through MascotGenericFormatMetadata from line \"{line}\""
    )]
    RecordFieldNotMetadata {
        /// Field name.
        field: &'static str,
        /// Original input line.
        line: String,
    },
    /// A line appeared before the parser was in a state that can accept it.
    #[error("line \"{line}\" appeared outside an open MGF ion section")]
    LineOutsideIonSection {
        /// Original input line.
        line: String,
    },
    /// A new ion section started before the previous one was closed.
    #[error("line \"{line}\" starts a new MGF ion section before the previous section was closed")]
    NestedIonSection {
        /// Original input line.
        line: String,
    },
    /// Input ended while an ion section was still open.
    #[error(
        "MGF input ended before the ion section opened at line {begin_line_number} was closed with END IONS"
    )]
    UnclosedIonSection {
        /// One-based line number where the unclosed section started.
        begin_line_number: usize,
    },
    /// A parsed charge value is invalid.
    #[error("invalid charge in line \"{line}\": {reason}")]
    InvalidCharge {
        /// Original input line.
        line: String,
        /// Reason the charge is invalid.
        reason: &'static str,
    },
    /// Merged scan statistics are internally inconsistent.
    #[error("merged scan statistics do not add up to the total scan count")]
    MergedScanStatisticsMismatch,
    /// Peak m/z and intensity vectors have different lengths.
    #[error("m/z and intensity vectors have different lengths: {mz_len} and {intensity_len}")]
    PeakVectorLengthMismatch {
        /// Number of m/z values.
        mz_len: usize,
        /// Number of intensity values.
        intensity_len: usize,
    },
    /// A peak vector is empty.
    #[error("the MGF record contains no usable peaks after parsing and zero-intensity filtering")]
    EmptyPeakVectors,
    /// A single-record parser received zero or multiple records.
    #[error("expected exactly one MGF record, found {found}")]
    SingleRecordExpected {
        /// Number of parsed records.
        found: usize,
    },
    /// Spectrum validation failed in the shared mass-spectrometry model.
    #[error("could not create spectrum: {0}")]
    SpectrumMutation(#[from] GenericSpectrumMutationError),
    /// SPLASH calculation failed while validating metadata.
    #[error("could not calculate SPLASH for metadata validation: {0}")]
    SplashValidation(#[from] SplashError),
    /// A `SPLASH` metadata value does not match the parsed peaks.
    #[error(
        "SPLASH validation failed: the MGF header reports {observed}, but the SPLASH calculated from the parsed peaks is {expected}"
    )]
    SplashMismatch {
        /// SPLASH reported by the MGF header.
        observed: String,
        /// SPLASH calculated from the parsed peaks.
        expected: String,
    },
    /// First-level data is incompatible with the precursor m/z.
    #[error(
        "first-level minimum m/z {first_level_min_mz:?} does not match precursor m/z {precursor_mz:?}"
    )]
    FirstLevelPrecursorMzMismatch {
        /// Precursor m/z.
        precursor_mz: f64,
        /// Minimum first-level m/z.
        first_level_min_mz: f64,
    },
    /// A validated metadata filename is empty.
    #[error("filename must not be empty")]
    EmptyFilename,
}
