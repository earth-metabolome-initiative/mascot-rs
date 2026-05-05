#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;

/// Annotated harmonized MS2 dataset helpers.
#[cfg(feature = "std")]
pub mod annotated_ms2;
/// Downloadable dataset traits.
#[cfg(feature = "std")]
pub mod dataset;
/// Error types returned by this crate.
pub mod error;
/// `GeMS-A10` dataset helpers.
#[cfg(feature = "std")]
pub mod gems_a10;
/// GNPS spectral library helpers.
#[cfg(feature = "std")]
pub mod gnps;
/// MGF record and collection types.
pub mod mascot_generic_format;
#[doc(hidden)]
mod mascot_generic_format_builder;
/// MGF metadata.
pub mod mascot_generic_format_metadata;
#[doc(hidden)]
mod mascot_generic_format_metadata_builder;
/// `MassSpecGym` benchmark dataset helpers.
#[cfg(feature = "std")]
pub mod mass_spec_gym;
#[doc(hidden)]
mod numeric;

/// Commonly used crate exports.
pub mod prelude {
    #[cfg(feature = "std")]
    pub use crate::annotated_ms2::AnnotatedMs2Builder;
    #[cfg(feature = "std")]
    pub use crate::annotated_ms2::AnnotatedMs2Download;
    #[cfg(feature = "std")]
    pub use crate::annotated_ms2::AnnotatedMs2Load;
    #[cfg(feature = "std")]
    pub use crate::annotated_ms2::AnnotatedMs2Variant;
    #[cfg(feature = "std")]
    pub use crate::annotated_ms2::ANNOTATED_MS2_MGF_FILE_NAME;
    #[cfg(feature = "std")]
    pub use crate::annotated_ms2::ANNOTATED_MS2_MGF_URL;
    #[cfg(feature = "std")]
    pub use crate::annotated_ms2::ANNOTATED_MS2_SPECTRA_COUNT;
    #[cfg(feature = "std")]
    pub use crate::annotated_ms2::ANNOTATED_MS2_TOP_128_MGF_FILE_NAME;
    #[cfg(feature = "std")]
    pub use crate::annotated_ms2::ANNOTATED_MS2_TOP_128_MGF_URL;
    #[cfg(feature = "std")]
    pub use crate::annotated_ms2::ANNOTATED_MS2_TOP_128_SPECTRA_COUNT;
    #[cfg(feature = "std")]
    pub use crate::annotated_ms2::ANNOTATED_MS2_TOP_128_ZENODO_DOI;
    #[cfg(feature = "std")]
    pub use crate::annotated_ms2::ANNOTATED_MS2_TOP_128_ZENODO_RECORD_ID;
    #[cfg(feature = "std")]
    pub use crate::annotated_ms2::ANNOTATED_MS2_TOP_60_MGF_FILE_NAME;
    #[cfg(feature = "std")]
    pub use crate::annotated_ms2::ANNOTATED_MS2_TOP_60_MGF_URL;
    #[cfg(feature = "std")]
    pub use crate::annotated_ms2::ANNOTATED_MS2_TOP_60_SPECTRA_COUNT;
    #[cfg(feature = "std")]
    pub use crate::annotated_ms2::ANNOTATED_MS2_TOP_60_ZENODO_DOI;
    #[cfg(feature = "std")]
    pub use crate::annotated_ms2::ANNOTATED_MS2_TOP_60_ZENODO_RECORD_ID;
    #[cfg(feature = "std")]
    pub use crate::annotated_ms2::ANNOTATED_MS2_ZENODO_DOI;
    #[cfg(feature = "std")]
    pub use crate::annotated_ms2::ANNOTATED_MS2_ZENODO_RECORD_ID;
    #[cfg(feature = "std")]
    pub use crate::dataset::Dataset;
    #[cfg(feature = "std")]
    pub use crate::dataset::DatasetFuture;
    pub use crate::error::MascotError;
    pub use crate::error::Result;
    #[cfg(feature = "std")]
    pub use crate::gems_a10::GemsA10Builder;
    #[cfg(feature = "std")]
    pub use crate::gems_a10::GemsA10Download;
    #[cfg(feature = "std")]
    pub use crate::gems_a10::GemsA10FileDownload;
    #[cfg(feature = "std")]
    pub use crate::gems_a10::GemsA10FileLoad;
    #[cfg(feature = "std")]
    pub use crate::gems_a10::GemsA10Load;
    #[cfg(feature = "std")]
    pub use crate::gems_a10::GemsA10Variant;
    #[cfg(feature = "std")]
    pub use crate::gems_a10::GEMS_A10_MGF_PART_COUNT;
    #[cfg(feature = "std")]
    pub use crate::gems_a10::GEMS_A10_TOP_100_ZENODO_DOI;
    #[cfg(feature = "std")]
    pub use crate::gems_a10::GEMS_A10_TOP_100_ZENODO_RECORD_ID;
    #[cfg(feature = "std")]
    pub use crate::gems_a10::GEMS_A10_TOP_128_ZENODO_DOI;
    #[cfg(feature = "std")]
    pub use crate::gems_a10::GEMS_A10_TOP_128_ZENODO_RECORD_ID;
    #[cfg(feature = "std")]
    pub use crate::gems_a10::GEMS_A10_TOP_20_ZENODO_DOI;
    #[cfg(feature = "std")]
    pub use crate::gems_a10::GEMS_A10_TOP_20_ZENODO_RECORD_ID;
    #[cfg(feature = "std")]
    pub use crate::gems_a10::GEMS_A10_TOP_40_ZENODO_DOI;
    #[cfg(feature = "std")]
    pub use crate::gems_a10::GEMS_A10_TOP_40_ZENODO_RECORD_ID;
    #[cfg(feature = "std")]
    pub use crate::gems_a10::GEMS_A10_TOP_60_ZENODO_DOI;
    #[cfg(feature = "std")]
    pub use crate::gems_a10::GEMS_A10_TOP_60_ZENODO_RECORD_ID;
    #[cfg(feature = "std")]
    pub use crate::gems_a10::GEMS_A10_ZENODO_DOI;
    #[cfg(feature = "std")]
    pub use crate::gems_a10::GEMS_A10_ZENODO_RECORD_ID;
    #[cfg(feature = "std")]
    pub use crate::gnps::GNPSBuilder;
    #[cfg(feature = "std")]
    pub use crate::gnps::GNPSDownload;
    #[cfg(feature = "std")]
    pub use crate::gnps::GNPSLoad;
    #[cfg(feature = "std")]
    pub use crate::gnps::GNPS_ALL_MGF_URL;
    pub use crate::mascot_generic_format::MGFIter;
    pub use crate::mascot_generic_format::MGFVec;
    pub use crate::mascot_generic_format::MascotGenericFormat;
    pub use crate::mascot_generic_format_metadata::Instrument;
    pub use crate::mascot_generic_format_metadata::IonMode;
    pub use crate::mascot_generic_format_metadata::MascotGenericFormatMetadata;
    #[cfg(feature = "std")]
    pub use crate::mass_spec_gym::MassSpecGymBuilder;
    #[cfg(feature = "std")]
    pub use crate::mass_spec_gym::MassSpecGymDownload;
    #[cfg(feature = "std")]
    pub use crate::mass_spec_gym::MassSpecGymLoad;
    #[cfg(feature = "std")]
    pub use crate::mass_spec_gym::MASS_SPEC_GYM_MGF_FILE_NAME;
    #[cfg(feature = "std")]
    pub use crate::mass_spec_gym::MASS_SPEC_GYM_MGF_URL;
    #[cfg(feature = "std")]
    pub use crate::mass_spec_gym::MASS_SPEC_GYM_SPECTRA_COUNT;
    pub use mass_spectrometry::prelude::{
        GenericSpectrum, Spectra, Spectrum, SpectrumAlloc, SpectrumFloat, SpectrumMut,
        SpectrumSplash,
    };
    #[cfg(feature = "mem_dbg")]
    pub use mem_dbg::{DbgFlags, MemDbg};
    #[cfg(feature = "mem_size")]
    pub use mem_dbg::{MemSize, SizeFlags};
    pub use molecular_formulas::prelude::ChemicalFormula;
    pub use smiles_parser::prelude::Smiles;
}
