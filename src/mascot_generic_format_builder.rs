use alloc::{string::ToString, vec::Vec};

use crate::mascot_generic_format::MascotGenericFormat;
use crate::mascot_generic_format_metadata_builder::MascotGenericFormatMetadataBuilder;
use crate::numeric;
use crate::prelude::*;

const MZ_FIELD: &str = "mass divided by charge ratio";
const INTENSITY_FIELD: &str = "fragment intensity";

#[derive(Debug, Clone)]
/// A builder for [`MascotGenericFormat`].
pub struct MascotGenericFormatBuilder<P: SpectrumFloat = f64> {
    metadata_builder: MascotGenericFormatMetadataBuilder<P>,
    peaks: Vec<(P, P)>,
    peaks_are_strictly_increasing: bool,
    section_open: bool,
}

impl<P: SpectrumFloat> Default for MascotGenericFormatBuilder<P> {
    fn default() -> Self {
        Self {
            metadata_builder: MascotGenericFormatMetadataBuilder::<P>::default(),
            peaks: Vec::new(),
            peaks_are_strictly_increasing: true,
            section_open: false,
        }
    }
}

impl<P: SpectrumFloat> MascotGenericFormatBuilder<P> {
    pub(super) const fn section_open(&self) -> bool {
        self.section_open
    }
}

impl<P: SpectrumFloat> MascotGenericFormatBuilder<P> {
    /// Builds a [`MascotGenericFormat`] from the given data.
    ///
    /// # Errors
    /// Returns an error if the parsed metadata or data blocks are incomplete or
    /// invalid.
    pub(super) fn build(self) -> Result<MascotGenericFormat<P>> {
        let (metadata, precursor_mz) = self.metadata_builder.build()?;

        MascotGenericFormat::from_parsed_peaks(
            metadata,
            precursor_mz,
            self.peaks,
            self.peaks_are_strictly_increasing,
        )
    }
}

impl<P: SpectrumFloat> MascotGenericFormatBuilder<P> {
    fn digest_peak_line(&mut self, line: &str) -> Result<()> {
        let mut split = line.split_whitespace();

        let mass_divided_by_charge_ratio = split
            .next()
            .ok_or_else(|| MascotError::ParseField {
                field: MZ_FIELD,
                line: line.to_string(),
            })
            .and_then(|value| numeric::parse_spectrum_float_lossy(value, MZ_FIELD, line))?;

        let fragment_intensity: P = split
            .next()
            .ok_or_else(|| MascotError::ParseField {
                field: INTENSITY_FIELD,
                line: line.to_string(),
            })
            .and_then(|value| numeric::parse_spectrum_float_lossy(value, INTENSITY_FIELD, line))?;

        if matches!(
            fragment_intensity.to_f64().classify(),
            core::num::FpCategory::Zero
        ) {
            return Ok(());
        }

        MascotGenericFormat::<P>::push_peak_tracking_order(
            &mut self.peaks,
            &mut self.peaks_are_strictly_increasing,
            mass_divided_by_charge_ratio,
            fragment_intensity,
        );

        Ok(())
    }

    /// Digests the given line.
    ///
    /// # Arguments
    /// * `line` - The line to digest.
    ///
    /// # Returns
    /// Whether the line was successfully digested.
    ///
    /// # Errors
    /// * If the line cannot be digested.
    pub(super) fn digest_line(&mut self, line: &str) -> Result<()> {
        if line == "BEGIN IONS" {
            if self.section_open {
                return Err(MascotError::NestedIonSection {
                    line: line.to_string(),
                });
            }
            self.section_open = true;
        } else if line == "END IONS" {
            if !self.section_open {
                return Err(MascotError::LineOutsideIonSection {
                    line: line.to_string(),
                });
            }
            self.section_open = false;
        } else if MascotGenericFormatMetadataBuilder::<P>::can_parse_line(line) {
            self.metadata_builder.digest_line(line)?;
        } else if self.section_open
            && MascotGenericFormatMetadataBuilder::<P>::can_parse_arbitrary_metadata_line(line)
        {
            self.metadata_builder.digest_arbitrary_metadata_line(line)?;
        } else if self.section_open {
            self.digest_peak_line(line)?;
        } else {
            return Err(MascotError::LineOutsideIonSection {
                line: line.to_string(),
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digests_ion_section_boundaries_and_rejects_unknown_lines() -> Result<()> {
        let mut mascot_generic_format_builder = MascotGenericFormatBuilder::<f64>::default();

        mascot_generic_format_builder.digest_line("BEGIN IONS")?;
        mascot_generic_format_builder.digest_line("END IONS")?;
        assert!(mascot_generic_format_builder
            .digest_line("TITLE=File:")
            .is_err());
        Ok(())
    }

    #[test]
    fn rejects_invalid_peak_lines() -> Result<()> {
        for line in [" ", "100.0", "not-a-number 1.0", "100.0 not-a-number"] {
            let mut builder = MascotGenericFormatBuilder::<f64>::default();
            builder.digest_line("BEGIN IONS")?;
            assert!(builder.digest_line(line).is_err());
        }

        for line in ["NaN 1.0", "0.0 1.0", "100.0 NaN", "100.0 -1.0"] {
            let mut builder = MascotGenericFormatBuilder::<f64>::default();
            builder.digest_line("BEGIN IONS")?;
            builder.digest_line("PEPMASS=500.0")?;
            builder.digest_line("MSLEVEL=2")?;
            builder.digest_line("SCANS=-1")?;
            builder.digest_line("CHARGE=1")?;
            builder.digest_line(line)?;
            builder.digest_line("END IONS")?;
            assert!(matches!(
                builder.build(),
                Err(MascotError::SpectrumMutation(_))
            ));
        }

        Ok(())
    }

    #[test]
    fn peak_line_parser_rejects_missing_values_and_filters_zero_intensity() -> Result<()> {
        let mut builder = MascotGenericFormatBuilder::<f64>::default();

        assert!(matches!(
            builder.digest_peak_line(""),
            Err(MascotError::ParseField {
                field: "mass divided by charge ratio",
                ..
            })
        ));
        assert!(matches!(
            builder.digest_peak_line("100.0"),
            Err(MascotError::ParseField {
                field: "fragment intensity",
                ..
            })
        ));
        builder.digest_peak_line("100.0 0.0")?;
        assert!(builder.peaks.is_empty());

        Ok(())
    }

    #[test]
    fn filters_zero_intensity_peak_lines() -> Result<()> {
        let mut builder = MascotGenericFormatBuilder::<f64>::default();
        builder.digest_line("BEGIN IONS")?;
        builder.digest_line("PEPMASS=500.0")?;
        builder.digest_line("MSLEVEL=2")?;
        builder.digest_line("SCANS=-1")?;
        builder.digest_line("CHARGE=1")?;
        builder.digest_line("100.0 0.0")?;
        builder.digest_line("200.0 3.0")?;
        builder.digest_line("END IONS")?;

        let record = builder.build()?;
        assert_eq!(record.len(), 1);
        assert_eq!(record.peak_nth(0).0.to_bits(), 200.0_f64.to_bits());
        assert_eq!(record.peak_nth(0).1.to_bits(), 3.0_f64.to_bits());

        Ok(())
    }
}
