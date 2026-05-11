//! Integration tests for structured MGF header validation.

use mascot_rs::prelude::*;

const VALID_SPLASH: &str = "splash10-0udi-0490000000-4425acda10ed7d4709bd";

fn valid_document_with(splash: &str, formula: &str) -> String {
    valid_document_with_smiles(splash, formula, "CCO")
}

fn valid_document_with_smiles(splash: &str, formula: &str, smiles: &str) -> String {
    format!(
        "BEGIN IONS\n\
PEPMASS=250.0\n\
CHARGE=1\n\
MSLEVEL=2\n\
SMILES={smiles}\n\
FORMULA={formula}\n\
SPLASH={splash}\n\
100.0 10.0\n\
200.0 20.0\n\
END IONS\n"
    )
}

const fn valid_document_without_splash() -> &'static str {
    concat!(
        "BEGIN IONS\n",
        "PEPMASS=250.0\n",
        "CHARGE=1\n",
        "MSLEVEL=2\n",
        "SMILES=CCO\n",
        "FORMULA=C2H6O\n",
        "100.0 10.0\n",
        "200.0 20.0\n",
        "END IONS\n",
    )
}

fn valid_splash_record() -> std::result::Result<MascotGenericFormat, String> {
    valid_document_with(VALID_SPLASH, "C2H6O")
        .parse()
        .map_err(|error| format!("test fixture should parse with a valid SPLASH: {error}"))
}

fn valid_splash_collection() -> std::result::Result<MGFVec, String> {
    valid_document_with(VALID_SPLASH, "C2H6O")
        .parse()
        .map_err(|error| format!("test fixture should parse with a valid SPLASH: {error}"))
}

fn assert_current_structured_splash(
    record: &MascotGenericFormat,
) -> std::result::Result<(), String> {
    let observed = record
        .metadata()
        .splash()
        .ok_or_else(|| "edited record should keep structured SPLASH metadata".to_string())?;
    let expected = SpectrumSplash::splash(record)
        .map_err(|error| format!("edited record should remain splashable: {error}"))?;

    if observed != expected {
        return Err(format!(
            "edited record has stale structured SPLASH: observed {observed}, calculated {expected}"
        ));
    }

    Ok(())
}

fn assert_structured_splash_was_recomputed(
    original_splash: &str,
    record: &MascotGenericFormat,
) -> std::result::Result<(), String> {
    assert_current_structured_splash(record)?;
    let edited_splash = record
        .metadata()
        .splash()
        .ok_or_else(|| "edited record should keep structured SPLASH metadata".to_string())?;
    if edited_splash == original_splash {
        return Err(format!(
            "edited record should not keep stale source SPLASH {original_splash}"
        ));
    }

    Ok(())
}

#[test]
fn parses_structured_formula_and_validates_splash() -> Result<()> {
    let document = valid_document_with(VALID_SPLASH, "C2H6O");

    let spectra: MGFVec = document.parse()?;

    assert_eq!(spectra.len(), 1);
    assert!(spectra[0].metadata().formula().is_some());
    assert_eq!(spectra[0].metadata().splash(), Some(VALID_SPLASH));
    assert_eq!(SpectrumSplash::splash(&spectra[0])?, VALID_SPLASH);

    Ok(())
}

#[test]
fn accepts_formula_that_matches_smiles_after_mixture_merge() -> Result<()> {
    let document = valid_document_with_smiles(VALID_SPLASH, "C8H14BrNO2", "Br.COC(=O)C1=CCCN(C)C1");

    let spectra: MGFVec = document.parse()?;

    assert_eq!(spectra.len(), 1);
    assert!(spectra[0].metadata().formula().is_some());

    Ok(())
}

#[test]
fn accepts_formula_that_matches_smiles_after_isotopic_normalization() -> Result<()> {
    let document = valid_document_with_smiles(VALID_SPLASH, "CH3Br", "C[79Br]");

    let spectra: MGFVec = document.parse()?;

    assert_eq!(spectra.len(), 1);
    assert!(spectra[0].metadata().formula().is_some());

    Ok(())
}

#[test]
fn treats_missing_formula_and_splash_markers_as_absent() -> Result<()> {
    let document = concat!(
        "BEGIN IONS\n",
        "PEPMASS=250.0\n",
        "CHARGE=1\n",
        "MSLEVEL=2\n",
        "SMILES=CCO\n",
        "FORMULA=N/A\n",
        "SPLASH=N/A\n",
        "100.0 10.0\n",
        "200.0 20.0\n",
        "END IONS\n",
    );

    let spectra: MGFVec = document.parse()?;

    assert_eq!(spectra.len(), 1);
    assert!(spectra[0].metadata().formula().is_none());
    assert_eq!(spectra[0].metadata().splash(), None);

    Ok(())
}

#[test]
fn accepts_mass_spec_gym_isotopic_bromine_formula_order() -> Result<()> {
    let document = valid_document_with_smiles(
        VALID_SPLASH,
        "C24H47BrNO8P",
        "C[N+](C)(C)CCOP(=O)([O-])OCC(CO)OC(=O)CCCC(CCCCCC/C=C\\CCC[79Br])O",
    );

    let spectra: MGFVec = document.parse()?;

    assert_eq!(spectra.len(), 1);
    assert!(spectra[0].metadata().formula().is_some());

    Ok(())
}

#[test]
fn rejects_formula_that_disagrees_with_smiles() -> std::result::Result<(), String> {
    let document = valid_document_with(VALID_SPLASH, "C3H8O");
    let result: Result<MGFVec> = document.parse();

    let source = match result {
        Err(MascotError::InputLine { source, .. }) => source,
        result => return Err(format!("expected formula/SMILES mismatch, got {result:?}")),
    };
    assert!(matches!(
        source.as_ref(),
        MascotError::FormulaSmilesMismatch { .. }
    ));
    let message = source.to_string();
    assert!(message.contains("FORMULA/SMILES validation failed"));
    assert!(message.contains("MGF FORMULA header is C3H8O"));
    assert!(message.contains("SMILES-derived formula is C₂H₆O"));
    assert!(message.contains("isotope-insensitive atom-count vectors are different"));

    Ok(())
}

#[test]
fn rejects_splash_that_disagrees_with_peaks() -> std::result::Result<(), String> {
    let document = valid_document_with("splash10-0000-0000000000-00000000000000000000", "C2H6O");
    let result: Result<MGFVec> = document.parse();

    let source = match result {
        Err(MascotError::InputLine { source, .. }) => source,
        result => return Err(format!("expected SPLASH mismatch, got {result:?}")),
    };
    assert!(matches!(
        source.as_ref(),
        MascotError::SplashMismatch { .. }
    ));
    let message = source.to_string();
    assert!(message.contains("SPLASH validation failed"));
    assert!(message.contains("the MGF header reports"));
    assert!(message.contains("calculated from the parsed peaks"));

    Ok(())
}

#[test]
fn top_k_peaks_from_valid_splash_record_does_not_fail() -> std::result::Result<(), String> {
    let record = valid_splash_record()?;

    let top = record.top_k_peaks(1).map_err(|error| {
        format!("top-k should not fail solely because SPLASH metadata was present: {error}")
    })?;

    assert_eq!(top.len(), 1);
    assert_structured_splash_was_recomputed(VALID_SPLASH, &top)?;

    Ok(())
}

#[test]
fn top_k_peaks_zero_returns_an_error_instead_of_panicking() -> std::result::Result<(), String> {
    let record = valid_splash_record()?;

    let Ok(result) =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| record.top_k_peaks(0)))
    else {
        return Err("top_k_peaks(0) should return an error instead of panicking".into());
    };

    if result.is_ok() {
        return Err("top_k_peaks(0) should reject empty MGF records explicitly".into());
    }

    Ok(())
}

#[test]
fn top_k_peaks_from_empty_record_returns_an_error() -> std::result::Result<(), String> {
    let record: MascotGenericFormat = MascotGenericFormat::with_capacity(250.0, 0)
        .map_err(|error| format!("empty allocatable record should be constructible: {error}"))?;

    if record.top_k_peaks(1).is_ok() {
        return Err("top_k_peaks should reject an empty source record".into());
    }

    Ok(())
}

#[test]
fn top_k_peaks_without_splash_does_not_add_splash() -> std::result::Result<(), String> {
    let record: MascotGenericFormat = valid_document_without_splash()
        .parse()
        .map_err(|error| format!("test fixture should parse without SPLASH: {error}"))?;

    let top = record
        .top_k_peaks(1)
        .map_err(|error| format!("top-k should work without SPLASH metadata: {error}"))?;

    assert_eq!(top.len(), 1);
    assert_eq!(top.metadata().splash(), None);

    Ok(())
}

#[test]
fn add_peak_updates_structured_splash_metadata() -> std::result::Result<(), String> {
    let mut record = valid_splash_record()?;

    record
        .add_peak(300.0, 30.0)
        .map_err(|error| format!("add_peak should update structured SPLASH metadata: {error}"))?;

    assert_eq!(record.len(), 3);
    assert_structured_splash_was_recomputed(VALID_SPLASH, &record)?;

    Ok(())
}

#[test]
fn add_peak_is_atomic_when_peak_fails() -> std::result::Result<(), String> {
    let mut record = valid_splash_record()?;
    let original_peaks = record.peaks().collect::<Vec<_>>();
    let original_splash = record.metadata().splash().map(ToString::to_string);

    let result = record.add_peak(150.0, 30.0);
    if result.is_ok() {
        return Err("add_peak should reject a peak that breaks m/z ordering".into());
    }

    assert_eq!(
        record.peaks().collect::<Vec<_>>(),
        original_peaks,
        "add_peak should leave the record unchanged when the edit fails"
    );
    assert_eq!(
        record.metadata().splash().map(ToString::to_string),
        original_splash,
        "add_peak should leave structured SPLASH unchanged when the edit fails"
    );

    Ok(())
}

#[test]
fn add_peak_without_splash_does_not_add_splash() -> std::result::Result<(), String> {
    let mut record: MascotGenericFormat = valid_document_without_splash()
        .parse()
        .map_err(|error| format!("test fixture should parse without SPLASH: {error}"))?;

    record
        .add_peak(300.0, 30.0)
        .map_err(|error| format!("add_peak should work without SPLASH metadata: {error}"))?;

    assert_eq!(record.len(), 3);
    assert_eq!(record.metadata().splash(), None);

    Ok(())
}

#[test]
fn add_peaks_updates_structured_splash_metadata() -> std::result::Result<(), String> {
    let mut record = valid_splash_record()?;

    record
        .add_peaks([(300.0, 30.0), (400.0, 40.0)])
        .map_err(|error| {
            format!("add_peaks should update structured SPLASH metadata after the batch: {error}")
        })?;

    assert_eq!(record.len(), 4);
    assert_structured_splash_was_recomputed(VALID_SPLASH, &record)?;

    Ok(())
}

#[test]
fn add_peaks_is_atomic_when_later_peak_fails() -> std::result::Result<(), String> {
    let mut record: MascotGenericFormat = valid_document_without_splash()
        .parse()
        .map_err(|error| format!("test fixture should parse without SPLASH: {error}"))?;
    let original_peaks = record.peaks().collect::<Vec<_>>();

    let result = record.add_peaks([(300.0, 30.0), (250.0, 40.0)]);
    if result.is_ok() {
        return Err("add_peaks should reject a batch whose later peak breaks m/z ordering".into());
    }

    assert_eq!(
        record.peaks().collect::<Vec<_>>(),
        original_peaks,
        "add_peaks should leave the record unchanged when the batch fails"
    );

    Ok(())
}

#[test]
fn iter_mut_peak_edits_update_structured_splash_metadata() -> std::result::Result<(), String> {
    let mut spectra = valid_splash_collection()?;
    let record = spectra
        .iter_mut()
        .next()
        .ok_or_else(|| "test fixture should contain one record".to_string())?;

    record.add_peak(300.0, 30.0).map_err(|error| {
        format!("iter_mut peak edits should update structured SPLASH metadata: {error}")
    })?;

    assert_eq!(record.len(), 3);
    assert_structured_splash_was_recomputed(VALID_SPLASH, record)?;

    Ok(())
}

#[test]
fn mutable_collection_iterator_peak_edits_update_structured_splash_metadata(
) -> std::result::Result<(), String> {
    let mut spectra = valid_splash_collection()?;
    let mut edited = false;

    for record in &mut spectra {
        record.add_peak(300.0, 30.0).map_err(|error| {
            format!(
                "mutable collection iterator peak edits should update structured SPLASH metadata: {error}"
            )
        })?;
        assert_eq!(record.len(), 3);
        assert_structured_splash_was_recomputed(VALID_SPLASH, record)?;
        edited = true;
    }

    if !edited {
        return Err("test fixture should contain one record".to_string());
    }

    Ok(())
}
