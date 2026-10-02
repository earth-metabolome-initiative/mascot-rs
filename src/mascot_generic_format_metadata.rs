use alloc::{
    boxed::Box,
    string::{String, ToString},
    vec::Vec,
};
use core::{fmt, str::FromStr};

use molecular_formulas::prelude::ChemicalFormula;

use crate::mascot_generic_format_metadata_builder::MascotGenericFormatMetadataBuilder;
use crate::numeric;
use crate::prelude::*;

const RETENTION_TIME_FIELD: &str = "retention time";

/// Ionization polarity reported for an MGF ion block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "mem_size", derive(mem_dbg::MemSize))]
#[cfg_attr(feature = "mem_dbg", derive(mem_dbg::MemDbg))]
#[cfg_attr(feature = "mem_size", mem_size(flat))]
pub enum IonMode {
    /// Positive ionization mode.
    Positive,
    /// Negative ionization mode.
    Negative,
}

impl IonMode {
    /// Returns the canonical MGF-style string for this ion mode.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Positive => "Positive",
            Self::Negative => "Negative",
        }
    }

    /// Returns whether this is positive ionization mode.
    #[must_use]
    pub const fn is_positive(self) -> bool {
        matches!(self, Self::Positive)
    }

    /// Returns whether this is negative ionization mode.
    #[must_use]
    pub const fn is_negative(self) -> bool {
        matches!(self, Self::Negative)
    }
}

impl fmt::Display for IonMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for IonMode {
    type Err = MascotError;

    fn from_str(s: &str) -> Result<Self> {
        let trimmed = s.trim();
        if trimmed.eq_ignore_ascii_case("positive")
            || trimmed.eq_ignore_ascii_case("pos")
            || trimmed == "+"
        {
            return Ok(Self::Positive);
        }
        if trimmed.eq_ignore_ascii_case("negative")
            || trimmed.eq_ignore_ascii_case("neg")
            || trimmed == "-"
        {
            return Ok(Self::Negative);
        }

        Err(MascotError::ParseField {
            field: "ion mode",
            line: s.to_string(),
        })
    }
}

/// Instrument identity parsed from MGF `SOURCE_INSTRUMENT` metadata.
///
/// Raw GNPS-style values often include non-instrument prefixes and acquisition
/// labels. Parsing keeps only the instrument or analyzer identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "mem_size", derive(mem_dbg::MemSize))]
#[cfg_attr(feature = "mem_dbg", derive(mem_dbg::MemDbg))]
#[cfg_attr(feature = "mem_size", mem_size(flat))]
pub enum Instrument {
    /// Orbitrap.
    Orbitrap,
    /// Time-of-flight.
    TimeOfFlight,
    /// Quadrupole.
    Quadrupole,
    /// Ion trap.
    IonTrap,
    /// Fourier-transform instrument.
    FourierTransform,
    /// Magnetic-sector instrument.
    MagneticSector,
    /// Present instrument metadata that is not yet normalized.
    Other,
}

impl Instrument {
    /// Returns the canonical display string for this instrument class.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Orbitrap => "Orbitrap",
            Self::TimeOfFlight => "TOF",
            Self::Quadrupole => "Quadrupole",
            Self::IonTrap => "Ion trap",
            Self::FourierTransform => "Fourier transform",
            Self::MagneticSector => "Magnetic sector",
            Self::Other => "Other",
        }
    }

    fn normalized_key(value: &str) -> String {
        let mut normalized = value.trim().to_ascii_lowercase();
        normalized.retain(|character| character.is_ascii_alphanumeric());
        normalized
    }

    pub(crate) fn from_metadata_value(value: &str) -> Self {
        match Self::normalized_key(value).as_str() {
            "orbitrap"
            | "lcesiorbitrap"
            | "esiorbitrap"
            | "diesiorbitrap"
            | "apciorbitrap"
            | "qexactiveplusorbitrapres14k"
            | "qexactiveplusorbitrapres70k"
            | "lcesiqexactiveplusorbitrapres14k"
            | "lcesiqexactiveplusorbitrapres70k"
            | "diesiqexactive" => Self::Orbitrap,
            "tof"
            | "timeofflight"
            | "esiqtof"
            | "esilcesiqtof"
            | "esilcqtofms"
            | "esiuplcesiqtof"
            | "lcesiqtof"
            | "lcesiqtofms"
            | "diesiqtof"
            | "apciqtof"
            | "lcapciqtof"
            | "maldiqtof"
            | "lcesimaxisiihdqtofbruker"
            | "lcesimaxishdqtof"
            | "maxishdqtof"
            | "lcesiimpacthd"
            | "esihplcesitof"
            | "lcesitof" => Self::TimeOfFlight,
            "quadrupole"
            | "esilcesiqq"
            | "esilcappiqq"
            | "lcesiqq"
            | "esiqqq"
            | "lcesiqqq"
            | "esiflowinjectionqqqms"
            | "diesiqqq"
            | "apciqqq"
            | "eiqqq"
            | "negativequattroqqq10ev"
            | "negativequattroqqq25ev"
            | "negativequattroqqq40ev"
            | "positivequattroqqq10ev"
            | "positivequattroqqq25ev"
            | "positivequattroqqq40ev"
            | "esilcesiq" => Self::Quadrupole,
            "iontrap" | "esiiontrap" | "esilcesiit" | "lcesiiontrap" | "diesiiontrap"
            | "apciiontrap" | "esilcesiittof" => Self::IonTrap,
            "fouriertransform" | "esiesiitft" | "esilcesiitft" | "esiapciitft" | "esihybridft"
            | "lcesihybridft" | "diesihybridft" | "esilcesiqft" | "esiesifticr"
            | "diesiltqfticr" => Self::FourierTransform,
            "magneticsector" | "esifabebeb" => Self::MagneticSector,
            _ => Self::Other,
        }
    }
}

impl fmt::Display for Instrument {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for Instrument {
    type Err = MascotError;

    fn from_str(s: &str) -> Result<Self> {
        Ok(Self::from_metadata_value(s))
    }
}

/// Metadata for one Mascot Generic Format ion block.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "mem_size", derive(mem_dbg::MemSize))]
#[cfg_attr(feature = "mem_dbg", derive(mem_dbg::MemDbg))]
pub struct MascotGenericFormatMetadata {
    feature_id: Option<String>,
    scans: Option<String>,
    level: Option<u8>,
    retention_time: Option<f64>,
    charge: Option<i8>,
    filename: Option<String>,
    smiles: Option<SmilesMetadata>,
    formula: Option<FormulaMetadata>,
    splash: Option<Box<str>>,
    ion_mode: Option<IonMode>,
    source_instrument: Option<Instrument>,
    arbitrary_metadata: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
struct SmilesMetadata(Smiles);

impl SmilesMetadata {
    const fn as_smiles(&self) -> &Smiles {
        &self.0
    }
}

#[derive(Debug, Clone)]
pub(crate) struct FormulaMetadata {
    formula: ChemicalFormula<u32, i32>,
    original: String,
}

impl FormulaMetadata {
    pub(crate) const fn new(formula: ChemicalFormula<u32, i32>, original: String) -> Self {
        Self { formula, original }
    }

    pub(crate) const fn formula(&self) -> &ChemicalFormula<u32, i32> {
        &self.formula
    }

    pub(crate) const fn original(&self) -> &str {
        self.original.as_str()
    }
}

pub(crate) fn insert_sorted_arbitrary_metadata(
    arbitrary_metadata: &mut Vec<(String, String)>,
    key: String,
    value: String,
) -> Option<String> {
    match arbitrary_metadata
        .binary_search_by(|(observed_key, _)| observed_key.as_str().cmp(key.as_str()))
    {
        Ok(index) => Some(core::mem::replace(&mut arbitrary_metadata[index].1, value)),
        Err(index) => {
            arbitrary_metadata.insert(index, (key, value));
            None
        }
    }
}

fn sorted_arbitrary_metadata(
    arbitrary_metadata: impl IntoIterator<Item = (String, String)>,
) -> Vec<(String, String)> {
    let mut sorted_metadata = Vec::new();
    for (key, value) in arbitrary_metadata {
        let _ = insert_sorted_arbitrary_metadata(&mut sorted_metadata, key, value);
    }
    sorted_metadata
}

impl MascotGenericFormatMetadata {
    /// Creates a new [`MascotGenericFormatMetadata`].
    ///
    /// Use [`Self::new_with_smiles`] when SMILES metadata is available.
    ///
    /// # Arguments
    /// * `feature_id` - The feature ID of the metadata, if present.
    /// * `level` - The MS fragmentation level, if reported.
    /// * `retention_time` - The retention time of the metadata, if present.
    /// * `charge` - The precursor charge of the metadata, if known.
    /// * `filename` - The filename of the metadata.
    ///
    /// # Returns
    /// A new [`MascotGenericFormatMetadata`].
    ///
    /// # Errors
    /// * If `level` is zero.
    /// * If `retention_time` is present but not finite and strictly positive.
    /// * If `charge` and `ion_mode` report opposite polarities.
    /// * If `filename` is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use mascot_rs::prelude::*;
    ///
    /// let feature_id = Some("1".to_string());
    /// let level = Some(2);
    /// let retention_time = Some(37.083);
    /// let charge = Some(1);
    /// let filename = Some("20220513_PMA_DBGI_01_04_003.mzML".to_string());
    ///
    /// let mascot_generic_format_metadata: MascotGenericFormatMetadata = MascotGenericFormatMetadata::new(
    ///     feature_id.clone(),
    ///     level,
    ///     retention_time,
    ///     charge,
    ///     filename.clone(),
    /// ).unwrap();
    ///
    /// assert_eq!(mascot_generic_format_metadata.feature_id(), feature_id.as_deref());
    /// assert_eq!(mascot_generic_format_metadata.level(), level);
    /// assert_eq!(mascot_generic_format_metadata.retention_time(), retention_time);
    /// assert_eq!(mascot_generic_format_metadata.charge(), charge);
    /// assert_eq!(mascot_generic_format_metadata.filename(), filename.as_deref());
    /// assert!(mascot_generic_format_metadata.smiles().is_none());
    ///
    /// assert!(
    ///     MascotGenericFormatMetadata::new(
    ///         feature_id.clone(),
    ///         Some(0),
    ///         retention_time,
    ///         charge,
    ///         filename.clone(),
    ///     ).is_err()
    /// );
    ///
    /// assert!(
    ///     MascotGenericFormatMetadata::new(
    ///         feature_id.clone(),
    ///         level,
    ///         Some(-1.0),
    ///         charge,
    ///         filename.clone(),
    ///     ).is_err()
    /// );
    ///
    /// assert!(
    ///     MascotGenericFormatMetadata::new(
    ///         feature_id.clone(),
    ///         level,
    ///         retention_time,
    ///         charge,
    ///         Some("".to_string()),
    ///     ).is_err()
    /// );
    ///
    /// ```
    pub fn new(
        feature_id: Option<String>,
        level: impl Into<Option<u8>>,
        retention_time: Option<f64>,
        charge: Option<i8>,
        filename: Option<String>,
    ) -> Result<Self> {
        Self::new_with_smiles(feature_id, level, retention_time, charge, filename, None)
    }

    /// Creates a new [`MascotGenericFormatMetadata`] with optional SMILES
    /// metadata.
    ///
    /// # Arguments
    /// * `feature_id` - The feature ID of the metadata, if present.
    /// * `level` - The MS fragmentation level, if reported.
    /// * `retention_time` - The retention time of the metadata, if present.
    /// * `charge` - The precursor charge of the metadata, if known.
    /// * `filename` - The filename of the metadata.
    /// * `smiles` - The parsed SMILES metadata, if present.
    ///
    /// # Errors
    /// * If `level` is zero.
    /// * If `retention_time` is present but not finite and strictly positive.
    /// * If `filename` is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use mascot_rs::prelude::*;
    ///
    /// let smiles: Smiles = "CCO".parse().unwrap();
    /// let metadata: MascotGenericFormatMetadata = MascotGenericFormatMetadata::new_with_smiles(
    ///     Some("1".to_string()),
    ///     2,
    ///     Some(37.083),
    ///     Some(1),
    ///     None,
    ///     Some(smiles),
    /// ).unwrap();
    ///
    /// assert_eq!(metadata.smiles().map(ToString::to_string).as_deref(), Some("CCO"));
    /// ```
    pub fn new_with_smiles(
        feature_id: Option<String>,
        level: impl Into<Option<u8>>,
        retention_time: Option<f64>,
        charge: Option<i8>,
        filename: Option<String>,
        smiles: Option<Smiles>,
    ) -> Result<Self> {
        Self::new_with_smiles_and_ion_mode(
            feature_id,
            level,
            retention_time,
            charge,
            filename,
            smiles,
            None,
        )
    }

    /// Creates a new [`MascotGenericFormatMetadata`] with optional SMILES and
    /// ion-mode metadata.
    ///
    /// # Arguments
    /// * `feature_id` - The feature ID of the metadata, if present.
    /// * `level` - The MS fragmentation level, if reported.
    /// * `retention_time` - The retention time of the metadata, if present.
    /// * `charge` - The precursor charge of the metadata, if known.
    /// * `filename` - The filename of the metadata.
    /// * `smiles` - The parsed SMILES metadata, if present.
    /// * `ion_mode` - The ionization polarity, if present.
    ///
    /// # Errors
    /// * If `level` is zero.
    /// * If `retention_time` is present but not finite and strictly positive.
    /// * If `filename` is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use mascot_rs::prelude::*;
    ///
    /// let metadata: MascotGenericFormatMetadata =
    ///     MascotGenericFormatMetadata::new_with_smiles_and_ion_mode(
    ///         Some("1".to_string()),
    ///         2,
    ///         None,
    ///         Some(1),
    ///         None,
    ///         None,
    ///         Some(IonMode::Positive),
    ///     ).unwrap();
    ///
    /// assert_eq!(metadata.ion_mode(), Some(IonMode::Positive));
    /// ```
    pub fn new_with_smiles_and_ion_mode(
        feature_id: Option<String>,
        level: impl Into<Option<u8>>,
        retention_time: Option<f64>,
        charge: Option<i8>,
        filename: Option<String>,
        smiles: Option<Smiles>,
        ion_mode: Option<IonMode>,
    ) -> Result<Self> {
        let level = level.into();
        let charge = charge.filter(|charge| *charge != 0);
        if level == Some(0) {
            return Err(MascotError::NonPositiveField {
                field: "fragmentation level",
                line: "0".to_string(),
            });
        }

        if let Some(retention_time) = retention_time {
            numeric::validate_positive_f64(
                retention_time,
                RETENTION_TIME_FIELD,
                &retention_time.to_string(),
            )?;
        }

        if let Some(filename) = &filename {
            if filename.is_empty() {
                return Err(MascotError::EmptyFilename);
            }
        }

        if let (Some(charge), Some(ion_mode)) = (charge, ion_mode) {
            if (charge.is_negative() && !ion_mode.is_negative())
                || (charge.is_positive() && !ion_mode.is_positive())
            {
                return Err(MascotError::ChargeIonModeMismatch {
                    charge,
                    ion_mode: ion_mode.as_str(),
                });
            }
        }

        Ok(Self {
            feature_id,
            scans: None,
            level,
            retention_time,
            charge,
            filename,
            smiles: smiles.map(SmilesMetadata),
            formula: None,
            splash: None,
            ion_mode,
            source_instrument: None,
            arbitrary_metadata: Vec::new(),
        })
    }

    #[must_use]
    pub(crate) fn with_formula_metadata(mut self, formula: Option<FormulaMetadata>) -> Self {
        self.formula = formula;
        self
    }

    #[must_use]
    pub(crate) fn with_splash(mut self, splash: Option<String>) -> Self {
        self.splash = splash.map(String::into_boxed_str);
        self
    }

    #[must_use]
    pub(crate) fn with_raw_arbitrary_metadata(
        mut self,
        arbitrary_metadata: Vec<(String, String)>,
    ) -> Self {
        self.arbitrary_metadata = sorted_arbitrary_metadata(arbitrary_metadata);
        self
    }

    /// Returns this metadata with normalized instrument metadata set.
    ///
    /// # Examples
    ///
    /// ```
    /// use mascot_rs::prelude::*;
    ///
    /// let metadata: MascotGenericFormatMetadata =
    ///     MascotGenericFormatMetadata::new_with_smiles_and_ion_mode(
    ///         Some("1".to_string()),
    ///         2,
    ///         None,
    ///         Some(1),
    ///         None,
    ///         None,
    ///         Some(IonMode::Positive),
    ///     ).unwrap()
    ///     .with_source_instrument(Some(
    ///         Instrument::Orbitrap,
    ///     ));
    ///
    /// assert_eq!(
    ///     metadata.source_instrument(),
    ///     Some(Instrument::Orbitrap)
    /// );
    /// ```
    #[must_use]
    pub const fn with_source_instrument(mut self, source_instrument: Option<Instrument>) -> Self {
        self.source_instrument = source_instrument;
        self
    }

    /// Returns this metadata with scan metadata set.
    ///
    /// `SCANS=-1` is treated as missing scan metadata.
    ///
    /// # Examples
    ///
    /// ```
    /// use mascot_rs::prelude::*;
    ///
    /// let metadata = MascotGenericFormatMetadata::new(
    ///     Some("feature-a".to_string()),
    ///     2,
    ///     None,
    ///     Some(1),
    ///     None,
    /// ).unwrap()
    /// .with_scans(Some("176-199".to_string()));
    ///
    /// assert_eq!(metadata.feature_id(), Some("feature-a"));
    /// assert_eq!(metadata.scans(), Some("176-199"));
    /// ```
    #[must_use]
    pub fn with_scans(mut self, scans: Option<String>) -> Self {
        self.scans = scans.and_then(|scans| {
            let scans = scans.trim();
            (!scans.is_empty() && scans != "-1").then(|| scans.to_string())
        });
        self
    }

    /// Returns this metadata with arbitrary MGF header metadata set.
    ///
    /// Known structured header keys are routed to structured metadata fields.
    /// Unknown entries are stored sorted by key. Repeated unknown keys keep the
    /// last value.
    ///
    /// # Examples
    ///
    /// ```
    /// use mascot_rs::prelude::*;
    ///
    /// let metadata: MascotGenericFormatMetadata =
    ///     MascotGenericFormatMetadata::new(
    ///         Some("1".to_string()),
    ///         2,
    ///         None,
    ///         Some(1),
    ///         None,
    ///     ).unwrap()
    ///     .with_arbitrary_metadata(vec![
    ///         ("NAME".to_string(), "Ethanol".to_string()),
    ///         ("SPECTRUMID".to_string(), "CCMSLIB00000000001".to_string()),
    ///     ]).unwrap();
    ///
    /// assert_eq!(
    ///     metadata.arbitrary_metadata_value("NAME"),
    ///     Some("Ethanol")
    /// );
    /// ```
    /// # Errors
    /// Returns an error when a known structured key cannot be parsed, conflicts
    /// with existing structured metadata, or belongs to the owning MGF record
    /// instead of metadata alone.
    pub fn with_arbitrary_metadata(
        mut self,
        arbitrary_metadata: Vec<(String, String)>,
    ) -> Result<Self> {
        self.arbitrary_metadata.clear();
        for (key, value) in arbitrary_metadata {
            self.insert_arbitrary_metadata(key, value)?;
        }
        Ok(self)
    }

    /// Inserts or replaces one MGF header metadata entry.
    ///
    /// Known structured header keys are routed to structured metadata fields.
    /// Unknown entries are kept sorted by key.
    ///
    /// # Examples
    ///
    /// ```
    /// use mascot_rs::prelude::*;
    ///
    /// let mut metadata: MascotGenericFormatMetadata =
    ///     MascotGenericFormatMetadata::new(
    ///         Some("1".to_string()),
    ///         2,
    ///         None,
    ///         Some(1),
    ///         None,
    ///     ).unwrap();
    ///
    /// assert!(metadata.insert_arbitrary_metadata("NAME", "Ethanol").unwrap());
    /// assert!(metadata.insert_arbitrary_metadata("NAME", "Updated ethanol").unwrap());
    /// assert_eq!(
    ///     metadata.arbitrary_metadata_value("NAME"),
    ///     Some("Updated ethanol")
    /// );
    /// ```
    /// # Errors
    /// Returns an error when a known structured key cannot be parsed, conflicts
    /// with existing structured metadata, or belongs to the owning MGF record
    /// instead of metadata alone.
    pub fn insert_arbitrary_metadata(
        &mut self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<bool> {
        let key = key.into();
        let value = value.into();
        let mut builder = MascotGenericFormatMetadataBuilder::<f64>::from_metadata(self);
        let stored_as_arbitrary = builder.digest_metadata_pair(&key, &value)?;
        *self = builder.build_metadata()?;
        Ok(stored_as_arbitrary)
    }

    /// Returns the feature ID of the metadata, if present.
    #[must_use]
    pub fn feature_id(&self) -> Option<&str> {
        self.feature_id.as_deref()
    }

    /// Returns the scan metadata, if present.
    #[must_use]
    pub fn scans(&self) -> Option<&str> {
        self.scans.as_deref()
    }

    /// Returns the MS fragmentation level, if reported.
    #[must_use]
    pub const fn level(&self) -> Option<u8> {
        self.level
    }

    /// Returns the retention time of the metadata.
    #[must_use]
    pub const fn retention_time(&self) -> Option<f64> {
        self.retention_time
    }

    /// Returns the precursor charge of the metadata, if known.
    #[must_use]
    pub const fn charge(&self) -> Option<i8> {
        self.charge
    }

    /// Returns the ionization polarity of the metadata, if present.
    #[must_use]
    pub const fn ion_mode(&self) -> Option<IonMode> {
        self.ion_mode
    }

    /// Returns the normalized instrument metadata, if present.
    #[must_use]
    pub const fn source_instrument(&self) -> Option<Instrument> {
        self.source_instrument
    }

    /// Returns the parsed chemical formula metadata, if present.
    #[must_use]
    pub const fn formula(&self) -> Option<&ChemicalFormula<u32, i32>> {
        match self.formula.as_ref() {
            Some(formula) => Some(formula.formula()),
            None => None,
        }
    }

    #[cfg(feature = "std")]
    pub(crate) fn formula_original(&self) -> Option<&str> {
        self.formula.as_ref().map(FormulaMetadata::original)
    }

    pub(crate) const fn formula_metadata(&self) -> Option<&FormulaMetadata> {
        self.formula.as_ref()
    }

    /// Returns the `SPLASH` metadata value, if present.
    #[must_use]
    pub fn splash(&self) -> Option<&str> {
        self.splash.as_deref()
    }

    /// Returns arbitrary MGF header metadata sorted by key.
    #[must_use]
    pub const fn arbitrary_metadata(&self) -> &[(String, String)] {
        self.arbitrary_metadata.as_slice()
    }

    /// Returns an arbitrary MGF header metadata value by key.
    #[must_use]
    pub fn arbitrary_metadata_value(&self, key: &str) -> Option<&str> {
        self.arbitrary_metadata
            .binary_search_by(|(observed_key, _)| observed_key.as_str().cmp(key))
            .ok()
            .map(|index| self.arbitrary_metadata[index].1.as_str())
    }

    /// Returns the filename of the metadata.
    #[must_use]
    pub fn filename(&self) -> Option<&str> {
        self.filename.as_deref()
    }

    /// Returns the parsed SMILES metadata.
    #[must_use]
    pub const fn smiles(&self) -> Option<&Smiles> {
        match self.smiles.as_ref() {
            Some(smiles) => Some(smiles.as_smiles()),
            None => None,
        }
    }
}

#[cfg(feature = "mem_size")]
impl mem_dbg::FlatType for SmilesMetadata {
    type Flat = mem_dbg::False;
}

#[cfg(feature = "mem_size")]
impl mem_dbg::MemSize for SmilesMetadata {
    fn mem_size_rec(
        &self,
        _flags: mem_dbg::SizeFlags,
        _refs: &mut mem_dbg::HashMap<usize, usize>,
    ) -> usize {
        core::mem::size_of::<Self>()
    }
}

#[cfg(feature = "mem_dbg")]
impl mem_dbg::MemDbgImpl for SmilesMetadata {}

#[cfg(feature = "mem_size")]
impl mem_dbg::FlatType for FormulaMetadata {
    type Flat = mem_dbg::False;
}

#[cfg(feature = "mem_size")]
impl mem_dbg::MemSize for FormulaMetadata {
    fn mem_size_rec(
        &self,
        _flags: mem_dbg::SizeFlags,
        _refs: &mut mem_dbg::HashMap<usize, usize>,
    ) -> usize {
        core::mem::size_of::<Self>()
    }
}

#[cfg(feature = "mem_dbg")]
impl mem_dbg::MemDbgImpl for FormulaMetadata {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_instrument_from_str() -> Result<()> {
        assert_eq!("LC-ESI-QQ".parse::<Instrument>()?, Instrument::Quadrupole);
        assert_eq!("unknown".parse::<Instrument>()?, Instrument::Other);
        Ok(())
    }
}
