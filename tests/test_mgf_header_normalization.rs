//! Integration tests for non-canonical MGF header spellings.

use mascot_rs::prelude::*;

const MONA_STYLE_RECORD: &str = r"BEGIN IONS
CHARGE=1+
INCHIKEY=QZXATCCPQKOEIH-UHFFFAOYSA-N
INSTRUMENT_TYPE=LC-ESI-QQ
INSTRUMENT=QuattroPremier XE, Waters
COLLISION_ENERGY=80 V
FORMULA=C12H8F3N5O3S
SMILES=COC(=N3)n(n1)c(C(F)=C3)nc1S(=O)(=O)Nc(c(F)2)c(F)ccc2
CAS=145701-23-1
INCHI=InChI=1S/C12H8F3N5O3S/c1-23-12-16-5-8(15)10-17-11(18-20(10)12)24(21,22)19-9-6(13)3-2-4-7(9)14/h2-5,19H,1H3
AUTHOR=Nihon Waters K.K.
LICENSE=CC BY-NC
SPLASH=splash10-053r-9500000000-8d502c5cbc4f21b31bbc
SUBMITTER=submitter = Nihon Waters (Nihon Waters K.K.)
SYNON_$=00in-source
COMPUTED_SMILES=O=S(=O)(NC=1C(F)=CC=CC1F)C=2N=C3C(F)=CN=C(OC)N3N2
COMPUTED_[2M+H-H2O]+=701.0526195519999
COMPUTED_[2M+K]+=757.1582895519999
COMPUTED_[M+H]+=360.03796477599997
COMPUTED_[2M+NA]+=741.049759552
COMPUTED_[2M+CL]-=753.5129895519999
COMPUTED_[2M+HAC-H]-=777.104019552
COMPUTED_[M-H20-H]-=340.01160477599996
COMPUTED_[M-H]-=358.022718776
COMPUTED_[2M-H]-=717.0527135519999
COMPUTED_[M+NA]+=382.019764776
COMPUTED_[M+H-H2O]+=342.022624776
COMPUTED_[2M+NH4]+=736.098569552
COMPUTED_[M+CL]-=394.48299477599994
COMPUTED_[M+NH4]+=377.068574776
COMPUTED_[M+K]+=398.12829477599996
COMPUTED_[2M+H]+=719.0679595519999
COMPUTED_[M+HAC-H]-=418.074024776
FIND_PEAK=ignore rel.int. < 5
COMPUTED_SPECTRAL_ENTROPY=2.62235782938493
COMPUTED_NORMALIZED_ENTROPY=0.7209060388502744
COMPUTED_MASS_ACCURACY=105.45771111101962
COMPUTED_MASS_ERROR=-0.03796477599996706
MONA_RATING=4.545454545454546
NUM_PEAKS=38
COMPOUND_NAME=Florasulam
SPECTRUM_ID=WA000260
ADDUCT=[M+H]+
MS_LEVEL=MS2
PRECURSOR_MZ=360.0
IONMODE=positive
NOMINAL_MASS=359
PARENT_MASS=359.02999477599997
RECORD_DATE=2016.01.19 (Created 2007.08.01, modified 2011.05.06)
51.0 0.08608608999999999
53.0 0.02702703
57.0 0.08208208
59.0 0.02402402
61.0 0.03103103
62.0 0.3963964
64.0 0.06306306
65.0 0.00800801
67.0 0.00800801
69.0 0.01201201
71.0 0.01601602
72.0 0.02002002
75.0 0.03903904
76.0 0.08608608999999999
78.0 0.01601602
80.0 0.14114114
82.0 1.0
83.0 0.055055059999999996
85.0 0.05905906
87.0 0.02002002
89.0 0.32932933
93.0 0.00800801
97.0 0.06306306
98.0 0.01201201
99.0 0.02002002
101.0 0.45445445
102.0 0.05105105
104.0 0.00800801
107.0 0.03103103
109.0 0.6906906899999999
110.0 0.02402402
112.0 0.00800801
114.0 0.02402402
115.0 0.01201201
126.0 0.00800801
129.0 0.41141141
130.0 0.01601602
139.0 0.00800801
END IONS";

fn assert_mona_style_spectrum(spectra: &MGFVec) {
    assert_eq!(spectra.len(), 1);
    assert_eq!(spectra[0].charge(), Some(1));
    assert_eq!(spectra[0].level(), Some(2));
    assert_eq!(spectra[0].precursor_mz().to_bits(), 360.0_f64.to_bits());
    assert_eq!(spectra[0].ion_mode(), Some(IonMode::Positive));
    assert_eq!(spectra[0].source_instrument(), Some(Instrument::Quadrupole));
    assert_eq!(spectra[0].len(), 38);
    assert!(spectra[0].metadata().formula().is_some());
    assert_eq!(
        spectra[0].metadata().arbitrary_metadata_value("ADDUCT"),
        Some("[M+H]+")
    );
}

#[test]
fn test_mona_style_header_aliases_parse() -> Result<()> {
    let spectra: MGFVec = MONA_STYLE_RECORD.parse()?;
    assert_mona_style_spectrum(&spectra);

    let path = std::env::temp_dir().join(format!(
        "mascot-rs-mona-header-normalization-{}.mgf",
        std::process::id()
    ));
    std::fs::write(&path, MONA_STYLE_RECORD).map_err(|source| MascotError::Io {
        path: path.display().to_string(),
        source,
    })?;
    let spectra = MGFVec::<f64>::from_path(&path)?;
    let _ = std::fs::remove_file(&path);
    assert_mona_style_spectrum(&spectra);

    Ok(())
}

#[test]
fn test_mona_style_header_aliases_parse_with_surrounding_whitespace() -> Result<()> {
    let mut document = MONA_STYLE_RECORD
        .lines()
        .map(|line| format!("    {line}    "))
        .collect::<Vec<_>>()
        .join("\n");
    document.push_str("\n    ");

    let spectra: MGFVec = document.parse()?;
    assert_mona_style_spectrum(&spectra);

    Ok(())
}

#[test]
fn test_precursor_and_level_aliases_can_repeat_canonical_fields() -> Result<()> {
    let document = r"BEGIN IONS
PEPMASS=360.0000305175781 123.4 1+
PRECURSOR_MZ=360.0
MSLEVEL=2
MS_LEVEL=MS2
CHARGE=1
CHARGE=1+
ION_MODE=positive
SOURCE_INSTRUMENT=LC-ESI-QQ
INSTRUMENT_TYPE=LC-ESI-QQ
ADDUCT=[M+H]+
FORMULA=C12H8F3N5O3S
51.0 0.08608609
END IONS
";

    let spectra: MGFVec = document.parse()?;

    assert_eq!(spectra.len(), 1);
    assert_eq!(spectra[0].charge(), Some(1));
    assert_eq!(spectra[0].level(), Some(2));
    assert_eq!(
        spectra[0].precursor_mz().to_bits(),
        360.000_030_517_578_1_f64.to_bits()
    );
    assert_eq!(spectra[0].source_instrument(), Some(Instrument::Quadrupole));
    assert_eq!(
        spectra[0].metadata().arbitrary_metadata_value("ADDUCT"),
        Some("[M+H]+")
    );
    assert!(spectra[0].metadata().formula().is_some());

    Ok(())
}
