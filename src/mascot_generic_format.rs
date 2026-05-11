#[cfg(feature = "std")]
use alloc::string::String;
use alloc::{boxed::Box, string::ToString, vec::Vec};
use core::{marker::PhantomData, ops::Index, str::FromStr};
#[cfg(feature = "std")]
use std::{
    fs::File,
    io::{BufRead, BufReader, BufWriter, Write},
    path::Path,
};

use mass_spectrometry::prelude::{
    GenericSpectrum, Spectra, Spectrum, SpectrumAlloc, SpectrumFloat, SpectrumMut, SpectrumSplash,
};
use molecular_formulas::prelude::ChemicalFormula;

#[cfg(feature = "std")]
use crate::annotated_ms2::AnnotatedMs2Builder;
use crate::error::{MascotError, Result};
#[cfg(feature = "std")]
use crate::gems_a10::GemsA10Builder;
#[cfg(feature = "std")]
use crate::gnps::GNPSBuilder;
use crate::mascot_generic_format_builder::MascotGenericFormatBuilder;
use crate::mascot_generic_format_metadata::{Instrument, IonMode, MascotGenericFormatMetadata};
#[cfg(feature = "std")]
use crate::mass_spec_gym::MassSpecGymBuilder;

/// A single Mascot Generic Format ion block with metadata and spectra.
///
/// When used as a [`Spectrum`], the record exposes the peaks from this MGF ion
/// block.
#[derive(Debug)]
#[cfg_attr(feature = "mem_size", derive(mem_dbg::MemSize))]
#[cfg_attr(feature = "mem_dbg", derive(mem_dbg::MemDbg))]
pub struct MascotGenericFormat<P: SpectrumFloat = f64> {
    metadata: MascotGenericFormatMetadata,
    spectrum: GenericSpectrum<P>,
}

impl<P: SpectrumFloat> MascotGenericFormat<P> {
    /// Creates a new [`MascotGenericFormat`].
    ///
    /// # Errors
    /// Returns an error if no peak data is provided, if the peak values are
    /// invalid, or if first-level data is incompatible with the precursor m/z.
    pub fn new(
        metadata: MascotGenericFormatMetadata,
        precursor_mz: P,
        mass_divided_by_charge_ratios: Vec<P>,
        fragment_intensities: Vec<P>,
    ) -> Result<Self> {
        if mass_divided_by_charge_ratios.len() != fragment_intensities.len() {
            return Err(MascotError::PeakVectorLengthMismatch {
                mz_len: mass_divided_by_charge_ratios.len(),
                intensity_len: fragment_intensities.len(),
            });
        }

        if mass_divided_by_charge_ratios.is_empty() {
            return Err(MascotError::EmptyPeakVectors);
        }

        let mut peaks = Vec::with_capacity(mass_divided_by_charge_ratios.len());
        let mut peaks_are_strictly_increasing = true;
        for (mz, intensity) in mass_divided_by_charge_ratios
            .into_iter()
            .zip(fragment_intensities)
        {
            Self::push_peak_tracking_order(
                &mut peaks,
                &mut peaks_are_strictly_increasing,
                mz,
                intensity,
            );
        }

        Self::from_parsed_peaks(metadata, precursor_mz, peaks, peaks_are_strictly_increasing)
    }

    /// Adds a peak to a temporary buffer and tracks whether m/z remains sorted.
    pub(crate) fn push_peak_tracking_order(
        peaks: &mut Vec<(P, P)>,
        peaks_are_strictly_increasing: &mut bool,
        mz: P,
        intensity: P,
    ) {
        if *peaks_are_strictly_increasing {
            if let Some((last_mz, _)) = peaks.last() {
                *peaks_are_strictly_increasing = mz.to_f64() > last_mz.to_f64();
            }
        }
        peaks.push((mz, intensity));
    }

    /// Builds a record from parsed peaks and lets [`GenericSpectrum`] validate them.
    pub(crate) fn from_parsed_peaks(
        metadata: MascotGenericFormatMetadata,
        precursor_mz: P,
        mut peaks: Vec<(P, P)>,
        peaks_are_strictly_increasing: bool,
    ) -> Result<Self> {
        if peaks.is_empty() {
            return Err(MascotError::EmptyPeakVectors);
        }

        let mut spectrum =
            GenericSpectrum::<P>::try_with_capacity(precursor_mz.to_f64(), peaks.len())?;
        if peaks_are_strictly_increasing {
            for (mz, intensity) in peaks {
                spectrum.add_peak(mz, intensity)?;
            }
            return Self::from_spectrum(metadata, spectrum);
        }

        peaks.sort_by(|(left_mz, _), (right_mz, _)| left_mz.to_f64().total_cmp(&right_mz.to_f64()));
        let (mut current_mz, mut current_intensity) = peaks[0];

        for (mz, intensity) in peaks.into_iter().skip(1) {
            if current_mz.to_f64().to_bits() == mz.to_f64().to_bits() {
                current_intensity =
                    P::from_f64_lossy(current_intensity.to_f64() + intensity.to_f64());
            } else {
                spectrum.add_peak(current_mz, current_intensity)?;
                current_mz = mz;
                current_intensity = intensity;
            }
        }
        spectrum.add_peak(current_mz, current_intensity)?;

        Self::from_spectrum(metadata, spectrum)
    }

    fn from_spectrum(
        metadata: MascotGenericFormatMetadata,
        spectrum: GenericSpectrum<P>,
    ) -> Result<Self> {
        Self::validate_spectrum_metadata_shape(&metadata, &spectrum)?;

        let record = Self { metadata, spectrum };
        record.validate_splash_metadata()?;
        Ok(record)
    }

    fn from_edited_spectrum(
        metadata: MascotGenericFormatMetadata,
        spectrum: GenericSpectrum<P>,
    ) -> Result<Self> {
        Self::validate_spectrum_metadata_shape(&metadata, &spectrum)?;

        let mut record = Self { metadata, spectrum };
        record.refresh_derived_metadata_after_peak_edit()?;
        record.validate_splash_metadata()?;
        Ok(record)
    }

    fn validate_spectrum_metadata_shape(
        metadata: &MascotGenericFormatMetadata,
        spectrum: &GenericSpectrum<P>,
    ) -> Result<()> {
        if spectrum.len() == 0 {
            return Err(MascotError::EmptyPeakVectors);
        }

        let stored_precursor_mz = spectrum.precursor_mz().to_f64();
        let first_level_min_mz = spectrum.mz_nth(0).to_f64();
        if metadata.level() == 1 && stored_precursor_mz.to_bits() != first_level_min_mz.to_bits() {
            return Err(MascotError::FirstLevelPrecursorMzMismatch {
                precursor_mz: stored_precursor_mz,
                first_level_min_mz,
            });
        }

        Ok(())
    }

    fn validate_splash_metadata(&self) -> Result<()> {
        if let Some(observed) = self.metadata.splash() {
            let expected = SpectrumSplash::splash(self)?;
            if observed != expected {
                return Err(MascotError::SplashMismatch {
                    observed: observed.to_string(),
                    expected,
                });
            }
        }

        Ok(())
    }

    fn refresh_derived_metadata_after_peak_edit(&mut self) -> Result<()> {
        if self.metadata.splash().is_some() {
            let splash = SpectrumSplash::splash(self)?;
            self.metadata = self.metadata.clone().with_splash(Some(splash));
        }

        Ok(())
    }

    /// Returns the feature ID of the metadata, if present.
    #[must_use]
    pub fn feature_id(&self) -> Option<&str> {
        self.metadata.feature_id()
    }

    /// Returns the scan metadata, if present.
    #[must_use]
    pub fn scans(&self) -> Option<&str> {
        self.metadata.scans()
    }

    /// Returns the MS fragmentation level.
    pub const fn level(&self) -> u8 {
        self.metadata.level()
    }

    /// Returns the precursor charge of the metadata, if known.
    pub const fn charge(&self) -> Option<i8> {
        self.metadata.charge()
    }

    /// Returns the ionization polarity of the metadata, if present.
    pub const fn ion_mode(&self) -> Option<IonMode> {
        self.metadata.ion_mode()
    }

    /// Returns the normalized instrument metadata parsed from `SOURCE_INSTRUMENT`, if present.
    pub const fn source_instrument(&self) -> Option<Instrument> {
        self.metadata.source_instrument()
    }

    /// Returns the parsed chemical formula metadata, if present.
    pub const fn formula(&self) -> Option<&ChemicalFormula<u32, i32>> {
        self.metadata.formula()
    }

    /// Returns the metadata for this MGF record.
    #[must_use]
    pub const fn metadata(&self) -> &MascotGenericFormatMetadata {
        &self.metadata
    }

    /// Returns mutable metadata for this MGF record.
    pub const fn metadata_mut(&mut self) -> &mut MascotGenericFormatMetadata {
        &mut self.metadata
    }

    #[cfg(feature = "std")]
    fn map_output_io(result: std::io::Result<()>) -> Result<()> {
        result.map_err(|source| MascotError::OutputIo { source })
    }
}

#[cfg(feature = "std")]
impl<P: SpectrumFloat> MascotGenericFormat<P> {
    /// Writes this record in canonical MGF syntax to a writer.
    ///
    /// # Errors
    /// Returns an error if the writer cannot be written.
    pub fn write_to<W>(&self, mut writer: W) -> Result<()>
    where
        W: Write,
    {
        self.write_record_to(&mut writer)
    }

    /// Writes this record to a path.
    ///
    /// Files ending in `.zst`, `.zstd`, `.gz`, or `.gzip` are compressed while
    /// they are written.
    ///
    /// # Errors
    /// Returns an error if the file cannot be created, compressed, or written.
    pub fn to_path<PathLike>(&self, path: PathLike) -> Result<()>
    where
        PathLike: AsRef<Path>,
    {
        MGFVec::<P>::write_to_path(path, |writer| self.write_record_to(writer))
    }

    fn write_record_to<W>(&self, writer: &mut W) -> Result<()>
    where
        W: Write + ?Sized,
    {
        Self::map_output_io(writeln!(writer, "BEGIN IONS"))?;
        if let Some(feature_id) = self.metadata.feature_id() {
            Self::map_output_io(writeln!(writer, "FEATURE_ID={feature_id}"))?;
        }
        Self::map_output_io(writeln!(
            writer,
            "PEPMASS={}",
            self.spectrum.precursor_mz().to_f64()
        ))?;
        if let Some(charge) = self.metadata.charge() {
            Self::map_output_io(writeln!(writer, "CHARGE={charge}"))?;
        }
        if let Some(retention_time) = self.metadata.retention_time() {
            Self::map_output_io(writeln!(writer, "RTINSECONDS={retention_time}"))?;
        }
        Self::map_output_io(writeln!(writer, "MSLEVEL={}", self.metadata.level()))?;
        if let Some(filename) = self.metadata.filename() {
            Self::map_output_io(writeln!(writer, "FILENAME={filename}"))?;
        }
        if let Some(smiles) = self.metadata.smiles() {
            Self::map_output_io(writeln!(writer, "SMILES={smiles}"))?;
        }
        if let Some(formula) = self.metadata.formula_original() {
            Self::map_output_io(writeln!(writer, "FORMULA={formula}"))?;
        }
        if let Some(splash) = self.metadata.splash() {
            Self::map_output_io(writeln!(writer, "SPLASH={splash}"))?;
        }
        if let Some(ion_mode) = self.metadata.ion_mode() {
            Self::map_output_io(writeln!(writer, "IONMODE={ion_mode}"))?;
        }
        if let Some(source_instrument) = self.metadata.source_instrument() {
            Self::map_output_io(writeln!(writer, "SOURCE_INSTRUMENT={source_instrument}"))?;
        }
        for (key, value) in self.metadata.arbitrary_metadata() {
            Self::map_output_io(writeln!(writer, "{key}={value}"))?;
        }
        for (mass_divided_by_charge_ratio, fragment_intensity) in self.spectrum.peaks() {
            Self::map_output_io(writeln!(
                writer,
                "{} {}",
                mass_divided_by_charge_ratio.to_f64(),
                fragment_intensity.to_f64()
            ))?;
        }
        match self.metadata.scans() {
            Some(scans) => Self::map_output_io(writeln!(writer, "SCANS={scans}"))?,
            None => Self::map_output_io(writeln!(writer, "SCANS=-1"))?,
        }
        Self::map_output_io(writeln!(writer, "END IONS"))?;

        Ok(())
    }
}

impl<P: SpectrumFloat> AsRef<GenericSpectrum<P>> for MascotGenericFormat<P> {
    fn as_ref(&self) -> &GenericSpectrum<P> {
        &self.spectrum
    }
}

impl<P: SpectrumFloat> From<MascotGenericFormat<P>> for GenericSpectrum<P> {
    fn from(value: MascotGenericFormat<P>) -> Self {
        value.spectrum
    }
}

impl<P: SpectrumFloat> FromStr for MascotGenericFormat<P> {
    type Err = MascotError;

    fn from_str(s: &str) -> Result<Self> {
        let records = MGFVec::<P>::from_str(s)?;
        let found = records.mascot_generic_formats.len();
        if found != 1 {
            return Err(MascotError::SingleRecordExpected { found });
        }

        records
            .mascot_generic_formats
            .into_iter()
            .next()
            .ok_or(MascotError::SingleRecordExpected { found })
    }
}

impl<P: SpectrumFloat> Spectrum for MascotGenericFormat<P> {
    type Precision = P;
    type SortedIntensitiesIter<'a>
        = <GenericSpectrum<P> as Spectrum>::SortedIntensitiesIter<'a>
    where
        Self: 'a;
    type SortedMzIter<'a>
        = <GenericSpectrum<P> as Spectrum>::SortedMzIter<'a>
    where
        Self: 'a;
    type SortedPeaksIter<'a>
        = <GenericSpectrum<P> as Spectrum>::SortedPeaksIter<'a>
    where
        Self: 'a;

    fn len(&self) -> usize {
        self.spectrum.len()
    }

    fn intensities(&self) -> Self::SortedIntensitiesIter<'_> {
        self.spectrum.intensities()
    }

    fn intensity_nth(&self, n: usize) -> Self::Precision {
        self.spectrum.intensity_nth(n)
    }

    fn mz(&self) -> Self::SortedMzIter<'_> {
        self.spectrum.mz()
    }

    fn mz_from(&self, index: usize) -> Self::SortedMzIter<'_> {
        self.spectrum.mz_from(index)
    }

    fn mz_nth(&self, n: usize) -> Self::Precision {
        self.spectrum.mz_nth(n)
    }

    fn peaks(&self) -> Self::SortedPeaksIter<'_> {
        self.spectrum.peaks()
    }

    fn peak_nth(&self, n: usize) -> (Self::Precision, Self::Precision) {
        self.spectrum.peak_nth(n)
    }

    fn precursor_mz(&self) -> Self::Precision {
        self.spectrum.precursor_mz()
    }
}

impl<P: SpectrumFloat> SpectrumMut for MascotGenericFormat<P> {
    type MutationError = MascotError;

    fn add_peak(&mut self, mz: P, intensity: P) -> Result<&mut Self> {
        let mut spectrum = self.spectrum.clone();
        spectrum.add_peak(mz, intensity)?;
        let candidate = Self::from_edited_spectrum(self.metadata.clone(), spectrum)?;
        *self = candidate;
        Ok(self)
    }

    fn add_peaks<I>(&mut self, peaks: I) -> Result<&mut Self>
    where
        I: IntoIterator<Item = (P, P)>,
    {
        let mut spectrum = self.spectrum.clone();
        for (mz, intensity) in peaks {
            spectrum.add_peak(mz, intensity)?;
        }

        let candidate = Self::from_edited_spectrum(self.metadata.clone(), spectrum)?;
        *self = candidate;
        Ok(self)
    }
}

impl<P: SpectrumFloat> SpectrumAlloc for MascotGenericFormat<P> {
    fn with_capacity(precursor_mz: f64, capacity: usize) -> Result<Self> {
        Ok(Self {
            metadata: MascotGenericFormatMetadata::new(None, 2, None, None, None)?,
            spectrum: GenericSpectrum::<P>::try_with_capacity(precursor_mz, capacity)?,
        })
    }

    fn top_k_peaks(&self, k: usize) -> Result<Self> {
        if k == 0 {
            return Err(MascotError::EmptyPeakEdit {
                operation: "top_k_peaks",
            });
        }

        Self::from_edited_spectrum(
            self.metadata.clone(),
            <GenericSpectrum<P> as SpectrumAlloc>::top_k_peaks(&self.spectrum, k)?,
        )
    }
}

struct MGFRecordParser<P: SpectrumFloat = f64> {
    builder: MascotGenericFormatBuilder<P>,
}

impl<P: SpectrumFloat> Default for MGFRecordParser<P> {
    fn default() -> Self {
        Self {
            builder: MascotGenericFormatBuilder::<P>::default(),
        }
    }
}

impl<P: SpectrumFloat> MGFRecordParser<P> {
    fn reset(&mut self) {
        self.builder = MascotGenericFormatBuilder::<P>::default();
    }

    const fn section_open(&self) -> bool {
        self.builder.section_open()
    }
}

impl<P: SpectrumFloat> MGFRecordParser<P> {
    fn digest_line(&mut self, line: &str) -> Result<Option<MascotGenericFormat<P>>> {
        self.builder.digest_line(line)?;

        if line == "END IONS" {
            let builder = core::mem::take(&mut self.builder);
            return builder.build().map(Some);
        }

        Ok(None)
    }
}

/// A source of MGF document lines for [`MGFIter`].
pub trait MGFLineSource {
    #[doc(hidden)]
    /// Borrowed line type yielded by this source.
    type Line<'line>: AsRef<str>
    where
        Self: 'line;

    #[doc(hidden)]
    /// Returns the next input line.
    fn next_line(&mut self) -> Option<Result<Self::Line<'_>>>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MGFIterMode {
    Strict,
    SkipInvalidRecords,
}

/// Borrowed string line source for [`MGFIter`].
#[derive(Debug, Clone)]
pub struct MGFStrLines<'a, T = core::str::Lines<'a>> {
    lines: T,
    lifetime: PhantomData<&'a str>,
}

impl<'a> MGFStrLines<'a> {
    /// Creates a borrowed line source from an MGF document string.
    #[must_use]
    pub fn new(document: &'a str) -> Self {
        Self::from_iterator(document.lines())
    }
}

impl<T> MGFStrLines<'_, T> {
    const fn from_iterator(lines: T) -> Self {
        Self {
            lines,
            lifetime: PhantomData,
        }
    }
}

impl<'a, T> MGFLineSource for MGFStrLines<'a, T>
where
    T: Iterator<Item = &'a str>,
{
    type Line<'line>
        = &'a str
    where
        Self: 'line;

    fn next_line(&mut self) -> Option<Result<Self::Line<'_>>> {
        self.lines.next().map(Ok)
    }
}

#[cfg(feature = "std")]
/// Buffered reader line source for [`MGFIter`].
#[derive(Debug)]
pub struct MGFReader<R> {
    reader: R,
    line: String,
}

#[cfg(feature = "std")]
impl<R> MGFReader<R> {
    /// Creates a line source from a buffered reader.
    #[must_use]
    pub const fn new(reader: R) -> Self {
        Self {
            reader,
            line: String::new(),
        }
    }
}

#[cfg(feature = "std")]
impl<R> MGFLineSource for MGFReader<R>
where
    R: BufRead,
{
    type Line<'line>
        = &'line str
    where
        Self: 'line;

    fn next_line(&mut self) -> Option<Result<Self::Line<'_>>> {
        self.line.clear();
        match self.reader.read_line(&mut self.line) {
            Ok(0) => None,
            Ok(_) => Some(Ok(self.line.trim_end_matches(['\r', '\n']))),
            Err(source) => Some(Err(MascotError::InputIo { source })),
        }
    }
}

/// Streaming iterator over MGF ion blocks.
///
/// Each successful item is one fully parsed [`MascotGenericFormat`]. The
/// iterator reads input incrementally and stops after the first I/O, parse, or
/// build error because the parser state is no longer reliable after that point.
pub struct MGFIter<P: SpectrumFloat = f64, S = MGFStrLines<'static>> {
    source: S,
    parser: MGFRecordParser<P>,
    line_number: usize,
    open_section_line_number: Option<usize>,
    skipped_records: usize,
    mode: MGFIterMode,
    discarding_invalid_record: bool,
    finished: bool,
}

#[cfg(feature = "std")]
/// MGF iterator type returned for path-based readers.
pub type MGFPathIter<P = f64> = MGFIter<P, MGFReader<Box<dyn BufRead>>>;

impl<P, S> MGFIter<P, S>
where
    P: SpectrumFloat,
    S: MGFLineSource,
{
    /// Creates a streaming MGF iterator from a line source.
    #[must_use]
    pub fn from_line_source(source: S) -> Self {
        Self {
            source,
            parser: MGFRecordParser::<P>::default(),
            line_number: 0,
            open_section_line_number: None,
            skipped_records: 0,
            mode: MGFIterMode::Strict,
            discarding_invalid_record: false,
            finished: false,
        }
    }

    /// Skips invalid records instead of stopping at the first parse error.
    ///
    /// I/O errors are still returned. Use [`Self::skipped_records`] after
    /// exhausting the iterator to inspect how many malformed ion blocks were
    /// skipped.
    #[must_use]
    pub const fn skipping_invalid_records(mut self) -> Self {
        self.mode = MGFIterMode::SkipInvalidRecords;
        self
    }

    /// Returns the number of malformed records skipped by this iterator.
    #[must_use]
    pub const fn skipped_records(&self) -> usize {
        self.skipped_records
    }
}

impl<'a, P> MGFIter<P, MGFStrLines<'a>>
where
    P: SpectrumFloat,
{
    /// Creates a streaming MGF iterator over a borrowed document string.
    #[must_use]
    pub fn from_document(document: &'a str) -> Self {
        Self::from_line_source(MGFStrLines::new(document))
    }
}

#[cfg(feature = "std")]
impl<P, R> MGFIter<P, MGFReader<R>>
where
    P: SpectrumFloat,
    R: BufRead,
{
    /// Creates a streaming MGF iterator from a buffered reader.
    ///
    /// Use [`Self::from_path`] when loading from a file path with automatic
    /// decompression.
    #[must_use]
    pub fn from_reader(reader: R) -> Self {
        Self::from_line_source(MGFReader::new(reader))
    }
}

#[cfg(feature = "std")]
impl<P> MGFIter<P, MGFReader<Box<dyn BufRead>>>
where
    P: SpectrumFloat,
{
    /// Creates a streaming MGF iterator from a path.
    ///
    /// Files ending in `.zst`, `.zstd`, `.gz`, or `.gzip` are decompressed
    /// while they are read.
    ///
    /// # Errors
    /// Returns an error if the path cannot be opened or the decompressor cannot
    /// be initialized.
    pub fn from_path<PathLike>(path: PathLike) -> Result<Self>
    where
        PathLike: AsRef<Path>,
    {
        Ok(Self::from_reader(MGFVec::<P>::reader_from_path(
            path.as_ref(),
        )?))
    }
}

impl<P, S> Iterator for MGFIter<P, S>
where
    P: SpectrumFloat,
    S: MGFLineSource,
{
    type Item = Result<MascotGenericFormat<P>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }

        loop {
            let line = match self.source.next_line() {
                Some(Ok(line)) => line,
                Some(Err(source)) => {
                    self.finished = true;
                    return Some(Err(source));
                }
                None => {
                    if self.parser.section_open() {
                        if self.mode == MGFIterMode::SkipInvalidRecords {
                            self.skipped_records += 1;
                            self.parser.reset();
                            self.open_section_line_number = None;
                            self.finished = true;
                            return None;
                        }

                        self.finished = true;
                        return Some(Err(MascotError::UnclosedIonSection {
                            begin_line_number: self
                                .open_section_line_number
                                .unwrap_or(self.line_number),
                        }));
                    }
                    self.finished = true;
                    return None;
                }
            };

            self.line_number += 1;
            let original_line = line.as_ref();
            let line = original_line.trim();
            if line.is_empty() {
                continue;
            }

            if self.mode == MGFIterMode::SkipInvalidRecords {
                if self.discarding_invalid_record {
                    if matches!(line, "BEGIN IONS" | "END IONS") {
                        self.discarding_invalid_record = false;
                    } else {
                        continue;
                    }
                }

                if line != "BEGIN IONS" && !self.parser.section_open() {
                    continue;
                }

                if line == "BEGIN IONS" && self.parser.section_open() {
                    self.skipped_records += 1;
                    self.parser.reset();
                    self.open_section_line_number = None;
                }
            }

            let section_was_open = self.parser.section_open();
            match self.parser.digest_line(line) {
                Ok(Some(record)) => {
                    self.open_section_line_number = None;
                    return Some(Ok(record));
                }
                Ok(None) => {
                    if line == "BEGIN IONS" && !section_was_open {
                        self.open_section_line_number = Some(self.line_number);
                    }
                }
                Err(source) => {
                    if self.mode == MGFIterMode::SkipInvalidRecords {
                        self.skipped_records += 1;
                        self.parser.reset();
                        self.open_section_line_number = None;
                        self.discarding_invalid_record = true;
                        if line == "END IONS" {
                            self.discarding_invalid_record = false;
                        }
                        continue;
                    }
                    self.finished = true;
                    return Some(Err(MascotError::InputLine {
                        line_number: self.line_number,
                        line: original_line.to_string(),
                        source: Box::new(source),
                    }));
                }
            }
        }
    }
}

impl<P, S> core::iter::FusedIterator for MGFIter<P, S>
where
    P: SpectrumFloat,
    S: MGFLineSource,
{
}

#[cfg(feature = "std")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MGFPathCompression {
    Zstd,
    Gzip,
    Plain,
}

#[cfg(feature = "std")]
impl MGFPathCompression {
    fn from_path(path: &Path) -> Self {
        if Self::has_extension(path, &["zst", "zstd"]) {
            Self::Zstd
        } else if Self::has_extension(path, &["gz", "gzip"]) {
            Self::Gzip
        } else {
            Self::Plain
        }
    }

    fn has_extension(path: &Path, expected_extensions: &[&str]) -> bool {
        path.extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                expected_extensions
                    .iter()
                    .any(|expected_extension| extension.eq_ignore_ascii_case(expected_extension))
            })
    }
}

#[repr(transparent)]
/// A collection of parsed [`MascotGenericFormat`] records.
#[derive(Debug)]
#[cfg_attr(feature = "mem_size", derive(mem_dbg::MemSize))]
#[cfg_attr(feature = "mem_dbg", derive(mem_dbg::MemDbg))]
pub struct MGFVec<P: SpectrumFloat = f64> {
    mascot_generic_formats: Vec<MascotGenericFormat<P>>,
}

impl<P: SpectrumFloat> MGFVec<P> {
    /// Create a new vector of MGF objects from the file at the provided path.
    ///
    /// # Arguments
    /// * `path` - The path to the file to read.
    ///
    /// # Returns
    /// A new vector of MGF objects.
    ///
    /// # Errors
    /// * If the file at the provided path cannot be read.
    /// * If the file at the provided path cannot be parsed.
    ///
    /// Files ending in `.zst`, `.zstd`, `.gz`, or `.gzip` are decompressed
    /// while they are read.
    ///
    /// # Examples
    ///
    /// An example of a document that contains one fragmentation spectrum per
    /// ion block:
    ///
    /// ```
    /// use mascot_rs::prelude::*;
    ///
    /// let path = "tests/data/20220513_PMA_DBGI_01_04_003.mgf";
    ///
    /// let mascot_generic_formats: MGFVec = MGFVec::from_path(path).unwrap();
    ///
    /// assert_eq!(mascot_generic_formats.len(), 74, concat!(
    ///     "The number of MascotGenericFormat objects in the vector should be 74, ",
    ///     "but it is {}."
    /// ), mascot_generic_formats.len());
    /// ```
    ///
    /// An example of another type of documents that contains both the first and
    /// second level of fragmentation spectra:
    ///
    /// ```
    /// use mascot_rs::prelude::*;
    ///
    /// let path = "tests/data/20220513_PMA_DBGI_01_04_001.mzML_chromatograms_deconvoluted_deisotoped_filtered_enpkg_sirius.mgf";
    ///
    /// let mascot_generic_formats: MGFVec = MGFVec::from_path(path).unwrap();
    ///
    /// assert_eq!(mascot_generic_formats.len(), 278);
    ///
    /// ```
    ///
    ///
    #[cfg(feature = "std")]
    pub fn from_path<PathLike>(path: PathLike) -> Result<Self>
    where
        PathLike: AsRef<Path>,
    {
        let path = path.as_ref();
        Self::from_reader(Self::reader_from_path(path)?)
    }

    /// Creates a streaming MGF iterator from the file at the provided path.
    ///
    /// Files ending in `.zst`, `.zstd`, `.gz`, or `.gzip` are decompressed
    /// while they are read.
    ///
    /// # Errors
    /// Returns an error if the path cannot be opened or the decompressor cannot
    /// be initialized.
    #[cfg(feature = "std")]
    pub fn iter_from_path<PathLike>(path: PathLike) -> Result<MGFPathIter<P>>
    where
        PathLike: AsRef<Path>,
    {
        MGFIter::from_path(path)
    }

    #[cfg(feature = "std")]
    pub(crate) fn reader_from_path(path: &Path) -> Result<Box<dyn BufRead>> {
        let file = File::open(path).map_err(|source| MascotError::Io {
            path: path.display().to_string(),
            source,
        })?;

        match MGFPathCompression::from_path(path) {
            MGFPathCompression::Zstd => {
                let decoder =
                    zstd::stream::read::Decoder::new(file).map_err(|source| MascotError::Io {
                        path: path.display().to_string(),
                        source,
                    })?;
                Ok(Box::new(BufReader::new(decoder)))
            }
            MGFPathCompression::Gzip => Ok(Box::new(BufReader::new(
                flate2::read::MultiGzDecoder::new(file),
            ))),
            MGFPathCompression::Plain => Ok(Box::new(BufReader::new(file))),
        }
    }

    #[cfg(feature = "std")]
    fn write_to_path<PathLike, F>(path: PathLike, write: F) -> Result<()>
    where
        PathLike: AsRef<Path>,
        F: FnOnce(&mut dyn Write) -> Result<()>,
    {
        let path = path.as_ref();
        let file = File::create(path).map_err(|source| MascotError::Io {
            path: path.display().to_string(),
            source,
        })?;

        match MGFPathCompression::from_path(path) {
            MGFPathCompression::Zstd => {
                let writer = BufWriter::new(file);
                let mut encoder =
                    zstd::stream::write::Encoder::new(writer, 0).map_err(|source| {
                        MascotError::Io {
                            path: path.display().to_string(),
                            source,
                        }
                    })?;
                write(&mut encoder)?;
                encoder.finish().map_err(|source| MascotError::Io {
                    path: path.display().to_string(),
                    source,
                })?;
                Ok(())
            }
            MGFPathCompression::Gzip => {
                let writer = BufWriter::new(file);
                let mut encoder =
                    flate2::write::GzEncoder::new(writer, flate2::Compression::default());
                write(&mut encoder)?;
                encoder.finish().map_err(|source| MascotError::Io {
                    path: path.display().to_string(),
                    source,
                })?;
                Ok(())
            }
            MGFPathCompression::Plain => {
                let mut writer = BufWriter::new(file);
                write(&mut writer)?;
                writer.flush().map_err(|source| MascotError::Io {
                    path: path.display().to_string(),
                    source,
                })?;
                Ok(())
            }
        }
    }

    fn collect_mgf_iter<S>(iterator: MGFIter<P, S>) -> Result<Self>
    where
        S: MGFLineSource,
    {
        let mascot_generic_formats = iterator.collect::<Result<Vec<MascotGenericFormat<P>>>>()?;

        Ok(Self {
            mascot_generic_formats,
        })
    }

    /// Creates a new vector of MGF objects from a buffered line reader.
    ///
    /// # Errors
    /// Returns an error if the reader cannot be read, if any input line cannot
    /// be parsed, or if any MGF section cannot be built.
    #[cfg(feature = "std")]
    pub fn from_reader<R>(reader: R) -> Result<Self>
    where
        R: BufRead,
    {
        Self::collect_mgf_iter(MGFIter::<P, MGFReader<R>>::from_reader(reader))
    }

    /// Creates a streaming MGF iterator over a borrowed document string.
    #[must_use]
    pub fn iter_from_str(document: &str) -> MGFIter<P, MGFStrLines<'_>> {
        MGFIter::from_document(document)
    }

    /// Creates a streaming MGF iterator from a buffered line reader.
    ///
    /// # Examples
    /// ```
    /// use mascot_rs::prelude::*;
    ///
    /// let document = "BEGIN IONS\n\
    /// PEPMASS=500.0\n\
    /// CHARGE=1\n\
    /// MSLEVEL=2\n\
    /// 100.0 2.0\n\
    /// SCANS=-1\n\
    /// END IONS\n";
    /// let mut records = MGFVec::<f64>::iter_from_reader(std::io::Cursor::new(document));
    ///
    /// let record = records
    ///     .next()
    ///     .transpose()?
    ///     .ok_or_else(|| std::io::Error::other("missing MGF record"))?;
    ///
    /// assert_eq!(record.precursor_mz().to_bits(), 500.0_f64.to_bits());
    /// assert!(records.next().is_none());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[cfg(feature = "std")]
    #[must_use]
    pub fn iter_from_reader<R>(reader: R) -> MGFIter<P, MGFReader<R>>
    where
        R: BufRead,
    {
        MGFIter::from_reader(reader)
    }

    /// Creates a new vector of MGF objects from an iterator over document lines.
    ///
    /// # Errors
    /// Returns an error if any input line cannot be parsed, if any MGF section
    /// cannot be built.
    pub fn try_from_iter<'a, T>(iter: T) -> Result<Self>
    where
        T: IntoIterator<Item = &'a str>,
    {
        Self::collect_mgf_iter(
            MGFIter::<P, MGFStrLines<'a, T::IntoIter>>::from_line_source(
                MGFStrLines::from_iterator(iter.into_iter()),
            ),
        )
    }

    /// Returns an iterator over the MGF records in the collection.
    pub fn iter(&self) -> core::slice::Iter<'_, MascotGenericFormat<P>> {
        self.mascot_generic_formats.iter()
    }

    /// Returns a mutable iterator over the MGF records in the collection.
    pub fn iter_mut(&mut self) -> core::slice::IterMut<'_, MascotGenericFormat<P>> {
        self.mascot_generic_formats.iter_mut()
    }

    /// Adds one MGF record to the end of the collection.
    pub fn push(&mut self, mascot_generic_format: MascotGenericFormat<P>) {
        self.mascot_generic_formats.push(mascot_generic_format);
    }

    /// Moves all records from another collection to the end of this one.
    ///
    /// The other collection is left empty.
    pub fn append(&mut self, other: &mut Self) {
        self.mascot_generic_formats
            .append(&mut other.mascot_generic_formats);
    }

    /// Returns the number of MGF records in the collection.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.mascot_generic_formats.len()
    }

    /// Returns `true` if the collection contains no MGF records.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.mascot_generic_formats.is_empty()
    }
}

#[cfg(feature = "std")]
impl<P: SpectrumFloat> MGFVec<P> {
    /// Writes all records in canonical MGF syntax to a writer.
    ///
    /// Records are separated by one blank line.
    ///
    /// # Errors
    /// Returns an error if the writer cannot be written.
    pub fn write_to<W>(&self, mut writer: W) -> Result<()>
    where
        W: Write,
    {
        for (index, mascot_generic_format) in self.mascot_generic_formats.iter().enumerate() {
            if index > 0 {
                MascotGenericFormat::<P>::map_output_io(writeln!(writer))?;
            }
            mascot_generic_format.write_record_to(&mut writer)?;
        }

        Ok(())
    }

    /// Writes all records to a path.
    ///
    /// Files ending in `.zst`, `.zstd`, `.gz`, or `.gzip` are compressed while
    /// they are written.
    ///
    /// # Errors
    /// Returns an error if the file cannot be created, compressed, or written.
    pub fn to_path<PathLike>(&self, path: PathLike) -> Result<()>
    where
        PathLike: AsRef<Path>,
    {
        Self::write_to_path(path, |writer| self.write_to(writer))
    }
}

#[cfg(feature = "std")]
impl<P: SpectrumFloat> MGFVec<P> {
    /// Returns a builder for the annotated harmonized MS2 MGF dataset on Zenodo.
    #[must_use]
    pub fn annotated_ms2() -> AnnotatedMs2Builder<P> {
        AnnotatedMs2Builder::default()
    }

    /// Returns a builder for the top-128 annotated harmonized MS2 MGF dataset on Zenodo.
    #[must_use]
    pub fn annotated_ms2_top_128_peaks() -> AnnotatedMs2Builder<P> {
        AnnotatedMs2Builder::default().top_128_peaks()
    }

    /// Returns a builder for the top-60 annotated harmonized MS2 MGF dataset on Zenodo.
    #[must_use]
    pub fn annotated_ms2_top_60_peaks() -> AnnotatedMs2Builder<P> {
        AnnotatedMs2Builder::default().top_60_peaks()
    }

    /// Returns a builder for the converted `GeMS-A10` MGF dataset on Zenodo.
    #[must_use]
    pub fn gems_a10() -> GemsA10Builder<P> {
        GemsA10Builder::default()
    }

    /// Returns a builder for the top-128 peaks `GeMS-A10` MGF dataset on Zenodo.
    #[must_use]
    pub fn gems_a10_top_128_peaks() -> GemsA10Builder<P> {
        GemsA10Builder::default().top_128_peaks()
    }

    /// Returns a builder for the top-60 peaks `GeMS-A10` MGF dataset on Zenodo.
    #[must_use]
    pub fn gems_a10_top_60_peaks() -> GemsA10Builder<P> {
        GemsA10Builder::default().top_60_peaks()
    }

    /// Returns a builder for the top-40 peaks `GeMS-A10` MGF dataset on Zenodo.
    #[must_use]
    pub fn gems_a10_top_40_peaks() -> GemsA10Builder<P> {
        GemsA10Builder::default().top_40_peaks()
    }

    /// Returns a builder for the top-20 peaks `GeMS-A10` MGF dataset on Zenodo.
    #[must_use]
    pub fn gems_a10_top_20_peaks() -> GemsA10Builder<P> {
        GemsA10Builder::default().top_20_peaks()
    }

    /// Returns a builder for the GNPS public MGF spectral library.
    #[must_use]
    pub fn gnps() -> GNPSBuilder<P> {
        GNPSBuilder::default()
    }

    /// Returns a builder for the `MassSpecGym` benchmark MGF dataset.
    #[must_use]
    pub fn mass_spec_gym() -> MassSpecGymBuilder<P> {
        MassSpecGymBuilder::default()
    }
}

impl<P: SpectrumFloat> Default for MGFVec<P> {
    fn default() -> Self {
        Self {
            mascot_generic_formats: Vec::new(),
        }
    }
}

impl<P: SpectrumFloat> FromIterator<MascotGenericFormat<P>> for MGFVec<P> {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = MascotGenericFormat<P>>,
    {
        Self {
            mascot_generic_formats: iter.into_iter().collect(),
        }
    }
}

impl<P: SpectrumFloat> Extend<MascotGenericFormat<P>> for MGFVec<P> {
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = MascotGenericFormat<P>>,
    {
        self.mascot_generic_formats.extend(iter);
    }
}

impl<P: SpectrumFloat> From<Vec<MascotGenericFormat<P>>> for MGFVec<P> {
    fn from(mascot_generic_formats: Vec<MascotGenericFormat<P>>) -> Self {
        Self {
            mascot_generic_formats,
        }
    }
}

impl<P: SpectrumFloat> IntoIterator for MGFVec<P> {
    type IntoIter = alloc::vec::IntoIter<MascotGenericFormat<P>>;
    type Item = MascotGenericFormat<P>;

    fn into_iter(self) -> Self::IntoIter {
        self.mascot_generic_formats.into_iter()
    }
}

impl<'a, P: SpectrumFloat> IntoIterator for &'a MGFVec<P> {
    type IntoIter = core::slice::Iter<'a, MascotGenericFormat<P>>;
    type Item = &'a MascotGenericFormat<P>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, P: SpectrumFloat> IntoIterator for &'a mut MGFVec<P> {
    type IntoIter = core::slice::IterMut<'a, MascotGenericFormat<P>>;
    type Item = &'a mut MascotGenericFormat<P>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<P: SpectrumFloat> FromStr for MGFVec<P> {
    type Err = MascotError;

    fn from_str(s: &str) -> Result<Self> {
        Self::collect_mgf_iter(Self::iter_from_str(s))
    }
}

impl<P: SpectrumFloat> AsRef<[MascotGenericFormat<P>]> for MGFVec<P> {
    fn as_ref(&self) -> &[MascotGenericFormat<P>] {
        self.mascot_generic_formats.as_slice()
    }
}

impl<P: SpectrumFloat> Spectra for MGFVec<P> {
    type Spectrum = MascotGenericFormat<P>;
    type SpectraIter<'a>
        = core::slice::Iter<'a, MascotGenericFormat<P>>
    where
        Self: 'a;

    fn spectra(&self) -> Self::SpectraIter<'_> {
        self.mascot_generic_formats.iter()
    }

    fn len(&self) -> usize {
        self.mascot_generic_formats.len()
    }
}

impl<P: SpectrumFloat> Index<usize> for MGFVec<P> {
    type Output = MascotGenericFormat<P>;

    fn index(&self, index: usize) -> &Self::Output {
        &self.mascot_generic_formats[index]
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;

    fn sample_record() -> Result<MascotGenericFormat<f64>> {
        concat!(
            "BEGIN IONS\n",
            "FEATURE_ID=unit\n",
            "PEPMASS=500.0\n",
            "CHARGE=1\n",
            "MSLEVEL=2\n",
            "100.0 2.0\n",
            "SCANS=1\n",
            "END IONS\n",
        )
        .parse()
    }

    #[test]
    fn with_capacity_allocates_empty_record() -> Result<()> {
        let record = MascotGenericFormat::<f64>::with_capacity(250.0, 4)?;

        assert_eq!(record.len(), 0);
        assert_eq!(record.level(), 2);
        assert_eq!(record.precursor_mz().to_bits(), 250.0_f64.to_bits());

        Ok(())
    }

    #[test]
    fn iterator_over_empty_document_is_fused() {
        let mut iterator = MGFIter::<f64, _>::from_document("");

        assert!(iterator.next().is_none());
        assert!(iterator.next().is_none());
    }

    #[test]
    fn writes_record_and_collection_to_all_path_compressions(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let target_directory = std::env::temp_dir().join(format!(
            "mascot-rs-mgf-module-write-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&target_directory);
        std::fs::create_dir_all(&target_directory)?;
        let record = sample_record()?;
        let collection = MGFVec::from(vec![sample_record()?]);
        let plain_path = target_directory.join("record.mgf");
        let zstd_path = target_directory.join("records.mgf.zst");
        let gzip_path = target_directory.join("record.mgf.gz");

        record.to_path(&plain_path)?;
        collection.to_path(&zstd_path)?;
        record.to_path(&gzip_path)?;

        let plain: MGFVec = MGFVec::from_path(&plain_path)?;
        let zstd: MGFVec = MGFVec::from_path(&zstd_path)?;
        let gzip: MGFVec = MGFVec::from_path(&gzip_path)?;
        std::fs::remove_dir_all(&target_directory)?;

        assert_eq!(plain.len(), 1);
        assert_eq!(zstd.len(), 1);
        assert_eq!(gzip.len(), 1);

        Ok(())
    }
}
