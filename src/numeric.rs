use alloc::string::ToString;

use mass_spectrometry::prelude::SpectrumFloat;

use crate::error::{MascotError, Result};

pub fn parse_positive_f64(value: &str, field: &'static str, line: &str) -> Result<f64> {
    let value = value.parse::<f64>().map_err(|_| MascotError::ParseField {
        field,
        line: line.to_string(),
    })?;
    validate_positive_f64(value, field, line)?;
    Ok(value)
}

pub fn parse_positive_spectrum_float<P: SpectrumFloat>(
    value: &str,
    field: &'static str,
    line: &str,
) -> Result<P> {
    let value = parse_positive_f64(value, field, line)?;
    let value = P::from_f64(value).ok_or_else(|| MascotError::UnrepresentablePrecisionField {
        field,
        line: line.to_string(),
    })?;
    validate_positive_f64(value.to_f64(), field, line)?;
    Ok(value)
}

pub fn parse_spectrum_float_lossy<P: SpectrumFloat>(
    value: &str,
    field: &'static str,
    line: &str,
) -> Result<P> {
    value
        .parse::<f64>()
        .map(P::from_f64_lossy)
        .map_err(|_| MascotError::ParseField {
            field,
            line: line.to_string(),
        })
}

pub fn validate_positive_f64(value: f64, field: &'static str, line: &str) -> Result<()> {
    if !value.is_finite() {
        return Err(MascotError::NonFiniteField {
            field,
            line: line.to_string(),
        });
    }

    if value <= 0.0 {
        return Err(MascotError::NonPositiveField {
            field,
            line: line.to_string(),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_values_not_representable_in_requested_precision() {
        assert!(matches!(
            parse_positive_spectrum_float::<f32>("1e100", "precursor m/z", "PEPMASS=1e100"),
            Err(MascotError::UnrepresentablePrecisionField {
                field: "precursor m/z",
                ..
            })
        ));
    }
}
