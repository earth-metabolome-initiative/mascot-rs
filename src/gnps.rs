use core::marker::PhantomData;
use std::path::{Path, PathBuf};

use mass_spectrometry::prelude::SpectrumFloat;

use crate::dataset::{Dataset, DatasetFuture, SingleFileDatasetConfig, SingleFileDatasetDownload};
use crate::error::Result;
use crate::mascot_generic_format::{MGFIter, MGFPathIter, MGFVec};

/// Current GNPS endpoint for the aggregated public MGF spectral library.
pub const GNPS_ALL_MGF_URL: &str = "https://external.gnps2.org/gnpslibrary/ALL_GNPS.mgf";

const GNPS_ALL_MGF_FILE_NAME: &str = "ALL_GNPS.mgf";

/// Builder for downloading and loading the GNPS MGF library.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "mem_size", derive(mem_dbg::MemSize))]
#[cfg_attr(feature = "mem_dbg", derive(mem_dbg::MemDbg))]
pub struct GNPSBuilder<P: SpectrumFloat = f64> {
    config: SingleFileDatasetConfig,
    precision: PhantomData<fn() -> P>,
}

impl<P: SpectrumFloat> Default for GNPSBuilder<P> {
    fn default() -> Self {
        Self {
            config: SingleFileDatasetConfig::new(
                GNPS_ALL_MGF_URL,
                std::env::temp_dir().join("mascot-rs-gnps"),
                GNPS_ALL_MGF_FILE_NAME,
                "Downloading GNPS MGF library",
            ),
            precision: PhantomData,
        }
    }
}

impl<P: SpectrumFloat> GNPSBuilder<P> {
    /// Sets the source URL.
    #[must_use]
    pub fn url<S: Into<String>>(mut self, url: S) -> Self {
        self.config.set_url(url);
        self
    }

    /// Sets the directory where the GNPS MGF file is stored.
    #[must_use]
    pub fn target_directory<PathLike: AsRef<Path>>(mut self, target_directory: PathLike) -> Self {
        self.config.set_target_directory(target_directory);
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

    /// Sets whether to redownload the library even if the target file exists.
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

    /// Downloads the GNPS library if needed without loading the MGF records.
    ///
    /// # Errors
    /// Returns an error if the configured file name is empty, if the target
    /// directory cannot be created, if the existing local file cannot be
    /// inspected, or if the remote library cannot be downloaded.
    pub async fn download(self) -> Result<GNPSDownload> {
        std::future::ready(()).await;
        self.config.download().map(GNPSDownload::from_single_file)
    }

    /// Downloads the GNPS library if needed and loads the valid MGF records.
    ///
    /// GNPS library exports can contain empty or malformed ion blocks. Those
    /// records are skipped and counted in the returned [`GNPSLoad`].
    ///
    /// # Errors
    /// Returns an error if the download fails, if the target file cannot be
    /// written, or if the downloaded file cannot be read back.
    pub async fn load(self) -> Result<GNPSLoad<P>> {
        let download = self.download().await?;
        let mut iterator = Self::iter_path(download.path())?;
        let spectra = iterator
            .by_ref()
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .collect();
        let skipped_records = iterator.skipped_records();

        Ok(GNPSLoad {
            spectra,
            skipped_records,
            path: download.path,
            bytes: download.bytes,
        })
    }

    /// Downloads the GNPS library if needed and returns a streaming MGF iterator.
    ///
    /// GNPS library exports can contain empty or malformed ion blocks. The
    /// returned iterator skips malformed records and reports the skipped count
    /// through [`MGFIter::skipped_records`](crate::mascot_generic_format::MGFIter::skipped_records)
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

impl<P> Dataset for GNPSBuilder<P>
where
    P: SpectrumFloat + Send + 'static,
{
    type Download = GNPSDownload;
    type Iter = MGFPathIter<P>;
    type Load = GNPSLoad<P>;

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

/// Result of downloading the GNPS MGF library.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "mem_size", derive(mem_dbg::MemSize))]
#[cfg_attr(feature = "mem_dbg", derive(mem_dbg::MemDbg))]
pub struct GNPSDownload {
    path: PathBuf,
    bytes: u64,
}

impl GNPSDownload {
    fn from_single_file(download: SingleFileDatasetDownload) -> Self {
        let (path, bytes) = download.into_parts();
        Self { path, bytes }
    }

    /// Returns the local path used for the GNPS MGF file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the size of the local GNPS MGF file in bytes.
    #[must_use]
    pub const fn bytes(&self) -> u64 {
        self.bytes
    }
}

/// Result of loading the GNPS MGF library.
#[derive(Debug)]
#[cfg_attr(feature = "mem_size", derive(mem_dbg::MemSize))]
#[cfg_attr(feature = "mem_dbg", derive(mem_dbg::MemDbg))]
pub struct GNPSLoad<P: SpectrumFloat = f64> {
    spectra: MGFVec<P>,
    skipped_records: usize,
    path: PathBuf,
    bytes: u64,
}

impl<P: SpectrumFloat> GNPSLoad<P> {
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

    /// Returns the number of GNPS ion blocks skipped during tolerant loading.
    #[must_use]
    pub const fn skipped_records(&self) -> usize {
        self.skipped_records
    }

    /// Returns the local path used for the GNPS MGF file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the size of the local GNPS MGF file in bytes.
    #[must_use]
    pub const fn bytes(&self) -> u64 {
        self.bytes
    }
}

impl<P: SpectrumFloat> AsRef<MGFVec<P>> for GNPSLoad<P> {
    fn as_ref(&self) -> &MGFVec<P> {
        self.spectra()
    }
}
