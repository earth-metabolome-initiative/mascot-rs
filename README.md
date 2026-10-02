# mascot-rs

[![Build status](https://github.com/lucacappelletti94/mascot-rs/actions/workflows/rust.yml/badge.svg)](https://github.com/lucacappelletti94/mascot-rs/actions)
[![codecov](https://codecov.io/gh/lucacappelletti94/mascot-rs/branch/main/graph/badge.svg)](https://codecov.io/gh/lucacappelletti94/mascot-rs)
[![Crates.io](https://img.shields.io/crates/v/mascot-rs.svg)](https://crates.io/crates/mascot-rs)
[![Documentation](https://docs.rs/mascot-rs/badge.svg)](https://docs.rs/mascot-rs)

Parsing utilities for Mascot Generic Format (MGF) spectra. Algorithmic work is delegated to the shared [`mass_spectrometry`](https://github.com/earth-metabolome-initiative/mass-spectrometry-traits) traits and structs exposed through the prelude.

## Feature Flags

Default features enable `std` and `mem_dbg`. Disabling defaults keeps the
string and iterator parser APIs available for `no_std` targets with `alloc`.
File IO, dataset downloading/loading, and progress reporting require `std`.
Path-based loading supports uncompressed MGF plus `.zst`, `.zstd`, `.gz`, and
`.gzip` files.

## Parsing Documents

Use [`MGFVec`] when parsing a full MGF document. Parsed records can be
filtered, processed, and written back out with the same extension convention as
loading: `.mgf.zst` and `.mgf.gz` files are compressed automatically.

```rust
# #[cfg(feature = "std")]
# fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
use mascot_rs::prelude::*;

let document = r#"BEGIN IONS
FEATURE_ID=1
PEPMASS=500.0
CHARGE=1
RTINSECONDS=10.0
MSLEVEL=2
SMILES=CCO
IONMODE=Positive
SOURCE_INSTRUMENT=LC-ESI-Orbitrap
NAME=Ethanol example
100.0 2.0
SCANS=1
END IONS

BEGIN IONS
FEATURE_ID=2
PEPMASS=600.0
CHARGE=1
RTINSECONDS=12.0
MSLEVEL=2
200.0 3.0
SCANS=2
END IONS
"#;

let spectra: MGFVec = document.parse()?;

assert_eq!(spectra.len(), 2);
assert_eq!(spectra[0].feature_id(), Some("1"));
assert_eq!(
    spectra[0].metadata().smiles().map(ToString::to_string).as_deref(),
    Some("CCO")
);
assert_eq!(spectra[0].ion_mode(), Some(IonMode::Positive));
assert_eq!(
    spectra[0].source_instrument(),
    Some(Instrument::Orbitrap)
);
assert_eq!(
    spectra[0].metadata().arbitrary_metadata_value("NAME"),
    Some("Ethanol example")
);
assert_eq!(spectra[1].precursor_mz().to_bits(), 600.0_f64.to_bits());

let mut positive_orbitrap: MGFVec = spectra
    .into_iter()
    .filter(|record| record.ion_mode() == Some(IonMode::Positive))
    .filter(|record| record.source_instrument() == Some(Instrument::Orbitrap))
    .collect();
for record in &mut positive_orbitrap {
    record
        .metadata_mut()
        .insert_arbitrary_metadata("EXPORT_BATCH", "curated")?;
}
let total_peaks = positive_orbitrap
    .spectra()
    .map(Spectrum::len)
    .sum::<usize>();

let mut buffer = Vec::new();
positive_orbitrap.write_to(&mut buffer)?;
let reparsed: MGFVec = std::str::from_utf8(&buffer)?.parse()?;

let path = std::env::temp_dir().join(format!(
    "mascot-rs-parse-write-example-{}.mgf.zst",
    std::process::id()
));
positive_orbitrap.to_path(&path)?;
let from_disk: MGFVec = MGFVec::from_path(&path)?;
std::fs::remove_file(path)?;

assert_eq!(total_peaks, 1);
assert_eq!(reparsed.len(), 1);
assert_eq!(
    reparsed[0]
        .metadata()
        .arbitrary_metadata_value("EXPORT_BATCH"),
    Some("curated")
);
assert_eq!(from_disk.len(), 1);
# Ok(())
# }
# #[cfg(not(feature = "std"))]
# fn main() {}
```

Files can also be parsed directly from a path, including compressed `.mgf.zst`
and `.mgf.gz` files.

```rust
# #[cfg(feature = "std")]
# fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
use mascot_rs::prelude::*;

let spectra: MGFVec =
    MGFVec::from_path("tests/data/20220513_PMA_DBGI_01_04_003.mgf")?;

assert_eq!(spectra.len(), 74);
# Ok(())
# }
# #[cfg(not(feature = "std"))]
# fn main() {}
```

## Streaming Records

Use `MGFIter` when records should be read one by one instead of collecting a
whole document into memory. This is the preferred shape for very large MGF
documents and sharded corpora.

```rust
# #[cfg(feature = "std")]
# fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
use mascot_rs::prelude::*;

let document = r#"BEGIN IONS
PEPMASS=500.0
CHARGE=1
MSLEVEL=2
100.0 2.0
SCANS=-1
END IONS

BEGIN IONS
PEPMASS=600.0
CHARGE=1
MSLEVEL=2
200.0 3.0
SCANS=-1
END IONS
"#;

let mut records = MGFVec::<f64>::iter_from_str(document);

let first = records
    .next()
    .transpose()?
    .ok_or_else(|| std::io::Error::other("missing first MGF record"))?;
let second = records
    .next()
    .transpose()?
    .ok_or_else(|| std::io::Error::other("missing second MGF record"))?;

assert_eq!(first.precursor_mz().to_bits(), 500.0_f64.to_bits());
assert_eq!(second.precursor_mz().to_bits(), 600.0_f64.to_bits());
assert!(records.next().is_none());
# Ok(())
# }
# #[cfg(not(feature = "std"))]
# fn main() {}
```

## Parsing One Record

Use [`MascotGenericFormat`] when the input must contain exactly one ion block.
Parsing zero or multiple blocks returns [`MascotError::SingleRecordExpected`].

```rust
use mascot_rs::prelude::*;

# fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
let document = r#"BEGIN IONS
FEATURE_ID=1
PEPMASS=500.0
CHARGE=1
RTINSECONDS=10.0
MSLEVEL=2
100.0 2.0
SCANS=1
END IONS
"#;

let record: MascotGenericFormat = document.parse()?;

assert_eq!(record.feature_id(), Some("1"));
assert_eq!(record.len(), 1);
assert!(matches!(
    "".parse::<MascotGenericFormat>(),
    Err(MascotError::SingleRecordExpected { found: 0 })
));
# Ok(())
# }
```

## Precision

Spectra use `f64` storage by default. Select another precision with the second
generic parameter.

```rust
use mascot_rs::prelude::*;

# fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
let document = r#"BEGIN IONS
FEATURE_ID=1
PEPMASS=500.0
CHARGE=1
RTINSECONDS=10.0
MSLEVEL=2
100.0 2.0
200.0 3.0
SCANS=1
END IONS
"#;

let spectra: MGFVec<f32> = document.parse()?;
let spectrum: &GenericSpectrum<f32> = spectra[0].as_ref();

assert_eq!(spectra[0].precursor_mz().to_bits(), 500.0_f32.to_bits());
assert_eq!(spectrum.mz_nth(0).to_bits(), 100.0_f32.to_bits());
# Ok(())
# }
```

## GNPS

The GNPS helper is exposed through `MGFVec::<P>::gnps()`. Dataset builders
implement `Dataset`: `download()` only ensures that the local file exists,
`mgf_iter()` downloads if needed and streams records through `MGFIter`, while
`load()` downloads if needed and parses the MGF records into memory.
The builder supports `.target_directory(...)`, `.file_name(...)`, `.verbose()`,
and `.force_download(...)`.

## `MassSpecGym`

The `MassSpecGym` helper is exposed through
`MGFVec::<P>::mass_spec_gym()`. It targets the public Hugging Face
`data/auxiliary/MassSpecGym.mgf` file, which contains 231,104 benchmark spectra.
The loader normalizes `MassSpecGym`-specific headers such as `IDENTIFIER`,
`PRECURSOR_MZ` and `INSTRUMENT_TYPE` into the strict parser while preserving
the original keys as arbitrary metadata. `ADDUCT` is handled by the generic MGF
metadata parser, which derives charge and ion mode when the adduct is
unambiguous.

## Annotated MS2

The annotated MS2 helper is exposed through `MGFVec::<P>::annotated_ms2()`.
It targets Zenodo record `20042904`, a zstd-compressed harmonized top-128
subset of GNPS public library and `MassSpecGym` MS/MS spectra. The record
reports 443,905 validated spectra with canonical `SMILES`, `INCHIKEY`,
`FORMULA`, NPC, `ClassyFire`, and source-provenance annotations. The previous
top-60 release remains available with `MGFVec::<P>::annotated_ms2_top_60_peaks()`
or `.top_60_peaks()` on the builder.

## GeMS-A10

The GeMS-A10 helper is exposed through `MGFVec::<P>::gems_a10()`.
By default it targets Zenodo record `19980668` and the 24 compressed MGF part
files from the top-100 peaks conversion. The top-128, top-60, top-40, and
top-20 peaks conversions are available with
`MGFVec::<P>::gems_a10_top_128_peaks()`,
`MGFVec::<P>::gems_a10_top_60_peaks()`,
`MGFVec::<P>::gems_a10_top_40_peaks()`, or
`MGFVec::<P>::gems_a10_top_20_peaks()`, and with `.top_128_peaks()`,
`.top_60_peaks()`, `.top_40_peaks()`, or `.top_20_peaks()` on the builder.
They target Zenodo records `20040772`, `20001888`, `20002962`, and `20027219`,
respectively. Uncached downloads use `zenodo-rs` and should be awaited inside a
Tokio runtime.

[`MascotError::SingleRecordExpected`]: crate::error::MascotError::SingleRecordExpected
[`MGFVec`]: crate::mascot_generic_format::MGFVec
[`MascotGenericFormat`]: crate::mascot_generic_format::MascotGenericFormat
