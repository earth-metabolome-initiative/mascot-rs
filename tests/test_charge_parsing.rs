//! Integration tests for numeric charge parsing.

use mascot_rs::prelude::*;

#[test]
fn test_parse_i8_charge_values() -> Result<()> {
    for (line, expected_charge) in [
        ("CHARGE=2", Some(2)),
        ("CHARGE=5+", Some(5)),
        ("CHARGE=0", None),
        ("CHARGE=0+", None),
        ("CHARGE=0-", None),
        ("CHARGE=-1", Some(-1)),
        ("CHARGE=2-", Some(-2)),
    ] {
        let lines = [
            "FEATURE_ID=1",
            "PEPMASS=381.0795",
            "SCANS=1",
            line,
            "RTINSECONDS=37.083",
            "BEGIN IONS",
            "MSLEVEL=2",
            "60.5425 2.4E5",
            "END IONS",
        ];

        let mgf: MGFVec = MGFVec::try_from_iter(lines)?;
        assert_eq!(mgf[0].charge(), expected_charge);
    }

    Ok(())
}

#[test]
fn test_missing_charge_is_preserved_as_unknown() -> Result<()> {
    let lines = [
        "FEATURE_ID=1",
        "PEPMASS=381.0795",
        "SCANS=1",
        "RTINSECONDS=37.083",
        "BEGIN IONS",
        "MSLEVEL=2",
        "60.5425 2.4E5",
        "END IONS",
    ];

    let mgf: MGFVec = MGFVec::try_from_iter(lines)?;

    assert_eq!(mgf[0].charge(), None);

    Ok(())
}

fn single_record_with_metadata_line(line: &str) -> Result<MascotGenericFormat> {
    let document =
        format!("BEGIN IONS\nPEPMASS=381.0795\nMSLEVEL=2\n{line}\n60.5425 2.4E5\nEND IONS\n");
    document.parse()
}

#[test]
fn test_adduct_infers_charge_and_ion_mode() -> Result<()> {
    for (adduct, expected_charge, expected_ion_mode) in [
        ("[M+H]+", 1, IonMode::Positive),
        ("[M+2H]2+", 2, IonMode::Positive),
        ("[M-H]-", -1, IonMode::Negative),
        ("[M-2H]2-", -2, IonMode::Negative),
        ("[M-128H]128-", i8::MIN, IonMode::Negative),
    ] {
        let record = single_record_with_metadata_line(&format!("ADDUCT={adduct}"))?;

        assert_eq!(record.charge(), Some(expected_charge), "{adduct}");
        assert_eq!(record.ion_mode(), Some(expected_ion_mode), "{adduct}");
        assert_eq!(
            record.metadata().arbitrary_metadata_value("ADDUCT"),
            Some(adduct)
        );
    }

    Ok(())
}

#[test]
fn test_zero_charge_can_be_imputed_from_adduct() -> Result<()> {
    let document = concat!(
        "BEGIN IONS\n",
        "PEPMASS=381.0795\n",
        "MSLEVEL=2\n",
        "CHARGE=0\n",
        "ADDUCT=[M+H]+\n",
        "60.5425 2.4E5\n",
        "END IONS\n",
    );

    let record: MascotGenericFormat = document.parse()?;

    assert_eq!(record.charge(), Some(1));
    assert_eq!(record.ion_mode(), Some(IonMode::Positive));

    Ok(())
}

#[test]
fn test_unknown_adduct_is_preserved_without_imputation() -> Result<()> {
    for adduct in [
        "not-a-standard-adduct",
        "[M+H]",
        "[M+H]0+",
        "[M+129H]129+",
        "[M-129H]129-",
    ] {
        let record = single_record_with_metadata_line(&format!("ADDUCT={adduct}"))?;

        assert_eq!(record.charge(), None, "{adduct}");
        assert_eq!(record.ion_mode(), None, "{adduct}");
        assert_eq!(
            record.metadata().arbitrary_metadata_value("ADDUCT"),
            Some(adduct)
        );
    }

    Ok(())
}

#[test]
fn test_matching_negative_charge_and_ion_mode_are_accepted_in_either_order() -> Result<()> {
    for lines in [
        [
            "BEGIN IONS",
            "PEPMASS=381.0795",
            "MSLEVEL=2",
            "CHARGE=-1",
            "IONMODE=Negative",
            "60.5425 2.4E5",
            "END IONS",
        ],
        [
            "BEGIN IONS",
            "PEPMASS=381.0795",
            "MSLEVEL=2",
            "IONMODE=Negative",
            "CHARGE=-1",
            "60.5425 2.4E5",
            "END IONS",
        ],
    ] {
        let record: MascotGenericFormat = lines.join("\n").parse()?;

        assert_eq!(record.charge(), Some(-1));
        assert_eq!(record.ion_mode(), Some(IonMode::Negative));
    }

    Ok(())
}

#[test]
fn test_matching_adduct_and_existing_ion_metadata_are_accepted() -> Result<()> {
    let document = concat!(
        "BEGIN IONS\n",
        "PEPMASS=381.0795\n",
        "MSLEVEL=2\n",
        "CHARGE=0\n",
        "IONMODE=Negative\n",
        "ADDUCT=[M-H]-\n",
        "60.5425 2.4E5\n",
        "END IONS\n",
    );

    let record: MascotGenericFormat = document.parse()?;

    assert_eq!(record.charge(), Some(-1));
    assert_eq!(record.ion_mode(), Some(IonMode::Negative));

    Ok(())
}

#[test]
fn test_adduct_charge_conflict_is_an_error() {
    let document = concat!(
        "BEGIN IONS\n",
        "PEPMASS=381.0795\n",
        "MSLEVEL=2\n",
        "CHARGE=1\n",
        "ADDUCT=[M-H]-\n",
        "60.5425 2.4E5\n",
        "END IONS\n",
    );

    assert!(matches!(
        document.parse::<MascotGenericFormat>(),
        Err(MascotError::InputLine {
            line_number: 5,
            source,
            ..
        }) if matches!(
            source.as_ref(),
            MascotError::AdductChargeMismatch {
                adduct_charge: -1,
                charge: 1,
                ..
            }
        )
    ));
}

#[test]
fn test_adduct_ion_mode_conflict_is_an_error() {
    let document = concat!(
        "BEGIN IONS\n",
        "PEPMASS=381.0795\n",
        "MSLEVEL=2\n",
        "IONMODE=Positive\n",
        "ADDUCT=[M-H]-\n",
        "60.5425 2.4E5\n",
        "END IONS\n",
    );

    assert!(matches!(
        document.parse::<MascotGenericFormat>(),
        Err(MascotError::InputLine {
            line_number: 5,
            source,
            ..
        }) if matches!(
            source.as_ref(),
            MascotError::AdductIonModeMismatch {
                adduct_ion_mode: "Negative",
                ion_mode: "Positive",
                ..
            }
        )
    ));
}

#[test]
fn test_charge_ion_mode_conflict_is_an_error() {
    let document = concat!(
        "BEGIN IONS\n",
        "PEPMASS=381.0795\n",
        "MSLEVEL=2\n",
        "CHARGE=1\n",
        "IONMODE=Negative\n",
        "60.5425 2.4E5\n",
        "END IONS\n",
    );

    assert!(matches!(
        document.parse::<MascotGenericFormat>(),
        Err(MascotError::InputLine {
            line_number: 5,
            source,
            ..
        }) if matches!(
            source.as_ref(),
            MascotError::ChargeIonModeMismatch {
                charge: 1,
                ion_mode: "Negative",
            }
        )
    ));
}
