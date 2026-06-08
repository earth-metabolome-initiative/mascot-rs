//! Spectral similarity selection and similarity-graph construction.
//!
//! Edges are built with the FLASH search indices from `mass_spectrometry`: each
//! spectrum is queried for its top-k most similar neighbours and edges are kept
//! when the similarity score clears a minimum threshold (the combined top-k and
//! threshold rule). The undirected graph is then handed to `geometric-traits`
//! for connected-component labelling.
//!
//! Index construction uses the sequential build path (no `rayon`, no `std`
//! threads) so it runs in single-threaded browser WASM.

use std::collections::BTreeMap;

use geometric_traits::impls::{SortedVec, SymmetricCSR2D, ValuedCSR2D, CSR2D};
use geometric_traits::prelude::{
    GenericEdgesBuilder, GenericVocabularyBuilder, UndiEdgesBuilder, UndiGraph,
};
use geometric_traits::traits::{
    ConnectedComponents, EdgesBuilder, Leiden, LeidenConfig, Louvain, LouvainConfig,
    VocabularyBuilder,
};
use mascot_rs::prelude::MascotGenericFormat;
use mass_spectrometry::prelude::{
    FlashCosineIndex, FlashEntropyIndex, FlashSearchResult, GenericSpectrum, LinearCosine,
    LinearEntropy, ModifiedLinearCosine, ModifiedLinearEntropy, ScalarSimilarity,
    SiriusMergeClosePeaks, SpectraIndexBuilder, SpectralProcessor,
};

/// The spectral similarity measure used to weight graph edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimilarityMethod {
    /// Linear cosine similarity.
    Cosine,
    /// Linear cosine with precursor-shifted (neutral-loss) matching.
    ModifiedCosine,
    /// Spectral entropy similarity.
    Entropy,
    /// Spectral entropy with precursor-shifted matching.
    ModifiedEntropy,
}

impl SimilarityMethod {
    /// All methods, in display order.
    pub const ALL: [Self; 4] = [
        Self::Cosine,
        Self::ModifiedCosine,
        Self::Entropy,
        Self::ModifiedEntropy,
    ];

    /// A stable identifier used as the `<option>` value.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Cosine => "cosine",
            Self::ModifiedCosine => "modified-cosine",
            Self::Entropy => "entropy",
            Self::ModifiedEntropy => "modified-entropy",
        }
    }

    /// A human-readable label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Cosine => "Cosine",
            Self::ModifiedCosine => "Modified cosine",
            Self::Entropy => "Entropy",
            Self::ModifiedEntropy => "Modified entropy",
        }
    }

    /// A full explanation, used for button tooltips and screen-reader labels.
    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Cosine => {
                "Cosine similarity: aligns matching fragment peaks within the m/z tolerance and compares their intensity patterns. Fast baseline measure."
            }
            Self::ModifiedCosine => {
                "Modified cosine: like cosine, but also matches peaks shifted by the precursor mass difference, linking analogues that differ by a chemical modification."
            }
            Self::Entropy => {
                "Spectral entropy similarity: weights matched peaks by Shannon entropy, often more selective than cosine."
            }
            Self::ModifiedEntropy => {
                "Modified spectral entropy: entropy similarity that also matches precursor-shifted (neutral-loss) peaks."
            }
        }
    }

    /// A distinct accent colour for this method's button.
    #[must_use]
    pub const fn accent(self) -> &'static str {
        match self {
            Self::Cosine => "#205e8c",
            Self::ModifiedCosine => "#6b4a9e",
            Self::Entropy => "#2f7d7d",
            Self::ModifiedEntropy => "#9d4133",
        }
    }

    /// Whether this method uses precursor-shifted (modified) matching.
    const fn is_modified(self) -> bool {
        matches!(self, Self::ModifiedCosine | Self::ModifiedEntropy)
    }
}

/// Parameters controlling similarity-graph construction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GraphParams {
    /// The similarity measure.
    pub method: SimilarityMethod,
    /// m/z matching tolerance, in Da.
    pub mz_tolerance: f64,
    /// Exponent applied to peak m/z when weighting matches.
    pub mz_power: f64,
    /// Exponent applied to peak intensity when weighting matches.
    pub intensity_power: f64,
    /// Number of nearest neighbours queried per spectrum.
    pub top_k: usize,
    /// Minimum similarity score for an edge to be kept.
    pub min_score: f64,
}

impl Default for GraphParams {
    fn default() -> Self {
        Self {
            method: SimilarityMethod::ModifiedCosine,
            mz_tolerance: 0.02,
            mz_power: 0.0,
            intensity_power: 1.0,
            top_k: 10,
            min_score: 0.3,
        }
    }
}

/// An undirected, weighted edge between two spectrum indices.
pub type Edge = (usize, usize, f64);

/// A computed similarity graph over a set of spectra.
#[derive(Debug, Clone, Default)]
pub struct SimilarityGraph {
    /// Number of nodes (one per spectrum, indices align with the dataset).
    pub node_count: usize,
    /// Deduplicated undirected edges `(u, v, score)` with `u < v`.
    pub edges: Vec<Edge>,
    /// Number of connected components.
    pub component_count: usize,
    /// Louvain community label per node.
    pub community_of_node: Vec<usize>,
    /// Number of Louvain communities.
    pub community_count: usize,
    /// Leiden community label per node.
    pub leiden_of_node: Vec<usize>,
    /// Number of Leiden communities.
    pub leiden_count: usize,
    /// 2D layout coordinate per node, index-aligned with the dataset.
    pub coordinates: Vec<[f64; 2]>,
}

/// Appends qualifying neighbour edges for query `source` to `edges`.
///
/// Self-matches and scores below `min_score` are skipped. Edges are stored in
/// canonical `(min, max)` orientation.
fn collect_edges(
    edges: &mut Vec<Edge>,
    source: usize,
    results: &[FlashSearchResult],
    min_score: f64,
) {
    for result in results {
        let target = result.spectrum_id as usize;
        if target == source || result.score < min_score {
            continue;
        }
        let (u, v) = if source < target {
            (source, target)
        } else {
            (target, source)
        };
        edges.push((u, v, result.score));
    }
}

/// Deduplicates undirected edges, keeping the maximum score per pair.
fn dedupe_edges(raw: Vec<Edge>) -> Vec<Edge> {
    let mut best: BTreeMap<(usize, usize), f64> = BTreeMap::new();
    for (u, v, score) in raw {
        best.entry((u, v))
            .and_modify(|existing| {
                if score > *existing {
                    *existing = score;
                }
            })
            .or_insert(score);
    }
    best.into_iter()
        .map(|((u, v), score)| (u, v, score))
        .collect()
}

/// Runs top-k searches over a built index and collects qualifying edges.
///
/// `query` performs one search per spectrum; `k` already accounts for the
/// self-match by querying one extra neighbour.
fn edges_from_searches<F>(
    node_count: usize,
    min_score: f64,
    mut query: F,
) -> Result<Vec<Edge>, String>
where
    F: FnMut(usize) -> Result<Vec<FlashSearchResult>, String>,
{
    let mut raw = Vec::new();
    for source in 0..node_count {
        let results = query(source)?;
        collect_edges(&mut raw, source, &results, min_score);
    }
    Ok(dedupe_edges(raw))
}

/// Computes deduplicated similarity edges for the given spectra.
pub fn compute_edges(
    records: &[MascotGenericFormat],
    params: &GraphParams,
) -> Result<Vec<Edge>, String> {
    let node_count = records.len();
    // Query one extra neighbour so the self-match can be dropped.
    let k = params.top_k.saturating_add(1);
    let modified = params.method.is_modified();

    // The FLASH indices use the linear matcher, which requires well-separated
    // spectra (consecutive peaks more than `2 * mz_tolerance` apart). The
    // SIRIUS close-peak merger with the same tolerance guarantees this and
    // matches how these fixtures were produced. Index and queries use the same
    // cleaned spectra so result ids align with the original record indices.
    let processor = SiriusMergeClosePeaks::<f64>::new(params.mz_tolerance.max(1e-6))
        .map_err(|error| error.to_string())?;
    let cleaned: Vec<GenericSpectrum<f64>> = records
        .iter()
        .map(|record| processor.process(record.as_ref()))
        .collect();

    match params.method {
        SimilarityMethod::Cosine | SimilarityMethod::ModifiedCosine => {
            let index = FlashCosineIndex::<f64>::builder()
                .mz_power(params.mz_power)
                .intensity_power(params.intensity_power)
                .mz_tolerance(params.mz_tolerance)
                .build(&cleaned)
                .map_err(|error| error.to_string())?;
            let mut state = index.new_search_state();
            edges_from_searches(node_count, params.min_score, |source| {
                let query = &cleaned[source];
                if modified {
                    index.search_modified_top_k_with_state(query, k, &mut state)
                } else {
                    index.search_top_k_with_state(query, k, &mut state)
                }
                .map_err(|error| error.to_string())
            })
        }
        SimilarityMethod::Entropy | SimilarityMethod::ModifiedEntropy => {
            let index = FlashEntropyIndex::<f64>::builder()
                .mz_power(params.mz_power)
                .intensity_power(params.intensity_power)
                .mz_tolerance(params.mz_tolerance)
                .build(&cleaned)
                .map_err(|error| error.to_string())?;
            let mut state = index.new_search_state();
            edges_from_searches(node_count, params.min_score, |source| {
                let query = &cleaned[source];
                if modified {
                    index.search_modified_top_k_with_state(query, k, &mut state)
                } else {
                    index.search_top_k_with_state(query, k, &mut state)
                }
                .map_err(|error| error.to_string())
            })
        }
    }
}

/// All four pairwise similarity scores between two spectra.
///
/// Scores are computed directly (not via the top-k index) so an edge can report
/// every measure, not just the one that built the graph. The two spectra are
/// cleaned with the same `SiriusMergeClosePeaks` preprocessing and scored with
/// the same tolerance and power settings as graph construction, so the active
/// method's value matches the edge's graph weight. A method's score is `None`
/// when it cannot be evaluated for this pair.
#[must_use]
pub fn pairwise_similarities(
    left: &MascotGenericFormat,
    right: &MascotGenericFormat,
    params: &GraphParams,
) -> Vec<(SimilarityMethod, Option<f64>)> {
    let tolerance = params.mz_tolerance.max(1e-6);
    let Ok(processor) = SiriusMergeClosePeaks::<f64>::new(tolerance) else {
        return SimilarityMethod::ALL
            .into_iter()
            .map(|method| (method, None))
            .collect();
    };
    let left = processor.process(left.as_ref());
    let right = processor.process(right.as_ref());
    SimilarityMethod::ALL
        .into_iter()
        .map(|method| (method, score_pair(method, params, &left, &right)))
        .collect()
}

/// Scores one method for an already-cleaned spectrum pair.
fn score_pair(
    method: SimilarityMethod,
    params: &GraphParams,
    left: &GenericSpectrum<f64>,
    right: &GenericSpectrum<f64>,
) -> Option<f64> {
    let mz_power = params.mz_power;
    let intensity_power = params.intensity_power;
    let tolerance = params.mz_tolerance;
    let score = match method {
        SimilarityMethod::Cosine => LinearCosine::new(mz_power, intensity_power, tolerance)
            .ok()?
            .similarity(left, right),
        SimilarityMethod::ModifiedCosine => {
            ModifiedLinearCosine::new(mz_power, intensity_power, tolerance)
                .ok()?
                .similarity(left, right)
        }
        // `weighted = true` matches the `FlashEntropyIndex` default used to build
        // the graph, so this agrees with the entropy edge weights.
        SimilarityMethod::Entropy => LinearEntropy::new(mz_power, intensity_power, tolerance, true)
            .ok()?
            .similarity(left, right),
        SimilarityMethod::ModifiedEntropy => {
            ModifiedLinearEntropy::new(mz_power, intensity_power, tolerance, true)
                .ok()?
                .similarity(left, right)
        }
    };
    score.ok().map(|(value, _matches)| value)
}

/// Labels connected components of the undirected graph defined by `edges`.
///
/// Returns `(component_count, component_of_node)`.
fn label_components(node_count: usize, edges: &[Edge]) -> Result<(usize, Vec<usize>), String> {
    let nodes: SortedVec<usize> = GenericVocabularyBuilder::default()
        .expected_number_of_symbols(node_count)
        .symbols((0..node_count).enumerate())
        .build()
        .map_err(|error| format!("{error:?}"))?;

    let mut edge_data: Vec<(usize, usize)> = edges.iter().map(|&(u, v, _)| (u, v)).collect();
    edge_data.sort_unstable();

    let csr: SymmetricCSR2D<CSR2D<usize, usize, usize>> = UndiEdgesBuilder::default()
        .expected_number_of_edges(edge_data.len())
        .expected_shape(node_count)
        .edges(edge_data.into_iter())
        .build()
        .map_err(|error| format!("{error:?}"))?;

    let graph: UndiGraph<usize> = UndiGraph::from((nodes, csr));
    let components = graph
        .connected_components()
        .map_err(|error| format!("{error:?}"))?;

    let component_count = components.number_of_components();
    let component_of_node: Vec<usize> = (0..node_count)
        .map(|node| components.component_of_node(node))
        .collect();
    Ok((component_count, component_of_node))
}

/// Labels Louvain communities of the undirected graph weighted by similarity.
///
/// Returns `(community_count, community_of_node)`. With no edges, every node is
/// its own community.
/// The number of distinct labels in a partition.
fn distinct_count(partition: &[usize]) -> usize {
    let mut sorted = partition.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    sorted.len()
}

/// A labelled community partition: `(count, label_per_node)`.
type Partition = (usize, Vec<usize>);

/// Labels Louvain and Leiden communities of the similarity-weighted graph.
///
/// Returns `(louvain, leiden)` partitions. With no edges, every node is its own
/// community under both.
fn label_communities(node_count: usize, edges: &[Edge]) -> Result<(Partition, Partition), String> {
    if edges.is_empty() {
        let trivial: Vec<usize> = (0..node_count).collect();
        return Ok(((node_count, trivial.clone()), (node_count, trivial)));
    }

    let mut weighted: Vec<(usize, usize, f64)> = Vec::with_capacity(edges.len() * 2);
    for &(u, v, score) in edges {
        // Both algorithms require strictly positive, finite weights.
        let weight = score.max(1e-9);
        weighted.push((u, v, weight));
        weighted.push((v, u, weight));
    }
    weighted.sort_unstable_by_key(|edge| (edge.0, edge.1));

    let matrix: ValuedCSR2D<usize, usize, usize, f64> = GenericEdgesBuilder::default()
        .expected_number_of_edges(weighted.len())
        .expected_shape((node_count, node_count))
        .edges(weighted.into_iter())
        .build()
        .map_err(|error| format!("{error:?}"))?;

    let louvain = Louvain::<usize>::louvain(&matrix, &LouvainConfig::default())
        .map_err(|error| format!("{error:?}"))?
        .final_partition()
        .to_vec();
    let leiden = Leiden::<usize>::leiden(&matrix, &LeidenConfig::default())
        .map_err(|error| format!("{error:?}"))?
        .final_partition()
        .to_vec();

    Ok((
        (distinct_count(&louvain), louvain),
        (distinct_count(&leiden), leiden),
    ))
}

/// Builds a full similarity graph: edges, component and community labels, and
/// a 2D layout.
pub fn build_graph(
    records: &[MascotGenericFormat],
    params: &GraphParams,
) -> Result<SimilarityGraph, String> {
    let node_count = records.len();
    let edges = compute_edges(records, params)?;
    let (component_count, _component_of_node) = label_components(node_count, &edges)?;
    let ((community_count, community_of_node), (leiden_count, leiden_of_node)) =
        label_communities(node_count, &edges)?;
    let coordinates = crate::layout::compute(node_count, &edges);
    Ok(SimilarityGraph {
        node_count,
        edges,
        component_count,
        community_of_node,
        community_count,
        leiden_of_node,
        leiden_count,
        coordinates,
    })
}
