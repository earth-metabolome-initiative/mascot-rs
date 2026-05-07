use alloc::collections::VecDeque;
use core::marker::PhantomData;
use std::io::BufRead;
use std::path::{Path, PathBuf};

use mass_spectrometry::prelude::SpectrumFloat;

use crate::dataset::{Dataset, DatasetFuture, SingleFileDatasetConfig, SingleFileDatasetDownload};
use crate::error::Result;
use crate::mascot_generic_format::{MGFIter, MGFLineSource, MGFReader, MGFVec};

/// Current Hugging Face endpoint for the `MassSpecGym` benchmark MGF file.
pub const MASS_SPEC_GYM_MGF_URL: &str = "https://huggingface.co/datasets/roman-bushuiev/MassSpecGym/resolve/main/data/auxiliary/MassSpecGym.mgf?download=true";

/// File name used for the `MassSpecGym` benchmark MGF file.
pub const MASS_SPEC_GYM_MGF_FILE_NAME: &str = "MassSpecGym.mgf";

/// Number of spectra reported by the `MassSpecGym` Hugging Face dataset viewer.
pub const MASS_SPEC_GYM_SPECTRA_COUNT: usize = 231_104;

/// Streaming iterator returned for the `MassSpecGym` MGF dataset.
pub type MassSpecGymIter<P = f64> = MGFIter<P, MassSpecGymLineSource<MGFReader<Box<dyn BufRead>>>>;

/// Builder for downloading and loading the `MassSpecGym` benchmark MGF dataset.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "mem_size", derive(mem_dbg::MemSize))]
#[cfg_attr(feature = "mem_dbg", derive(mem_dbg::MemDbg))]
pub struct MassSpecGymBuilder<P: SpectrumFloat = f64> {
    config: SingleFileDatasetConfig,
    precision: PhantomData<fn() -> P>,
}

impl<P: SpectrumFloat> Default for MassSpecGymBuilder<P> {
    fn default() -> Self {
        Self {
            config: SingleFileDatasetConfig::new(
                MASS_SPEC_GYM_MGF_URL,
                std::env::temp_dir().join("mascot-rs-mass-spec-gym"),
                MASS_SPEC_GYM_MGF_FILE_NAME,
                "Downloading MassSpecGym MGF dataset",
            ),
            precision: PhantomData,
        }
    }
}

impl<P: SpectrumFloat> MassSpecGymBuilder<P> {
    /// Sets the source URL.
    #[must_use]
    pub fn url<S: Into<String>>(mut self, url: S) -> Self {
        self.config.set_url(url);
        self
    }

    /// Sets the directory where the `MassSpecGym` MGF file is stored.
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

    /// Downloads the `MassSpecGym` MGF file if needed without loading records.
    ///
    /// # Errors
    /// Returns an error if the configured file name is empty, if the target
    /// directory cannot be created, if the existing local file cannot be
    /// inspected, or if the remote dataset cannot be downloaded.
    pub async fn download(self) -> Result<MassSpecGymDownload> {
        std::future::ready(()).await;
        self.config
            .download()
            .map(MassSpecGymDownload::from_single_file)
    }

    /// Downloads the `MassSpecGym` MGF file if needed and loads valid records.
    ///
    /// The published MGF uses MassSpecGym-specific header keys. Loading
    /// normalizes those keys into the strict MGF parser while preserving the
    /// original keys as arbitrary metadata.
    ///
    /// # Errors
    /// Returns an error if the download fails, if the target file cannot be
    /// written, or if the downloaded file cannot be read back.
    pub async fn load(self) -> Result<MassSpecGymLoad<P>> {
        let download = self.download().await?;
        let mut iterator = Self::iter_path(download.path())?;
        let spectra = iterator
            .by_ref()
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .collect();
        let skipped_records = iterator.skipped_records();

        Ok(MassSpecGymLoad {
            spectra,
            skipped_records,
            path: download.path,
            bytes: download.bytes,
        })
    }

    /// Downloads the `MassSpecGym` MGF file if needed and returns a streaming iterator.
    ///
    /// The returned iterator normalizes `MassSpecGym`-specific headers while it
    /// streams records, skips malformed records, and reports the skipped count
    /// through [`MGFIter::skipped_records`](crate::mascot_generic_format::MGFIter::skipped_records)
    /// after it has been exhausted.
    ///
    /// # Errors
    /// Returns an error if the download fails, if the target file cannot be
    /// written, or if the downloaded file cannot be opened for streaming.
    pub async fn mgf_iter(self) -> Result<MassSpecGymIter<P>> {
        let download = self.download().await?;
        Self::iter_path(download.path())
    }

    fn iter_path(path: &Path) -> Result<MassSpecGymIter<P>> {
        let source =
            MassSpecGymLineSource::new(MGFReader::new(MGFVec::<P>::reader_from_path(path)?));
        Ok(MGFIter::<P, _>::from_line_source(source).skipping_invalid_records())
    }
}

impl<P> Dataset for MassSpecGymBuilder<P>
where
    P: SpectrumFloat + Send + 'static,
{
    type Download = MassSpecGymDownload;
    type Iter = MassSpecGymIter<P>;
    type Load = MassSpecGymLoad<P>;

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

/// Result of downloading the `MassSpecGym` MGF dataset.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "mem_size", derive(mem_dbg::MemSize))]
#[cfg_attr(feature = "mem_dbg", derive(mem_dbg::MemDbg))]
pub struct MassSpecGymDownload {
    path: PathBuf,
    bytes: u64,
}

impl MassSpecGymDownload {
    fn from_single_file(download: SingleFileDatasetDownload) -> Self {
        let (path, bytes) = download.into_parts();
        Self { path, bytes }
    }

    /// Returns the local path used for the `MassSpecGym` MGF file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the size of the local `MassSpecGym` MGF file in bytes.
    #[must_use]
    pub const fn bytes(&self) -> u64 {
        self.bytes
    }
}

/// Result of loading the `MassSpecGym` MGF dataset.
#[derive(Debug)]
#[cfg_attr(feature = "mem_size", derive(mem_dbg::MemSize))]
#[cfg_attr(feature = "mem_dbg", derive(mem_dbg::MemDbg))]
pub struct MassSpecGymLoad<P: SpectrumFloat = f64> {
    spectra: MGFVec<P>,
    skipped_records: usize,
    path: PathBuf,
    bytes: u64,
}

impl<P: SpectrumFloat> MassSpecGymLoad<P> {
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

    /// Returns the number of `MassSpecGym` ion blocks skipped during tolerant loading.
    #[must_use]
    pub const fn skipped_records(&self) -> usize {
        self.skipped_records
    }

    /// Returns the local path used for the `MassSpecGym` MGF file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the size of the local `MassSpecGym` MGF file in bytes.
    #[must_use]
    pub const fn bytes(&self) -> u64 {
        self.bytes
    }
}

impl<P: SpectrumFloat> AsRef<MGFVec<P>> for MassSpecGymLoad<P> {
    fn as_ref(&self) -> &MGFVec<P> {
        self.spectra()
    }
}

/// Line source that normalizes `MassSpecGym` headers before MGF parsing.
pub struct MassSpecGymLineSource<S> {
    source: S,
    queued: VecDeque<String>,
    saw_level: bool,
}

impl<S> MassSpecGymLineSource<S> {
    const fn new(source: S) -> Self {
        Self {
            source,
            queued: VecDeque::new(),
            saw_level: false,
        }
    }

    fn reset_record_state(&mut self) {
        self.queued.clear();
        self.saw_level = false;
    }

    fn normalize_line(&mut self, line: &str) -> String {
        match line {
            "BEGIN IONS" => {
                self.reset_record_state();
                return line.to_string();
            }
            "END IONS" => {
                self.queue_missing_level_metadata();
                if let Some(queued) = self.queued.pop_front() {
                    self.queued.push_back(line.to_string());
                    return queued;
                }
                return line.to_string();
            }
            _ => {}
        }

        if line.starts_with("MSLEVEL=") {
            self.saw_level = true;
        } else if let Some(identifier) = line.strip_prefix("IDENTIFIER=") {
            let identifier = identifier.trim();
            if !identifier.is_empty() {
                self.queued.push_back(format!("FEATURE_ID={identifier}"));
            }
        } else if let Some(precursor_mz) = line.strip_prefix("PRECURSOR_MZ=") {
            self.queued.push_back(format!("PEPMASS={precursor_mz}"));
        } else if let Some(instrument) = line.strip_prefix("INSTRUMENT_TYPE=") {
            self.queued
                .push_back(format!("SOURCE_INSTRUMENT={instrument}"));
        }

        line.to_string()
    }

    fn queue_missing_level_metadata(&mut self) {
        if !self.saw_level {
            self.queued.push_back("MSLEVEL=2".to_string());
            self.saw_level = true;
        }
    }
}

impl<S> MGFLineSource for MassSpecGymLineSource<S>
where
    S: MGFLineSource,
{
    type Line<'line>
        = String
    where
        Self: 'line;

    fn next_line(&mut self) -> Option<Result<Self::Line<'_>>> {
        if let Some(queued) = self.queued.pop_front() {
            return Some(Ok(queued));
        }

        let line = match self.source.next_line()? {
            Ok(line) => line.as_ref().to_string(),
            Err(source) => return Some(Err(source)),
        };
        Some(Ok(self.normalize_line(&line)))
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use crate::error::MascotError;

    fn next_line<S>(source: &mut MassSpecGymLineSource<S>) -> Result<Option<String>>
    where
        S: MGFLineSource,
    {
        match source.next_line() {
            Some(Ok(line)) => Ok(Some(line)),
            Some(Err(error)) => Err(error),
            None => Ok(None),
        }
    }

    #[test]
    fn preserves_records_that_already_have_standard_level_metadata() -> Result<()> {
        let document = "BEGIN IONS\nMSLEVEL=2\nEND IONS\n";
        let reader = MGFReader::new(Cursor::new(document));
        let mut source = MassSpecGymLineSource::new(reader);

        assert_eq!(next_line(&mut source)?.as_deref(), Some("BEGIN IONS"));
        assert_eq!(next_line(&mut source)?.as_deref(), Some("MSLEVEL=2"));
        assert_eq!(next_line(&mut source)?.as_deref(), Some("END IONS"));
        assert_eq!(next_line(&mut source)?, None);

        Ok(())
    }

    #[test]
    fn queues_only_missing_level_metadata_at_end_of_record() -> Result<()> {
        let document = "BEGIN IONS\nEND IONS\n";
        let reader = MGFReader::new(Cursor::new(document));
        let mut source = MassSpecGymLineSource::new(reader);

        assert_eq!(next_line(&mut source)?.as_deref(), Some("BEGIN IONS"));
        assert_eq!(next_line(&mut source)?.as_deref(), Some("MSLEVEL=2"));
        assert_eq!(next_line(&mut source)?.as_deref(), Some("END IONS"));
        assert_eq!(next_line(&mut source)?, None);

        Ok(())
    }

    #[test]
    fn forwards_line_source_errors() {
        struct FailingLineSource;

        impl MGFLineSource for FailingLineSource {
            type Line<'line>
                = &'line str
            where
                Self: 'line;

            fn next_line(&mut self) -> Option<Result<Self::Line<'_>>> {
                Some(Err(MascotError::InputIo {
                    source: std::io::Error::other("forced line-source failure"),
                }))
            }
        }

        let mut source = MassSpecGymLineSource::new(FailingLineSource);

        assert!(matches!(
            source.next_line(),
            Some(Err(MascotError::InputIo { .. }))
        ));
    }

    #[test]
    fn iter_path_reports_open_errors() {
        let path = std::env::temp_dir().join(format!(
            "mascot-rs-missing-mass-spec-gym-{}.mgf",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);

        assert!(matches!(
            MassSpecGymBuilder::<f64>::iter_path(&path),
            Err(MascotError::Io { .. })
        ));
    }
}
