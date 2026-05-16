//! Generic iteration over selected properties of MGF records.
//!
//! The [`IterMGFProperty<T>`](crate::iter_mgf_property::IterMGFProperty)
//! trait abstracts "give me an iterator over property `T`" across the three
//! MGF container shapes:
//! [`MascotGenericFormat`](crate::mascot_generic_format::MascotGenericFormat),
//! [`MGFVec`](crate::mascot_generic_format::MGFVec), and
//! [`MGFIter`](crate::mascot_generic_format::MGFIter). The first property
//! exposed is [`Smiles`](smiles_parser::prelude::Smiles); the design admits
//! other optional metadata fields without reshaping the API.
//!
//! For owned containers the items are borrowed (`&Smiles`); for the
//! streaming iterator they are owned (`Smiles`), because the iterator
//! produces records lazily and there is no stable storage to borrow from.
//! The streaming impl silently skips records that fail to parse.

use mass_spectrometry::prelude::SpectrumFloat;
use smiles_parser::prelude::Smiles;

use crate::mascot_generic_format::{MGFIter, MGFLineSource, MGFVec, MascotGenericFormat};

/// Iteration over a selected MGF record property.
///
/// `T` is the item type yielded by [`Self::properties`]; the matching
/// [`Self::maybe_properties`] yields `Option<T>` and preserves the original
/// record count.
pub trait IterMGFProperty<T> {
    /// Returns an iterator over present property values. Records lacking
    /// the property are skipped.
    fn properties(self) -> impl Iterator<Item = T>;

    /// Returns an iterator over the property, yielding `None` for records
    /// that do not carry it.
    fn maybe_properties(self) -> impl Iterator<Item = Option<T>>;
}

// MascotGenericFormat: a single record, borrowed items.

impl<'a, P: SpectrumFloat> IterMGFProperty<&'a Smiles> for &'a MascotGenericFormat<P> {
    fn properties(self) -> impl Iterator<Item = &'a Smiles> {
        self.metadata().smiles().into_iter()
    }

    fn maybe_properties(self) -> impl Iterator<Item = Option<&'a Smiles>> {
        core::iter::once(self.metadata().smiles())
    }
}

impl<P: SpectrumFloat> MascotGenericFormat<P> {
    /// Returns an iterator yielding this record's [`Smiles`] when present
    /// (0 or 1 items).
    pub fn smiles(&self) -> impl Iterator<Item = &Smiles> {
        self.metadata().smiles().into_iter()
    }

    /// Returns a single-item iterator yielding this record's `Option<&Smiles>`.
    pub fn maybe_smiles(&self) -> impl Iterator<Item = Option<&Smiles>> {
        core::iter::once(self.metadata().smiles())
    }
}

// MGFVec: an in-memory collection, borrowed items.

impl<'a, P: SpectrumFloat> IterMGFProperty<&'a Smiles> for &'a MGFVec<P> {
    fn properties(self) -> impl Iterator<Item = &'a Smiles> {
        self.iter().filter_map(|record| record.metadata().smiles())
    }

    fn maybe_properties(self) -> impl Iterator<Item = Option<&'a Smiles>> {
        self.iter().map(|record| record.metadata().smiles())
    }
}

impl<P: SpectrumFloat> MGFVec<P> {
    /// Returns an iterator over the [`Smiles`] values held by this
    /// collection. Records without SMILES are skipped.
    pub fn smiles(&self) -> impl Iterator<Item = &Smiles> {
        self.iter().filter_map(|record| record.metadata().smiles())
    }

    /// Returns an iterator yielding one `Option<&Smiles>` per record.
    pub fn maybe_smiles(&self) -> impl Iterator<Item = Option<&Smiles>> {
        self.iter().map(|record| record.metadata().smiles())
    }
}

// MGFIter: streaming, owned items, errors skipped silently.

impl<P: SpectrumFloat, S: MGFLineSource> IterMGFProperty<Smiles> for MGFIter<P, S> {
    fn properties(self) -> impl Iterator<Item = Smiles> {
        self.filter_map(|result| {
            result
                .ok()
                .and_then(|record| record.metadata().smiles().cloned())
        })
    }

    fn maybe_properties(self) -> impl Iterator<Item = Option<Smiles>> {
        self.filter_map(Result::ok)
            .map(|record| record.metadata().smiles().cloned())
    }
}

impl<P: SpectrumFloat, S: MGFLineSource> MGFIter<P, S> {
    /// Consumes the iterator and yields the owned [`Smiles`] of every
    /// successfully parsed record that carries SMILES metadata. Parse
    /// errors are skipped silently.
    pub fn smiles(self) -> impl Iterator<Item = Smiles> {
        self.filter_map(|result| {
            result
                .ok()
                .and_then(|record| record.metadata().smiles().cloned())
        })
    }

    /// Consumes the iterator and yields one `Option<Smiles>` per
    /// successfully parsed record. Parse errors are skipped silently.
    pub fn maybe_smiles(self) -> impl Iterator<Item = Option<Smiles>> {
        self.filter_map(Result::ok)
            .map(|record| record.metadata().smiles().cloned())
    }
}
