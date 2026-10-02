use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use molecular_formulas::prelude::{ChemicalFormula, MolecularFormula};

use crate::mascot_generic_format_metadata::{
    insert_sorted_arbitrary_metadata, FormulaMetadata, MascotGenericFormatMetadata,
};
use crate::numeric;
use crate::prelude::*;

const PRECURSOR_MZ_FIELD: &str = "precursor m/z";
const PRECURSOR_MZ_ALIAS_TOLERANCE: f64 = 1e-4;
const FRAGMENTATION_LEVEL_FIELD: &str = "fragmentation level";
const RETENTION_TIME_FIELD: &str = "retention time";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrecursorMzSource {
    Pepmass,
    PrecursorMz,
}

enum MGFMetadataLine<'a> {
    FeatureId(&'a str),
    PrecursorMz(PrecursorMzSource, &'a str),
    MsLevel(&'a str),
    Scans(&'a str),
    Charge(&'a str),
    RetentionTime(&'a str),
    Filename(&'a str),
    Smiles(&'a str),
    Formula(&'a str),
    Splash(&'a str),
    IonMode(&'a str),
    SourceInstrument(&'a str),
    MergedScanMetadata,
}

/// Builder for metadata parsed from MGF header lines.
#[derive(Debug, Clone)]
pub struct MascotGenericFormatMetadataBuilder<P: SpectrumFloat = f64> {
    feature_id: Option<String>,
    scans: Option<String>,
    level: Option<u8>,
    precursor_mz: Option<P>,
    precursor_mz_source: Option<PrecursorMzSource>,
    precursor_mz_precision: Option<usize>,
    retention_time: Option<f64>,
    charge: Option<i8>,
    merged_scan_count: Option<usize>,
    retained_merged_scan_count: Option<usize>,
    merged_scans_removed_due_to_low_quality: Option<usize>,
    merged_scans_removed_due_to_low_cosine: Option<usize>,
    merged_total_scan_count: Option<usize>,
    filename: Option<String>,
    smiles: Option<Smiles>,
    formula: Option<FormulaMetadata>,
    splash: Option<String>,
    ion_mode: Option<IonMode>,
    source_instrument: Option<Instrument>,
    arbitrary_metadata: Vec<(String, String)>,
}

impl<P: SpectrumFloat> Default for MascotGenericFormatMetadataBuilder<P> {
    fn default() -> Self {
        Self {
            feature_id: None,
            scans: None,
            level: None,
            precursor_mz: None,
            precursor_mz_source: None,
            precursor_mz_precision: None,
            retention_time: None,
            charge: None,
            merged_scan_count: None,
            retained_merged_scan_count: None,
            merged_scans_removed_due_to_low_quality: None,
            merged_scans_removed_due_to_low_cosine: None,
            merged_total_scan_count: None,
            filename: None,
            smiles: None,
            formula: None,
            splash: None,
            ion_mode: None,
            source_instrument: None,
            arbitrary_metadata: Vec::new(),
        }
    }
}

impl<P: SpectrumFloat> MascotGenericFormatMetadataBuilder<P> {
    const fn has_merged_scan_metadata(&self) -> bool {
        self.merged_scan_count.is_some()
            || self.retained_merged_scan_count.is_some()
            || self.merged_scans_removed_due_to_low_quality.is_some()
            || self.merged_scans_removed_due_to_low_cosine.is_some()
            || self.merged_total_scan_count.is_some()
    }
}

impl<P: SpectrumFloat> MascotGenericFormatMetadataBuilder<P> {
    fn required_merged_scan_field(value: Option<usize>, field: &'static str) -> Result<usize> {
        value.ok_or(MascotError::MissingField {
            builder: "MascotGenericFormatMetadata",
            field,
        })
    }

    fn validate_merged_scan_metadata(&self) -> Result<()> {
        if !self.has_merged_scan_metadata() {
            return Ok(());
        }

        let merged_scan_count =
            Self::required_merged_scan_field(self.merged_scan_count, "merged_scan_count")?;
        let retained_merged_scan_count = Self::required_merged_scan_field(
            self.retained_merged_scan_count,
            "retained_merged_scan_count",
        )?;
        let removed_due_to_low_quality = Self::required_merged_scan_field(
            self.merged_scans_removed_due_to_low_quality,
            "merged_scans_removed_due_to_low_quality",
        )?;
        let removed_due_to_low_cosine = Self::required_merged_scan_field(
            self.merged_scans_removed_due_to_low_cosine,
            "merged_scans_removed_due_to_low_cosine",
        )?;
        let total_scan_count = Self::required_merged_scan_field(
            self.merged_total_scan_count,
            "merged_total_scan_count",
        )?;

        if retained_merged_scan_count + removed_due_to_low_quality + removed_due_to_low_cosine
            != total_scan_count
            || merged_scan_count != retained_merged_scan_count
        {
            return Err(MascotError::MergedScanStatisticsMismatch);
        }

        Ok(())
    }

    fn validate_formula_matches_smiles(&self) -> Result<()> {
        if let (Some(formula), Some(smiles)) = (&self.formula, &self.smiles) {
            let smiles_formula = ChemicalFormula::<u32, i32>::from(smiles);
            let formula_merged = formula.formula().merge_mixtures().map_err(|source| {
                MascotError::FormulaMixtureMerge {
                    formula_source: "MGF header",
                    formula: formula.original().to_string(),
                    source,
                }
            })?;
            let smiles_formula_merged = smiles_formula.merge_mixtures().map_err(|source| {
                MascotError::FormulaMixtureMerge {
                    formula_source: "SMILES-derived",
                    formula: smiles_formula.to_string(),
                    source,
                }
            })?;
            let formula_atom_counts =
                formula_merged
                    .element_count_vector::<u32>()
                    .map_err(|source| MascotError::FormulaAtomCountVector {
                        formula_source: "MGF header",
                        formula: formula_merged.to_string(),
                        source,
                    })?;
            let smiles_formula_atom_counts = smiles_formula_merged
                .element_count_vector::<u32>()
                .map_err(|source| MascotError::FormulaAtomCountVector {
                    formula_source: "SMILES-derived",
                    formula: smiles_formula_merged.to_string(),
                    source,
                })?;
            if formula_atom_counts != smiles_formula_atom_counts {
                return Err(MascotError::FormulaSmilesMismatch {
                    formula: formula.original().to_string(),
                    merged_formula: formula_merged.to_string(),
                    smiles_formula: smiles_formula.to_string(),
                    merged_smiles_formula: smiles_formula_merged.to_string(),
                });
            }
        }

        Ok(())
    }

    fn into_metadata(self) -> Result<MascotGenericFormatMetadata> {
        Ok(MascotGenericFormatMetadata::new_with_smiles_and_ion_mode(
            self.feature_id,
            self.level,
            self.retention_time,
            self.charge,
            self.filename,
            self.smiles,
            self.ion_mode,
        )?
        .with_scans(self.scans)
        .with_formula_metadata(self.formula)
        .with_splash(self.splash)
        .with_source_instrument(self.source_instrument)
        .with_raw_arbitrary_metadata(self.arbitrary_metadata))
    }

    pub(crate) fn from_metadata(metadata: &MascotGenericFormatMetadata) -> Self {
        Self {
            feature_id: metadata.feature_id().map(ToString::to_string),
            scans: metadata.scans().map(ToString::to_string),
            level: metadata.level(),
            precursor_mz: None,
            precursor_mz_source: None,
            precursor_mz_precision: None,
            retention_time: metadata.retention_time(),
            charge: metadata.charge(),
            merged_scan_count: None,
            retained_merged_scan_count: None,
            merged_scans_removed_due_to_low_quality: None,
            merged_scans_removed_due_to_low_cosine: None,
            merged_total_scan_count: None,
            filename: metadata.filename().map(ToString::to_string),
            smiles: metadata.smiles().cloned(),
            formula: metadata.formula_metadata().cloned(),
            splash: metadata.splash().map(ToString::to_string),
            ion_mode: metadata.ion_mode(),
            source_instrument: metadata.source_instrument(),
            arbitrary_metadata: metadata.arbitrary_metadata().to_vec(),
        }
    }

    pub(crate) fn build_metadata(self) -> Result<MascotGenericFormatMetadata> {
        self.validate_merged_scan_metadata()?;
        self.validate_formula_matches_smiles()?;
        self.into_metadata()
    }

    /// Builds parsed MGF metadata and the precursor m/z.
    ///
    /// # Errors
    /// Returns an error if required fields are missing or merged-scan metadata
    /// is invalid.
    pub(super) fn build(self) -> Result<(MascotGenericFormatMetadata, P)> {
        self.validate_merged_scan_metadata()?;
        self.validate_formula_matches_smiles()?;
        let precursor_mz = self.precursor_mz;
        let metadata = self.into_metadata()?;
        let precursor_mz = precursor_mz.ok_or(MascotError::MissingField {
            builder: "MascotGenericFormat",
            field: "precursor_mz",
        })?;

        Ok((metadata, precursor_mz))
    }
}

impl<P: SpectrumFloat> MascotGenericFormatMetadataBuilder<P> {
    fn set_parsed_field<T>(
        slot: &mut Option<T>,
        value: T,
        field: &'static str,
        line: &str,
        matches_existing: impl FnOnce(&T, &T) -> bool,
    ) -> Result<()> {
        match slot {
            Some(observed_value) if !matches_existing(observed_value, &value) => {
                Err(MascotError::ConflictingField {
                    field,
                    line: line.to_string(),
                })
            }
            Some(_) => Ok(()),
            None => {
                *slot = Some(value);
                Ok(())
            }
        }
    }

    fn digest_feature_id_line(&mut self, stripped: &str, line: &str) -> Result<()> {
        let stripped = stripped.trim();
        if stripped.is_empty() {
            return Err(MascotError::ParseField {
                field: "feature ID",
                line: line.to_string(),
            });
        }
        let feature_id = stripped.to_string();
        Self::set_parsed_field(
            &mut self.feature_id,
            feature_id,
            "feature_id",
            line,
            |observed, value| observed == value,
        )
    }

    fn precursor_mz_precision(value: &str) -> usize {
        let mantissa = value
            .split_once('e')
            .or_else(|| value.split_once('E'))
            .map_or(value, |(mantissa, _exponent)| mantissa);
        let mantissa = mantissa
            .strip_prefix('+')
            .or_else(|| mantissa.strip_prefix('-'))
            .unwrap_or(mantissa);

        mantissa.chars().filter(char::is_ascii_digit).count()
    }

    fn digest_precursor_mz_line(
        &mut self,
        source: PrecursorMzSource,
        stripped: &str,
        line: &str,
    ) -> Result<()> {
        let precursor_mz_value =
            stripped
                .split_whitespace()
                .next()
                .ok_or_else(|| MascotError::ParseField {
                    field: PRECURSOR_MZ_FIELD,
                    line: line.to_string(),
                })?;
        let precursor_mz = numeric::parse_positive_spectrum_float::<P>(
            precursor_mz_value,
            PRECURSOR_MZ_FIELD,
            line,
        )?;
        let precision = Self::precursor_mz_precision(precursor_mz_value);

        let Some(observed) = self.precursor_mz else {
            self.precursor_mz = Some(precursor_mz);
            self.precursor_mz_source = Some(source);
            self.precursor_mz_precision = Some(precision);
            return Ok(());
        };

        if observed.to_f64().to_bits() == precursor_mz.to_f64().to_bits() {
            if precision > self.precursor_mz_precision.unwrap_or_default() {
                self.precursor_mz_source = Some(source);
                self.precursor_mz_precision = Some(precision);
            }
            return Ok(());
        }

        if self.precursor_mz_source == Some(source)
            || (observed.to_f64() - precursor_mz.to_f64()).abs() > PRECURSOR_MZ_ALIAS_TOLERANCE
        {
            return Err(MascotError::ConflictingField {
                field: "precursor_mz",
                line: line.to_string(),
            });
        }

        if precision > self.precursor_mz_precision.unwrap_or_default() {
            self.precursor_mz = Some(precursor_mz);
            self.precursor_mz_source = Some(source);
            self.precursor_mz_precision = Some(precision);
        }

        Ok(())
    }

    fn parse_ms_level_value(stripped: &str, line: &str) -> Result<u8> {
        let stripped = stripped.trim();
        let stripped = stripped
            .strip_prefix("MS")
            .or_else(|| stripped.strip_prefix("Ms"))
            .or_else(|| stripped.strip_prefix("mS"))
            .or_else(|| stripped.strip_prefix("ms"))
            .unwrap_or(stripped);
        let level = stripped
            .parse::<u8>()
            .map_err(|_| MascotError::ParseField {
                field: FRAGMENTATION_LEVEL_FIELD,
                line: line.to_string(),
            })?;

        if level == 0 {
            return Err(MascotError::NonPositiveField {
                field: FRAGMENTATION_LEVEL_FIELD,
                line: line.to_string(),
            });
        }

        Ok(level)
    }

    fn digest_ms_level_line(&mut self, stripped: &str, line: &str) -> Result<()> {
        let level = Self::parse_ms_level_value(stripped, line)?;
        Self::set_parsed_field(&mut self.level, level, "level", line, |observed, value| {
            observed == value
        })
    }

    fn digest_scans_line(&mut self, stripped: &str, line: &str) -> Result<()> {
        let stripped = stripped.trim();
        if stripped == "-1" {
            return Ok(());
        }

        if stripped.is_empty() {
            return Err(MascotError::ParseField {
                field: "scans",
                line: line.to_string(),
            });
        }

        Self::set_parsed_field(
            &mut self.scans,
            stripped.to_string(),
            "scans",
            line,
            |observed, value| observed == value,
        )
    }

    fn parse_trailing_sign_charge(magnitude: &str, sign: i8, line: &str) -> Result<i8> {
        if magnitude.starts_with('+') || magnitude.starts_with('-') {
            return Err(MascotError::InvalidCharge {
                line: line.to_string(),
                reason: "signed magnitude is ambiguous",
            });
        }

        let magnitude = magnitude
            .parse::<u8>()
            .map_err(|_| MascotError::InvalidCharge {
                line: line.to_string(),
                reason: "could not parse charge magnitude",
            })?;
        if sign.is_positive() {
            i8::try_from(magnitude).map_err(|_| MascotError::InvalidCharge {
                line: line.to_string(),
                reason: "positive charge is out of range",
            })
        } else if magnitude == 128 {
            Ok(i8::MIN)
        } else {
            i8::try_from(magnitude)
                .map(|charge| -charge)
                .map_err(|_| MascotError::InvalidCharge {
                    line: line.to_string(),
                    reason: "negative charge is out of range",
                })
        }
    }

    fn parse_charge_value(charge: &str, line: &str) -> Result<Option<i8>> {
        let charge = if let Some(magnitude) = charge.strip_suffix('+') {
            Self::parse_trailing_sign_charge(magnitude, 1, line)?
        } else if let Some(magnitude) = charge.strip_suffix('-') {
            Self::parse_trailing_sign_charge(magnitude, -1, line)?
        } else {
            charge
                .parse::<i8>()
                .map_err(|_| MascotError::InvalidCharge {
                    line: line.to_string(),
                    reason: "could not parse charge",
                })?
        };

        Ok((charge != 0).then_some(charge))
    }

    fn digest_charge_line(&mut self, stripped: &str, line: &str) -> Result<()> {
        let Some(charge) = Self::parse_charge_value(stripped, line)? else {
            return Ok(());
        };
        self.set_charge(charge, line)
    }

    const fn ion_mode_from_charge(charge: i8) -> IonMode {
        if charge.is_negative() {
            IonMode::Negative
        } else {
            IonMode::Positive
        }
    }

    fn set_charge(&mut self, charge: i8, line: &str) -> Result<()> {
        if let Some(ion_mode) = self.ion_mode {
            if ion_mode != Self::ion_mode_from_charge(charge) {
                return Err(MascotError::ChargeIonModeMismatch {
                    charge,
                    ion_mode: ion_mode.as_str(),
                });
            }
        }

        Self::set_parsed_field(
            &mut self.charge,
            charge,
            "charge",
            line,
            |observed, value| observed == value,
        )
    }

    fn parse_adduct_magnitude(magnitude: &str, sign: i8) -> Option<i8> {
        let magnitude = magnitude.parse::<u8>().ok()?;
        if sign.is_positive() {
            i8::try_from(magnitude).ok()
        } else if magnitude == 128 {
            Some(i8::MIN)
        } else {
            i8::try_from(magnitude).ok().map(|charge| -charge)
        }
    }

    fn charge_and_ion_mode_from_adduct(adduct: &str) -> Option<(i8, IonMode)> {
        let adduct = adduct.trim();
        let (without_sign, sign, ion_mode) = if let Some(without_sign) = adduct.strip_suffix('+') {
            (without_sign, 1_i8, IonMode::Positive)
        } else {
            (adduct.strip_suffix('-')?, -1_i8, IonMode::Negative)
        };

        let charge = without_sign
            .char_indices()
            .rev()
            .find_map(|(index, character)| {
                (!character.is_ascii_digit())
                    .then_some(&without_sign[index + character.len_utf8()..])
            })
            .filter(|digits| !digits.is_empty())
            .map_or(Some(sign), |digits| {
                Self::parse_adduct_magnitude(digits, sign)
            })?;

        (charge != 0).then_some((charge, ion_mode))
    }

    fn digest_adduct_line(&mut self, adduct: &str, line: &str) -> Result<()> {
        if let Some((charge, ion_mode)) = Self::charge_and_ion_mode_from_adduct(adduct) {
            let adduct = adduct.trim();
            if let Some(observed_charge) = self.charge {
                if observed_charge != charge {
                    return Err(MascotError::AdductChargeMismatch {
                        adduct: adduct.to_string(),
                        adduct_charge: charge,
                        charge: observed_charge,
                    });
                }
            }
            if let Some(observed_ion_mode) = self.ion_mode {
                if observed_ion_mode != ion_mode {
                    return Err(MascotError::AdductIonModeMismatch {
                        adduct: adduct.to_string(),
                        adduct_ion_mode: ion_mode.as_str(),
                        ion_mode: observed_ion_mode.as_str(),
                    });
                }
            }
            self.set_charge(charge, line)?;
            self.set_ion_mode(ion_mode, line)?;
        }
        Ok(())
    }

    fn digest_retention_time_line(&mut self, stripped: &str, line: &str) -> Result<()> {
        let retention_time = numeric::parse_positive_f64(stripped, RETENTION_TIME_FIELD, line)?;
        Self::set_parsed_field(
            &mut self.retention_time,
            retention_time,
            "retention_time",
            line,
            |observed, value| observed.to_bits() == value.to_bits(),
        )
    }

    fn digest_filename_line(&mut self, stripped: &str, line: &str) -> Result<()> {
        let filename = stripped.to_string();
        Self::set_parsed_field(
            &mut self.filename,
            filename,
            "filename",
            line,
            |observed, value| observed == value,
        )
    }

    const fn missing_optional_metadata_value(stripped: &str) -> bool {
        stripped.is_empty()
            || stripped.eq_ignore_ascii_case("N/A")
            || stripped.eq_ignore_ascii_case("NA")
            || stripped.eq_ignore_ascii_case("NONE")
            || stripped.eq_ignore_ascii_case("NULL")
    }

    fn digest_smiles_line(&mut self, stripped: &str, line: &str) -> Result<()> {
        let stripped = stripped.trim();
        if Self::missing_optional_metadata_value(stripped) {
            return Ok(());
        }

        let smiles = stripped
            .parse::<Smiles>()
            .map_err(|error| MascotError::InvalidSmiles {
                line: line.to_string(),
                error,
            })?;
        Self::set_parsed_field(
            &mut self.smiles,
            smiles,
            "smiles",
            line,
            |observed, value| observed.to_string() == value.to_string(),
        )
    }

    fn digest_formula_line(&mut self, stripped: &str, line: &str) -> Result<()> {
        let stripped = stripped.trim();
        if Self::missing_optional_metadata_value(stripped) {
            return Ok(());
        }

        let formula = stripped
            .parse::<ChemicalFormula<u32, i32>>()
            .map_err(|error| MascotError::InvalidFormula {
                line: line.to_string(),
                error,
            })?;
        let formula = FormulaMetadata::new(formula, stripped.to_string());
        Self::set_parsed_field(
            &mut self.formula,
            formula,
            "formula",
            line,
            |observed, value| observed.formula() == value.formula(),
        )
    }

    fn digest_splash_line(&mut self, stripped: &str, line: &str) -> Result<()> {
        let stripped = stripped.trim();
        if Self::missing_optional_metadata_value(stripped) {
            return Ok(());
        }

        Self::set_parsed_field(
            &mut self.splash,
            stripped.to_string(),
            "splash",
            line,
            |observed, value| observed == value,
        )
    }

    fn digest_ion_mode_line(&mut self, stripped: &str, line: &str) -> Result<()> {
        let stripped = stripped.trim();
        if Self::missing_optional_metadata_value(stripped) {
            return Ok(());
        }

        let ion_mode = stripped
            .parse::<IonMode>()
            .map_err(|_| MascotError::ParseField {
                field: "ion mode",
                line: line.to_string(),
            })?;
        self.set_ion_mode(ion_mode, line)
    }

    fn set_ion_mode(&mut self, ion_mode: IonMode, line: &str) -> Result<()> {
        if let Some(charge) = self.charge {
            if Self::ion_mode_from_charge(charge) != ion_mode {
                return Err(MascotError::ChargeIonModeMismatch {
                    charge,
                    ion_mode: ion_mode.as_str(),
                });
            }
        }

        Self::set_parsed_field(
            &mut self.ion_mode,
            ion_mode,
            "ion_mode",
            line,
            |observed, value| observed == value,
        )
    }

    fn digest_source_instrument_line(&mut self, stripped: &str, line: &str) -> Result<()> {
        let stripped = stripped.trim();
        if Self::missing_optional_metadata_value(stripped)
            || stripped.eq_ignore_ascii_case("N/A-N/A")
        {
            return Ok(());
        }

        let source_instrument = Instrument::from_metadata_value(stripped);
        Self::set_parsed_field(
            &mut self.source_instrument,
            source_instrument,
            "source_instrument",
            line,
            |observed, value| observed == value,
        )
    }

    pub(super) fn digest_arbitrary_metadata_line(&mut self, line: &str) -> Result<()> {
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| Self::unsupported_merged_scan_line_error(line))?;
        if key.eq_ignore_ascii_case("ADDUCT") {
            self.digest_adduct_line(value, line)?;
        }
        let _ = insert_sorted_arbitrary_metadata(
            &mut self.arbitrary_metadata,
            key.to_string(),
            value.to_string(),
        );
        Ok(())
    }

    fn is_merged_scan_metadata_line(line: &str) -> bool {
        line.starts_with("MERGED_SCANS=") || line.starts_with("MERGED_STATS=")
    }

    fn unsupported_merged_scan_line_error(line: &str) -> MascotError {
        MascotError::UnsupportedLine {
            parser: "MascotGenericFormatMetadataBuilder",
            line: line.to_string(),
        }
    }

    fn parse_merged_scan_count(value: &str, line: &str, label: &'static str) -> Result<usize> {
        value
            .trim()
            .parse::<usize>()
            .map_err(|_| MascotError::ParseField {
                field: label,
                line: line.to_string(),
            })
    }

    fn parse_first_merged_scan_count(
        fragment: &str,
        line: &str,
        label: &'static str,
    ) -> Result<usize> {
        let value = fragment
            .split_whitespace()
            .next()
            .ok_or_else(|| Self::unsupported_merged_scan_line_error(line))?;
        Self::parse_merged_scan_count(value, line, label)
    }

    fn digest_merged_scans_line(&mut self, stripped: &str, line: &str) -> Result<()> {
        let mut scan_count = 0_usize;
        for scan in stripped.split(',') {
            scan.parse::<usize>().map_err(|_| MascotError::ParseField {
                field: "merged scan numbers",
                line: line.to_string(),
            })?;
            scan_count += 1;
        }

        if self
            .retained_merged_scan_count
            .is_some_and(|retained_count| retained_count != scan_count)
        {
            return Err(MascotError::MergedScanStatisticsMismatch);
        }
        self.merged_scan_count = Some(scan_count);
        Ok(())
    }

    fn digest_merged_stats_line(&mut self, stripped: &str) -> Result<()> {
        let (fraction, removed_scans) = stripped
            .split_once('(')
            .ok_or_else(|| Self::unsupported_merged_scan_line_error(stripped))?;
        let (scans_merged, total_scans) = fraction
            .split_once('/')
            .ok_or_else(|| Self::unsupported_merged_scan_line_error(stripped))?;
        let (low_quality, low_cosine) = removed_scans
            .split_once(',')
            .ok_or_else(|| Self::unsupported_merged_scan_line_error(stripped))?;

        let scans_merged = Self::parse_merged_scan_count(
            scans_merged,
            stripped,
            "the number of scans that were merged",
        )?;
        let total_scans =
            Self::parse_merged_scan_count(total_scans, stripped, "the total number of scans")?;
        let removed_due_to_low_quality = Self::parse_first_merged_scan_count(
            low_quality,
            stripped,
            "the number of scans that were removed due to low quality",
        )?;
        let removed_due_to_low_cosine = Self::parse_first_merged_scan_count(
            low_cosine,
            stripped,
            "the number of scans that were removed due to low cosine",
        )?;

        if scans_merged + removed_due_to_low_quality + removed_due_to_low_cosine != total_scans {
            return Err(MascotError::MergedScanStatisticsMismatch);
        }
        if self
            .merged_scan_count
            .is_some_and(|scan_count| scan_count != scans_merged)
        {
            return Err(MascotError::MergedScanStatisticsMismatch);
        }

        self.retained_merged_scan_count = Some(scans_merged);
        self.merged_scans_removed_due_to_low_quality = Some(removed_due_to_low_quality);
        self.merged_scans_removed_due_to_low_cosine = Some(removed_due_to_low_cosine);
        self.merged_total_scan_count = Some(total_scans);
        Ok(())
    }

    fn digest_merge_scans_line(&mut self, line: &str) -> Result<()> {
        if let Some(stripped) = line.strip_prefix("MERGED_SCANS=") {
            return self.digest_merged_scans_line(stripped, line);
        }

        if let Some(stripped) = line.strip_prefix("MERGED_STATS=") {
            return self.digest_merged_stats_line(stripped);
        }

        Err(Self::unsupported_merged_scan_line_error(line))
    }
}

impl<P: SpectrumFloat> MascotGenericFormatMetadataBuilder<P> {
    fn classify_line(line: &str) -> Option<MGFMetadataLine<'_>> {
        if let Some(stripped) = line.strip_prefix("FEATURE_ID=") {
            return Some(MGFMetadataLine::FeatureId(stripped));
        }

        if let Some(stripped) = line.strip_prefix("PEPMASS=") {
            return Some(MGFMetadataLine::PrecursorMz(
                PrecursorMzSource::Pepmass,
                stripped,
            ));
        }

        if let Some(stripped) = line.strip_prefix("PRECURSOR_MZ=") {
            return Some(MGFMetadataLine::PrecursorMz(
                PrecursorMzSource::PrecursorMz,
                stripped,
            ));
        }

        if let Some(stripped) = line
            .strip_prefix("MSLEVEL=")
            .or_else(|| line.strip_prefix("MS_LEVEL="))
        {
            return Some(MGFMetadataLine::MsLevel(stripped));
        }

        if let Some(stripped) = line.strip_prefix("SCANS=") {
            return Some(MGFMetadataLine::Scans(stripped));
        }

        if let Some(stripped) = line.strip_prefix("CHARGE=") {
            return Some(MGFMetadataLine::Charge(stripped));
        }

        if let Some(stripped) = line.strip_prefix("RTINSECONDS=") {
            return Some(MGFMetadataLine::RetentionTime(stripped));
        }

        if let Some(stripped) = line.strip_prefix("FILENAME=") {
            return Some(MGFMetadataLine::Filename(stripped));
        }

        if let Some(stripped) = line.strip_prefix("SMILES=") {
            return Some(MGFMetadataLine::Smiles(stripped));
        }

        if let Some(stripped) = line
            .strip_prefix("FORMULA=")
            .or_else(|| line.strip_prefix("MOLECULAR_FORMULA="))
            .or_else(|| line.strip_prefix("CHEMICAL_FORMULA="))
        {
            return Some(MGFMetadataLine::Formula(stripped));
        }

        if let Some(stripped) = line.strip_prefix("SPLASH=") {
            return Some(MGFMetadataLine::Splash(stripped));
        }

        if let Some(stripped) = line
            .strip_prefix("IONMODE=")
            .or_else(|| line.strip_prefix("ION_MODE="))
        {
            return Some(MGFMetadataLine::IonMode(stripped));
        }

        if let Some(stripped) = line
            .strip_prefix("SOURCE_INSTRUMENT=")
            .or_else(|| line.strip_prefix("INSTRUMENT_TYPE="))
        {
            return Some(MGFMetadataLine::SourceInstrument(stripped));
        }

        Self::is_merged_scan_metadata_line(line).then_some(MGFMetadataLine::MergedScanMetadata)
    }

    /// Returns whether the line can be parsed by this parser.
    ///
    /// # Arguments
    /// * `line` - The line to parse.
    ///
    /// # Examples
    /// The known-field parser should be able to parse any of the following lines:
    /// feature IDs, precursor m/z values, scan ids, charges, retention times,
    /// filenames, SMILES, ion-mode metadata, source-instrument metadata,
    /// partial-read scan markers, and merged-scan metadata lines.
    pub(super) fn can_parse_line(line: &str) -> bool {
        Self::classify_line(line).is_some()
    }

    /// Returns whether the line can be stored as arbitrary metadata.
    pub(super) fn can_parse_arbitrary_metadata_line(line: &str) -> bool {
        line.contains('=')
    }

    pub(crate) fn digest_metadata_pair(&mut self, key: &str, value: &str) -> Result<bool> {
        let line = alloc::format!("{key}={value}");
        match Self::classify_line(&line) {
            Some(MGFMetadataLine::PrecursorMz(..)) => Err(MascotError::RecordFieldNotMetadata {
                field: "precursor_mz",
                line,
            }),
            Some(_) => {
                self.digest_line(&line)?;
                Ok(false)
            }
            None => {
                self.digest_arbitrary_metadata_line(&line)?;
                Ok(true)
            }
        }
    }

    /// Parses a line to a [`MascotGenericFormatMetadataBuilder`].
    ///
    /// # Arguments
    /// * `line` - The line to parse.
    ///
    /// # Error
    /// * If feature ID was already encountered and it is now different.
    /// * If scans was already encountered and it is now different.
    /// * If `PEPMASS`/`PRECURSOR_MZ` aliases disagree beyond the precursor tolerance.
    /// * If rtinseconds was already encountered and it is now different.
    pub(super) fn digest_line(&mut self, line: &str) -> Result<()> {
        match Self::classify_line(line) {
            Some(MGFMetadataLine::FeatureId(stripped)) => {
                self.digest_feature_id_line(stripped, line)
            }
            Some(MGFMetadataLine::PrecursorMz(source, stripped)) => {
                self.digest_precursor_mz_line(source, stripped, line)
            }
            Some(MGFMetadataLine::MsLevel(stripped)) => self.digest_ms_level_line(stripped, line),
            Some(MGFMetadataLine::Scans(stripped)) => self.digest_scans_line(stripped, line),
            Some(MGFMetadataLine::Charge(stripped)) => self.digest_charge_line(stripped, line),
            Some(MGFMetadataLine::RetentionTime(stripped)) => {
                self.digest_retention_time_line(stripped, line)
            }
            Some(MGFMetadataLine::Filename(stripped)) => self.digest_filename_line(stripped, line),
            Some(MGFMetadataLine::Smiles(stripped)) => self.digest_smiles_line(stripped, line),
            Some(MGFMetadataLine::Formula(stripped)) => self.digest_formula_line(stripped, line),
            Some(MGFMetadataLine::Splash(stripped)) => self.digest_splash_line(stripped, line),
            Some(MGFMetadataLine::IonMode(stripped)) => self.digest_ion_mode_line(stripped, line),
            Some(MGFMetadataLine::SourceInstrument(stripped)) => {
                self.digest_source_instrument_line(stripped, line)
            }
            Some(MGFMetadataLine::MergedScanMetadata) => self.digest_merge_scans_line(line),
            None => Err(MascotError::UnsupportedLine {
                parser: "MascotGenericFormatMetadataBuilder",
                line: line.to_string(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;

    #[test]
    fn can_parse_expected_metadata_lines() {
        for line in [
            "FEATURE_ID=1",
            "PEPMASS=381.0795",
            "MSLEVEL=2",
            "SCANS=1",
            "CHARGE=1",
            "CHARGE=1+",
            "CHARGE=2+",
            "CHARGE=3+",
            "CHARGE=4+",
            "CHARGE=5+",
            "CHARGE=-1",
            "CHARGE=1-",
            "RTINSECONDS=37.083",
            "FILENAME=20220513_PMA_DBGI_01_04_003.mzML",
            "SMILES=CCO",
            "SMILES=N/A",
            "FORMULA=C2H6O",
            "MOLECULAR_FORMULA=C2H6O",
            "CHEMICAL_FORMULA=C2H6O",
            "SPLASH=splash10-0udi-0490000000-4425acda10ed7d4709bd",
            "IONMODE=Positive",
            "IONMODE=Negative",
            "IONMODE=N/A",
            "SOURCE_INSTRUMENT=LC-ESI-Orbitrap",
            "SOURCE_INSTRUMENT=N/A-N/A",
            "SCANS=-1",
        ] {
            assert!(MascotGenericFormatMetadataBuilder::<f64>::can_parse_line(
                line
            ));
        }
    }

    #[test]
    fn builds_metadata_from_lines() -> Result<()> {
        let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();

        parser.digest_line("FEATURE_ID=1")?;
        parser.digest_line("PEPMASS=381.0795")?;
        parser.digest_line("MSLEVEL=2")?;
        parser.digest_line("SCANS=1")?;
        parser.digest_line("CHARGE=1")?;
        parser.digest_line("MERGED_SCANS=1567,1540")?;
        parser.digest_line(
            "MERGED_STATS=2 / 2 (0 removed due to low quality, 0 removed due to low cosine).",
        )?;
        parser.digest_line("RTINSECONDS=37.083")?;
        parser.digest_line("FILENAME=20220513_PMA_DBGI_01_04_003.mzML")?;
        parser.digest_line("SMILES=CCO")?;
        parser.digest_line("FORMULA=C2H6O")?;
        parser.digest_line("SPLASH=splash10-0udi-0490000000-4425acda10ed7d4709bd")?;
        parser.digest_line("IONMODE=Positive")?;
        parser.digest_line("SOURCE_INSTRUMENT=LC-ESI-Orbitrap")?;

        let (mascot_generic_format_metadata, precursor_mz) = parser.build()?;

        assert_eq!(mascot_generic_format_metadata.feature_id(), Some("1"));
        assert_eq!(mascot_generic_format_metadata.scans(), Some("1"));
        assert_eq!(mascot_generic_format_metadata.level(), Some(2));
        assert_eq!(precursor_mz.to_bits(), 381.0795_f64.to_bits());
        assert_eq!(
            mascot_generic_format_metadata
                .retention_time()
                .map(f64::to_bits),
            Some(37.083_f64.to_bits())
        );
        assert_eq!(mascot_generic_format_metadata.charge(), Some(1));
        assert_eq!(
            mascot_generic_format_metadata.filename(),
            Some("20220513_PMA_DBGI_01_04_003.mzML")
        );
        assert_eq!(
            mascot_generic_format_metadata
                .smiles()
                .map(ToString::to_string)
                .as_deref(),
            Some("CCO")
        );
        assert!(mascot_generic_format_metadata.formula().is_some());
        assert_eq!(
            mascot_generic_format_metadata.splash(),
            Some("splash10-0udi-0490000000-4425acda10ed7d4709bd")
        );
        assert_eq!(
            mascot_generic_format_metadata.ion_mode(),
            Some(IonMode::Positive)
        );
        assert_eq!(
            mascot_generic_format_metadata.source_instrument(),
            Some(Instrument::Orbitrap)
        );
        Ok(())
    }

    #[test]
    fn stores_arbitrary_metadata_sorted_by_key() -> Result<()> {
        let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();

        parser.digest_line("FEATURE_ID=1")?;
        parser.digest_line("PEPMASS=381.0795")?;
        parser.digest_line("MSLEVEL=2")?;
        parser.digest_line("SCANS=1")?;
        parser.digest_line("CHARGE=1")?;
        parser.digest_arbitrary_metadata_line("SPECTRUMID=CCMSLIB00000000001")?;
        parser.digest_arbitrary_metadata_line("NAME=Old name")?;
        parser.digest_arbitrary_metadata_line("NAME=New name")?;

        let (mascot_generic_format_metadata, _precursor_mz) = parser.build()?;

        assert_eq!(
            mascot_generic_format_metadata.arbitrary_metadata(),
            &[
                ("NAME".to_string(), "New name".to_string()),
                ("SPECTRUMID".to_string(), "CCMSLIB00000000001".to_string(),),
            ]
        );
        assert_eq!(
            mascot_generic_format_metadata.arbitrary_metadata_value("NAME"),
            Some("New name")
        );
        assert_eq!(
            mascot_generic_format_metadata.arbitrary_metadata_value("UNKNOWN"),
            None
        );
        Ok(())
    }

    #[test]
    fn rejects_empty_feature_id_and_precursor_values() {
        let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();
        assert!(matches!(
            parser.digest_line("FEATURE_ID= "),
            Err(MascotError::ParseField {
                field: "feature ID",
                ..
            })
        ));

        let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();
        assert!(matches!(
            parser.digest_precursor_mz_line(PrecursorMzSource::Pepmass, " ", "PEPMASS= "),
            Err(MascotError::ParseField {
                field: "precursor m/z",
                ..
            })
        ));
    }

    #[test]
    fn tracks_precursor_mz_precision_with_signs_and_exponents() {
        assert_eq!(
            MascotGenericFormatMetadataBuilder::<f64>::precursor_mz_precision("+1.230e2"),
            4
        );
        assert_eq!(
            MascotGenericFormatMetadataBuilder::<f64>::precursor_mz_precision("-12.30E-1"),
            4
        );
    }

    #[test]
    fn updates_precursor_source_when_repeated_value_has_more_precision() -> Result<()> {
        let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();

        parser.digest_precursor_mz_line(PrecursorMzSource::Pepmass, "1.0", "PEPMASS=1.0")?;
        parser.digest_precursor_mz_line(
            PrecursorMzSource::PrecursorMz,
            "1.0000",
            "PRECURSOR_MZ=1.0000",
        )?;

        assert_eq!(
            parser.precursor_mz_source,
            Some(PrecursorMzSource::PrecursorMz)
        );
        assert_eq!(parser.precursor_mz_precision, Some(5));

        Ok(())
    }

    #[test]
    fn rejects_invalid_ms_level_values() {
        let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();
        assert!(matches!(
            parser.digest_line("MSLEVEL=MSx"),
            Err(MascotError::ParseField {
                field: "fragmentation level",
                ..
            })
        ));

        let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();
        assert!(matches!(
            parser.digest_line("MSLEVEL=0"),
            Err(MascotError::NonPositiveField {
                field: "fragmentation level",
                ..
            })
        ));
    }

    #[test]
    fn ignores_missing_optional_metadata_markers() -> Result<()> {
        let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();

        parser.digest_line("SCANS=-1")?;
        parser.digest_line("IONMODE=N/A")?;
        parser.digest_line("SOURCE_INSTRUMENT=N/A-N/A")?;

        assert_eq!(parser.scans, None);
        assert_eq!(parser.ion_mode, None);
        assert_eq!(parser.source_instrument, None);
        Ok(())
    }

    #[test]
    fn rejects_invalid_ion_mode_values() {
        let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();

        assert!(matches!(
            parser.digest_line("IONMODE=sideways"),
            Err(MascotError::ParseField {
                field: "ion mode",
                ..
            })
        ));
    }

    #[test]
    fn rejects_edge_case_charge_syntax() {
        for line in ["CHARGE=+1+", "CHARGE=abc+", "CHARGE=128+", "CHARGE=129-"] {
            let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();
            assert!(
                matches!(
                    parser.digest_line(line),
                    Err(MascotError::InvalidCharge { .. })
                ),
                "{line}"
            );
        }

        let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();
        assert!(matches!(
            parser.digest_line("CHARGE=not-a-charge"),
            Err(MascotError::InvalidCharge { .. })
        ));
    }

    #[test]
    fn rejects_malformed_merged_scan_metadata() {
        let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();
        assert!(matches!(
            parser.digest_arbitrary_metadata_line("MERGED_SCANS"),
            Err(MascotError::UnsupportedLine { .. })
        ));

        let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();
        assert!(matches!(
            parser.digest_line("MERGED_SCANS=1,a"),
            Err(MascotError::ParseField {
                field: "merged scan numbers",
                ..
            })
        ));

        let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();
        assert!(matches!(
            parser.digest_line(
                "MERGED_STATS=not / 2 (0 removed due to low quality, 0 removed due to low cosine)."
            ),
            Err(MascotError::ParseField { .. } | MascotError::UnsupportedLine { .. })
        ));
    }

    #[test]
    fn digest_metadata_pair_reports_whether_value_is_arbitrary() -> Result<()> {
        let metadata =
            MascotGenericFormatMetadata::new(Some("1".to_string()), 2, None, Some(1), None)?;
        let mut parser = MascotGenericFormatMetadataBuilder::<f64>::from_metadata(&metadata);

        assert!(!parser.digest_metadata_pair("IONMODE", "positive")?);
        assert!(parser.digest_metadata_pair("NAME", "Ethanol")?);
        assert!(matches!(
            parser.digest_metadata_pair("PEPMASS", "250.0"),
            Err(MascotError::RecordFieldNotMetadata { .. })
        ));

        Ok(())
    }

    #[test]
    fn builds_metadata_without_feature_id() -> Result<()> {
        let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();

        parser.digest_line("PEPMASS=381.0795")?;
        parser.digest_line("MSLEVEL=2")?;
        parser.digest_line("SCANS=-1")?;
        parser.digest_line("CHARGE=1")?;

        let (mascot_generic_format_metadata, precursor_mz) = parser.build()?;

        assert_eq!(mascot_generic_format_metadata.feature_id(), None);
        assert_eq!(mascot_generic_format_metadata.scans(), None);
        assert_eq!(mascot_generic_format_metadata.level(), Some(2));
        assert_eq!(precursor_mz.to_bits(), 381.0795_f64.to_bits());
        assert_eq!(mascot_generic_format_metadata.charge(), Some(1));
        Ok(())
    }

    #[test]
    fn builds_metadata_without_charge() -> Result<()> {
        let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();

        parser.digest_line("PEPMASS=381.0795")?;
        parser.digest_line("MSLEVEL=2")?;

        let (mascot_generic_format_metadata, precursor_mz) = parser.build()?;

        assert_eq!(mascot_generic_format_metadata.level(), Some(2));
        assert_eq!(precursor_mz.to_bits(), 381.0795_f64.to_bits());
        assert_eq!(mascot_generic_format_metadata.charge(), None);
        Ok(())
    }

    #[test]
    fn builds_metadata_without_level() -> Result<()> {
        let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();

        parser.digest_line("PEPMASS=381.0795")?;
        parser.digest_line("CHARGE=1")?;

        let (mascot_generic_format_metadata, precursor_mz) = parser.build()?;

        assert_eq!(mascot_generic_format_metadata.level(), None);
        assert_eq!(precursor_mz.to_bits(), 381.0795_f64.to_bits());
        assert_eq!(mascot_generic_format_metadata.charge(), Some(1));
        Ok(())
    }

    #[test]
    fn build_returns_precursor_mz_in_requested_precision() -> Result<()> {
        let mut parser = MascotGenericFormatMetadataBuilder::<f32>::default();

        parser.digest_line("FEATURE_ID=1")?;
        parser.digest_line("PEPMASS=381.0795")?;
        parser.digest_line("MSLEVEL=2")?;
        parser.digest_line("SCANS=1")?;
        parser.digest_line("CHARGE=1")?;

        let (_mascot_generic_format_metadata, precursor_mz) = parser.build()?;

        assert_eq!(precursor_mz.to_bits(), 381.0795_f32.to_bits());
        Ok(())
    }

    #[test]
    fn partial_merged_scan_metadata_prevents_building() -> Result<()> {
        let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();

        parser.digest_line("FEATURE_ID=1")?;
        parser.digest_line("PEPMASS=381.0795")?;
        parser.digest_line("MSLEVEL=2")?;
        parser.digest_line("SCANS=1")?;
        parser.digest_line("CHARGE=1")?;
        parser.digest_line("RTINSECONDS=37.083")?;
        parser.digest_line("MERGED_SCANS=1567,1540")?;

        assert!(matches!(
            parser.build(),
            Err(MascotError::MissingField {
                field: "retained_merged_scan_count",
                ..
            })
        ));
        Ok(())
    }

    #[test]
    fn rejects_mismatched_merged_scan_metadata() -> Result<()> {
        let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();
        parser.digest_line("MERGED_SCANS=1567,1540")?;

        assert!(matches!(
            parser.digest_line(
                "MERGED_STATS=1 / 1 (0 removed due to low quality, 0 removed due to low cosine)."
            ),
            Err(MascotError::MergedScanStatisticsMismatch)
        ));
        Ok(())
    }

    #[test]
    fn rejects_conflicting_feature_id() -> Result<()> {
        let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();
        parser.digest_line("FEATURE_ID=1")?;
        assert!(parser.digest_line("FEATURE_ID=2").is_err());
        Ok(())
    }

    #[test]
    fn rejects_conflicting_scan_id() -> Result<()> {
        let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();
        parser.digest_line("SCANS=1")?;
        assert!(parser.digest_line("SCANS=2").is_err());
        Ok(())
    }

    #[test]
    fn accepts_distinct_feature_id_and_scan_metadata() -> Result<()> {
        let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();
        parser.digest_line("FEATURE_ID=feature-a")?;
        parser.digest_line("SCANS=176-199")?;
        parser.digest_line("PEPMASS=381.0795")?;
        parser.digest_line("MSLEVEL=2")?;
        parser.digest_line("CHARGE=1")?;

        let (metadata, _precursor_mz) = parser.build()?;

        assert_eq!(metadata.feature_id(), Some("feature-a"));
        assert_eq!(metadata.scans(), Some("176-199"));
        Ok(())
    }

    #[test]
    fn rejects_conflicting_precursor_mz() -> Result<()> {
        let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();
        parser.digest_line("PEPMASS=381.0795")?;
        assert!(parser.digest_line("PEPMASS=381.0796").is_err());
        Ok(())
    }

    #[test]
    fn rejects_same_precursor_field_with_rounded_difference() -> Result<()> {
        let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();
        parser.digest_line("PEPMASS=288.12249755859375")?;
        assert!(parser.digest_line("PEPMASS=288.1225").is_err());
        Ok(())
    }

    #[test]
    fn accepts_rounded_precursor_aliases_and_keeps_higher_precision() -> Result<()> {
        let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();
        parser.digest_line("PRECURSOR_MZ=288.1225")?;
        parser.digest_line("PEPMASS=288.12249755859375")?;
        parser.digest_line("MSLEVEL=2")?;

        let (_metadata, precursor_mz) = parser.build()?;

        assert_eq!(precursor_mz.to_bits(), 288.122_497_558_593_75_f64.to_bits());
        Ok(())
    }

    #[test]
    fn accepts_rounded_precursor_aliases_when_higher_precision_is_first() -> Result<()> {
        let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();
        parser.digest_line("PEPMASS=288.12249755859375")?;
        parser.digest_line("PRECURSOR_MZ=288.1225")?;
        parser.digest_line("MSLEVEL=2")?;

        let (_metadata, precursor_mz) = parser.build()?;

        assert_eq!(precursor_mz.to_bits(), 288.122_497_558_593_75_f64.to_bits());
        Ok(())
    }

    #[test]
    fn rejects_precursor_aliases_that_differ_beyond_tolerance() -> Result<()> {
        let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();
        parser.digest_line("PEPMASS=381.0795")?;
        assert!(parser.digest_line("PRECURSOR_MZ=381.0800").is_err());
        Ok(())
    }

    #[test]
    fn rejects_conflicting_retention_time() -> Result<()> {
        let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();
        parser.digest_line("RTINSECONDS=37.083")?;
        assert!(parser.digest_line("RTINSECONDS=37.084").is_err());
        Ok(())
    }

    #[test]
    fn rejects_conflicting_charge() -> Result<()> {
        let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();
        parser.digest_line("CHARGE=1")?;
        assert!(parser.digest_line("CHARGE=2").is_err());
        Ok(())
    }

    #[test]
    fn derives_charge_and_ion_mode_from_adduct() -> Result<()> {
        for (adduct, expected_charge, expected_ion_mode) in [
            ("[M+H]+", 1, IonMode::Positive),
            ("[M+2H]2+", 2, IonMode::Positive),
            ("[M-H]-", -1, IonMode::Negative),
            ("[M-2H]2-", -2, IonMode::Negative),
        ] {
            let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();
            parser.digest_arbitrary_metadata_line(&format!("ADDUCT={adduct}"))?;

            assert_eq!(parser.charge, Some(expected_charge), "{adduct}");
            assert_eq!(parser.ion_mode, Some(expected_ion_mode), "{adduct}");
        }

        Ok(())
    }

    #[test]
    fn ignores_adducts_without_parseable_charge() -> Result<()> {
        for adduct in ["N/A", "not-a-standard-adduct", "[M+H]"] {
            let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();
            parser.digest_arbitrary_metadata_line(&format!("ADDUCT={adduct}"))?;

            assert_eq!(parser.charge, None, "{adduct}");
            assert_eq!(parser.ion_mode, None, "{adduct}");
        }

        Ok(())
    }

    #[test]
    fn rejects_adduct_charge_conflicts() -> Result<()> {
        let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();
        parser.digest_line("CHARGE=1")?;

        assert!(matches!(
            parser.digest_arbitrary_metadata_line("ADDUCT=[M-H]-"),
            Err(MascotError::AdductChargeMismatch {
                adduct_charge: -1,
                charge: 1,
                ..
            })
        ));

        Ok(())
    }

    #[test]
    fn rejects_adduct_ion_mode_conflicts() -> Result<()> {
        let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();
        parser.digest_line("IONMODE=Positive")?;

        assert!(matches!(
            parser.digest_arbitrary_metadata_line("ADDUCT=[M-H]-"),
            Err(MascotError::AdductIonModeMismatch {
                adduct_ion_mode: "Negative",
                ion_mode: "Positive",
                ..
            })
        ));

        Ok(())
    }

    #[test]
    fn rejects_charge_ion_mode_conflicts() -> Result<()> {
        let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();
        parser.digest_line("CHARGE=1")?;
        assert!(matches!(
            parser.digest_line("IONMODE=Negative"),
            Err(MascotError::ChargeIonModeMismatch {
                charge: 1,
                ion_mode: "Negative",
            })
        ));

        let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();
        parser.digest_line("IONMODE=Negative")?;
        assert!(matches!(
            parser.digest_line("CHARGE=1"),
            Err(MascotError::ChargeIonModeMismatch {
                charge: 1,
                ion_mode: "Negative",
            })
        ));

        Ok(())
    }

    #[test]
    fn treats_zero_charge_as_missing() -> Result<()> {
        let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();

        parser.digest_line("CHARGE=0")?;
        assert_eq!(parser.charge, None);
        parser.digest_line("CHARGE=1")?;
        parser.digest_line("CHARGE=0+")?;
        parser.digest_line("CHARGE=0-")?;

        assert_eq!(parser.charge, Some(1));
        Ok(())
    }

    #[test]
    fn rejects_conflicting_smiles() -> Result<()> {
        let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();
        parser.digest_line("SMILES=CCO")?;
        assert!(parser.digest_line("SMILES=CCC").is_err());
        Ok(())
    }

    #[test]
    fn rejects_conflicting_formula() -> Result<()> {
        let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();
        parser.digest_line("FORMULA=C2H6O")?;
        assert!(parser.digest_line("FORMULA=C3H8O").is_err());
        Ok(())
    }

    #[test]
    fn rejects_conflicting_splash() -> Result<()> {
        let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();
        parser.digest_line("SPLASH=splash10-0udi-0490000000-4425acda10ed7d4709bd")?;
        assert!(parser
            .digest_line("SPLASH=splash10-0000-0000000000-00000000000000000000")
            .is_err());
        Ok(())
    }

    #[test]
    fn rejects_formula_smiles_mismatch_on_build() -> Result<()> {
        let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();
        parser.digest_line("PEPMASS=381.0795")?;
        parser.digest_line("MSLEVEL=2")?;
        parser.digest_line("CHARGE=1")?;
        parser.digest_line("SMILES=CCO")?;
        parser.digest_line("FORMULA=C3H8O")?;

        assert!(matches!(
            parser.build(),
            Err(MascotError::FormulaSmilesMismatch { .. })
        ));
        Ok(())
    }

    #[test]
    fn rejects_conflicting_ion_mode() -> Result<()> {
        let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();
        parser.digest_line("IONMODE=Positive")?;
        assert!(parser.digest_line("IONMODE=Negative").is_err());
        Ok(())
    }

    #[test]
    fn rejects_conflicting_source_instrument() -> Result<()> {
        let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();
        parser.digest_line("SOURCE_INSTRUMENT=LC-ESI-Orbitrap")?;
        parser.digest_line("SOURCE_INSTRUMENT=ESI-Orbitrap")?;
        assert!(parser.digest_line("SOURCE_INSTRUMENT=ESI-qTof").is_err());
        Ok(())
    }

    #[test]
    fn accepts_repeated_identical_metadata_lines() -> Result<()> {
        let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();

        for line in [
            "FEATURE_ID=1",
            "FEATURE_ID=1",
            "PEPMASS=381.0795",
            "PEPMASS=381.0795",
            "MSLEVEL=2",
            "MSLEVEL=2",
            "SCANS=1",
            "SCANS=1",
            "CHARGE=1",
            "CHARGE=1",
            "RTINSECONDS=37.083",
            "RTINSECONDS=37.083",
            "FILENAME=20220513_PMA_DBGI_01_04_003.mzML",
            "FILENAME=20220513_PMA_DBGI_01_04_003.mzML",
            "SMILES=CCO",
            "SMILES=CCO",
            "SMILES=N/A",
            "FORMULA=C2H6O",
            "FORMULA=C2H6O",
            "SPLASH=splash10-0udi-0490000000-4425acda10ed7d4709bd",
            "SPLASH=N/A",
            "IONMODE=Positive",
            "IONMODE=pos",
            "IONMODE=N/A",
            "SOURCE_INSTRUMENT=LC-ESI-qTof",
            "SOURCE_INSTRUMENT=ESI-LC-ESI-QTOF",
            "SOURCE_INSTRUMENT=N/A-N/A",
        ] {
            parser.digest_line(line)?;
        }

        let (metadata, _precursor_mz) = parser.build()?;
        assert_eq!(metadata.charge(), Some(1));
        Ok(())
    }

    #[test]
    fn rejects_invalid_scalar_metadata_lines() {
        for line in [
            "FEATURE_ID=",
            "PEPMASS=not-a-number",
            "PEPMASS=NaN",
            "PEPMASS=0",
            "MSLEVEL=not-a-number",
            "MSLEVEL=0",
            "SCANS=",
            "CHARGE=+1+",
            "CHARGE=abc+",
            "CHARGE=not-a-number",
            "CHARGE=128+",
            "CHARGE=129+",
            "CHARGE=129-",
            "RTINSECONDS=not-a-number",
            "RTINSECONDS=NaN",
            "RTINSECONDS=0",
            "SMILES=C(",
            "FORMULA=not-a-formula",
            "IONMODE=unknown",
            "MERGED_SCANS=not-a-number",
            "MERGED_STATS=not-a-fraction",
            "MERGED_STATS=1 / 1",
            "MERGED_STATS=1 / 1 (",
            "MERGED_STATS=one / 1 (0 removed due to low quality, 0 removed due to low cosine).",
            "MERGED_STATS=1 / one (0 removed due to low quality, 0 removed due to low cosine).",
            "MERGED_STATS=1 / 1 (one removed due to low quality, 0 removed due to low cosine).",
            "MERGED_STATS=1 / 1 (0 removed due to low quality, one removed due to low cosine).",
            "MERGED_STATS=1 / 2 (0 removed due to low quality, 0 removed due to low cosine).",
            "UNKNOWN=1",
        ] {
            let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();
            assert!(parser.digest_line(line).is_err(), "{line}");
        }

        assert!(
            MascotGenericFormatMetadataBuilder::<f64>::parse_ms_level_value(
                "not-a-level",
                "MSLEVEL=not-a-level",
            )
            .is_err()
        );

        let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();
        assert!(parser.digest_merge_scans_line("MERGED_OTHER=1").is_err());
    }

    #[test]
    fn parses_minimum_negative_charge() -> Result<()> {
        let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();
        parser.digest_line("CHARGE=128-")?;
        assert_eq!(parser.charge, Some(i8::MIN));
        Ok(())
    }

    #[test]
    fn reports_missing_required_fields_on_build() -> Result<()> {
        let parser = MascotGenericFormatMetadataBuilder::<f64>::default();
        assert!(matches!(
            parser.build(),
            Err(MascotError::MissingField {
                field: "precursor_mz",
                ..
            })
        ));

        let mut parser = MascotGenericFormatMetadataBuilder::<f64>::default();
        parser.digest_line("MSLEVEL=2")?;
        parser.digest_line("CHARGE=1")?;
        assert!(matches!(
            parser.build(),
            Err(MascotError::MissingField {
                field: "precursor_mz",
                ..
            })
        ));

        Ok(())
    }

    #[test]
    fn validates_complete_merged_scan_metadata_on_build() {
        let parser = MascotGenericFormatMetadataBuilder {
            feature_id: Some("1".to_string()),
            scans: Some("1".to_string()),
            level: Some(2),
            precursor_mz: Some(381.0795),
            precursor_mz_source: Some(PrecursorMzSource::Pepmass),
            precursor_mz_precision: Some(8),
            retention_time: None,
            charge: Some(1),
            merged_scan_count: Some(1),
            retained_merged_scan_count: Some(1),
            merged_scans_removed_due_to_low_quality: Some(1),
            merged_scans_removed_due_to_low_cosine: Some(0),
            merged_total_scan_count: Some(1),
            filename: None,
            smiles: None,
            formula: None,
            splash: None,
            ion_mode: None,
            source_instrument: None,
            arbitrary_metadata: Vec::new(),
        };

        assert!(matches!(
            parser.build(),
            Err(MascotError::MergedScanStatisticsMismatch)
        ));
    }
}
