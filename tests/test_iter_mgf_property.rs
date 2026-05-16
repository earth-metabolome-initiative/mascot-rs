//! Integration tests for [`IterMGFProperty`] and the SMILES convenience methods.

use mascot_rs::prelude::*;

const SMILES_VALUE: &str = "CCO";

fn document_with_smiles(smiles: &str) -> String {
    format!(
        "BEGIN IONS\n\
PEPMASS=250.0\n\
CHARGE=1\n\
MSLEVEL=2\n\
SMILES={smiles}\n\
100.0 10.0\n\
200.0 20.0\n\
END IONS\n"
    )
}

const fn document_without_smiles() -> &'static str {
    "BEGIN IONS\n\
PEPMASS=250.0\n\
CHARGE=1\n\
MSLEVEL=2\n\
100.0 10.0\n\
200.0 20.0\n\
END IONS\n"
}

const fn malformed_record_then_two_good() -> &'static str {
    // The middle record is missing PEPMASS, which is required.
    "BEGIN IONS\n\
PEPMASS=250.0\n\
CHARGE=1\n\
MSLEVEL=2\n\
SMILES=CCO\n\
100.0 10.0\n\
200.0 20.0\n\
END IONS\n\
BEGIN IONS\n\
CHARGE=1\n\
MSLEVEL=2\n\
100.0 10.0\n\
END IONS\n\
BEGIN IONS\n\
PEPMASS=250.0\n\
CHARGE=1\n\
MSLEVEL=2\n\
SMILES=CCN\n\
100.0 10.0\n\
200.0 20.0\n\
END IONS\n"
}

fn parse_record(document: &str) -> std::result::Result<MascotGenericFormat, String> {
    document
        .parse::<MascotGenericFormat>()
        .map_err(|error| format!("test fixture should parse: {error}"))
}

fn parse_vec(document: &str) -> std::result::Result<MGFVec, String> {
    document
        .parse::<MGFVec>()
        .map_err(|error| format!("test fixture should parse: {error}"))
}

#[test]
fn mgf_with_smiles_yields_one_present_and_one_some() -> std::result::Result<(), String> {
    let record = parse_record(&document_with_smiles(SMILES_VALUE))?;

    let present: Vec<&Smiles> = IterMGFProperty::<&Smiles>::properties(&record).collect();
    assert_eq!(present.len(), 1);
    assert_eq!(present[0].to_string(), SMILES_VALUE);

    let maybe: Vec<Option<&Smiles>> =
        IterMGFProperty::<&Smiles>::maybe_properties(&record).collect();
    assert_eq!(maybe.len(), 1);
    let inner = maybe[0].ok_or_else(|| "record with SMILES should yield Some(_)".to_string())?;
    assert_eq!(inner.to_string(), SMILES_VALUE);

    Ok(())
}

#[test]
fn mgf_without_smiles_yields_zero_present_and_one_none() -> std::result::Result<(), String> {
    let record = parse_record(document_without_smiles())?;

    let present: Vec<&Smiles> = IterMGFProperty::<&Smiles>::properties(&record).collect();
    assert!(
        present.is_empty(),
        "record without SMILES should yield no present items, got {} items",
        present.len()
    );

    let maybe: Vec<Option<&Smiles>> =
        IterMGFProperty::<&Smiles>::maybe_properties(&record).collect();
    assert_eq!(maybe.len(), 1);
    if maybe[0].is_some() {
        return Err("record without SMILES should yield None".to_string());
    }

    Ok(())
}

#[test]
fn mgfvec_skips_records_without_smiles_in_properties() -> std::result::Result<(), String> {
    let document = format!(
        "{}\n{}\n{}\n",
        document_with_smiles("CCO"),
        document_without_smiles(),
        document_with_smiles("CCN"),
    );
    let collection = parse_vec(&document)?;
    assert_eq!(collection.len(), 3);

    let present: Vec<&Smiles> = IterMGFProperty::<&Smiles>::properties(&collection).collect();
    assert_eq!(present.len(), 2);
    assert_eq!(present[0].to_string(), "CCO");
    assert_eq!(present[1].to_string(), "CCN");

    let maybe: Vec<Option<&Smiles>> =
        IterMGFProperty::<&Smiles>::maybe_properties(&collection).collect();
    assert_eq!(maybe.len(), 3);
    let first = maybe[0].ok_or_else(|| "first record should yield Some(_)".to_string())?;
    assert_eq!(first.to_string(), "CCO");
    if maybe[1].is_some() {
        return Err("middle record should yield None".to_string());
    }
    let third = maybe[2].ok_or_else(|| "third record should yield Some(_)".to_string())?;
    assert_eq!(third.to_string(), "CCN");

    Ok(())
}

#[test]
fn mgfiter_skips_parse_errors_in_properties() {
    let document = malformed_record_then_two_good();
    let iter = MGFIter::<f64, _>::from_document(document).skipping_invalid_records();

    let present: Vec<Smiles> = IterMGFProperty::<Smiles>::properties(iter).collect();
    let names: Vec<String> = present.iter().map(ToString::to_string).collect();
    assert_eq!(names, vec!["CCO".to_string(), "CCN".to_string()]);
}

#[test]
fn mgfiter_maybe_properties_yields_one_per_ok_record() -> std::result::Result<(), String> {
    let document = malformed_record_then_two_good();
    let iter = MGFIter::<f64, _>::from_document(document).skipping_invalid_records();

    let maybe: Vec<Option<Smiles>> = IterMGFProperty::<Smiles>::maybe_properties(iter).collect();
    assert_eq!(maybe.len(), 2);
    let first = maybe[0]
        .as_ref()
        .ok_or_else(|| "first parsed record should yield Some(_)".to_string())?;
    assert_eq!(first.to_string(), "CCO");
    let second = maybe[1]
        .as_ref()
        .ok_or_else(|| "second parsed record should yield Some(_)".to_string())?;
    assert_eq!(second.to_string(), "CCN");

    Ok(())
}

#[test]
fn convenience_smiles_matches_trait_properties() -> std::result::Result<(), String> {
    let record = parse_record(&document_with_smiles(SMILES_VALUE))?;
    let from_trait: Vec<String> = IterMGFProperty::<&Smiles>::properties(&record)
        .map(ToString::to_string)
        .collect();
    let from_method: Vec<String> = record.smiles().map(ToString::to_string).collect();
    assert_eq!(from_trait, from_method);

    let from_trait_maybe: Vec<Option<String>> =
        IterMGFProperty::<&Smiles>::maybe_properties(&record)
            .map(|maybe| maybe.map(ToString::to_string))
            .collect();
    let from_method_maybe: Vec<Option<String>> = record
        .maybe_smiles()
        .map(|maybe| maybe.map(ToString::to_string))
        .collect();
    assert_eq!(from_trait_maybe, from_method_maybe);

    let document = format!(
        "{}\n{}\n",
        document_with_smiles("CCO"),
        document_without_smiles(),
    );
    let collection = parse_vec(&document)?;
    let from_trait: Vec<String> = IterMGFProperty::<&Smiles>::properties(&collection)
        .map(ToString::to_string)
        .collect();
    let from_method: Vec<String> = collection.smiles().map(ToString::to_string).collect();
    assert_eq!(from_trait, from_method);

    let from_trait_maybe: Vec<Option<String>> =
        IterMGFProperty::<&Smiles>::maybe_properties(&collection)
            .map(|maybe| maybe.map(ToString::to_string))
            .collect();
    let from_method_maybe: Vec<Option<String>> = collection
        .maybe_smiles()
        .map(|maybe| maybe.map(ToString::to_string))
        .collect();
    assert_eq!(from_trait_maybe, from_method_maybe);

    let iter_trait = MGFIter::<f64, _>::from_document(&document).skipping_invalid_records();
    let from_trait_iter: Vec<String> = IterMGFProperty::<Smiles>::properties(iter_trait)
        .map(|smiles| smiles.to_string())
        .collect();
    let iter_method = MGFIter::<f64, _>::from_document(&document).skipping_invalid_records();
    let from_method_iter: Vec<String> = iter_method
        .smiles()
        .map(|smiles| smiles.to_string())
        .collect();
    assert_eq!(from_trait_iter, from_method_iter);

    Ok(())
}
