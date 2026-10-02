use core::marker::PhantomData;
use std::collections::VecDeque;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::time::Duration;

use indicatif::ProgressBar;
use mass_spectrometry::prelude::SpectrumFloat;
use zenodo_rs::{ArtifactSelector, Auth, RecordId, TransferProgress, ZenodoClient};

use crate::dataset::{Dataset, DatasetFuture};
use crate::error::{MascotError, Result};
use crate::mascot_generic_format::{MGFIter, MGFLineSource, MGFVec};

/// Zenodo record ID for the top-100 peaks `GeMS-A10` MGF dataset.
pub const GEMS_A10_TOP_100_ZENODO_RECORD_ID: u64 = 19_980_668;

/// DOI for the top-100 peaks `GeMS-A10` MGF dataset.
pub const GEMS_A10_TOP_100_ZENODO_DOI: &str = "10.5281/zenodo.19980668";

/// Zenodo record ID for the default top-100 peaks `GeMS-A10` MGF dataset.
pub const GEMS_A10_ZENODO_RECORD_ID: u64 = GEMS_A10_TOP_100_ZENODO_RECORD_ID;

/// DOI for the default top-100 peaks `GeMS-A10` MGF dataset.
pub const GEMS_A10_ZENODO_DOI: &str = GEMS_A10_TOP_100_ZENODO_DOI;

/// Zenodo record ID for the top-128 peaks `GeMS-A10` MGF dataset.
pub const GEMS_A10_TOP_128_ZENODO_RECORD_ID: u64 = 20_040_772;

/// DOI for the top-128 peaks `GeMS-A10` MGF dataset.
pub const GEMS_A10_TOP_128_ZENODO_DOI: &str = "10.5281/zenodo.20040772";

/// Zenodo record ID for the top-60 peaks `GeMS-A10` MGF dataset.
pub const GEMS_A10_TOP_60_ZENODO_RECORD_ID: u64 = 20_001_888;

/// DOI for the top-60 peaks `GeMS-A10` MGF dataset.
pub const GEMS_A10_TOP_60_ZENODO_DOI: &str = "10.5281/zenodo.20001888";

/// Zenodo record ID for the top-40 peaks `GeMS-A10` MGF dataset.
pub const GEMS_A10_TOP_40_ZENODO_RECORD_ID: u64 = 20_002_962;

/// DOI for the top-40 peaks `GeMS-A10` MGF dataset.
pub const GEMS_A10_TOP_40_ZENODO_DOI: &str = "10.5281/zenodo.20002962";

/// Zenodo record ID for the top-20 peaks `GeMS-A10` MGF dataset.
pub const GEMS_A10_TOP_20_ZENODO_RECORD_ID: u64 = 20_027_219;

/// DOI for the top-20 peaks `GeMS-A10` MGF dataset.
pub const GEMS_A10_TOP_20_ZENODO_DOI: &str = "10.5281/zenodo.20027219";

/// Number of compressed MGF part files in the converted `GeMS-A10` dataset.
pub const GEMS_A10_MGF_PART_COUNT: u8 = 24;

/// Streaming iterator returned for selected `GeMS-A10` MGF files.
pub type GemsA10Iter<P = f64> = MGFIter<P, GemsA10LineSource>;

/// Line source that streams the selected `GeMS-A10` MGF files as one document.
pub struct GemsA10LineSource {
    paths: VecDeque<PathBuf>,
    current: Option<Box<dyn BufRead>>,
    line: String,
}

impl GemsA10LineSource {
    fn new(paths: Vec<PathBuf>) -> Self {
        Self {
            paths: paths.into(),
            current: None,
            line: String::new(),
        }
    }

    fn open_next_path(&mut self) -> Result<bool> {
        let Some(path) = self.paths.pop_front() else {
            return Ok(false);
        };
        self.current = Some(MGFVec::<f64>::reader_from_path(&path)?);
        Ok(true)
    }
}

impl MGFLineSource for GemsA10LineSource {
    type Line<'line>
        = &'line str
    where
        Self: 'line;

    fn next_line(&mut self) -> Option<Result<Self::Line<'_>>> {
        loop {
            if self.current.is_none() {
                match self.open_next_path() {
                    Ok(true) => {}
                    Ok(false) => return None,
                    Err(source) => return Some(Err(source)),
                }
            }

            let current = self.current.as_mut()?;
            self.line.clear();
            match current.read_line(&mut self.line) {
                Ok(0) => {
                    self.current = None;
                }
                Ok(_) => return Some(Ok(self.line.trim_end_matches(['\r', '\n']))),
                Err(source) => return Some(Err(MascotError::InputIo { source })),
            }
        }
    }
}

/// Published `GeMS-A10` MGF conversion variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "mem_size", derive(mem_dbg::MemSize))]
#[cfg_attr(feature = "mem_dbg", derive(mem_dbg::MemDbg))]
#[cfg_attr(feature = "mem_size", mem_size(flat))]
pub enum GemsA10Variant {
    /// Larger conversion capped to the top 128 fragment peaks per spectrum.
    Top128Peaks,
    /// Original conversion capped to the top 100 fragment peaks per spectrum.
    #[default]
    Top100Peaks,
    /// Smaller conversion capped to the top 60 fragment peaks per spectrum.
    Top60Peaks,
    /// Smaller conversion capped to the top 40 fragment peaks per spectrum.
    Top40Peaks,
    /// Smaller conversion capped to the top 20 fragment peaks per spectrum.
    Top20Peaks,
}

impl GemsA10Variant {
    /// Returns the Zenodo record ID for this published `GeMS-A10` variant.
    #[must_use]
    pub const fn record_id(self) -> u64 {
        match self {
            Self::Top128Peaks => GEMS_A10_TOP_128_ZENODO_RECORD_ID,
            Self::Top100Peaks => GEMS_A10_TOP_100_ZENODO_RECORD_ID,
            Self::Top60Peaks => GEMS_A10_TOP_60_ZENODO_RECORD_ID,
            Self::Top40Peaks => GEMS_A10_TOP_40_ZENODO_RECORD_ID,
            Self::Top20Peaks => GEMS_A10_TOP_20_ZENODO_RECORD_ID,
        }
    }

    /// Returns the DOI for this published `GeMS-A10` variant.
    #[must_use]
    pub const fn doi(self) -> &'static str {
        match self {
            Self::Top128Peaks => GEMS_A10_TOP_128_ZENODO_DOI,
            Self::Top100Peaks => GEMS_A10_TOP_100_ZENODO_DOI,
            Self::Top60Peaks => GEMS_A10_TOP_60_ZENODO_DOI,
            Self::Top40Peaks => GEMS_A10_TOP_40_ZENODO_DOI,
            Self::Top20Peaks => GEMS_A10_TOP_20_ZENODO_DOI,
        }
    }

    fn default_target_directory(self) -> PathBuf {
        let directory = match self {
            Self::Top128Peaks => "mascot-rs-gems-a10-top-128-peaks",
            Self::Top100Peaks => "mascot-rs-gems-a10",
            Self::Top60Peaks => "mascot-rs-gems-a10-top-60-peaks",
            Self::Top40Peaks => "mascot-rs-gems-a10-top-40-peaks",
            Self::Top20Peaks => "mascot-rs-gems-a10-top-20-peaks",
        };
        std::env::temp_dir().join(directory)
    }
}

/// Builder for downloading and loading the converted `GeMS-A10` MGF dataset.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "mem_size", derive(mem_dbg::MemSize))]
#[cfg_attr(feature = "mem_dbg", derive(mem_dbg::MemDbg))]
pub struct GemsA10Builder<P: SpectrumFloat = f64> {
    config: GemsA10BuilderConfig,
    precision: PhantomData<fn() -> P>,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "mem_size", derive(mem_dbg::MemSize))]
#[cfg_attr(feature = "mem_dbg", derive(mem_dbg::MemDbg))]
struct GemsA10BuilderConfig {
    variant: GemsA10Variant,
    target_directory: PathBuf,
    target_directory_is_default: bool,
    file_keys: Vec<String>,
    verbose: bool,
    force_download: bool,
    token: Option<String>,
    request_timeout_nanos: Option<u64>,
    connect_timeout_nanos: Option<u64>,
}

impl<P: SpectrumFloat> Default for GemsA10Builder<P> {
    fn default() -> Self {
        let variant = GemsA10Variant::default();
        Self {
            config: GemsA10BuilderConfig {
                variant,
                target_directory: variant.default_target_directory(),
                target_directory_is_default: true,
                file_keys: (0..GEMS_A10_MGF_PART_COUNT)
                    .map(Self::part_file_key_unchecked)
                    .collect(),
                verbose: false,
                force_download: false,
                token: None,
                request_timeout_nanos: None,
                connect_timeout_nanos: None,
            },
            precision: PhantomData,
        }
    }
}

impl<P: SpectrumFloat> GemsA10Builder<P> {
    /// Returns the Zenodo file key for a published MGF part.
    ///
    /// # Errors
    /// Returns an error if the part number is outside the published range.
    pub fn part_file_key(part: u8) -> Result<String> {
        Self::validate_part(part)?;
        Ok(Self::part_file_key_unchecked(part))
    }

    /// Selects the published `GeMS-A10` conversion variant.
    ///
    /// If the target directory was not set explicitly, changing the variant
    /// also switches to a variant-specific cache directory so files with the
    /// same Zenodo keys do not collide across variants.
    #[must_use]
    pub fn variant(mut self, variant: GemsA10Variant) -> Self {
        self.config.variant = variant;
        if self.config.target_directory_is_default {
            self.config.target_directory = variant.default_target_directory();
        }
        self
    }

    /// Selects the top-128 peaks conversion.
    #[must_use]
    pub fn top_128_peaks(self) -> Self {
        self.variant(GemsA10Variant::Top128Peaks)
    }

    /// Selects the original top-100 peaks conversion.
    #[must_use]
    pub fn top_100_peaks(self) -> Self {
        self.variant(GemsA10Variant::Top100Peaks)
    }

    /// Selects the top-60 peaks conversion.
    #[must_use]
    pub fn top_60_peaks(self) -> Self {
        self.variant(GemsA10Variant::Top60Peaks)
    }

    /// Selects the top-40 peaks conversion.
    #[must_use]
    pub fn top_40_peaks(self) -> Self {
        self.variant(GemsA10Variant::Top40Peaks)
    }

    /// Selects the top-20 peaks conversion.
    #[must_use]
    pub fn top_20_peaks(self) -> Self {
        self.variant(GemsA10Variant::Top20Peaks)
    }

    /// Returns the selected published `GeMS-A10` conversion variant.
    #[must_use]
    pub const fn selected_variant(&self) -> GemsA10Variant {
        self.config.variant
    }

    /// Returns the Zenodo record ID for the selected variant.
    #[must_use]
    pub const fn record_id(&self) -> u64 {
        self.config.variant.record_id()
    }

    /// Returns the DOI for the selected published `GeMS-A10` conversion variant.
    #[must_use]
    pub const fn doi(&self) -> &'static str {
        self.config.variant.doi()
    }

    /// Sets the directory where the dataset files are stored.
    #[must_use]
    pub fn target_directory<PathLike: AsRef<Path>>(mut self, target_directory: PathLike) -> Self {
        self.config.target_directory = target_directory.as_ref().to_path_buf();
        self.config.target_directory_is_default = false;
        self
    }

    /// Selects one Zenodo file key to download and load.
    #[must_use]
    pub fn file_key<S: Into<String>>(mut self, file_key: S) -> Self {
        self.config.file_keys = vec![file_key.into()];
        self
    }

    /// Selects the Zenodo file keys to download and load.
    #[must_use]
    pub fn file_keys<T, S>(mut self, file_keys: T) -> Self
    where
        T: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.config.file_keys = file_keys.into_iter().map(Into::into).collect();
        self
    }

    /// Selects one published MGF part to download and load.
    ///
    /// # Errors
    /// Returns an error if the part number is outside the published range.
    pub fn part(mut self, part: u8) -> Result<Self> {
        self.config.file_keys = vec![Self::part_file_key(part)?];
        Ok(self)
    }

    /// Selects multiple published MGF parts to download and load.
    ///
    /// # Errors
    /// Returns an error if any part number is outside the published range.
    pub fn parts<T>(mut self, parts: T) -> Result<Self>
    where
        T: IntoIterator<Item = u8>,
    {
        self.config.file_keys = parts
            .into_iter()
            .map(Self::part_file_key)
            .collect::<Result<Vec<_>>>()?;
        Ok(self)
    }

    /// Selects all published MGF parts.
    #[must_use]
    pub fn all_parts(mut self) -> Self {
        self.config.file_keys = (0..GEMS_A10_MGF_PART_COUNT)
            .map(Self::part_file_key_unchecked)
            .collect();
        self
    }

    /// Returns the selected Zenodo file keys.
    #[must_use]
    pub fn selected_file_keys(&self) -> &[String] {
        &self.config.file_keys
    }

    /// Enables download progress reporting.
    #[must_use]
    pub const fn verbose(mut self) -> Self {
        self.config.verbose = true;
        self
    }

    /// Sets whether to redownload files even if they already exist locally.
    #[must_use]
    pub const fn force_download(mut self, force_download: bool) -> Self {
        self.config.force_download = force_download;
        self
    }

    /// Sets the Zenodo token used by `zenodo-rs`.
    ///
    /// The published `GeMS-A10` record is public, so the default empty token is
    /// sufficient for public downloads.
    #[must_use]
    pub fn token<S: Into<String>>(mut self, token: S) -> Self {
        self.config.token = Some(token.into());
        self
    }

    /// Sets the overall HTTP request timeout used by `zenodo-rs`.
    #[must_use]
    pub fn request_timeout(mut self, timeout: Duration) -> Self {
        self.config.request_timeout_nanos = Some(Self::duration_to_nanos(timeout));
        self
    }

    /// Sets the TCP connect timeout used by `zenodo-rs`.
    #[must_use]
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.config.connect_timeout_nanos = Some(Self::duration_to_nanos(timeout));
        self
    }

    /// Returns the configured local path for a Zenodo file key.
    #[must_use]
    pub fn path_for_file_key(&self, file_key: &str) -> PathBuf {
        self.config.target_directory.join(file_key)
    }

    /// Returns the configured local paths for the selected Zenodo file keys.
    #[must_use]
    pub fn paths(&self) -> Vec<PathBuf> {
        self.config
            .file_keys
            .iter()
            .map(|file_key| self.path_for_file_key(file_key))
            .collect()
    }

    /// Downloads the selected `GeMS-A10` files if needed without loading records.
    ///
    /// The published dataset is split into compressed MGF part files. This
    /// method only ensures that the selected local files exist.
    ///
    /// # Errors
    /// Returns an error if the selected file list is empty, if a file key is
    /// empty, if a Zenodo operation fails, or if a local file cannot be
    /// inspected or written.
    pub async fn download(self) -> Result<GemsA10Download> {
        std::future::ready(()).await;

        if self.config.file_keys.is_empty() {
            return Err(MascotError::MissingField {
                builder: "GemsA10Builder",
                field: "file_keys",
            });
        }
        if self.config.file_keys.iter().any(String::is_empty) {
            return Err(MascotError::EmptyFilename);
        }

        std::fs::create_dir_all(&self.config.target_directory).map_err(|source| {
            MascotError::Io {
                path: self.config.target_directory.display().to_string(),
                source,
            }
        })?;

        let mut client = None;
        let mut files = Vec::with_capacity(self.config.file_keys.len());
        let mut bytes = 0_u64;

        for file_key in &self.config.file_keys {
            let path = self.path_for_file_key(file_key);
            let file_bytes = if !self.config.force_download
                && path.try_exists().map_err(|source| MascotError::Io {
                    path: path.display().to_string(),
                    source,
                })? {
                std::fs::metadata(&path)
                    .map_err(|source| MascotError::Io {
                        path: path.display().to_string(),
                        source,
                    })?
                    .len()
            } else {
                if client.is_none() {
                    client = Some(self.client()?);
                }
                self.download_file(
                    client.as_ref().ok_or_else(|| MascotError::MissingField {
                        builder: "GemsA10Builder",
                        field: "zenodo_client",
                    })?,
                    file_key,
                    &path,
                )
                .await?
            };

            bytes += file_bytes;
            files.push(GemsA10FileDownload {
                key: file_key.clone(),
                path,
                bytes: file_bytes,
            });
        }

        Ok(GemsA10Download { files, bytes })
    }

    /// Downloads the selected `GeMS-A10` files if needed and loads valid MGF records.
    ///
    /// The published dataset is split into compressed MGF part files. All
    /// malformed records are skipped and counted in the returned [`GemsA10Load`].
    ///
    /// # Errors
    /// Returns an error if downloading fails or if a downloaded file cannot be
    /// read back.
    pub async fn load(self) -> Result<GemsA10Load<P>> {
        let download = self.download().await?;
        let mut iterator = Self::iter_download(&download);
        let records = iterator.by_ref().collect::<Result<Vec<_>>>()?;
        let skipped_records = iterator.skipped_records();

        Ok(GemsA10Load {
            spectra: records.into_iter().collect(),
            skipped_records,
            files: download.files,
            bytes: download.bytes,
        })
    }

    /// Downloads the selected `GeMS-A10` files if needed and returns a streaming iterator.
    ///
    /// The returned iterator streams the selected compressed MGF part files as
    /// one logical document, skips malformed records, and reports the skipped
    /// count through [`MGFIter::skipped_records`](crate::mascot_generic_format::MGFIter::skipped_records)
    /// after it has been exhausted.
    ///
    /// # Errors
    /// Returns an error if the selected file list is invalid or if downloading
    /// fails. File open and read errors after the iterator is created are
    /// returned by the iterator itself.
    pub async fn mgf_iter(self) -> Result<GemsA10Iter<P>> {
        let download = self.download().await?;
        Ok(Self::iter_download(&download))
    }

    fn iter_download(download: &GemsA10Download) -> GemsA10Iter<P> {
        let paths = download
            .files()
            .iter()
            .map(|file| file.path().to_path_buf())
            .collect();
        MGFIter::<P, _>::from_line_source(GemsA10LineSource::new(paths)).skipping_invalid_records()
    }

    fn client(&self) -> Result<ZenodoClient> {
        let token = self.config.token.clone().unwrap_or_default();
        let mut builder = ZenodoClient::builder(Auth::new(token))
            .user_agent(format!("mascot-rs/{}", env!("CARGO_PKG_VERSION")));
        if let Some(timeout_nanos) = self.config.request_timeout_nanos {
            builder = builder.request_timeout(Duration::from_nanos(timeout_nanos));
        }
        if let Some(timeout_nanos) = self.config.connect_timeout_nanos {
            builder = builder.connect_timeout(Duration::from_nanos(timeout_nanos));
        }
        builder.build().map_err(|source| MascotError::Zenodo {
            operation: "client initialization".to_string(),
            source: Box::new(source),
        })
    }

    async fn download_file(
        &self,
        client: &ZenodoClient,
        file_key: &str,
        path: &Path,
    ) -> Result<u64> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| MascotError::Io {
                path: parent.display().to_string(),
                source,
            })?;
        }

        let selector = ArtifactSelector::file(RecordId(self.record_id()), file_key);
        let progress = self.progress(file_key);
        let resolved = client
            .download_artifact_with_progress(&selector, path, progress)
            .await
            .map_err(|source| MascotError::Zenodo {
                operation: format!("download of record {} file {file_key}", self.record_id()),
                source: Box::new(source),
            })?;
        Ok(resolved.bytes_written)
    }

    fn progress(&self, file_key: &str) -> GemsA10Progress {
        if self.config.verbose {
            let progress_bar = ProgressBar::new_spinner();
            progress_bar.set_message(format!("Downloading {file_key}"));
            return GemsA10Progress::Indicatif(progress_bar);
        }
        GemsA10Progress::Quiet
    }

    const fn validate_part(part: u8) -> Result<()> {
        if part >= GEMS_A10_MGF_PART_COUNT {
            return Err(MascotError::InvalidGemsA10Part {
                part,
                part_count: GEMS_A10_MGF_PART_COUNT,
            });
        }
        Ok(())
    }

    fn part_file_key_unchecked(part: u8) -> String {
        format!("GeMS_A10.mgf.part-{part:05}.mgf.zst")
    }

    fn duration_to_nanos(timeout: Duration) -> u64 {
        u64::try_from(timeout.as_nanos()).unwrap_or(u64::MAX)
    }
}

impl<P> Dataset for GemsA10Builder<P>
where
    P: SpectrumFloat + Send + 'static,
{
    type Download = GemsA10Download;
    type Iter = GemsA10Iter<P>;
    type Load = GemsA10Load<P>;

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

enum GemsA10Progress {
    Quiet,
    Indicatif(ProgressBar),
}

impl TransferProgress for GemsA10Progress {
    fn begin(&self, total_bytes: Option<u64>) {
        if let Self::Indicatif(progress_bar) = self {
            progress_bar.set_position(0);
            if let Some(total_bytes) = total_bytes {
                progress_bar.set_length(total_bytes);
            }
        }
    }

    fn advance(&self, delta: u64) {
        if let Self::Indicatif(progress_bar) = self {
            progress_bar.inc(delta);
        }
    }

    fn finish(&self) {
        if let Self::Indicatif(progress_bar) = self {
            progress_bar.finish_and_clear();
        }
    }
}

/// Metadata for a downloaded `GeMS-A10` file.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "mem_size", derive(mem_dbg::MemSize))]
#[cfg_attr(feature = "mem_dbg", derive(mem_dbg::MemDbg))]
pub struct GemsA10FileDownload {
    key: String,
    path: PathBuf,
    bytes: u64,
}

/// Alias for per-file `GeMS-A10` metadata returned by load and download results.
pub type GemsA10FileLoad = GemsA10FileDownload;

impl GemsA10FileDownload {
    /// Returns the Zenodo file key.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Returns the local path used for the file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the size of the local file in bytes.
    #[must_use]
    pub const fn bytes(&self) -> u64 {
        self.bytes
    }
}

/// Result of downloading selected `GeMS-A10` MGF files.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "mem_size", derive(mem_dbg::MemSize))]
#[cfg_attr(feature = "mem_dbg", derive(mem_dbg::MemDbg))]
pub struct GemsA10Download {
    files: Vec<GemsA10FileDownload>,
    bytes: u64,
}

impl GemsA10Download {
    /// Returns per-file download metadata.
    #[must_use]
    pub fn files(&self) -> &[GemsA10FileDownload] {
        &self.files
    }

    /// Returns the total size of the local dataset files in bytes.
    #[must_use]
    pub const fn bytes(&self) -> u64 {
        self.bytes
    }
}

/// Result of loading selected `GeMS-A10` MGF files.
#[derive(Debug)]
#[cfg_attr(feature = "mem_size", derive(mem_dbg::MemSize))]
#[cfg_attr(feature = "mem_dbg", derive(mem_dbg::MemDbg))]
pub struct GemsA10Load<P: SpectrumFloat = f64> {
    spectra: MGFVec<P>,
    skipped_records: usize,
    files: Vec<GemsA10FileDownload>,
    bytes: u64,
}

impl<P: SpectrumFloat> GemsA10Load<P> {
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

    /// Returns the number of malformed ion blocks skipped during tolerant loading.
    #[must_use]
    pub const fn skipped_records(&self) -> usize {
        self.skipped_records
    }

    /// Returns per-file load metadata.
    #[must_use]
    pub fn files(&self) -> &[GemsA10FileDownload] {
        &self.files
    }

    /// Returns the total size of the local dataset files in bytes.
    #[must_use]
    pub const fn bytes(&self) -> u64 {
        self.bytes
    }
}

impl<P: SpectrumFloat> AsRef<MGFVec<P>> for GemsA10Load<P> {
    fn as_ref(&self) -> &MGFVec<P> {
        self.spectra()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, Read};

    fn block_on_dataset<T>(future: impl std::future::Future<Output = Result<T>>) -> Result<T> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|source| MascotError::Io {
                path: "tokio runtime".to_owned(),
                source,
            })?;

        runtime.block_on(future)
    }

    #[test]
    fn default_builder_selects_all_published_parts() {
        let builder = GemsA10Builder::<f64>::default();

        assert_eq!(builder.selected_variant(), GemsA10Variant::Top100Peaks);
        assert_eq!(builder.record_id(), GEMS_A10_ZENODO_RECORD_ID);
        assert_eq!(builder.doi(), GEMS_A10_ZENODO_DOI);
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
    }

    #[test]
    fn builder_setters_update_paths_and_client_options() -> Result<()> {
        let target_directory = PathBuf::from("custom-gems-a10-cache");
        let builder = GemsA10Builder::<f32>::default()
            .top_100_peaks()
            .target_directory(&target_directory)
            .part(3)?
            .all_parts()
            .parts([1, 2])?
            .token("test-token")
            .request_timeout(Duration::from_millis(250))
            .connect_timeout(Duration::from_millis(125))
            .verbose()
            .force_download(true);

        assert_eq!(builder.selected_variant(), GemsA10Variant::Top100Peaks);
        assert_eq!(
            builder.selected_file_keys(),
            &[
                "GeMS_A10.mgf.part-00001.mgf.zst".to_string(),
                "GeMS_A10.mgf.part-00002.mgf.zst".to_string(),
            ]
        );
        assert_eq!(
            builder.path_for_file_key("selected.mgf"),
            target_directory.join("selected.mgf")
        );
        let _client = builder.client()?;

        let quiet_progress = GemsA10Builder::<f32>::default().progress("quiet.mgf");
        quiet_progress.begin(Some(10));
        quiet_progress.advance(5);
        quiet_progress.finish();
        let progress = builder.progress("selected.mgf");
        progress.begin(Some(10));
        progress.advance(5);
        progress.finish();

        Ok(())
    }

    #[test]
    fn selects_top_128_peaks_variant() {
        let builder = GemsA10Builder::<f64>::default().top_128_peaks();

        assert_eq!(builder.selected_variant(), GemsA10Variant::Top128Peaks);
        assert_eq!(builder.record_id(), GEMS_A10_TOP_128_ZENODO_RECORD_ID);
        assert_eq!(builder.doi(), GEMS_A10_TOP_128_ZENODO_DOI);
        assert!(builder
            .paths()
            .first()
            .is_some_and(|path| path.display().to_string().contains("top-128-peaks")));
        assert_eq!(
            builder.selected_file_keys().last().map(String::as_str),
            Some("GeMS_A10.mgf.part-00023.mgf.zst")
        );
    }

    #[test]
    fn selects_top_60_peaks_variant() {
        let builder = GemsA10Builder::<f64>::default().top_60_peaks();

        assert_eq!(builder.selected_variant(), GemsA10Variant::Top60Peaks);
        assert_eq!(builder.record_id(), GEMS_A10_TOP_60_ZENODO_RECORD_ID);
        assert_eq!(builder.doi(), GEMS_A10_TOP_60_ZENODO_DOI);
        assert!(builder
            .paths()
            .first()
            .is_some_and(|path| path.display().to_string().contains("top-60-peaks")));
        assert_eq!(
            builder.selected_file_keys().last().map(String::as_str),
            Some("GeMS_A10.mgf.part-00023.mgf.zst")
        );
    }

    #[test]
    fn selects_top_40_peaks_variant() {
        let builder = GemsA10Builder::<f64>::default().top_40_peaks();

        assert_eq!(builder.selected_variant(), GemsA10Variant::Top40Peaks);
        assert_eq!(builder.record_id(), GEMS_A10_TOP_40_ZENODO_RECORD_ID);
        assert_eq!(builder.doi(), GEMS_A10_TOP_40_ZENODO_DOI);
        assert!(builder
            .paths()
            .first()
            .is_some_and(|path| path.display().to_string().contains("top-40-peaks")));
        assert_eq!(
            builder.selected_file_keys().last().map(String::as_str),
            Some("GeMS_A10.mgf.part-00023.mgf.zst")
        );
    }

    #[test]
    fn selects_top_20_peaks_variant() {
        let builder = GemsA10Builder::<f64>::default().top_20_peaks();

        assert_eq!(builder.selected_variant(), GemsA10Variant::Top20Peaks);
        assert_eq!(builder.record_id(), GEMS_A10_TOP_20_ZENODO_RECORD_ID);
        assert_eq!(builder.doi(), GEMS_A10_TOP_20_ZENODO_DOI);
        assert!(builder
            .paths()
            .first()
            .is_some_and(|path| path.display().to_string().contains("top-20-peaks")));
        assert_eq!(
            builder.selected_file_keys().last().map(String::as_str),
            Some("GeMS_A10.mgf.part-00023.mgf.zst")
        );
    }

    #[test]
    fn variant_keeps_explicit_target_directory() {
        let target_directory = PathBuf::from("custom-gems-cache");
        let builder = GemsA10Builder::<f64>::default()
            .target_directory(&target_directory)
            .top_60_peaks();

        assert_eq!(
            builder.path_for_file_key("cached.mgf"),
            target_directory.join("cached.mgf")
        );
    }

    #[test]
    fn line_source_reports_missing_path_errors() {
        let path = std::env::temp_dir().join(format!(
            "mascot-rs-missing-gems-a10-source-{}.mgf",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let mut source = GemsA10LineSource::new(vec![path]);

        assert!(matches!(
            source.next_line(),
            Some(Err(MascotError::Io { .. }))
        ));
    }

    #[test]
    fn line_source_reports_read_errors() {
        struct FailingBufRead;

        impl Read for FailingBufRead {
            fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("forced read failure"))
            }
        }

        impl BufRead for FailingBufRead {
            fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
                Err(std::io::Error::other("forced buffer failure"))
            }

            fn consume(&mut self, _amount: usize) {}
        }

        let mut source = GemsA10LineSource {
            paths: VecDeque::new(),
            current: Some(Box::new(FailingBufRead)),
            line: String::new(),
        };

        assert!(matches!(
            source.next_line(),
            Some(Err(MascotError::InputIo { .. }))
        ));
    }

    #[test]
    fn validates_part_file_keys() -> Result<()> {
        assert_eq!(
            GemsA10Builder::<f64>::part_file_key(7)?,
            "GeMS_A10.mgf.part-00007.mgf.zst"
        );
        assert!(matches!(
            GemsA10Builder::<f64>::part_file_key(GEMS_A10_MGF_PART_COUNT),
            Err(MascotError::InvalidGemsA10Part { .. })
        ));
        Ok(())
    }

    #[test]
    fn download_reuses_multiple_cached_files() -> Result<()> {
        let target_directory = std::env::temp_dir().join(format!(
            "mascot-rs-gems-a10-unit-download-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&target_directory);
        std::fs::create_dir_all(&target_directory).map_err(|source| MascotError::Io {
            path: target_directory.display().to_string(),
            source,
        })?;
        let builder = GemsA10Builder::<f64>::default()
            .target_directory(&target_directory)
            .parts([0, 1])?;
        let first_path = builder.path_for_file_key("GeMS_A10.mgf.part-00000.mgf.zst");
        let second_path = builder.path_for_file_key("GeMS_A10.mgf.part-00001.mgf.zst");
        std::fs::write(&first_path, b"first").map_err(|source| MascotError::Io {
            path: first_path.display().to_string(),
            source,
        })?;
        std::fs::write(&second_path, b"second").map_err(|source| MascotError::Io {
            path: second_path.display().to_string(),
            source,
        })?;

        let download = block_on_dataset(builder.download())?;
        std::fs::remove_dir_all(&target_directory).map_err(|source| MascotError::Io {
            path: target_directory.display().to_string(),
            source,
        })?;

        assert_eq!(download.files().len(), 2);
        assert_eq!(download.files()[0].key(), "GeMS_A10.mgf.part-00000.mgf.zst");
        assert_eq!(download.files()[0].path(), first_path.as_path());
        assert_eq!(download.files()[0].bytes(), 5);
        assert_eq!(download.files()[1].key(), "GeMS_A10.mgf.part-00001.mgf.zst");
        assert_eq!(download.files()[1].path(), second_path.as_path());
        assert_eq!(download.files()[1].bytes(), 6);
        assert_eq!(download.bytes(), 11);

        Ok(())
    }

    #[test]
    fn download_reports_target_directory_errors() -> Result<()> {
        let target_directory = std::env::temp_dir().join(format!(
            "mascot-rs-gems-a10-directory-error-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&target_directory);
        std::fs::write(&target_directory, b"not a directory").map_err(|source| {
            MascotError::Io {
                path: target_directory.display().to_string(),
                source,
            }
        })?;
        let builder = GemsA10Builder::<f64>::default()
            .target_directory(&target_directory)
            .file_key("cached.mgf");

        let result = block_on_dataset(builder.download());
        std::fs::remove_file(&target_directory).map_err(|source| MascotError::Io {
            path: target_directory.display().to_string(),
            source,
        })?;

        assert!(matches!(result, Err(MascotError::Io { .. })));

        Ok(())
    }

    #[test]
    fn download_file_reports_parent_directory_errors() -> Result<()> {
        let target_directory = std::env::temp_dir().join(format!(
            "mascot-rs-gems-a10-parent-error-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&target_directory);
        std::fs::create_dir_all(&target_directory).map_err(|source| MascotError::Io {
            path: target_directory.display().to_string(),
            source,
        })?;
        let parent = target_directory.join("parent-file");
        std::fs::write(&parent, b"not a directory").map_err(|source| MascotError::Io {
            path: parent.display().to_string(),
            source,
        })?;
        let path = parent.join("file.mgf.zst");
        let builder = GemsA10Builder::<f64>::default();
        let client = builder.client()?;

        let result = block_on_dataset(builder.download_file(&client, "missing.mgf.zst", &path));
        std::fs::remove_dir_all(&target_directory).map_err(|source| MascotError::Io {
            path: target_directory.display().to_string(),
            source,
        })?;

        assert!(matches!(result, Err(MascotError::Io { .. })));

        Ok(())
    }
}
