# SPLASH and Spectrum Editing API Review

This note reviews every current spectrum-editing surface that can change a peak
list, directly or indirectly, and therefore can make structured `SPLASH`
metadata stale.

Resolved upstream trait crate during this review:
`mass_spectrometry` from
`https://github.com/earth-metabolome-initiative/mass-spectrometry-traits`,
commit `937bdee730be861c4ca94285f7c478c719b4835e`.

## Working Invariant

Parsing should stay strict: if an input MGF header contains `SPLASH`, it must
match the parsed peaks.

After an edit, a returned or mutated `MascotGenericFormat` must not contain a
stale structured `SPLASH`. If the record had structured `SPLASH` before the
edit, the edited record should either contain the recomputed `SPLASH` for the
new peak list or return an error while leaving the original record unchanged.
If the record did not have structured `SPLASH`, editing should not invent one.

## mascot-rs Methods That Need Updates

### `MascotGenericFormat::from_spectrum`

Location: `src/mascot_generic_format.rs`.

Current behavior:

- Builds a record from metadata plus a `GenericSpectrum`.
- Always validates existing structured `SPLASH` against the supplied spectrum.
- Is used by parsed-record construction and by transformation code.

Needed update:

- Split the construction paths.
- Keep a strict parse/validation constructor for input records.
- Add a transformation constructor that reconciles derived metadata before
  validation, at least for `SPLASH`.
- Guard the level-1 precursor check against empty peak lists before calling
  `mz_nth(0)`.

Reason:

- Parsing needs strict validation.
- Transformations such as top-k naturally change peaks and therefore invalidate
  the old `SPLASH`.
- `top_k_peaks(0)` can currently produce an empty inner spectrum and then feed
  it into code that assumes at least one peak.

### `MascotGenericFormat::validate_splash_metadata`

Location: `src/mascot_generic_format.rs`.

Current behavior:

- Computes `SpectrumSplash::splash(self)` and compares it with the structured
  metadata value.

Needed update:

- Keep this as the strict validator used by parsing and final consistency
  checks.
- Add a separate helper for edit paths, for example
  `recompute_splash_metadata_if_present`, so edit code does not misuse
  validation as a reason to reject legitimate peak changes.

Reason:

- Validation and reconciliation are different operations.
- The current top-k failure comes from validating old metadata after the peak
  list was deliberately changed.

### `MascotGenericFormatMetadata::with_splash`

Location: `src/mascot_generic_format_metadata.rs`.

Current behavior:

- Private crate-level builder-style setter used by parsing.

Needed update:

- Reuse it, or replace it with a clearer crate-private setter, from the edit
  reconciliation helper.
- The helper needs to set `Some(recomputed_splash)` when a record already had
  structured `SPLASH`.

Reason:

- The metadata type already owns the structured field.
- Recomputing in `MascotGenericFormat` needs one internal place to update it.

### `SpectrumMut for MascotGenericFormat::add_peak`

Location: `src/mascot_generic_format.rs`.

Current behavior:

- If structured `SPLASH` is present, clones the inner spectrum, adds the peak,
  builds a candidate with the old metadata, validates the old `SPLASH`, and
  rejects the edit with `MascotError::SplashMismatch`.
- If no structured `SPLASH` is present, mutates the inner spectrum directly.

Needed update:

- Preserve atomicity: validate the peak and build the candidate first.
- If the original metadata had structured `SPLASH`, recompute it on the
  candidate before committing.
- Commit the candidate only after all validation succeeds.

Reason:

- Adding a valid peak should not fail merely because the source record had a
  valid old `SPLASH`.
- The method currently guarantees the original record remains unchanged on
  failure; that should be kept.

### `SpectrumMut for MascotGenericFormat::add_peaks`

Location: default method in upstream
`mass_spectrometry::traits::SpectrumMut`; inherited by `MascotGenericFormat`.

Current behavior:

- The upstream default loops over `add_peak`.

Needed update:

- Override it in `mascot-rs`.
- Apply all peaks to a temporary candidate.
- Recompute structured `SPLASH` once at the end if it was present.
- Commit only after the whole batch succeeds.

Reason:

- If `add_peak` recomputes `SPLASH`, the inherited default would recompute it
  once per peak.
- A batch edit should be atomic; the current default can leave earlier peaks
  applied if a later peak fails.

### `SpectrumAlloc for MascotGenericFormat::top_k_peaks`

Location: `src/mascot_generic_format.rs`.

Current behavior:

- Builds a top-k `GenericSpectrum`.
- Reuses cloned metadata unchanged.
- Calls `from_spectrum`, which validates the old `SPLASH` against the new peak
  list and returns `MascotError::SplashMismatch`.

Needed update:

- Build the top-k spectrum.
- Recompute structured `SPLASH` if the original record had one.
- Decide and encode the `k == 0` behavior explicitly:
  - either reject zero retained peaks with a normal `MascotError`, or
  - allow empty transformed records and update all code paths that assume at
    least one peak.

Reason:

- This is the currently reproduced red case.
- The old `SPLASH` is no longer true after top-k filtering.

### `MGFVec::iter_mut` and `IntoIterator for &mut MGFVec`

Location: `src/mascot_generic_format.rs`.

Current behavior:

- Expose mutable `MascotGenericFormat` records.

Needed update:

- No direct implementation change if record-level mutation is fixed.
- Add regression coverage that mutation through `iter_mut` or `&mut MGFVec`
  also preserves/recomputes structured `SPLASH`.

Reason:

- These methods are mutation entry points for users even though they do not
  mutate peaks themselves.

### `MascotGenericFormatMetadata::with_arbitrary_metadata`

Location: `src/mascot_generic_format_metadata.rs`.

Current behavior:

- Accepts any key-value pairs and stores them as arbitrary metadata.

Needed update:

- Decide whether arbitrary metadata should reject structured keys such as
  `SPLASH`, `FORMULA`, `SMILES`, `PEPMASS`, `CHARGE`, and `MSLEVEL`.
- At minimum, document that arbitrary metadata must not be used to override
  structured metadata.

Reason:

- Serialization writes structured fields first and arbitrary metadata after
  them. A user can currently create duplicate header lines such as a structured
  `SPLASH` plus an arbitrary `SPLASH`.
- This is not the top-k bug, but it is another way to serialize inconsistent
  structured metadata.

### `MascotGenericFormatMetadata::insert_arbitrary_metadata`

Location: `src/mascot_generic_format_metadata.rs`.

Current behavior:

- Mutates arbitrary metadata in place and accepts any key.

Needed update:

- Same decision as `with_arbitrary_metadata`.
- If structured keys are rejected, this method needs to become fallible, which
  is a breaking API change.

Reason:

- It is the public in-place metadata editing method.
- It can create duplicate structured header keys during writing.

## Upstream Trait Methods That Need Documentation or API Consideration

These live in the `mass_spectrometry` crate, not in this repository.

### `SpectrumMut::add_peak`

Current contract:

- Implementers add one peak and enforce numeric and ordering invariants.

Needed upstream clarification:

- Implementers whose spectrum type carries derived metadata must keep that
  metadata consistent, clear it, or return an error without mutating the
  original value.

Reason:

- `SpectrumMut` is the trait-level editing surface. The current contract says
  what must happen to peaks, but not to metadata derived from peaks.

### `SpectrumMut::add_peaks`

Current contract:

- Default implementation repeatedly calls `add_peak`.

Needed upstream clarification:

- The default is not atomic and may be inefficient for metadata-bearing
  spectra.
- Implementers with derived metadata should override it.

Reason:

- Recomputing `SPLASH` after every peak is wasteful.
- Partial mutation on later failure is undesirable for structured records.

### `SpectrumAlloc::top_k_peaks`

Current contract:

- Default implementation creates a new `Self` with `with_capacity` and
  `add_peaks`.

Needed upstream clarification:

- Implementers with metadata/provenance should override this method if they
  need to preserve or reconcile metadata.

Reason:

- The default implementation only knows the `Spectrum` abstraction.
- It cannot know how to update `SPLASH`, formula annotations, source IDs, or
  other metadata-bearing fields.

### `SpectrumAlloc::random`

Current contract:

- Creates synthetic spectra with no source metadata.

Needed update:

- No `mascot-rs` change is needed for `SPLASH`.
- Upstream docs may explicitly state that generated spectra are synthetic and
  should not preserve source metadata.

Reason:

- It constructs new spectra rather than editing source records.

### `SpectralProcessor::process`

Current contract:

- Takes `&Self::Spectrum` and returns `Self::Spectrum`.

Needed upstream clarification:

- Processors that alter peaks on metadata-bearing spectra must reconcile or
  clear derived metadata on the returned spectrum.
- Current upstream concrete processors use `GenericSpectrum`, so they do not
  need code changes for `mascot-rs` structured metadata.

Reason:

- Processors are transformation APIs.
- A processor that filters, normalizes, cleans, or merges peaks can invalidate
  `SPLASH`.

### `SpectralPipeline::process`

Current contract:

- Applies filters and processors to owned spectra.

Needed upstream clarification:

- This inherits the metadata contract from each `SpectralProcessor`.
- No direct code change is needed if processors obey the updated contract.

Reason:

- The pipeline itself only chains processors.

## Upstream Concrete Processors Reviewed

The currently resolved upstream concrete processors that edit peak lists return
`GenericSpectrum`, not `MascotGenericFormat`, so they do not directly update
`mascot-rs` metadata:

- `SiriusMergeClosePeaks<P>::process`
- `MsEntropyCleanSpectrum<P>::process`

If analogous processors are later generalized to operate on
`MascotGenericFormat`, they need the same reconciliation behavior as
`top_k_peaks`.

## Methods That Do Not Need SPLASH Changes

These APIs move or expose already-built records but do not change their peak
lists:

- `MGFVec::push`
- `MGFVec::append`
- `Extend<MascotGenericFormat<P>> for MGFVec<P>`
- `FromIterator<MascotGenericFormat<P>> for MGFVec<P>`
- `From<Vec<MascotGenericFormat<P>>> for MGFVec<P>`
- `IntoIterator for MGFVec<P>`
- `IntoIterator for &MGFVec<P>`

They should not recompute metadata themselves. They should rely on the invariant
that each `MascotGenericFormat` is already internally consistent.

## Immediate Test Checklist

The implementation pass should add or update tests for:

- parsing still rejects a wrong input `SPLASH`;
- `top_k_peaks` on a record with valid `SPLASH` succeeds and returns a
  consistent `SPLASH`;
- `top_k_peaks` on a record without `SPLASH` does not invent one;
- `top_k_peaks(0)` has explicit non-panicking behavior;
- `add_peak` on a record with valid `SPLASH` succeeds and recomputes `SPLASH`;
- failed `add_peak` leaves the original record unchanged;
- `add_peaks` recomputes `SPLASH` once per batch and is atomic on failure;
- mutation through `MGFVec::iter_mut` or `&mut MGFVec` gets the same behavior as
  direct record mutation;
- arbitrary metadata cannot silently produce duplicate structured `SPLASH`
  headers, or this limitation is explicitly documented if the API remains
  permissive.
