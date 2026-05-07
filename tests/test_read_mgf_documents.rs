//! Integration tests for reading fixture MGF documents.

use mascot_rs::prelude::*;

#[test]
fn test_read_mgf_documents() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let mut mgf_files: Vec<String> = Vec::new();
    for entry in std::fs::read_dir("tests/data")? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|extension| extension == "mgf") {
            mgf_files.push(
                path.to_str()
                    .ok_or_else(|| std::io::Error::other("MGF path is not valid UTF-8"))?
                    .to_string(),
            );
        }
    }

    for mgf_file in mgf_files {
        let vec: MGFVec = MGFVec::from_path(&mgf_file)?;
        assert!(!vec.is_empty());
    }

    Ok(())
}

#[test]
fn test_from_reader_reports_line_context() {
    let document = concat!(
        "BEGIN IONS\n",
        "FEATURE_ID=\n",
        "PEPMASS=500.0\n",
        "CHARGE=1\n",
        "RTINSECONDS=10.0\n",
        "MSLEVEL=2\n",
        "SMILES=CCO\n",
        "100.0 2.0\n",
        "SCANS=1\n",
        "END IONS\n",
    );

    assert!(matches!(
        MGFVec::<f64>::from_reader(std::io::Cursor::new(document)),
        Err(MascotError::InputLine { line_number: 2, .. })
    ));
}

#[test]
fn test_from_reader_rejects_empty_records_with_line_context() {
    let document = concat!(
        "BEGIN IONS\n",
        "FEATURE_ID=1\n",
        "PEPMASS=500.0\n",
        "CHARGE=1\n",
        "RTINSECONDS=10.0\n",
        "MSLEVEL=2\n",
        "SCANS=1\n",
        "END IONS\n",
    );

    assert!(matches!(
        MGFVec::<f64>::from_reader(std::io::Cursor::new(document)),
        Err(MascotError::InputLine { line_number: 8, .. })
    ));
}

#[test]
fn test_from_reader_reports_incomplete_record_at_end_ions() {
    let document = concat!(
        "BEGIN IONS\n",
        "FEATURE_ID=9\n",
        "MSLEVEL=2\n",
        "CHARGE=1\n",
        "100.0 2.0\n",
        "END IONS\n",
        "\n",
        "BEGIN IONS\n",
        "FEATURE_ID=10\n",
        "PEPMASS=247.89272\n",
        "MSLEVEL=2\n",
        "CHARGE=1\n",
        "100.0 2.0\n",
        "END IONS\n",
    );

    assert!(matches!(
        MGFVec::<f64>::from_reader(std::io::Cursor::new(document)),
        Err(MascotError::InputLine {
            line_number: 6,
            source,
            ..
        }) if matches!(
            source.as_ref(),
            MascotError::MissingField {
                field: "precursor_mz",
                ..
            }
        )
    ));
}

#[test]
fn test_from_reader_rejects_nested_begin_ions() {
    let document = concat!(
        "BEGIN IONS\n",
        "FEATURE_ID=1\n",
        "PEPMASS=500.0\n",
        "CHARGE=1\n",
        "MSLEVEL=2\n",
        "BEGIN IONS\n",
        "FEATURE_ID=2\n",
        "PEPMASS=600.0\n",
        "CHARGE=1\n",
        "MSLEVEL=2\n",
        "100.0 2.0\n",
        "END IONS\n",
    );

    assert!(matches!(
        MGFVec::<f64>::from_reader(std::io::Cursor::new(document)),
        Err(MascotError::InputLine {
            line_number: 6,
            source,
            ..
        }) if matches!(source.as_ref(), MascotError::NestedIonSection { .. })
    ));
}

#[test]
fn test_from_reader_rejects_end_ions_outside_section() {
    let document = concat!(
        "END IONS\n",
        "BEGIN IONS\n",
        "FEATURE_ID=1\n",
        "PEPMASS=500.0\n",
        "CHARGE=1\n",
        "MSLEVEL=2\n",
        "100.0 2.0\n",
        "END IONS\n",
    );

    assert!(matches!(
        MGFVec::<f64>::from_reader(std::io::Cursor::new(document)),
        Err(MascotError::InputLine {
            line_number: 1,
            source,
            ..
        }) if matches!(source.as_ref(), MascotError::LineOutsideIonSection { .. })
    ));
}

#[test]
fn test_from_reader_rejects_unclosed_record_at_eof() {
    let document = concat!(
        "BEGIN IONS\n",
        "FEATURE_ID=1\n",
        "PEPMASS=500.0\n",
        "CHARGE=1\n",
        "MSLEVEL=2\n",
        "100.0 2.0\n",
    );

    assert!(matches!(
        MGFVec::<f64>::from_reader(std::io::Cursor::new(document)),
        Err(MascotError::UnclosedIonSection {
            begin_line_number: 1
        })
    ));
}

#[test]
fn test_from_reader_filters_zero_intensity_peak_lines() -> Result<()> {
    let document = concat!(
        "BEGIN IONS\n",
        "FEATURE_ID=1\n",
        "PEPMASS=500.0\n",
        "CHARGE=1\n",
        "RTINSECONDS=10.0\n",
        "MSLEVEL=2\n",
        "100.0 0.0\n",
        "200.0 3.0\n",
        "SCANS=1\n",
        "END IONS\n",
    );

    let records = MGFVec::<f64>::from_reader(std::io::Cursor::new(document))?;

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].len(), 1);
    assert_eq!(records[0].peak_nth(0).0.to_bits(), 200.0_f64.to_bits());
    assert_eq!(records[0].peak_nth(0).1.to_bits(), 3.0_f64.to_bits());

    Ok(())
}

#[test]
fn test_from_reader_reports_io_errors() {
    struct FailingReader;

    impl std::io::Read for FailingReader {
        fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("forced read failure"))
        }
    }

    impl std::io::BufRead for FailingReader {
        fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
            Err(std::io::Error::other("forced fill failure"))
        }

        fn consume(&mut self, _amt: usize) {}
    }

    assert!(matches!(
        MGFVec::<f64>::from_reader(FailingReader),
        Err(MascotError::InputIo { .. })
    ));
}

#[test]
fn test_mgf_iter_reads_records_one_by_one() -> Result<()> {
    let document = concat!(
        "BEGIN IONS\n",
        "FEATURE_ID=1\n",
        "PEPMASS=500.0\n",
        "CHARGE=1\n",
        "RTINSECONDS=10.0\n",
        "MSLEVEL=2\n",
        "100.0 2.0\n",
        "SCANS=1\n",
        "END IONS\n",
        "\n",
        "BEGIN IONS\n",
        "FEATURE_ID=2\n",
        "PEPMASS=600.0\n",
        "CHARGE=1\n",
        "RTINSECONDS=12.0\n",
        "MSLEVEL=2\n",
        "200.0 3.0\n",
        "SCANS=2\n",
        "END IONS\n",
    );
    let mut iter = MGFVec::<f32>::iter_from_reader(std::io::Cursor::new(document));

    let first = iter
        .next()
        .transpose()?
        .ok_or(MascotError::SingleRecordExpected { found: 0 })?;
    let second = iter
        .next()
        .transpose()?
        .ok_or(MascotError::SingleRecordExpected { found: 1 })?;

    assert_eq!(first.feature_id(), Some("1"));
    assert_eq!(first.precursor_mz().to_bits(), 500.0_f32.to_bits());
    assert_eq!(second.feature_id(), Some("2"));
    assert_eq!(second.precursor_mz().to_bits(), 600.0_f32.to_bits());
    assert!(iter.next().is_none());

    Ok(())
}

#[test]
fn test_mgf_iter_reads_borrowed_str_without_std_reader() -> Result<()> {
    let document = concat!(
        "BEGIN IONS\n",
        "PEPMASS=500.0\n",
        "CHARGE=1\n",
        "MSLEVEL=2\n",
        "100.0 2.0\n",
        "SCANS=-1\n",
        "END IONS\n",
    );
    let mut iter = MGFVec::<f32>::iter_from_str(document);

    let record = iter
        .next()
        .transpose()?
        .ok_or(MascotError::SingleRecordExpected { found: 0 })?;

    assert_eq!(record.feature_id(), None);
    assert_eq!(record.precursor_mz().to_bits(), 500.0_f32.to_bits());
    assert!(iter.next().is_none());

    Ok(())
}

#[test]
fn test_mgf_iter_reports_line_context_and_stops() {
    let document = concat!(
        "BEGIN IONS\n",
        "FEATURE_ID=\n",
        "PEPMASS=500.0\n",
        "CHARGE=1\n",
        "RTINSECONDS=10.0\n",
        "MSLEVEL=2\n",
        "100.0 2.0\n",
        "SCANS=1\n",
        "END IONS\n",
    );
    let mut iter = MGFVec::<f64>::iter_from_reader(std::io::Cursor::new(document));

    assert!(matches!(
        iter.next(),
        Some(Err(MascotError::InputLine { line_number: 2, .. }))
    ));
    assert!(iter.next().is_none());
}

#[test]
fn test_mgf_iter_skips_unclosed_record_at_eof_in_tolerant_mode() {
    let document = concat!(
        "BEGIN IONS\n",
        "FEATURE_ID=1\n",
        "PEPMASS=500.0\n",
        "MSLEVEL=2\n",
        "100.0 2.0\n",
    );
    let mut iter = MGFIter::<f64, _>::from_document(document).skipping_invalid_records();

    assert!(iter.next().is_none());
    assert_eq!(iter.skipped_records(), 1);
}

#[test]
fn test_mgf_iter_from_path_reads_compressed_records(
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let target_directory =
        std::env::temp_dir().join(format!("mascot-rs-iter-zstd-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&target_directory);
    std::fs::create_dir_all(&target_directory)?;
    let path = target_directory.join("compressed.mgf.zst");
    let document = concat!(
        "BEGIN IONS\n",
        "FEATURE_ID=3\n",
        "PEPMASS=700.0\n",
        "CHARGE=1\n",
        "RTINSECONDS=14.0\n",
        "MSLEVEL=2\n",
        "300.0 4.0\n",
        "SCANS=3\n",
        "END IONS\n",
    );
    let file = std::fs::File::create(&path)?;
    let mut encoder = zstd::stream::write::Encoder::new(file, 0)?;
    std::io::Write::write_all(&mut encoder, document.as_bytes())?;
    encoder.finish()?;

    let mut iter = MGFVec::<f32>::iter_from_path(&path)?;
    let record = iter
        .next()
        .transpose()?
        .ok_or_else(|| std::io::Error::other("missing MGF record"))?;

    assert_eq!(record.feature_id(), Some("3"));
    assert_eq!(record.precursor_mz().to_bits(), 700.0_f32.to_bits());
    assert!(iter.next().is_none());
    drop(iter);
    std::fs::remove_dir_all(&target_directory)?;

    Ok(())
}

#[test]
fn test_from_reader_wraps_build_errors_with_line_context() {
    let document = concat!(
        "BEGIN IONS\n",
        "FEATURE_ID=1\n",
        "PEPMASS=500.0\n",
        "CHARGE=1\n",
        "RTINSECONDS=10.0\n",
        "MSLEVEL=1\n",
        "100.0 2.0\n",
        "SCANS=1\n",
        "END IONS\n",
    );

    assert!(matches!(
        MGFVec::<f64>::from_reader(std::io::Cursor::new(document)),
        Err(MascotError::InputLine { line_number: 9, .. })
    ));
}

#[test]
fn test_from_str_reports_parse_and_build_line_context() {
    let invalid_field = concat!(
        "BEGIN IONS\n",
        "FEATURE_ID=\n",
        "PEPMASS=500.0\n",
        "CHARGE=1\n",
        "RTINSECONDS=10.0\n",
        "MSLEVEL=2\n",
        "100.0 2.0\n",
        "SCANS=1\n",
        "END IONS\n",
    );
    assert!(matches!(
        invalid_field.parse::<MGFVec>(),
        Err(MascotError::InputLine { line_number: 2, .. })
    ));

    let build_error = concat!(
        "BEGIN IONS\n",
        "FEATURE_ID=1\n",
        "PEPMASS=500.0\n",
        "CHARGE=1\n",
        "RTINSECONDS=10.0\n",
        "MSLEVEL=1\n",
        "100.0 2.0\n",
        "SCANS=1\n",
        "END IONS\n",
    );
    assert!(matches!(
        build_error.parse::<MGFVec>(),
        Err(MascotError::InputLine { line_number: 9, .. })
    ));
}

#[test]
fn test_from_str_rejects_empty_records_with_line_context() {
    let document = concat!(
        "BEGIN IONS\n",
        "FEATURE_ID=1\n",
        "PEPMASS=500.0\n",
        "CHARGE=1\n",
        "RTINSECONDS=10.0\n",
        "MSLEVEL=2\n",
        "SCANS=1\n",
        "END IONS\n",
    );

    assert!(matches!(
        document.parse::<MGFVec>(),
        Err(MascotError::InputLine { line_number: 8, .. })
    ));
}

#[test]
fn test_from_path_reads_zstd_compressed_mgf() -> std::result::Result<(), Box<dyn std::error::Error>>
{
    let target_directory =
        std::env::temp_dir().join(format!("mascot-rs-zstd-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&target_directory);
    std::fs::create_dir_all(&target_directory)?;
    let path = target_directory.join("compressed.mgf.zst");
    let document = concat!(
        "BEGIN IONS\n",
        "FEATURE_ID=1\n",
        "PEPMASS=500.0\n",
        "CHARGE=1\n",
        "RTINSECONDS=10.0\n",
        "MSLEVEL=2\n",
        "100.0 2.0\n",
        "SCANS=1\n",
        "END IONS\n",
    );
    let file = std::fs::File::create(&path)?;
    let mut encoder = zstd::stream::write::Encoder::new(file, 0)?;
    std::io::Write::write_all(&mut encoder, document.as_bytes())?;
    encoder.finish()?;

    let mgf: MGFVec<f32> = MGFVec::from_path(&path)?;

    std::fs::remove_dir_all(&target_directory)?;
    assert_eq!(mgf.len(), 1);
    assert_eq!(mgf[0].feature_id(), Some("1"));
    assert_eq!(mgf[0].precursor_mz().to_bits(), 500.0_f32.to_bits());

    Ok(())
}

#[test]
fn test_from_path_reports_open_and_decompression_errors(
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let missing_path =
        std::env::temp_dir().join(format!("mascot-rs-missing-{}.mgf", std::process::id()));
    assert!(matches!(
        MGFVec::<f64>::from_path(&missing_path),
        Err(MascotError::Io { .. })
    ));

    let target_directory = std::env::temp_dir().join(format!(
        "mascot-rs-corrupt-zstd-test-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&target_directory);
    std::fs::create_dir_all(&target_directory)?;
    let path = target_directory.join("corrupt.mgf.zst");
    std::fs::write(&path, b"not a zstd frame")?;

    let result = MGFVec::<f64>::from_path(&path);
    std::fs::remove_dir_all(&target_directory)?;
    assert!(matches!(
        result,
        Err(MascotError::Io { .. } | MascotError::InputIo { .. })
    ));

    Ok(())
}

#[test]
fn test_from_path_reads_gzip_compressed_mgf() -> std::result::Result<(), Box<dyn std::error::Error>>
{
    let target_directory =
        std::env::temp_dir().join(format!("mascot-rs-gzip-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&target_directory);
    std::fs::create_dir_all(&target_directory)?;
    let path = target_directory.join("compressed.mgf.gz");
    let document = concat!(
        "BEGIN IONS\n",
        "FEATURE_ID=2\n",
        "PEPMASS=600.0\n",
        "CHARGE=1\n",
        "RTINSECONDS=12.0\n",
        "MSLEVEL=2\n",
        "200.0 3.0\n",
        "SCANS=2\n",
        "END IONS\n",
    );
    let file = std::fs::File::create(&path)?;
    let mut encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    std::io::Write::write_all(&mut encoder, document.as_bytes())?;
    encoder.finish()?;

    let mgf: MGFVec<f32> = MGFVec::from_path(&path)?;

    std::fs::remove_dir_all(&target_directory)?;
    assert_eq!(mgf.len(), 1);
    assert_eq!(mgf[0].feature_id(), Some("2"));
    assert_eq!(mgf[0].precursor_mz().to_bits(), 600.0_f32.to_bits());

    Ok(())
}

#[test]
fn test_mgf_vec_writes_to_writer_and_round_trips(
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let document = r"BEGIN IONS
FEATURE_ID=1
PEPMASS=500.0
CHARGE=1
RTINSECONDS=10.0
MSLEVEL=2
FILENAME=sample.mzML
SMILES=CCO
FORMULA=C2H6O
IONMODE=Positive
SOURCE_INSTRUMENT=LC-ESI-Orbitrap
SPECTYPE=CORRELATED MS
NAME=Example spectrum
200.0 3.0
100.0 2.0
SCANS=1
END IONS

BEGIN IONS
PEPMASS=600.0
CHARGE=-1
MSLEVEL=2
IONMODE=Negative
300.0 4.0
SCANS=-1
END IONS
";
    let mgf: MGFVec = document.parse()?;
    let mut output = Vec::new();

    mgf.write_to(&mut output)?;

    let serialized = std::str::from_utf8(&output)?;
    let reparsed: MGFVec = serialized.parse()?;

    assert!(serialized.contains("SOURCE_INSTRUMENT=Orbitrap"));
    assert!(serialized.contains("FORMULA=C2H6O"));
    assert!(serialized.contains("NAME=Example spectrum"));
    assert!(serialized.contains("SPECTYPE=CORRELATED MS"));
    assert!(serialized.contains("SCANS=-1"));
    assert_eq!(reparsed.len(), 2);
    assert_eq!(reparsed[0].feature_id(), Some("1"));
    assert_eq!(reparsed[0].metadata().scans(), Some("1"));
    assert_eq!(reparsed[0].metadata().filename(), Some("sample.mzML"));
    assert_eq!(
        reparsed[0]
            .metadata()
            .smiles()
            .map(ToString::to_string)
            .as_deref(),
        Some("CCO")
    );
    assert!(reparsed[0].formula().is_some());
    assert_eq!(reparsed[0].ion_mode(), Some(IonMode::Positive));
    assert_eq!(reparsed[0].source_instrument(), Some(Instrument::Orbitrap));
    assert_eq!(
        reparsed[0].metadata().arbitrary_metadata_value("NAME"),
        Some("Example spectrum")
    );
    assert_eq!(
        reparsed[0].metadata().arbitrary_metadata_value("SPECTYPE"),
        Some("CORRELATED MS")
    );
    assert_eq!(
        reparsed[0]
            .peaks()
            .map(|(mass_divided_by_charge_ratio, fragment_intensity)| {
                (
                    mass_divided_by_charge_ratio.to_bits(),
                    fragment_intensity.to_bits(),
                )
            })
            .collect::<Vec<_>>(),
        vec![
            (100.0_f64.to_bits(), 2.0_f64.to_bits()),
            (200.0_f64.to_bits(), 3.0_f64.to_bits()),
        ]
    );
    assert_eq!(reparsed[1].feature_id(), None);
    assert_eq!(reparsed[1].charge(), Some(-1));
    assert_eq!(reparsed[1].ion_mode(), Some(IonMode::Negative));

    Ok(())
}

#[test]
fn test_single_mgf_writes_structured_validation_headers(
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    const VALID_SPLASH: &str = "splash10-0udi-0490000000-4425acda10ed7d4709bd";

    let document = concat!(
        "BEGIN IONS\n",
        "PEPMASS=250.0\n",
        "CHARGE=1\n",
        "MSLEVEL=2\n",
        "SMILES=CCO\n",
        "FORMULA=C2H6O\n",
        "SPLASH=splash10-0udi-0490000000-4425acda10ed7d4709bd\n",
        "100.0 10.0\n",
        "200.0 20.0\n",
        "END IONS\n",
    );
    let record: MascotGenericFormat = document.parse()?;
    let mut output = Vec::new();

    record.write_to(&mut output)?;

    let serialized = std::str::from_utf8(&output)?;
    let reparsed: MascotGenericFormat = serialized.parse()?;

    assert!(serialized.contains("FORMULA=C2H6O"));
    assert!(serialized.contains(&format!("SPLASH={VALID_SPLASH}")));
    assert_eq!(reparsed.metadata().splash(), Some(VALID_SPLASH));
    assert!(reparsed.formula().is_some());

    Ok(())
}

#[test]
fn test_single_mgf_writes_to_writer_and_round_trips(
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let document = r"BEGIN IONS
FEATURE_ID=7
PEPMASS=700.0
CHARGE=1
RTINSECONDS=14.0
MSLEVEL=2
100.0 2.0
SCANS=7
END IONS
";
    let record: MascotGenericFormat<f32> = document.parse()?;
    let mut output = Vec::new();

    record.write_to(&mut output)?;

    let serialized = std::str::from_utf8(&output)?;
    let reparsed: MascotGenericFormat<f32> = serialized.parse()?;

    assert_eq!(serialized.matches("BEGIN IONS").count(), 1);
    assert_eq!(reparsed.feature_id(), Some("7"));
    assert_eq!(reparsed.metadata().scans(), Some("7"));
    assert_eq!(reparsed.precursor_mz().to_bits(), 700.0_f32.to_bits());
    assert_eq!(reparsed.mz_nth(0).to_bits(), 100.0_f32.to_bits());

    Ok(())
}

#[test]
fn test_unknown_charge_is_not_serialized_as_zero(
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let document = r"BEGIN IONS
FEATURE_ID=8
PEPMASS=800.0
MSLEVEL=2
100.0 2.0
SCANS=8
END IONS
";
    let record: MascotGenericFormat<f32> = document.parse()?;
    let mut output = Vec::new();

    record.write_to(&mut output)?;

    let serialized = std::str::from_utf8(&output)?;
    let reparsed: MascotGenericFormat<f32> = serialized.parse()?;

    assert_eq!(record.charge(), None);
    assert!(!serialized.contains("CHARGE="));
    assert_eq!(reparsed.charge(), None);

    Ok(())
}

#[test]
fn test_mgf_vec_and_record_write_to_compressed_paths(
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let target_directory = std::env::temp_dir().join(format!(
        "mascot-rs-write-compressed-test-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&target_directory);
    std::fs::create_dir_all(&target_directory)?;
    let zstd_path = target_directory.join("written.mgf.zst");
    let gzip_path = target_directory.join("single.mgf.gz");
    let document = r"BEGIN IONS
FEATURE_ID=3
PEPMASS=500.0
CHARGE=1
MSLEVEL=2
100.0 2.0
SCANS=3
END IONS
";
    let mgf: MGFVec<f32> = document.parse()?;
    let record: MascotGenericFormat<f32> = document.parse()?;

    mgf.to_path(&zstd_path)?;
    record.to_path(&gzip_path)?;

    let read_zstd: MGFVec<f32> = MGFVec::from_path(&zstd_path)?;
    let read_gzip: MGFVec<f32> = MGFVec::from_path(&gzip_path)?;

    std::fs::remove_dir_all(&target_directory)?;

    assert_eq!(read_zstd.len(), 1);
    assert_eq!(read_zstd[0].feature_id(), Some("3"));
    assert_eq!(read_zstd[0].precursor_mz().to_bits(), 500.0_f32.to_bits());
    assert_eq!(read_gzip.len(), 1);
    assert_eq!(read_gzip[0].feature_id(), Some("3"));

    Ok(())
}

#[test]
fn test_write_to_reports_writer_errors() -> Result<()> {
    struct FailingWriter;

    impl std::io::Write for FailingWriter {
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("forced write failure"))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let document = r"BEGIN IONS
FEATURE_ID=1
PEPMASS=500.0
CHARGE=1
MSLEVEL=2
100.0 2.0
SCANS=1
END IONS
";
    let mgf: MGFVec = document.parse()?;

    assert!(matches!(
        mgf.write_to(FailingWriter),
        Err(MascotError::OutputIo { .. })
    ));

    Ok(())
}

#[test]
fn test_unsorted_duplicate_peaks_are_normalized() -> Result<()> {
    let lines = [
        "FEATURE_ID=1",
        "PEPMASS=500.0",
        "SCANS=1",
        "CHARGE=1",
        "RTINSECONDS=10.0",
        "BEGIN IONS",
        "MSLEVEL=2",
        "200.0 1.0",
        "100.0 2.0",
        "100.0 3.0",
        "END IONS",
    ];

    let mgf: MGFVec = MGFVec::try_from_iter(lines)?;
    let spectrum = &mgf[0];

    let mz_bits = spectrum.mz().map(f64::to_bits).collect::<Vec<_>>();
    assert_eq!(mz_bits, vec![100.0_f64.to_bits(), 200.0_f64.to_bits()]);

    let intensity_bits = spectrum.intensities().map(f64::to_bits).collect::<Vec<_>>();
    assert_eq!(intensity_bits, vec![5.0_f64.to_bits(), 1.0_f64.to_bits()]);

    Ok(())
}

#[test]
fn test_ms_level_is_u8_not_two_value_enum() -> Result<()> {
    let lines = [
        "FEATURE_ID=1",
        "PEPMASS=500.0",
        "SCANS=1",
        "CHARGE=1",
        "RTINSECONDS=10.0",
        "BEGIN IONS",
        "MSLEVEL=3",
        "100.0 2.0",
        "END IONS",
    ];

    let mgf: MGFVec = MGFVec::try_from_iter(lines)?;

    assert_eq!(mgf[0].level(), 3);
    assert_eq!(mgf[0].metadata().level(), 3);

    Ok(())
}

#[test]
fn test_duplicate_feature_ids_are_distinct_ion_blocks() -> Result<()> {
    let lines = [
        "BEGIN IONS",
        "FEATURE_ID=1",
        "PEPMASS=500.0",
        "CHARGE=1",
        "RTINSECONDS=10.0",
        "MSLEVEL=1",
        "500.0 1.0",
        "SCANS=-1",
        "END IONS",
        "BEGIN IONS",
        "FEATURE_ID=1",
        "PEPMASS=500.0",
        "CHARGE=1",
        "RTINSECONDS=10.0",
        "MSLEVEL=2",
        "100.0 2.0",
        "SCANS=1",
        "END IONS",
    ];

    let mgf: MGFVec = MGFVec::try_from_iter(lines)?;
    let records: &[MascotGenericFormat] = mgf.as_ref();

    assert_eq!(mgf.len(), 2);
    assert_eq!(records.len(), 2);
    assert_eq!(mgf[0].feature_id(), Some("1"));
    assert_eq!(mgf[0].level(), 1);
    assert_eq!(mgf[1].feature_id(), Some("1"));
    assert_eq!(mgf[1].level(), 2);

    Ok(())
}

#[test]
fn test_mgf_vec_from_str_accepts_multiple_ion_blocks() -> Result<()> {
    let document = concat!(
        "BEGIN IONS\n",
        "FEATURE_ID=1\n",
        "PEPMASS=500.0\n",
        "CHARGE=1\n",
        "RTINSECONDS=10.0\n",
        "MSLEVEL=2\n",
        "100.0 2.0\n",
        "SCANS=1\n",
        "END IONS\n",
        "\n",
        "BEGIN IONS\n",
        "FEATURE_ID=2\n",
        "PEPMASS=600.0\n",
        "CHARGE=1\n",
        "RTINSECONDS=12.0\n",
        "MSLEVEL=2\n",
        "200.0 3.0\n",
        "SCANS=2\n",
        "END IONS\n",
    );

    let mgf: MGFVec<f32> = document.parse()?;

    assert_eq!(mgf.len(), 2);
    assert_eq!(mgf[0].feature_id(), Some("1"));
    assert_eq!(mgf[1].feature_id(), Some("2"));
    assert_eq!(mgf[1].precursor_mz().to_bits(), 600.0_f32.to_bits());

    Ok(())
}

#[test]
fn test_mgf_vec_iteration_helpers_are_standard_collection_interfaces() -> Result<()> {
    let document = concat!(
        "BEGIN IONS\n",
        "FEATURE_ID=1\n",
        "PEPMASS=500.0\n",
        "CHARGE=1\n",
        "RTINSECONDS=10.0\n",
        "MSLEVEL=2\n",
        "100.0 2.0\n",
        "SCANS=1\n",
        "END IONS\n",
        "\n",
        "BEGIN IONS\n",
        "FEATURE_ID=2\n",
        "PEPMASS=600.0\n",
        "CHARGE=1\n",
        "RTINSECONDS=12.0\n",
        "MSLEVEL=2\n",
        "200.0 3.0\n",
        "SCANS=2\n",
        "END IONS\n",
    );
    let mut mgf: MGFVec = document.parse()?;

    let iter_ids = mgf
        .iter()
        .map(|record| record.feature_id().map(ToString::to_string))
        .collect::<Vec<_>>();
    let borrowed_iter_ids = (&mgf)
        .into_iter()
        .map(|record| record.feature_id().map(ToString::to_string))
        .collect::<Vec<_>>();
    mgf.iter_mut().for_each(|record| {
        record
            .metadata_mut()
            .insert_arbitrary_metadata("ITER_MUT", "yes");
    });
    for record in &mut mgf {
        record
            .metadata_mut()
            .insert_arbitrary_metadata("BORROWED_MUT", "yes");
    }
    assert!(mgf.iter().all(|record| {
        record.metadata().arbitrary_metadata_value("ITER_MUT") == Some("yes")
            && record.metadata().arbitrary_metadata_value("BORROWED_MUT") == Some("yes")
    }));
    let filtered: MGFVec = mgf
        .into_iter()
        .filter(|record| record.feature_id() == Some("2"))
        .collect();

    assert_eq!(iter_ids, vec![Some("1".to_string()), Some("2".to_string())]);
    assert_eq!(borrowed_iter_ids, iter_ids);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].feature_id(), Some("2"));

    Ok(())
}

#[test]
fn test_mgf_from_str_requires_exactly_one_ion_block() -> Result<()> {
    let document = concat!(
        "BEGIN IONS\n",
        "FEATURE_ID=1\n",
        "PEPMASS=500.0\n",
        "CHARGE=1\n",
        "RTINSECONDS=10.0\n",
        "MSLEVEL=2\n",
        "100.0 2.0\n",
        "SCANS=1\n",
        "END IONS\n",
    );
    let two_record_document = concat!(
        "BEGIN IONS\n",
        "FEATURE_ID=1\n",
        "PEPMASS=500.0\n",
        "CHARGE=1\n",
        "RTINSECONDS=10.0\n",
        "MSLEVEL=2\n",
        "100.0 2.0\n",
        "SCANS=1\n",
        "END IONS\n",
        "BEGIN IONS\n",
        "FEATURE_ID=2\n",
        "PEPMASS=600.0\n",
        "CHARGE=1\n",
        "RTINSECONDS=12.0\n",
        "MSLEVEL=2\n",
        "200.0 3.0\n",
        "SCANS=2\n",
        "END IONS\n",
    );

    let record: MascotGenericFormat = document.parse()?;

    assert_eq!(record.feature_id(), Some("1"));
    assert!(matches!(
        "".parse::<MascotGenericFormat>(),
        Err(MascotError::SingleRecordExpected { found: 0 })
    ));
    assert!(matches!(
        two_record_document.parse::<MascotGenericFormat>(),
        Err(MascotError::SingleRecordExpected { found: 2 })
    ));

    Ok(())
}

#[test]
fn test_spectrum_access_uses_standard_traits() -> Result<()> {
    let lines = [
        "BEGIN IONS",
        "FEATURE_ID=1",
        "PEPMASS=500.0",
        "CHARGE=1",
        "RTINSECONDS=10.0",
        "MSLEVEL=2",
        "SMILES=CCO",
        "IONMODE=Positive",
        "SOURCE_INSTRUMENT=LC-ESI-Orbitrap",
        "100.0 2.0",
        "200.0 3.0",
        "SCANS=1",
        "END IONS",
    ];

    let mgf: MGFVec = MGFVec::try_from_iter(lines)?;

    let spectrum_ref: &GenericSpectrum = mgf[0].as_ref();
    assert_eq!(spectrum_ref.len(), 2);
    assert_eq!(spectrum_ref.mz_nth(0).to_bits(), 100.0_f64.to_bits());
    assert_eq!(spectrum_ref.peak_nth(1).1.to_bits(), 3.0_f64.to_bits());
    assert_eq!(
        spectrum_ref.mz_from(1).next().map(f64::to_bits),
        Some(200.0_f64.to_bits())
    );
    assert_eq!(
        spectrum_ref
            .peaks()
            .map(|peak| peak.0.to_bits())
            .collect::<Vec<_>>(),
        vec![100.0_f64.to_bits(), 200.0_f64.to_bits()]
    );
    assert_eq!(mgf[0].intensity_nth(0).to_bits(), 2.0_f64.to_bits());
    assert_eq!(mgf[0].mz_nth(1).to_bits(), 200.0_f64.to_bits());
    assert_eq!(
        mgf[0].mz_from(1).next().map(f64::to_bits),
        Some(200.0_f64.to_bits())
    );
    assert_eq!(
        mgf[0]
            .peaks()
            .map(|peak| peak.1.to_bits())
            .collect::<Vec<_>>(),
        vec![2.0_f64.to_bits(), 3.0_f64.to_bits()]
    );
    assert_eq!(mgf[0].peak_nth(1).0.to_bits(), 200.0_f64.to_bits());
    assert_eq!(mgf[0].scans(), Some("1"));
    assert_eq!(mgf[0].charge(), Some(1));
    assert_eq!(mgf[0].ion_mode(), Some(IonMode::Positive));
    assert!(mgf[0].ion_mode().is_some_and(IonMode::is_positive));
    assert_eq!(mgf[0].source_instrument(), Some(Instrument::Orbitrap));
    assert_eq!(Spectra::len(&mgf), 1);
    assert_eq!(
        mgf.spectra().next().map(MascotGenericFormat::feature_id),
        Some(Some("1"))
    );

    let metadata =
        MascotGenericFormatMetadata::new(Some("1".to_string()), 2, Some(10.0), Some(1), None)?;
    let record = MascotGenericFormat::new(metadata, 500.0, vec![100.0, 200.0], vec![2.0, 3.0])?;
    let spectrum: GenericSpectrum = record.into();
    assert_eq!(spectrum.len(), 2);
    assert_eq!(spectrum.intensity_nth(1).to_bits(), 3.0_f64.to_bits());

    Ok(())
}

#[test]
fn test_metadata_parses_optional_smiles() -> Result<()> {
    let lines = [
        "BEGIN IONS",
        "FEATURE_ID=1",
        "PEPMASS=500.0",
        "CHARGE=1",
        "RTINSECONDS=10.0",
        "MSLEVEL=2",
        "SMILES=CCO",
        "100.0 2.0",
        "SCANS=1",
        "END IONS",
    ];

    let mgf: MGFVec = MGFVec::try_from_iter(lines)?;

    assert_eq!(
        mgf[0]
            .metadata()
            .smiles()
            .map(ToString::to_string)
            .as_deref(),
        Some("CCO")
    );

    Ok(())
}

#[test]
fn test_metadata_parses_optional_ion_mode() -> Result<()> {
    let positive_lines = [
        "BEGIN IONS",
        "PEPMASS=500.0",
        "CHARGE=1",
        "RTINSECONDS=10.0",
        "MSLEVEL=2",
        "IONMODE=Positive",
        "100.0 2.0",
        "SCANS=1",
        "END IONS",
    ];
    let negative_lines = [
        "BEGIN IONS",
        "PEPMASS=500.0",
        "CHARGE=-1",
        "RTINSECONDS=10.0",
        "MSLEVEL=2",
        "IONMODE=negative",
        "100.0 2.0",
        "SCANS=1",
        "END IONS",
    ];
    let missing_lines = [
        "BEGIN IONS",
        "PEPMASS=500.0",
        "CHARGE=1",
        "RTINSECONDS=10.0",
        "MSLEVEL=2",
        "IONMODE=N/A",
        "100.0 2.0",
        "SCANS=1",
        "END IONS",
    ];

    let positive_mgf: MGFVec = MGFVec::try_from_iter(positive_lines)?;
    let negative_mgf: MGFVec = MGFVec::try_from_iter(negative_lines)?;
    let missing_mgf: MGFVec = MGFVec::try_from_iter(missing_lines)?;
    let metadata: MascotGenericFormatMetadata =
        MascotGenericFormatMetadata::new_with_smiles_and_ion_mode(
            Some("1".to_string()),
            2,
            None,
            Some(-1),
            None,
            None,
            Some(IonMode::Negative),
        )?;

    assert_eq!(positive_mgf[0].ion_mode(), Some(IonMode::Positive));
    assert_eq!(
        positive_mgf[0].metadata().ion_mode(),
        Some(IonMode::Positive)
    );
    assert_eq!(negative_mgf[0].ion_mode(), Some(IonMode::Negative));
    assert!(negative_mgf[0].ion_mode().is_some_and(IonMode::is_negative));
    assert_eq!(IonMode::Positive.as_str(), "Positive");
    assert_eq!(IonMode::Negative.to_string(), "Negative");
    assert_eq!(metadata.ion_mode(), Some(IonMode::Negative));
    assert!(matches!(
        MascotGenericFormatMetadata::new_with_smiles_and_ion_mode(
            Some("1".to_string()),
            2,
            None,
            Some(1),
            None,
            None,
            Some(IonMode::Negative),
        ),
        Err(MascotError::ChargeIonModeMismatch {
            charge: 1,
            ion_mode: "Negative",
        })
    ));
    assert_eq!(missing_mgf[0].ion_mode(), None);

    Ok(())
}

#[test]
fn test_metadata_parses_optional_source_instrument() -> Result<()> {
    let liquid_chromatography_orbitrap = [
        "BEGIN IONS",
        "PEPMASS=500.0",
        "CHARGE=1",
        "RTINSECONDS=10.0",
        "MSLEVEL=2",
        "SOURCE_INSTRUMENT=LC-ESI-Orbitrap",
        "100.0 2.0",
        "SCANS=1",
        "END IONS",
    ];
    let normalized_qtof = [
        "BEGIN IONS",
        "PEPMASS=500.0",
        "CHARGE=1",
        "RTINSECONDS=10.0",
        "MSLEVEL=2",
        "SOURCE_INSTRUMENT=ESI-LC-ESI-QTOF",
        "100.0 2.0",
        "SCANS=1",
        "END IONS",
    ];
    let missing_source_instrument = [
        "BEGIN IONS",
        "PEPMASS=500.0",
        "CHARGE=1",
        "RTINSECONDS=10.0",
        "MSLEVEL=2",
        "SOURCE_INSTRUMENT=N/A-N/A",
        "100.0 2.0",
        "SCANS=1",
        "END IONS",
    ];
    let unknown_source_instrument = [
        "BEGIN IONS",
        "PEPMASS=500.0",
        "CHARGE=1",
        "RTINSECONDS=10.0",
        "MSLEVEL=2",
        "SOURCE_INSTRUMENT=Vendor new analyzer 9000",
        "100.0 2.0",
        "SCANS=1",
        "END IONS",
    ];

    let orbitrap_mgf: MGFVec = MGFVec::try_from_iter(liquid_chromatography_orbitrap)?;
    let qtof_mgf: MGFVec = MGFVec::try_from_iter(normalized_qtof)?;
    let missing_mgf: MGFVec = MGFVec::try_from_iter(missing_source_instrument)?;
    let unknown_mgf: MGFVec = MGFVec::try_from_iter(unknown_source_instrument)?;
    let metadata: MascotGenericFormatMetadata =
        MascotGenericFormatMetadata::new_with_smiles_and_ion_mode(
            Some("1".to_string()),
            2,
            None,
            Some(1),
            None,
            None,
            Some(IonMode::Positive),
        )?
        .with_source_instrument(Some(Instrument::Orbitrap));

    assert_eq!(
        orbitrap_mgf[0].source_instrument(),
        Some(Instrument::Orbitrap)
    );
    assert_eq!(
        orbitrap_mgf[0].metadata().source_instrument(),
        Some(Instrument::Orbitrap)
    );
    assert_eq!(
        qtof_mgf[0].source_instrument(),
        Some(Instrument::TimeOfFlight)
    );
    assert_eq!(missing_mgf[0].source_instrument(), None);
    assert_eq!(unknown_mgf[0].source_instrument(), Some(Instrument::Other));
    assert_eq!(missing_mgf[0].formula(), None);
    assert_eq!(Instrument::Quadrupole.to_string(), "Quadrupole");
    assert_eq!(Instrument::Orbitrap.as_str(), "Orbitrap");
    assert_eq!(Instrument::TimeOfFlight.as_str(), "TOF");
    assert_eq!(Instrument::IonTrap.as_str(), "Ion trap");
    assert_eq!(Instrument::FourierTransform.as_str(), "Fourier transform");
    assert_eq!(Instrument::MagneticSector.as_str(), "Magnetic sector");
    assert_eq!(Instrument::Other.as_str(), "Other");
    assert_eq!(metadata.source_instrument(), Some(Instrument::Orbitrap));

    Ok(())
}

#[test]
fn test_metadata_inserts_arbitrary_metadata() -> Result<()> {
    let mut metadata =
        MascotGenericFormatMetadata::new(Some("1".to_string()), 2, None, Some(1), None)?;

    let first_previous_value = metadata.insert_arbitrary_metadata("SPECTRUMID", "old-id");
    let name_previous_value = metadata.insert_arbitrary_metadata("NAME", "Example");
    let second_previous_value = metadata.insert_arbitrary_metadata("SPECTRUMID", "new-id");

    assert_eq!(first_previous_value, None);
    assert_eq!(name_previous_value, None);
    assert_eq!(second_previous_value.as_deref(), Some("old-id"));
    assert_eq!(
        metadata.arbitrary_metadata(),
        &[
            ("NAME".to_string(), "Example".to_string()),
            ("SPECTRUMID".to_string(), "new-id".to_string()),
        ]
    );

    Ok(())
}

#[test]
fn test_metadata_rejects_invalid_smiles() {
    let lines = [
        "BEGIN IONS",
        "FEATURE_ID=1",
        "PEPMASS=500.0",
        "CHARGE=1",
        "RTINSECONDS=10.0",
        "MSLEVEL=2",
        "SMILES=C(",
        "100.0 2.0",
        "SCANS=1",
        "END IONS",
    ];

    assert!(matches!(
        MGFVec::<f64>::try_from_iter(lines),
        Err(MascotError::InputLine {
            line_number: 7,
            source,
            ..
        }) if matches!(*source, MascotError::InvalidSmiles { .. })
    ));
}

#[test]
fn test_record_constructor_rejects_invalid_peak_inputs() -> Result<()> {
    let metadata =
        MascotGenericFormatMetadata::new(Some("1".to_string()), 2, Some(10.0), Some(1), None)?;
    assert!(matches!(
        MascotGenericFormat::new(metadata.clone(), 500.0, vec![100.0], vec![]),
        Err(MascotError::PeakVectorLengthMismatch { .. })
    ));
    assert!(matches!(
        MascotGenericFormat::new(metadata, 500.0, vec![], vec![]),
        Err(MascotError::EmptyPeakVectors)
    ));

    let first_level_metadata =
        MascotGenericFormatMetadata::new(Some("1".to_string()), 1, Some(10.0), Some(1), None)?;
    assert!(matches!(
        MascotGenericFormat::new(first_level_metadata, 500.0, vec![100.0], vec![1.0]),
        Err(MascotError::FirstLevelPrecursorMzMismatch { .. })
    ));

    Ok(())
}

#[test]
fn test_metadata_rejects_invalid_values() {
    assert!(matches!(
        MascotGenericFormatMetadata::new(Some("1".to_string()), 0, Some(10.0), Some(1), None),
        Err(MascotError::NonPositiveField {
            field: "fragmentation level",
            ..
        })
    ));
    assert!(matches!(
        MascotGenericFormatMetadata::new(Some("1".to_string()), 2, Some(0.0), Some(1), None),
        Err(MascotError::NonPositiveField {
            field: "retention time",
            ..
        })
    ));
    assert!(matches!(
        MascotGenericFormatMetadata::new(Some("1".to_string()), 2, Some(f64::NAN), Some(1), None),
        Err(MascotError::NonFiniteField {
            field: "retention time",
            ..
        })
    ));
    assert!(matches!(
        MascotGenericFormatMetadata::new(
            Some("1".to_string()),
            2,
            Some(10.0),
            Some(1),
            Some(String::new()),
        ),
        Err(MascotError::EmptyFilename)
    ));
}

#[test]
fn test_precision_generic_can_store_f32_spectra() -> Result<()> {
    let lines = [
        "BEGIN IONS",
        "FEATURE_ID=1",
        "PEPMASS=500.0",
        "CHARGE=1",
        "RTINSECONDS=10.0",
        "MSLEVEL=2",
        "100.0 2.0",
        "200.0 3.0",
        "SCANS=1",
        "END IONS",
    ];

    let mgf: MGFVec<f32> = MGFVec::try_from_iter(lines)?;
    let records: &[MascotGenericFormat<f32>] = mgf.as_ref();
    let spectrum_ref: &GenericSpectrum<f32> = records[0].as_ref();

    assert_eq!(mgf[0].precursor_mz().to_bits(), 500.0_f32.to_bits());
    assert_eq!(spectrum_ref.mz_nth(0).to_bits(), 100.0_f32.to_bits());
    assert_eq!(spectrum_ref.intensity_nth(1).to_bits(), 3.0_f32.to_bits());

    let metadata =
        MascotGenericFormatMetadata::new(Some("1".to_string()), 2, Some(10.0), Some(1), None)?;
    let record: MascotGenericFormat<f32> =
        MascotGenericFormat::new(metadata, 500.0, vec![100.0, 200.0], vec![2.0, 3.0])?;
    let spectrum: GenericSpectrum<f32> = record.into();
    assert_eq!(spectrum.precursor_mz().to_bits(), 500.0_f32.to_bits());

    Ok(())
}

#[test]
fn test_mgf_record_implements_allocable_spectrum_traits() -> Result<()> {
    fn assert_allocable<S: SpectrumAlloc<Precision = f32>>(_spectrum: &S) {}

    let mut allocated = MascotGenericFormat::<f32>::with_capacity(500.0, 2)?;
    assert_allocable(&allocated);
    assert_eq!(allocated.level(), 2);
    assert_eq!(allocated.charge(), None);
    assert!(allocated.feature_id().is_none());
    assert!(allocated.is_empty());

    allocated.add_peak(100.0, 2.0)?;
    allocated.add_peak(200.0, 3.0)?;
    assert_eq!(allocated.len(), 2);

    let metadata =
        MascotGenericFormatMetadata::new(Some("7".to_string()), 2, Some(10.0), Some(1), None)?;
    let record: MascotGenericFormat<f32> = MascotGenericFormat::new(
        metadata,
        500.0,
        vec![100.0, 150.0, 200.0, 250.0],
        vec![2.0, 5.0, 3.0, 5.0],
    )?;

    let top = record.top_k_peaks(2)?;
    assert_eq!(top.feature_id(), Some("7"));
    assert_eq!(top.len(), 2);
    assert_eq!(
        top.peaks()
            .map(|(mz, intensity)| (mz.to_bits(), intensity.to_bits()))
            .collect::<Vec<_>>(),
        vec![
            (150.0_f32.to_bits(), 5.0_f32.to_bits()),
            (250.0_f32.to_bits(), 5.0_f32.to_bits()),
        ]
    );

    Ok(())
}

#[test]
fn test_memory_footprint_is_available_from_prelude() -> Result<()> {
    let lines = [
        "BEGIN IONS",
        "FEATURE_ID=1",
        "PEPMASS=500.0",
        "CHARGE=1",
        "RTINSECONDS=10.0",
        "MSLEVEL=2",
        "100.0 2.0",
        "200.0 3.0",
        "SCANS=1",
        "END IONS",
    ];

    let mgf: MGFVec = MGFVec::try_from_iter(lines)?;
    let size = mgf.mem_size(SizeFlags::default());
    let capacity_size = mgf.mem_size(SizeFlags::CAPACITY);
    let metadata_size = mgf[0].metadata().mem_size(SizeFlags::default());
    let builder = MGFVec::<f64>::gnps();
    let builder_size = builder.mem_size(SizeFlags::default());
    let mut report = String::new();

    assert!(size >= std::mem::size_of_val(&mgf));
    assert!(capacity_size >= size);
    let metadata_without_smiles =
        MascotGenericFormatMetadata::new(Some("1".to_string()), 2, Some(10.0), Some(1), None)?
            .with_scans(Some("1".to_string()));
    assert!(metadata_size >= std::mem::size_of_val(mgf[0].metadata()));
    assert_eq!(
        metadata_size,
        metadata_without_smiles.mem_size(SizeFlags::default())
    );
    assert!(builder_size >= std::mem::size_of_val(&builder));
    assert!(mgf
        .mem_dbg_depth_on(&mut report, 2, DbgFlags::default())
        .is_ok());
    assert!(report.contains("MGFVec"));

    Ok(())
}

#[test]
fn test_gnps_library_records_parse_annotation_metadata() -> Result<()> {
    let lines = [
        "BEGIN IONS",
        "PEPMASS=561.365",
        "CHARGE=1",
        "MSLEVEL=2",
        "SOURCE_INSTRUMENT=DI-ESI-LTQ-FT-ICR",
        "FILENAME=Desferrioxamine_B_1H_561_3647.mzXML",
        "SEQ=..",
        "IONMODE=Positive",
        "ORGANISM=GNPS-LIBRARY",
        "NAME=Desferrioxamine B M+H",
        "PI=Dorrestein",
        "DATACOLLECTOR=J Watrous",
        "SMILES=N/A",
        "INCHI=N/A",
        "INCHIAUX=N/A",
        "PUBMED=N/A",
        "SUBMITUSER=jdwatrou",
        "TAGS=",
        "LIBRARYQUALITY=3",
        "SPECTRUMID=CCMSLIB00000072100",
        "SCANS=1",
        "161.0 2.216415",
        "161.27272 3.386504",
        "165.181824 94.756683",
        "END IONS",
    ];

    let mgf: MGFVec = MGFVec::try_from_iter(lines)?;

    assert_eq!(mgf.len(), 1);
    assert_eq!(mgf[0].feature_id(), None);
    assert_eq!(mgf[0].metadata().scans(), Some("1"));
    assert_eq!(mgf[0].metadata().retention_time(), None);
    assert!(mgf[0].metadata().smiles().is_none());
    assert_eq!(mgf[0].metadata().ion_mode(), Some(IonMode::Positive));
    assert_eq!(
        mgf[0].metadata().source_instrument(),
        Some(Instrument::FourierTransform)
    );
    assert_eq!(
        mgf[0].metadata().arbitrary_metadata(),
        &[
            ("DATACOLLECTOR".to_string(), "J Watrous".to_string()),
            ("INCHI".to_string(), "N/A".to_string()),
            ("INCHIAUX".to_string(), "N/A".to_string()),
            ("LIBRARYQUALITY".to_string(), "3".to_string()),
            ("NAME".to_string(), "Desferrioxamine B M+H".to_string()),
            ("ORGANISM".to_string(), "GNPS-LIBRARY".to_string()),
            ("PI".to_string(), "Dorrestein".to_string()),
            ("PUBMED".to_string(), "N/A".to_string()),
            ("SEQ".to_string(), "..".to_string()),
            ("SPECTRUMID".to_string(), "CCMSLIB00000072100".to_string(),),
            ("SUBMITUSER".to_string(), "jdwatrou".to_string()),
            ("TAGS".to_string(), String::new()),
        ]
    );
    assert_eq!(
        mgf[0].metadata().arbitrary_metadata_value("SPECTRUMID"),
        Some("CCMSLIB00000072100")
    );
    assert_eq!(mgf[0].level(), 2);
    assert_eq!(mgf[0].len(), 3);

    Ok(())
}

#[test]
fn test_records_without_feature_id_are_accepted() -> Result<()> {
    let lines = [
        "BEGIN IONS",
        "PEPMASS=370.165",
        "CHARGE=1",
        "MSLEVEL=2",
        "SOURCE_INSTRUMENT=ESI-Qtof",
        "FILENAME=20250403-FIMS-Positive-CE35CES15-Allocryptopine.mzML",
        "SMILES=CN1CCC2C=C3C(OCO3)=CC=2C(CC2=CC=C(C(OC)=C2C1)OC)=O",
        "SPECTRUMID=CCMSLIB00013748121",
        "SCANS=-1",
        "106.063454 3.229225",
        "109.028343 3.069424",
        "END IONS",
    ];

    let mgf: MGFVec = MGFVec::try_from_iter(lines)?;

    assert_eq!(mgf.len(), 1);
    assert_eq!(mgf[0].feature_id(), None);
    assert_eq!(mgf[0].metadata().feature_id(), None);
    assert_eq!(mgf[0].source_instrument(), Some(Instrument::TimeOfFlight));
    assert_eq!(
        mgf[0].metadata().arbitrary_metadata_value("SPECTRUMID"),
        Some("CCMSLIB00013748121")
    );
    assert_eq!(mgf[0].len(), 2);

    Ok(())
}

#[test]
fn test_nonnumeric_feature_ids_are_preserved() -> Result<()> {
    let lines = [
        "BEGIN IONS",
        "FEATURE_ID=20240502_pluskal_enamine_5008_G17_id_MSn_negative.mzML msn trees:2",
        "PEPMASS=370.165",
        "CHARGE=1",
        "MSLEVEL=2",
        "100.0 2.0",
        "SCANS=176-199",
        "END IONS",
    ];

    let mgf: MGFVec = MGFVec::try_from_iter(lines)?;

    assert_eq!(
        mgf[0].feature_id(),
        Some("20240502_pluskal_enamine_5008_G17_id_MSn_negative.mzML msn trees:2")
    );
    assert_eq!(mgf[0].metadata().scans(), Some("176-199"));

    Ok(())
}

#[test]
fn test_empty_records_are_rejected_by_strict_parser() {
    let lines = [
        "BEGIN IONS",
        "PEPMASS=763.0",
        "CHARGE=1",
        "MSLEVEL=2",
        "FILENAME=f.smascuch/Standards/STANDARD_Ferrichrome_FT01_50K_MS2_mz763.mzXML;",
        "NAME=Ferrichrome M+Na",
        "SPECTRUMID=CCMSLIB00000078897",
        "SCANS=1",
        "END IONS",
    ];

    assert!(matches!(
        MGFVec::<f64>::try_from_iter(lines),
        Err(MascotError::InputLine {
            line_number: 9,
            source,
            ..
        }) if matches!(*source, MascotError::EmptyPeakVectors)
    ));
}

#[test]
fn test_gnps_builder_loads_existing_downloaded_file() -> Result<()> {
    let target_directory =
        std::env::temp_dir().join(format!("mascot-rs-gnps-test-{}", std::process::id()));
    std::fs::create_dir_all(&target_directory).map_err(|source| MascotError::Io {
        path: target_directory.display().to_string(),
        source,
    })?;
    let builder = MGFVec::<f32>::gnps()
        .url("https://example.invalid/ALL_GNPS.mgf")
        .target_directory(&target_directory)
        .file_name("cached.mgf")
        .verbose()
        .force_download(false);
    let path = builder.path();
    let document = concat!(
        "BEGIN IONS\n",
        "PEPMASS=321.0\n",
        "BEGIN IONS\n",
        "BEGIN IONS\n",
        "PEPMASS=0.0\n",
        "CHARGE=1\n",
        "MSLEVEL=2\n",
        "NAME=Invalid GNPS record\n",
        "SPECTRUMID=CCMSLIB00000000000\n",
        "SCANS=1\n",
        "161.0 2.216415\n",
        "END IONS\n",
        "BEGIN IONS\n",
        "PEPMASS=763.0\n",
        "CHARGE=1\n",
        "MSLEVEL=2\n",
        "NAME=Empty GNPS record\n",
        "SPECTRUMID=CCMSLIB00000078897\n",
        "SCANS=2\n",
        "END IONS\n",
        "BEGIN IONS\n",
        "PEPMASS=561.365\n",
        "CHARGE=1\n",
        "MSLEVEL=2\n",
        "NAME=Desferrioxamine B M+H\n",
        "SPECTRUMID=CCMSLIB00000072100\n",
        "SCANS=3\n",
        "161.0 2.216415\n",
        "END IONS\n",
        "BEGIN IONS\n",
        "PEPMASS=500.0\n",
        "CHARGE=1\n",
        "MSLEVEL=2\n",
        "SCANS=4\n",
        "100.0 2.0\n",
    );
    std::fs::write(&path, document).map_err(|source| MascotError::Io {
        path: path.display().to_string(),
        source,
    })?;
    let expected_bytes = document.len() as u64;

    let gnps_load = pollster::block_on(builder.load())?;
    let _ = std::fs::remove_dir_all(&target_directory);

    assert!(gnps_load.mem_size(SizeFlags::default()) >= std::mem::size_of_val(&gnps_load));
    assert_eq!(gnps_load.spectra().len(), 1);
    assert_eq!(gnps_load.as_ref().len(), 1);
    assert_eq!(gnps_load.skipped_records(), 5);
    assert_eq!(gnps_load.path(), path.as_path());
    assert_eq!(gnps_load.bytes(), expected_bytes);
    assert_eq!(gnps_load.spectra()[0].metadata().retention_time(), None);
    assert_eq!(
        gnps_load.spectra()[0].precursor_mz().to_bits(),
        561.365_f32.to_bits()
    );
    let spectra = gnps_load.into_spectra();
    assert_eq!(spectra.len(), 1);

    Ok(())
}

#[test]
fn test_gnps_builder_iterates_existing_downloaded_file() -> Result<()> {
    let target_directory =
        std::env::temp_dir().join(format!("mascot-rs-gnps-iter-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&target_directory);
    std::fs::create_dir_all(&target_directory).map_err(|source| MascotError::Io {
        path: target_directory.display().to_string(),
        source,
    })?;
    let builder = MGFVec::<f32>::gnps()
        .url("https://example.invalid/ALL_GNPS.mgf")
        .target_directory(&target_directory)
        .file_name("cached.mgf")
        .force_download(false);
    let path = builder.path();
    let document = r"BEGIN IONS
PEPMASS=0.0
CHARGE=1
MSLEVEL=2
SCANS=1
100.0 2.0
END IONS
BEGIN IONS
PEPMASS=561.365
CHARGE=1
MSLEVEL=2
SCANS=2
161.0 2.216415
END IONS
";
    std::fs::write(&path, document).map_err(|source| MascotError::Io {
        path: path.display().to_string(),
        source,
    })?;

    let mut iter = pollster::block_on(<GNPSBuilder<f32> as Dataset>::mgf_iter(builder))?;
    let first = iter.next().transpose()?.ok_or(MascotError::MissingField {
        builder: "test",
        field: "first_record",
    })?;
    assert_eq!(first.precursor_mz().to_bits(), 561.365_f32.to_bits());
    assert!(iter.next().is_none());
    assert_eq!(iter.skipped_records(), 1);
    let _ = std::fs::remove_dir_all(&target_directory);

    Ok(())
}

#[test]
fn test_gnps_builder_downloads_existing_file_without_loading() -> Result<()> {
    let target_directory = std::env::temp_dir().join(format!(
        "mascot-rs-gnps-download-test-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&target_directory);
    std::fs::create_dir_all(&target_directory).map_err(|source| MascotError::Io {
        path: target_directory.display().to_string(),
        source,
    })?;
    let builder = MGFVec::<f32>::gnps()
        .url("https://example.invalid/ALL_GNPS.mgf")
        .target_directory(&target_directory)
        .file_name("cached-invalid.mgf")
        .force_download(false);
    let path = builder.path();
    let document = "this is cached but not a valid MGF document\n";
    std::fs::write(&path, document).map_err(|source| MascotError::Io {
        path: path.display().to_string(),
        source,
    })?;

    let download = pollster::block_on(<GNPSBuilder<f32> as Dataset>::download(builder))?;
    let _ = std::fs::remove_dir_all(&target_directory);

    assert_eq!(download.path(), path.as_path());
    assert_eq!(download.bytes(), document.len() as u64);

    Ok(())
}

#[test]
fn test_gnps_builder_rejects_empty_file_name() {
    assert!(matches!(
        pollster::block_on(MGFVec::<f64>::gnps().file_name("").load()),
        Err(MascotError::EmptyFilename)
    ));
}

#[test]
fn test_annotated_ms2_builder_defaults_to_zenodo_file() {
    let builder = MGFVec::<f64>::annotated_ms2();
    let top_128_builder = MGFVec::<f64>::annotated_ms2_top_128_peaks();

    assert_eq!(builder.selected_variant(), AnnotatedMs2Variant::Top128Peaks);
    assert_eq!(
        top_128_builder.selected_variant(),
        AnnotatedMs2Variant::Top128Peaks
    );
    assert_eq!(builder.record_id(), ANNOTATED_MS2_ZENODO_RECORD_ID);
    assert_eq!(builder.doi(), ANNOTATED_MS2_ZENODO_DOI);
    assert_eq!(builder.spectra_count(), ANNOTATED_MS2_SPECTRA_COUNT);
    assert_eq!(ANNOTATED_MS2_ZENODO_RECORD_ID, 20_042_904);
    assert_eq!(ANNOTATED_MS2_ZENODO_DOI, "10.5281/zenodo.20042904");
    assert_eq!(ANNOTATED_MS2_SPECTRA_COUNT, 443_905);
    assert_eq!(ANNOTATED_MS2_TOP_128_ZENODO_RECORD_ID, 20_042_904);
    assert_eq!(ANNOTATED_MS2_TOP_128_ZENODO_DOI, "10.5281/zenodo.20042904");
    assert_eq!(ANNOTATED_MS2_TOP_128_SPECTRA_COUNT, 443_905);
    assert_eq!(
        ANNOTATED_MS2_MGF_FILE_NAME,
        "combined-gnps-mass-spec-gym-npc-faithful.harmonized-subset.top128.mgf.zst"
    );
    assert!(ANNOTATED_MS2_MGF_URL.contains("20042904"));
    assert_eq!(
        builder.path().file_name().and_then(|name| name.to_str()),
        Some(ANNOTATED_MS2_MGF_FILE_NAME)
    );
}

#[test]
fn test_annotated_ms2_builder_selects_top_60_variant() {
    let builder = MGFVec::<f64>::annotated_ms2_top_60_peaks();

    assert_eq!(builder.selected_variant(), AnnotatedMs2Variant::Top60Peaks);
    assert_eq!(builder.record_id(), ANNOTATED_MS2_TOP_60_ZENODO_RECORD_ID);
    assert_eq!(builder.doi(), ANNOTATED_MS2_TOP_60_ZENODO_DOI);
    assert_eq!(builder.spectra_count(), ANNOTATED_MS2_TOP_60_SPECTRA_COUNT);
    assert_eq!(ANNOTATED_MS2_TOP_60_ZENODO_RECORD_ID, 20_039_648);
    assert_eq!(ANNOTATED_MS2_TOP_60_ZENODO_DOI, "10.5281/zenodo.20039648");
    assert_eq!(ANNOTATED_MS2_TOP_60_SPECTRA_COUNT, 439_403);
    assert_eq!(
        ANNOTATED_MS2_TOP_60_MGF_FILE_NAME,
        "combined-gnps-mass-spec-gym-npc-faithful.harmonized-subset.mgf.zst"
    );
    assert!(ANNOTATED_MS2_TOP_60_MGF_URL.contains("20039648"));
    assert_eq!(
        builder.path().file_name().and_then(|name| name.to_str()),
        Some(ANNOTATED_MS2_TOP_60_MGF_FILE_NAME)
    );
}

#[test]
fn test_annotated_ms2_variant_keeps_explicit_target_directory() {
    let target_directory = std::env::temp_dir().join("custom-annotated-ms2-cache");
    let builder = MGFVec::<f64>::annotated_ms2()
        .target_directory(&target_directory)
        .top_60_peaks();

    assert_eq!(builder.selected_variant(), AnnotatedMs2Variant::Top60Peaks);
    assert!(builder.path().starts_with(&target_directory));
    assert_eq!(
        builder.path().file_name().and_then(|name| name.to_str()),
        Some(ANNOTATED_MS2_TOP_60_MGF_FILE_NAME)
    );
}

#[test]
fn test_annotated_ms2_builder_loads_existing_downloaded_file() -> Result<()> {
    let target_directory = std::env::temp_dir().join(format!(
        "mascot-rs-annotated-ms2-test-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&target_directory);
    std::fs::create_dir_all(&target_directory).map_err(|source| MascotError::Io {
        path: target_directory.display().to_string(),
        source,
    })?;
    let builder = MGFVec::<f32>::annotated_ms2()
        .url("https://example.invalid/annotated-ms2.mgf")
        .target_directory(&target_directory)
        .file_name("cached-annotated-ms2.mgf")
        .verbose()
        .force_download(false);
    let path = builder.path();
    let document = r"BEGIN IONS
FEATURE_ID=annotated-1
PEPMASS=47.049141
CHARGE=1+
MSLEVEL=2
SCANS=1
SMILES=CCO
INCHIKEY=LFQSCWFLJHTTHZ-UHFFFAOYSA-N
FORMULA=C2H6O
NPC_PATHWAY=Fatty acids
CLASSYFIRE_DIRECT_PARENT=Alcohols
31.0184 1.0
45.0335 0.5
END IONS
BEGIN IONS
FEATURE_ID=annotated-invalid
PEPMASS=0.0
CHARGE=1+
MSLEVEL=2
SCANS=2
100.0 1.0
END IONS
";
    std::fs::write(&path, document).map_err(|source| MascotError::Io {
        path: path.display().to_string(),
        source,
    })?;
    let expected_bytes = document.len() as u64;

    let load = pollster::block_on(Dataset::load(builder))?;
    let _ = std::fs::remove_dir_all(&target_directory);

    assert!(load.mem_size(SizeFlags::default()) >= std::mem::size_of_val(&load));
    assert_eq!(load.spectra().len(), 1);
    assert_eq!(load.as_ref().len(), 1);
    assert_eq!(load.skipped_records(), 1);
    assert_eq!(load.path(), path.as_path());
    assert_eq!(load.bytes(), expected_bytes);
    assert_eq!(load.spectra()[0].feature_id(), Some("annotated-1"));
    assert_eq!(load.spectra()[0].charge(), Some(1));
    assert_eq!(
        load.spectra()[0]
            .metadata()
            .arbitrary_metadata_value("NPC_PATHWAY"),
        Some("Fatty acids")
    );
    assert_eq!(
        load.spectra()[0]
            .metadata()
            .arbitrary_metadata_value("CLASSYFIRE_DIRECT_PARENT"),
        Some("Alcohols")
    );
    let spectra = load.into_spectra();
    assert_eq!(spectra.len(), 1);

    Ok(())
}

#[test]
fn test_annotated_ms2_builder_iterates_existing_downloaded_file() -> Result<()> {
    let target_directory = std::env::temp_dir().join(format!(
        "mascot-rs-annotated-ms2-iter-test-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&target_directory);
    std::fs::create_dir_all(&target_directory).map_err(|source| MascotError::Io {
        path: target_directory.display().to_string(),
        source,
    })?;
    let builder = MGFVec::<f32>::annotated_ms2()
        .url("https://example.invalid/annotated-ms2.mgf")
        .target_directory(&target_directory)
        .file_name("cached-annotated-ms2.mgf")
        .force_download(false);
    let path = builder.path();
    let document = r"BEGIN IONS
FEATURE_ID=annotated-iter
PEPMASS=47.049141
CHARGE=1+
MSLEVEL=2
SCANS=1
31.0184 1.0
45.0335 0.5
END IONS
BEGIN IONS
FEATURE_ID=annotated-invalid
PEPMASS=0.0
MSLEVEL=2
100.0 1.0
END IONS
";
    std::fs::write(&path, document).map_err(|source| MascotError::Io {
        path: path.display().to_string(),
        source,
    })?;

    let mut iter = pollster::block_on(<AnnotatedMs2Builder<f32> as Dataset>::mgf_iter(builder))?;
    let first = iter.next().transpose()?.ok_or(MascotError::MissingField {
        builder: "test",
        field: "first_record",
    })?;
    assert_eq!(first.feature_id(), Some("annotated-iter"));
    assert!(iter.next().is_none());
    assert_eq!(iter.skipped_records(), 1);
    let _ = std::fs::remove_dir_all(&target_directory);

    Ok(())
}

#[test]
fn test_annotated_ms2_builder_downloads_existing_file_without_loading() -> Result<()> {
    let target_directory = std::env::temp_dir().join(format!(
        "mascot-rs-annotated-ms2-download-test-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&target_directory);
    std::fs::create_dir_all(&target_directory).map_err(|source| MascotError::Io {
        path: target_directory.display().to_string(),
        source,
    })?;
    let builder = MGFVec::<f32>::annotated_ms2()
        .url("https://example.invalid/annotated-ms2.mgf")
        .target_directory(&target_directory)
        .file_name("cached-invalid.mgf")
        .force_download(false);
    let path = builder.path();
    let document = "cached annotated MS2 file that should not be parsed\n";
    std::fs::write(&path, document).map_err(|source| MascotError::Io {
        path: path.display().to_string(),
        source,
    })?;

    let download = pollster::block_on(<AnnotatedMs2Builder<f32> as Dataset>::download(builder))?;
    let _ = std::fs::remove_dir_all(&target_directory);

    assert_eq!(download.path(), path.as_path());
    assert_eq!(download.bytes(), document.len() as u64);

    Ok(())
}

#[test]
fn test_annotated_ms2_builder_rejects_empty_file_name() {
    assert!(matches!(
        pollster::block_on(MGFVec::<f64>::annotated_ms2().file_name("").load()),
        Err(MascotError::EmptyFilename)
    ));
}

#[test]
fn test_mass_spec_gym_builder_loads_existing_downloaded_file() -> Result<()> {
    let target_directory = std::env::temp_dir().join(format!(
        "mascot-rs-mass-spec-gym-test-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&target_directory);
    std::fs::create_dir_all(&target_directory).map_err(|source| MascotError::Io {
        path: target_directory.display().to_string(),
        source,
    })?;
    let builder = MGFVec::<f32>::mass_spec_gym()
        .url("https://example.invalid/MassSpecGym.mgf")
        .target_directory(&target_directory)
        .verbose()
        .force_download(false);
    let path = builder.path();
    let document = r"BEGIN IONS
IDENTIFIER=MassSpecGymID0000001
SMILES=CCO
INCHIKEY=LFQSCWFLJHTTHZ
FORMULA=C2H6O
PRECURSOR_FORMULA=C2H7O
PARENT_MASS=46.041865
PRECURSOR_MZ=47.049141
ADDUCT=[M+H]+
INSTRUMENT_TYPE=Orbitrap
COLLISION_ENERGY=20.0
FOLD=train
SIMULATION_CHALLENGE=True
31.0184 1.0
45.0335 0.5
END IONS
BEGIN IONS
IDENTIFIER=MassSpecGymID0000002
PRECURSOR_MZ=0.0
ADDUCT=[M-H]-
INSTRUMENT_TYPE=QTOF
50.0 1.0
END IONS
BEGIN IONS
IDENTIFIER=MassSpecGymID0000003
PRECURSOR_MZ=150.0
INSTRUMENT_TYPE=Orbitrap
75.0 1.0
END IONS
";
    std::fs::write(&path, document).map_err(|source| MascotError::Io {
        path: path.display().to_string(),
        source,
    })?;
    let expected_bytes = document.len() as u64;

    let load = pollster::block_on(Dataset::load(builder))?;
    let _ = std::fs::remove_dir_all(&target_directory);

    assert!(load.mem_size(SizeFlags::default()) >= std::mem::size_of_val(&load));
    assert_eq!(load.spectra().len(), 2);
    assert_eq!(load.as_ref().len(), 2);
    assert_eq!(load.skipped_records(), 1);
    assert_eq!(load.path(), path.as_path());
    assert_eq!(load.bytes(), expected_bytes);
    assert_eq!(load.spectra()[0].feature_id(), Some("MassSpecGymID0000001"));
    assert_eq!(load.spectra()[0].level(), 2);
    assert_eq!(load.spectra()[0].charge(), Some(1));
    assert_eq!(load.spectra()[0].ion_mode(), Some(IonMode::Positive));
    assert_eq!(
        load.spectra()[0].source_instrument(),
        Some(Instrument::Orbitrap)
    );
    assert_eq!(
        load.spectra()[0]
            .metadata()
            .arbitrary_metadata_value("IDENTIFIER"),
        Some("MassSpecGymID0000001")
    );
    assert_eq!(
        load.spectra()[0]
            .metadata()
            .arbitrary_metadata_value("ADDUCT"),
        Some("[M+H]+")
    );
    assert_eq!(load.spectra()[1].feature_id(), Some("MassSpecGymID0000003"));
    assert_eq!(load.spectra()[1].charge(), None);
    assert_eq!(load.spectra()[1].ion_mode(), None);
    let spectra = load.into_spectra();
    assert_eq!(spectra.len(), 2);

    Ok(())
}

#[test]
fn test_mass_spec_gym_builder_iterates_existing_downloaded_file() -> Result<()> {
    let target_directory = std::env::temp_dir().join(format!(
        "mascot-rs-mass-spec-gym-iter-test-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&target_directory);
    std::fs::create_dir_all(&target_directory).map_err(|source| MascotError::Io {
        path: target_directory.display().to_string(),
        source,
    })?;
    let builder = MGFVec::<f32>::mass_spec_gym()
        .url("https://example.invalid/MassSpecGym.mgf")
        .target_directory(&target_directory)
        .force_download(false);
    let path = builder.path();
    let document = r"BEGIN IONS
IDENTIFIER=MassSpecGymID0000001
PRECURSOR_MZ=47.049141
ADDUCT=[M+H]+
INSTRUMENT_TYPE=Orbitrap
31.0184 1.0
45.0335 0.5
END IONS
";
    std::fs::write(&path, document).map_err(|source| MascotError::Io {
        path: path.display().to_string(),
        source,
    })?;

    let builder = MGFVec::<f32>::mass_spec_gym()
        .url("https://example.invalid/MassSpecGym.mgf")
        .target_directory(&target_directory)
        .force_download(false);
    let mut iter = pollster::block_on(<MassSpecGymBuilder<f32> as Dataset>::mgf_iter(builder))?;
    let first = iter.next().transpose()?.ok_or(MascotError::MissingField {
        builder: "test",
        field: "first_record",
    })?;
    assert_eq!(first.feature_id(), Some("MassSpecGymID0000001"));
    assert_eq!(first.level(), 2);
    assert_eq!(first.charge(), Some(1));
    assert_eq!(first.ion_mode(), Some(IonMode::Positive));
    assert_eq!(first.source_instrument(), Some(Instrument::Orbitrap));
    assert!(iter.next().is_none());
    assert_eq!(iter.skipped_records(), 0);
    let _ = std::fs::remove_dir_all(&target_directory);

    Ok(())
}

#[test]
fn test_mass_spec_gym_builder_downloads_existing_file_without_loading() -> Result<()> {
    let target_directory = std::env::temp_dir().join(format!(
        "mascot-rs-mass-spec-gym-download-test-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&target_directory);
    std::fs::create_dir_all(&target_directory).map_err(|source| MascotError::Io {
        path: target_directory.display().to_string(),
        source,
    })?;
    let builder = MGFVec::<f32>::mass_spec_gym()
        .url("https://example.invalid/MassSpecGym.mgf")
        .target_directory(&target_directory)
        .file_name("cached-invalid.mgf")
        .force_download(false);
    let path = builder.path();
    let document = "cached MassSpecGym file that should not be parsed\n";
    std::fs::write(&path, document).map_err(|source| MascotError::Io {
        path: path.display().to_string(),
        source,
    })?;

    let download = pollster::block_on(<MassSpecGymBuilder<f32> as Dataset>::download(builder))?;
    let _ = std::fs::remove_dir_all(&target_directory);

    assert_eq!(download.path(), path.as_path());
    assert_eq!(download.bytes(), document.len() as u64);

    Ok(())
}

#[test]
fn test_mass_spec_gym_builder_rejects_empty_file_name() {
    assert!(matches!(
        pollster::block_on(MGFVec::<f64>::mass_spec_gym().file_name("").load()),
        Err(MascotError::EmptyFilename)
    ));
}

#[test]
fn test_gems_a10_builder_defaults_to_published_parts() -> Result<()> {
    let builder = MGFVec::<f64>::gems_a10();

    assert_eq!(builder.selected_variant(), GemsA10Variant::Top100Peaks);
    assert_eq!(builder.record_id(), GEMS_A10_ZENODO_RECORD_ID);
    assert_eq!(GEMS_A10_ZENODO_DOI, "10.5281/zenodo.19980668");
    assert_eq!(GEMS_A10_TOP_100_ZENODO_RECORD_ID, 19_980_668);
    assert_eq!(GEMS_A10_TOP_100_ZENODO_DOI, "10.5281/zenodo.19980668");
    assert_eq!(
        builder.selected_file_keys().len(),
        usize::from(GEMS_A10_MGF_PART_COUNT)
    );
    assert_eq!(
        builder.selected_file_keys().first().map(String::as_str),
        Some("GeMS_A10.mgf.part-00000.mgf.zst")
    );
    assert_eq!(
        builder.selected_file_keys().last().map(String::as_str),
        Some("GeMS_A10.mgf.part-00023.mgf.zst")
    );
    assert_eq!(
        GemsA10Builder::<f32>::part_file_key(9)?,
        "GeMS_A10.mgf.part-00009.mgf.zst"
    );
    assert!(matches!(
        MGFVec::<f64>::gems_a10().part(GEMS_A10_MGF_PART_COUNT),
        Err(MascotError::InvalidGemsA10Part { .. })
    ));

    Ok(())
}

#[test]
fn test_gems_a10_builder_selects_top_128_variant() {
    let builder = MGFVec::<f64>::gems_a10_top_128_peaks();

    assert_eq!(builder.selected_variant(), GemsA10Variant::Top128Peaks);
    assert_eq!(builder.record_id(), GEMS_A10_TOP_128_ZENODO_RECORD_ID);
    assert_eq!(builder.doi(), GEMS_A10_TOP_128_ZENODO_DOI);
    assert_eq!(GEMS_A10_TOP_128_ZENODO_RECORD_ID, 20_040_772);
    assert_eq!(GEMS_A10_TOP_128_ZENODO_DOI, "10.5281/zenodo.20040772");
    assert_eq!(
        builder.selected_file_keys().len(),
        usize::from(GEMS_A10_MGF_PART_COUNT)
    );
    assert_eq!(
        builder.selected_file_keys().last().map(String::as_str),
        Some("GeMS_A10.mgf.part-00023.mgf.zst")
    );
}

#[test]
fn test_gems_a10_builder_selects_top_60_variant() {
    let builder = MGFVec::<f64>::gems_a10_top_60_peaks();

    assert_eq!(builder.selected_variant(), GemsA10Variant::Top60Peaks);
    assert_eq!(builder.record_id(), GEMS_A10_TOP_60_ZENODO_RECORD_ID);
    assert_eq!(builder.doi(), GEMS_A10_TOP_60_ZENODO_DOI);
    assert_eq!(GEMS_A10_TOP_60_ZENODO_RECORD_ID, 20_001_888);
    assert_eq!(GEMS_A10_TOP_60_ZENODO_DOI, "10.5281/zenodo.20001888");
    assert_eq!(
        builder.selected_file_keys().len(),
        usize::from(GEMS_A10_MGF_PART_COUNT)
    );
    assert_eq!(
        builder.selected_file_keys().last().map(String::as_str),
        Some("GeMS_A10.mgf.part-00023.mgf.zst")
    );
}

#[test]
fn test_gems_a10_builder_selects_top_40_variant() {
    let builder = MGFVec::<f64>::gems_a10_top_40_peaks();

    assert_eq!(builder.selected_variant(), GemsA10Variant::Top40Peaks);
    assert_eq!(builder.record_id(), GEMS_A10_TOP_40_ZENODO_RECORD_ID);
    assert_eq!(builder.doi(), GEMS_A10_TOP_40_ZENODO_DOI);
    assert_eq!(GEMS_A10_TOP_40_ZENODO_RECORD_ID, 20_002_962);
    assert_eq!(GEMS_A10_TOP_40_ZENODO_DOI, "10.5281/zenodo.20002962");
    assert_eq!(
        builder.selected_file_keys().len(),
        usize::from(GEMS_A10_MGF_PART_COUNT)
    );
    assert_eq!(
        builder.selected_file_keys().last().map(String::as_str),
        Some("GeMS_A10.mgf.part-00023.mgf.zst")
    );
}

#[test]
fn test_gems_a10_builder_selects_top_20_variant() {
    let builder = MGFVec::<f64>::gems_a10_top_20_peaks();

    assert_eq!(builder.selected_variant(), GemsA10Variant::Top20Peaks);
    assert_eq!(builder.record_id(), GEMS_A10_TOP_20_ZENODO_RECORD_ID);
    assert_eq!(builder.doi(), GEMS_A10_TOP_20_ZENODO_DOI);
    assert_eq!(GEMS_A10_TOP_20_ZENODO_RECORD_ID, 20_027_219);
    assert_eq!(GEMS_A10_TOP_20_ZENODO_DOI, "10.5281/zenodo.20027219");
    assert_eq!(
        builder.selected_file_keys().len(),
        usize::from(GEMS_A10_MGF_PART_COUNT)
    );
    assert_eq!(
        builder.selected_file_keys().last().map(String::as_str),
        Some("GeMS_A10.mgf.part-00023.mgf.zst")
    );
}

#[test]
fn test_gems_a10_builder_loads_existing_downloaded_file() -> Result<()> {
    let target_directory =
        std::env::temp_dir().join(format!("mascot-rs-gems-a10-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&target_directory);
    std::fs::create_dir_all(&target_directory).map_err(|source| MascotError::Io {
        path: target_directory.display().to_string(),
        source,
    })?;
    let builder = MGFVec::<f32>::gems_a10()
        .target_directory(&target_directory)
        .file_key("cached.mgf")
        .verbose()
        .force_download(false);
    let path = builder.path_for_file_key("cached.mgf");
    let document = r"BEGIN IONS
PEPMASS=0.0
CHARGE=1
MSLEVEL=2
FEATURE_ID=1
SCANS=1
100.0 2.0
END IONS
BEGIN IONS
PEPMASS=250.0
CHARGE=1
MSLEVEL=2
FEATURE_ID=2
SCANS=2
100.0 2.0
END IONS
";
    std::fs::write(&path, document).map_err(|source| MascotError::Io {
        path: path.display().to_string(),
        source,
    })?;
    let expected_bytes = document.len() as u64;

    let gems_a10_load = pollster::block_on(builder.load())?;
    let _ = std::fs::remove_dir_all(&target_directory);

    assert!(gems_a10_load.mem_size(SizeFlags::default()) >= std::mem::size_of_val(&gems_a10_load));
    assert_eq!(gems_a10_load.spectra().len(), 1);
    assert_eq!(gems_a10_load.as_ref().len(), 1);
    assert_eq!(gems_a10_load.skipped_records(), 1);
    assert_eq!(gems_a10_load.files().len(), 1);
    assert_eq!(gems_a10_load.files()[0].key(), "cached.mgf");
    assert_eq!(gems_a10_load.files()[0].path(), path.as_path());
    assert_eq!(gems_a10_load.files()[0].bytes(), expected_bytes);
    assert_eq!(gems_a10_load.bytes(), expected_bytes);
    assert_eq!(
        gems_a10_load.spectra()[0].precursor_mz().to_bits(),
        250.0_f32.to_bits()
    );
    let spectra = gems_a10_load.into_spectra();
    assert_eq!(spectra.len(), 1);

    Ok(())
}

#[test]
fn test_gems_a10_builder_iterates_selected_existing_files() -> Result<()> {
    let target_directory = std::env::temp_dir().join(format!(
        "mascot-rs-gems-a10-iter-test-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&target_directory);
    std::fs::create_dir_all(&target_directory).map_err(|source| MascotError::Io {
        path: target_directory.display().to_string(),
        source,
    })?;
    let builder = MGFVec::<f32>::gems_a10()
        .target_directory(&target_directory)
        .file_keys(["part-a.mgf", "part-b.mgf"])
        .force_download(false);
    let first_path = builder.path_for_file_key("part-a.mgf");
    let second_path = builder.path_for_file_key("part-b.mgf");
    let first_document = r"BEGIN IONS
PEPMASS=100.0
CHARGE=1
MSLEVEL=2
FEATURE_ID=gems-a
SCANS=1
50.0 1.0
END IONS
";
    let second_document = r"BEGIN IONS
PEPMASS=200.0
CHARGE=1
MSLEVEL=2
FEATURE_ID=gems-b
SCANS=2
75.0 1.0
END IONS
";
    std::fs::write(&first_path, first_document).map_err(|source| MascotError::Io {
        path: first_path.display().to_string(),
        source,
    })?;
    std::fs::write(&second_path, second_document).map_err(|source| MascotError::Io {
        path: second_path.display().to_string(),
        source,
    })?;

    let mut iter = pollster::block_on(<GemsA10Builder<f32> as Dataset>::mgf_iter(builder))?;
    let first = iter.next().transpose()?.ok_or(MascotError::MissingField {
        builder: "test",
        field: "first_record",
    })?;
    let second = iter.next().transpose()?.ok_or(MascotError::MissingField {
        builder: "test",
        field: "second_record",
    })?;
    assert_eq!(first.feature_id(), Some("gems-a"));
    assert_eq!(second.feature_id(), Some("gems-b"));
    assert!(iter.next().is_none());
    assert_eq!(iter.skipped_records(), 0);
    let _ = std::fs::remove_dir_all(&target_directory);

    Ok(())
}

#[test]
fn test_gems_a10_builder_downloads_existing_file_without_loading() -> Result<()> {
    let target_directory = std::env::temp_dir().join(format!(
        "mascot-rs-gems-a10-download-test-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&target_directory);
    std::fs::create_dir_all(&target_directory).map_err(|source| MascotError::Io {
        path: target_directory.display().to_string(),
        source,
    })?;
    let builder = MGFVec::<f32>::gems_a10()
        .target_directory(&target_directory)
        .file_key("cached-invalid.mgf")
        .force_download(false);
    let path = builder.path_for_file_key("cached-invalid.mgf");
    let document = "cached GeMS-A10 file that should not be parsed\n";
    std::fs::write(&path, document).map_err(|source| MascotError::Io {
        path: path.display().to_string(),
        source,
    })?;

    let download = pollster::block_on(<GemsA10Builder<f32> as Dataset>::download(builder))?;
    let _ = std::fs::remove_dir_all(&target_directory);

    assert_eq!(download.files().len(), 1);
    assert_eq!(download.files()[0].key(), "cached-invalid.mgf");
    assert_eq!(download.files()[0].path(), path.as_path());
    assert_eq!(download.files()[0].bytes(), document.len() as u64);
    assert_eq!(download.bytes(), document.len() as u64);

    Ok(())
}

#[test]
fn test_gems_a10_builder_rejects_empty_file_selection() {
    assert!(matches!(
        pollster::block_on(
            MGFVec::<f64>::gems_a10()
                .file_keys(core::iter::empty::<&str>())
                .load()
        ),
        Err(MascotError::MissingField {
            builder: "GemsA10Builder",
            field: "file_keys"
        })
    ));
    assert!(matches!(
        pollster::block_on(MGFVec::<f64>::gems_a10().file_key("").load()),
        Err(MascotError::EmptyFilename)
    ));
}
