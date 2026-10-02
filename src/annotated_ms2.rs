use core::marker::PhantomData;
use std::path::{Path, PathBuf};

use mass_spectrometry::prelude::SpectrumFloat;

use crate::dataset::{Dataset, DatasetFuture, SingleFileDatasetConfig, SingleFileDatasetDownload};
use crate::error::Result;
use crate::mascot_generic_format::{MGFIter, MGFPathIter, MGFVec};

/// Zenodo record ID for the top-128 annotated harmonized MS2 MGF dataset.
pub const ANNOTATED_MS2_TOP_128_ZENODO_RECORD_ID: u64 = 20_042_904;

/// DOI for the top-128 annotated harmonized MS2 MGF dataset.
pub const ANNOTATED_MS2_TOP_128_ZENODO_DOI: &str = "10.5281/zenodo.20042904";

/// Zenodo endpoint for the top-128 annotated harmonized MS2 MGF file.
pub const ANNOTATED_MS2_TOP_128_MGF_URL: &str = "https://zenodo.org/api/records/20042904/files/combined-gnps-mass-spec-gym-npc-faithful.harmonized-subset.top128.mgf.zst/content";

/// File name used for the top-128 annotated harmonized MS2 MGF file.
pub const ANNOTATED_MS2_TOP_128_MGF_FILE_NAME: &str =
    "combined-gnps-mass-spec-gym-npc-faithful.harmonized-subset.top128.mgf.zst";

/// Number of spectra reported by the top-128 Zenodo record.
pub const ANNOTATED_MS2_TOP_128_SPECTRA_COUNT: usize = 443_905;

/// Zenodo record ID for the top-60 annotated harmonized MS2 MGF dataset.
pub const ANNOTATED_MS2_TOP_60_ZENODO_RECORD_ID: u64 = 20_039_648;

/// DOI for the top-60 annotated harmonized MS2 MGF dataset.
pub const ANNOTATED_MS2_TOP_60_ZENODO_DOI: &str = "10.5281/zenodo.20039648";

/// Zenodo endpoint for the top-60 annotated harmonized MS2 MGF file.
pub const ANNOTATED_MS2_TOP_60_MGF_URL: &str = "https://zenodo.org/api/records/20039648/files/combined-gnps-mass-spec-gym-npc-faithful.harmonized-subset.mgf.zst/content";

/// File name used for the top-60 annotated harmonized MS2 MGF file.
pub const ANNOTATED_MS2_TOP_60_MGF_FILE_NAME: &str =
    "combined-gnps-mass-spec-gym-npc-faithful.harmonized-subset.mgf.zst";

/// Number of spectra reported by the top-60 Zenodo record.
pub const ANNOTATED_MS2_TOP_60_SPECTRA_COUNT: usize = 439_403;

/// Zenodo record ID for the default annotated harmonized MS2 MGF dataset.
pub const ANNOTATED_MS2_ZENODO_RECORD_ID: u64 = ANNOTATED_MS2_TOP_128_ZENODO_RECORD_ID;

/// DOI for the default annotated harmonized MS2 MGF dataset.
pub const ANNOTATED_MS2_ZENODO_DOI: &str = ANNOTATED_MS2_TOP_128_ZENODO_DOI;

/// Current Zenodo endpoint for the default annotated harmonized MS2 MGF file.
pub const ANNOTATED_MS2_MGF_URL: &str = ANNOTATED_MS2_TOP_128_MGF_URL;

/// File name used for the default annotated harmonized MS2 MGF file.
pub const ANNOTATED_MS2_MGF_FILE_NAME: &str = ANNOTATED_MS2_TOP_128_MGF_FILE_NAME;

/// Number of spectra reported by the default Zenodo record.
pub const ANNOTATED_MS2_SPECTRA_COUNT: usize = ANNOTATED_MS2_TOP_128_SPECTRA_COUNT;

/// Published annotated harmonized MS2 MGF conversion variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "mem_size", derive(mem_dbg::MemSize))]
#[cfg_attr(feature = "mem_dbg", derive(mem_dbg::MemDbg))]
#[cfg_attr(feature = "mem_size", mem_size(flat))]
pub enum AnnotatedMs2Variant {
    /// Current conversion capped to the top 128 fragment peaks per spectrum.
    #[default]
    Top128Peaks,
    /// Previous conversion capped to the top 60 fragment peaks per spectrum.
    Top60Peaks,
}

impl AnnotatedMs2Variant {
    /// Returns the Zenodo record ID for this published annotated MS2 variant.
    #[must_use]
    pub const fn record_id(self) -> u64 {
        match self {
            Self::Top128Peaks => ANNOTATED_MS2_TOP_128_ZENODO_RECORD_ID,
            Self::Top60Peaks => ANNOTATED_MS2_TOP_60_ZENODO_RECORD_ID,
        }
    }

    /// Returns the DOI for this published annotated MS2 variant.
    #[must_use]
    pub const fn doi(self) -> &'static str {
        match self {
            Self::Top128Peaks => ANNOTATED_MS2_TOP_128_ZENODO_DOI,
            Self::Top60Peaks => ANNOTATED_MS2_TOP_60_ZENODO_DOI,
        }
    }

    /// Returns the MGF download URL for this published annotated MS2 variant.
    #[must_use]
    pub const fn mgf_url(self) -> &'static str {
        match self {
            Self::Top128Peaks => ANNOTATED_MS2_TOP_128_MGF_URL,
            Self::Top60Peaks => ANNOTATED_MS2_TOP_60_MGF_URL,
        }
    }

    /// Returns the MGF file name for this published annotated MS2 variant.
    #[must_use]
    pub const fn mgf_file_name(self) -> &'static str {
        match self {
            Self::Top128Peaks => ANNOTATED_MS2_TOP_128_MGF_FILE_NAME,
            Self::Top60Peaks => ANNOTATED_MS2_TOP_60_MGF_FILE_NAME,
        }
    }

    /// Returns the reported validated spectrum count for this published variant.
    #[must_use]
    pub const fn spectra_count(self) -> usize {
        match self {
            Self::Top128Peaks => ANNOTATED_MS2_TOP_128_SPECTRA_COUNT,
            Self::Top60Peaks => ANNOTATED_MS2_TOP_60_SPECTRA_COUNT,
        }
    }

    fn default_target_directory(self) -> PathBuf {
        let directory = match self {
            Self::Top128Peaks => "mascot-rs-annotated-ms2-top-128-peaks",
            Self::Top60Peaks => "mascot-rs-annotated-ms2",
        };
        std::env::temp_dir().join(directory)
    }
}

/// Builder for downloading and loading the annotated harmonized MS2 dataset.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "mem_size", derive(mem_dbg::MemSize))]
#[cfg_attr(feature = "mem_dbg", derive(mem_dbg::MemDbg))]
pub struct AnnotatedMs2Builder<P: SpectrumFloat = f64> {
    variant: AnnotatedMs2Variant,
    config: SingleFileDatasetConfig,
    target_directory_is_default: bool,
    precision: PhantomData<fn() -> P>,
}

impl<P: SpectrumFloat> Default for AnnotatedMs2Builder<P> {
    fn default() -> Self {
        let variant = AnnotatedMs2Variant::default();
        Self {
            variant,
            config: SingleFileDatasetConfig::new(
                variant.mgf_url(),
                variant.default_target_directory(),
                variant.mgf_file_name(),
                "Downloading annotated MS2 MGF dataset",
            ),
            target_directory_is_default: true,
            precision: PhantomData,
        }
    }
}

impl<P: SpectrumFloat> AnnotatedMs2Builder<P> {
    /// Selects the published annotated MS2 conversion variant.
    ///
    /// If the target directory was not set explicitly, changing the variant
    /// also switches to a variant-specific cache directory.
    #[must_use]
    pub fn variant(mut self, variant: AnnotatedMs2Variant) -> Self {
        self.variant = variant;
        self.config.set_url(variant.mgf_url());
        self.config.set_file_name(variant.mgf_file_name());
        if self.target_directory_is_default {
            self.config
                .set_target_directory(variant.default_target_directory());
        }
        self
    }

    /// Selects the top-128 peaks annotated MS2 conversion.
    #[must_use]
    pub fn top_128_peaks(self) -> Self {
        self.variant(AnnotatedMs2Variant::Top128Peaks)
    }

    /// Selects the top-60 peaks annotated MS2 conversion.
    #[must_use]
    pub fn top_60_peaks(self) -> Self {
        self.variant(AnnotatedMs2Variant::Top60Peaks)
    }

    /// Returns the selected published annotated MS2 conversion variant.
    #[must_use]
    pub const fn selected_variant(&self) -> AnnotatedMs2Variant {
        self.variant
    }

    /// Returns the Zenodo record ID for the annotated MS2 dataset.
    #[must_use]
    pub const fn record_id(&self) -> u64 {
        self.variant.record_id()
    }

    /// Returns the DOI for the annotated MS2 dataset.
    #[must_use]
    pub const fn doi(&self) -> &'static str {
        self.variant.doi()
    }

    /// Returns the reported validated spectrum count for the selected variant.
    #[must_use]
    pub const fn spectra_count(&self) -> usize {
        self.variant.spectra_count()
    }

    /// Sets the source URL.
    #[must_use]
    pub fn url<S: Into<String>>(mut self, url: S) -> Self {
        self.config.set_url(url);
        self
    }

    /// Sets the directory where the annotated MS2 MGF file is stored.
    #[must_use]
    pub fn target_directory<PathLike: AsRef<Path>>(mut self, target_directory: PathLike) -> Self {
        self.config.set_target_directory(target_directory);
        self.target_directory_is_default = false;
        self
    }

    /// Sets the downloaded file name inside the target directory.
    #[must_use]
    pub fn file_name<S: Into<String>>(mut self, file_name: S) -> Self {
        self.config.set_file_name(file_name);
        self
    }

    /// Enables download progress reporting.
    #[must_use]
    pub const fn verbose(mut self) -> Self {
        self.config.enable_verbose();
        self
    }

    /// Sets whether to redownload the dataset even if the target file exists.
    #[must_use]
    pub const fn force_download(mut self, force_download: bool) -> Self {
        self.config.set_force_download(force_download);
        self
    }

    /// Returns the configured download path.
    #[must_use]
    pub fn path(&self) -> PathBuf {
        self.config.path()
    }

    /// Downloads the annotated MS2 MGF file if needed without loading records.
    ///
    /// # Errors
    /// Returns an error if the configured file name is empty, if the target
    /// directory cannot be created, if the existing local file cannot be
    /// inspected, or if the remote dataset cannot be downloaded.
    pub async fn download(self) -> Result<AnnotatedMs2Download> {
        std::future::ready(()).await;
        self.config
            .download()
            .map(AnnotatedMs2Download::from_single_file)
    }

    /// Downloads the annotated MS2 MGF file if needed and loads valid records.
    ///
    /// The published MGF is already normalized to the crate's structured
    /// headers. Malformed records are skipped and counted in the returned
    /// [`AnnotatedMs2Load`].
    ///
    /// # Errors
    /// Returns an error if the download fails, if the target file cannot be
    /// written, or if the downloaded file cannot be read back.
    pub async fn load(self) -> Result<AnnotatedMs2Load<P>> {
        let download = self.download().await?;
        let mut iterator = Self::iter_path(download.path())?;
        let spectra = iterator
            .by_ref()
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .collect();
        let skipped_records = iterator.skipped_records();

        Ok(AnnotatedMs2Load {
            spectra,
            skipped_records,
            path: download.path,
            bytes: download.bytes,
        })
    }

    /// Downloads the annotated MS2 MGF file if needed and returns a streaming iterator.
    ///
    /// The returned iterator skips malformed records and reports the skipped
    /// count through
    /// [`MGFIter::skipped_records`](crate::mascot_generic_format::MGFIter::skipped_records)
    /// after it has been exhausted.
    ///
    /// # Errors
    /// Returns an error if the download fails, if the target file cannot be
    /// written, or if the downloaded file cannot be opened for streaming.
    pub async fn mgf_iter(self) -> Result<MGFPathIter<P>> {
        let download = self.download().await?;
        Self::iter_path(download.path())
    }

    fn iter_path(path: &Path) -> Result<MGFPathIter<P>> {
        MGFVec::<P>::iter_from_path(path).map(MGFIter::skipping_invalid_records)
    }
}

impl<P> Dataset for AnnotatedMs2Builder<P>
where
    P: SpectrumFloat + Send + 'static,
{
    type Download = AnnotatedMs2Download;
    type Iter = MGFPathIter<P>;
    type Load = AnnotatedMs2Load<P>;

    fn download(self) -> DatasetFuture<Self::Download> {
        Box::pin(Self::download(self))
    }

    fn mgf_iter(self) -> DatasetFuture<Self::Iter> {
        Box::pin(Self::mgf_iter(self))
    }

    fn load(self) -> DatasetFuture<Self::Load> {
        Box::pin(Self::load(self))
    }
}

/// Result of downloading the annotated MS2 MGF dataset.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "mem_size", derive(mem_dbg::MemSize))]
#[cfg_attr(feature = "mem_dbg", derive(mem_dbg::MemDbg))]
pub struct AnnotatedMs2Download {
    path: PathBuf,
    bytes: u64,
}

impl AnnotatedMs2Download {
    fn from_single_file(download: SingleFileDatasetDownload) -> Self {
        let (path, bytes) = download.into_parts();
        Self { path, bytes }
    }

    /// Returns the local path used for the annotated MS2 MGF file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the size of the local annotated MS2 MGF file in bytes.
    #[must_use]
    pub const fn bytes(&self) -> u64 {
        self.bytes
    }
}

/// Result of loading the annotated MS2 MGF dataset.
#[derive(Debug)]
#[cfg_attr(feature = "mem_size", derive(mem_dbg::MemSize))]
#[cfg_attr(feature = "mem_dbg", derive(mem_dbg::MemDbg))]
pub struct AnnotatedMs2Load<P: SpectrumFloat = f64> {
    spectra: MGFVec<P>,
    skipped_records: usize,
    path: PathBuf,
    bytes: u64,
}

impl<P: SpectrumFloat> AnnotatedMs2Load<P> {
    /// Returns the loaded spectra.
    #[must_use]
    pub const fn spectra(&self) -> &MGFVec<P> {
        &self.spectra
    }

    /// Consumes the load result and returns the loaded spectra.
    #[must_use]
    pub fn into_spectra(self) -> MGFVec<P> {
        self.spectra
    }

    /// Returns the number of ion blocks skipped during tolerant loading.
    #[must_use]
    pub const fn skipped_records(&self) -> usize {
        self.skipped_records
    }

    /// Returns the local path used for the annotated MS2 MGF file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the size of the local annotated MS2 MGF file in bytes.
    #[must_use]
    pub const fn bytes(&self) -> u64 {
        self.bytes
    }
}

impl<P: SpectrumFloat> AsRef<MGFVec<P>> for AnnotatedMs2Load<P> {
    fn as_ref(&self) -> &MGFVec<P> {
        self.spectra()
    }
}
