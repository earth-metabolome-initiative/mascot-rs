//! Node colouring schemes for the similarity graph.
//!
//! Nodes can be coloured categorically (connected component, Louvain community,
//! or a chosen metadata field) or as a continuous heatmap of a per-spectrum
//! feature (intensity entropy, peak count, precursor m/z). A future
//! classifier-based scheme will slot in alongside these. Categorical schemes
//! map labels to a qualitative palette with a discrete legend; heatmap schemes
//! map values to a colour gradient with a min/max legend.

use std::collections::{BTreeMap, BTreeSet};

use mascot_rs::prelude::{MascotGenericFormat, Spectrum, SpectrumFloat};

use crate::render::{heat_color, palette_color};
use crate::similarity::SimilarityGraph;

/// How nodes are grouped or scaled for colouring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorScheme {
    /// Louvain community.
    Community,
    /// Leiden community.
    LeidenCommunity,
    /// Spectrum ion mode.
    IonMode,
    /// Spectrum precursor charge.
    Charge,
    /// Spectrum MS level.
    MsLevel,
    /// Spectrum source instrument.
    Instrument,
    /// Heatmap of spectral intensity entropy.
    IntensityEntropy,
    /// Heatmap of the number of peaks.
    PeakCount,
    /// Heatmap of precursor m/z (pepmass).
    PrecursorMz,
}

impl ColorScheme {
    /// All schemes, in display order.
    pub const ALL: [Self; 9] = [
        Self::Community,
        Self::LeidenCommunity,
        Self::IonMode,
        Self::Charge,
        Self::MsLevel,
        Self::Instrument,
        Self::IntensityEntropy,
        Self::PeakCount,
        Self::PrecursorMz,
    ];

    /// A stable identifier used for list keys.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Community => "community",
            Self::LeidenCommunity => "leiden-community",
            Self::IonMode => "ion-mode",
            Self::Charge => "charge",
            Self::MsLevel => "ms-level",
            Self::Instrument => "instrument",
            Self::IntensityEntropy => "intensity-entropy",
            Self::PeakCount => "peak-count",
            Self::PrecursorMz => "precursor-mz",
        }
    }

    /// A human-readable label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Community => "Community (Louvain)",
            Self::LeidenCommunity => "Community (Leiden)",
            Self::IonMode => "Ion mode",
            Self::Charge => "Charge",
            Self::MsLevel => "MS level",
            Self::Instrument => "Instrument",
            Self::IntensityEntropy => "Intensity entropy",
            Self::PeakCount => "Peak count",
            Self::PrecursorMz => "Precursor m/z",
        }
    }

    /// A full explanation, used for button tooltips and screen-reader labels.
    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Community => {
                "Colour by Louvain community: tightly interconnected clusters detected within the graph."
            }
            Self::LeidenCommunity => {
                "Colour by Leiden community: a refinement of Louvain that guarantees every community is internally connected."
            }
            Self::IonMode => "Colour by precursor ion mode (positive or negative).",
            Self::Charge => "Colour by precursor charge state.",
            Self::MsLevel => "Colour by MS level (for example MS2).",
            Self::Instrument => "Colour by the source instrument recorded in the spectrum metadata.",
            Self::IntensityEntropy => {
                "Heatmap of spectral intensity entropy: how evenly intensity is spread across peaks. Low values mean a few dominant peaks; high values mean many comparable peaks."
            }
            Self::PeakCount => "Heatmap of the number of fragment peaks in each spectrum.",
            Self::PrecursorMz => "Heatmap of the precursor m/z (pepmass) of each spectrum.",
        }
    }

    /// A distinct accent colour for this scheme's button.
    #[must_use]
    pub const fn accent(self) -> &'static str {
        match self {
            Self::Community => "#6b4a9e",
            Self::LeidenCommunity => "#205e8c",
            Self::IonMode => "#38755a",
            Self::Charge => "#9d4133",
            Self::MsLevel => "#b6792f",
            Self::Instrument => "#2f7d7d",
            Self::IntensityEntropy => "#c2553f",
            Self::PeakCount => "#3b6ea5",
            Self::PrecursorMz => "#7a5c3a",
        }
    }

    /// Whether this scheme is a continuous heatmap rather than categorical.
    #[must_use]
    pub const fn is_continuous(self) -> bool {
        matches!(
            self,
            Self::IntensityEntropy | Self::PeakCount | Self::PrecursorMz
        )
    }
}

/// The legend describing the active colouring.
#[derive(Debug, Clone, PartialEq)]
pub enum Legend {
    /// Discrete `(label, colour)` entries, in first-appearance order.
    Categorical(Vec<(String, String)>),
    /// A continuous scale spanning `[min, max]`.
    Continuous {
        /// Smallest value in the data.
        min: f64,
        /// Largest value in the data.
        max: f64,
    },
}

/// A computed colouring: a colour per node plus a legend.
#[derive(Debug, Clone, PartialEq)]
pub struct Coloring {
    /// Colour per node, index-aligned with the graph nodes.
    pub colors: Vec<String>,
    /// Group index per node (for categorical schemes), selecting marker shape
    /// and intra-group edge colour. All zero for continuous schemes.
    pub groups: Vec<usize>,
    /// Whether the scheme is categorical (groups and shapes are meaningful).
    pub categorical: bool,
    /// The legend describing the colouring.
    pub legend: Legend,
}

/// Shannon entropy of a spectrum's normalised peak intensities.
#[must_use]
pub fn intensity_entropy(record: &MascotGenericFormat) -> f64 {
    let intensities: Vec<f64> = record.intensities().map(SpectrumFloat::to_f64).collect();
    let total: f64 = intensities.iter().sum();
    if total <= 0.0 {
        return 0.0;
    }
    -intensities
        .iter()
        .filter(|&&intensity| intensity > 0.0)
        .map(|&intensity| {
            let probability = intensity / total;
            probability * probability.ln()
        })
        .sum::<f64>()
}

/// Returns the per-node value for a continuous `scheme`.
fn node_values(scheme: ColorScheme, records: &[MascotGenericFormat]) -> Vec<f64> {
    records
        .iter()
        .map(|record| match scheme {
            ColorScheme::IntensityEntropy => intensity_entropy(record),
            ColorScheme::PeakCount => record.len() as f64,
            ColorScheme::PrecursorMz => record.precursor_mz().to_f64(),
            _ => 0.0,
        })
        .collect()
}

/// Returns the group label for each node under a categorical `scheme`.
fn node_labels(
    scheme: ColorScheme,
    graph: &SimilarityGraph,
    records: &[MascotGenericFormat],
) -> Vec<String> {
    match scheme {
        ColorScheme::Community => graph
            .community_of_node
            .iter()
            .map(|community| format!("Community {community}"))
            .collect(),
        ColorScheme::LeidenCommunity => graph
            .leiden_of_node
            .iter()
            .map(|community| format!("Community {community}"))
            .collect(),
        ColorScheme::IonMode => records
            .iter()
            .map(|record| {
                record
                    .metadata()
                    .ion_mode()
                    .map_or_else(|| "unknown".to_string(), |mode| mode.as_str().to_string())
            })
            .collect(),
        ColorScheme::Charge => records
            .iter()
            .map(|record| {
                record
                    .metadata()
                    .charge()
                    .map_or_else(|| "unknown".to_string(), |charge| charge.to_string())
            })
            .collect(),
        ColorScheme::MsLevel => records
            .iter()
            .map(|record| {
                record
                    .metadata()
                    .level()
                    .map_or_else(|| "unknown".to_string(), |level| level.to_string())
            })
            .collect(),
        ColorScheme::Instrument => records
            .iter()
            .map(|record| {
                record.metadata().source_instrument().map_or_else(
                    || "unknown".to_string(),
                    |instrument| instrument.as_str().to_string(),
                )
            })
            .collect(),
        // Continuous schemes do not use labels.
        ColorScheme::IntensityEntropy | ColorScheme::PeakCount | ColorScheme::PrecursorMz => {
            Vec::new()
        }
    }
}

/// Whether a scheme distinguishes at least two nodes.
///
/// A scheme whose every node maps to the same group (or the same value) is
/// trivial and not worth offering, so the UI hides it.
#[must_use]
pub fn is_informative(
    scheme: ColorScheme,
    graph: &SimilarityGraph,
    records: &[MascotGenericFormat],
) -> bool {
    if scheme.is_continuous() {
        let values = node_values(scheme, records);
        let mut finite = values.into_iter().filter(|value| value.is_finite());
        match finite.next() {
            Some(first) => finite.any(|value| (value - first).abs() > f64::EPSILON),
            None => false,
        }
    } else {
        let mut seen: BTreeSet<String> = BTreeSet::new();
        for label in node_labels(scheme, graph, records) {
            seen.insert(label);
            if seen.len() >= 2 {
                return true;
            }
        }
        false
    }
}

/// Computes per-node colours and a legend for the chosen `scheme`.
#[must_use]
pub fn compute(
    scheme: ColorScheme,
    graph: &SimilarityGraph,
    records: &[MascotGenericFormat],
) -> Coloring {
    if scheme.is_continuous() {
        let values = node_values(scheme, records);
        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;
        for &value in values.iter().filter(|value| value.is_finite()) {
            min = min.min(value);
            max = max.max(value);
        }
        if !min.is_finite() {
            min = 0.0;
            max = 0.0;
        }
        let span = (max - min).max(f64::EPSILON);
        let colors = values
            .iter()
            .map(|&value| {
                let t = if value.is_finite() {
                    (value - min) / span
                } else {
                    0.0
                };
                heat_color(t)
            })
            .collect();
        return Coloring {
            colors,
            groups: vec![0; values.len()],
            categorical: false,
            legend: Legend::Continuous { min, max },
        };
    }

    let labels = node_labels(scheme, graph, records);
    let mut index_of: BTreeMap<String, usize> = BTreeMap::new();
    let mut order: Vec<String> = Vec::new();
    for label in &labels {
        if !index_of.contains_key(label) {
            index_of.insert(label.clone(), order.len());
            order.push(label.clone());
        }
    }
    let groups: Vec<usize> = labels.iter().map(|label| index_of[label]).collect();
    let colors = groups
        .iter()
        .map(|&group| palette_color(group).to_string())
        .collect();
    let legend = Legend::Categorical(
        order
            .iter()
            .enumerate()
            .map(|(index, label)| (label.clone(), palette_color(index).to_string()))
            .collect(),
    );
    Coloring {
        colors,
        groups,
        categorical: true,
        legend,
    }
}
