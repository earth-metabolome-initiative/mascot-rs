//! Integration tests for `MGFVec` collection helpers.

use mascot_rs::prelude::*;

fn record(feature_id: u32) -> Result<MascotGenericFormat> {
    let feature_id_as_f64 = f64::from(feature_id);
    let metadata = MascotGenericFormatMetadata::new(
        Some(feature_id.to_string()),
        2,
        Some(feature_id_as_f64),
        Some(1),
        None,
    )?;

    MascotGenericFormat::new(
        metadata,
        500.0 + feature_id_as_f64,
        vec![100.0 + feature_id_as_f64],
        vec![10.0 + feature_id_as_f64],
    )
}

#[test]
fn test_push_append_extend_and_from_vec_preserve_order() -> Result<()> {
    let mut records = MGFVec::default();
    records.push(record(1)?);

    let mut other = MGFVec::from(vec![record(2)?, record(3)?]);
    records.append(&mut other);
    records.extend([record(4)?, record(5)?]);

    let feature_ids: Vec<_> = records
        .iter()
        .map(MascotGenericFormat::feature_id)
        .collect();

    assert!(other.is_empty());
    assert_eq!(
        feature_ids,
        vec![Some("1"), Some("2"), Some("3"), Some("4"), Some("5")]
    );

    Ok(())
}

#[test]
fn test_as_ref_exposes_records_as_slice() -> Result<()> {
    let records = MGFVec::from(vec![record(1)?, record(2)?]);
    let slice: &[MascotGenericFormat] = records.as_ref();

    assert_eq!(slice.len(), 2);
    assert_eq!(slice[0].feature_id(), Some("1"));
    assert_eq!(slice[1].feature_id(), Some("2"));

    Ok(())
}
