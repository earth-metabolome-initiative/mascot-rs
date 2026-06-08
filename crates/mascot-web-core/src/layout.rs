//! 2D layout of the similarity graph via ForceAtlas2.
//!
//! The sparse, similarity-weighted k-nearest-neighbour graph is laid out with
//! the ForceAtlas2 force-directed algorithm from `geometric-traits`: nodes
//! repel each other, edges attract proportionally to their similarity, and a
//! gravity term keeps disconnected components from drifting away. The layout is
//! deterministic (fixed seed) and uses the Barnes-Hut approximation for larger
//! graphs. A circular layout is used as a fallback for trivial inputs.

use geometric_traits::impls::ValuedCSR2D;
use geometric_traits::prelude::GenericEdgesBuilder;
use geometric_traits::traits::{EdgesBuilder, ForceAtlas2, ForceAtlas2Config};

use crate::similarity::Edge;

/// Weighted, symmetric sparse matrix handed to ForceAtlas2.
type WeightedCsr = ValuedCSR2D<usize, usize, usize, f64>;

/// Iterations of ForceAtlas2 to run.
const ITERATIONS: usize = 500;

/// Repulsion scaling constant (`kr`); larger values spread the layout out.
const SCALING_RATIO: f64 = 10.0;

/// Gravity constant (`kg`); higher values keep disconnected nodes from being
/// flung out, so the connected component fills more of the view.
const GRAVITY: f64 = 8.0;

/// Node count beyond which the Barnes-Hut repulsion approximation is enabled.
const BARNES_HUT_THRESHOLD: usize = 1000;

/// Computes 2D coordinates for each node using ForceAtlas2.
///
/// Falls back to a deterministic circular layout for trivial graphs (fewer than
/// three nodes) or if the layout fails.
#[must_use]
pub fn compute(node_count: usize, edges: &[Edge]) -> Vec<[f64; 2]> {
    if node_count == 0 {
        return Vec::new();
    }
    if node_count < 3 {
        return circle_layout(node_count);
    }
    force_atlas2_layout(node_count, edges).unwrap_or_else(|_| circle_layout(node_count))
}

/// Runs ForceAtlas2 over the similarity-weighted graph.
fn force_atlas2_layout(node_count: usize, edges: &[Edge]) -> Result<Vec<[f64; 2]>, String> {
    // ForceAtlas2 expects a symmetric weighted matrix, so add both directions.
    // Edge weight is the similarity score (stronger pairs attract harder).
    let mut weighted: Vec<(usize, usize, f64)> = Vec::with_capacity(edges.len() * 2);
    for &(u, v, score) in edges {
        let weight = score.max(0.0);
        weighted.push((u, v, weight));
        weighted.push((v, u, weight));
    }
    weighted.sort_unstable_by_key(|edge| (edge.0, edge.1));

    let matrix: WeightedCsr = GenericEdgesBuilder::<_, WeightedCsr>::default()
        .expected_number_of_edges(weighted.len())
        .expected_shape((node_count, node_count))
        .edges(weighted.into_iter())
        .build()
        .map_err(|error| format!("{error:?}"))?;

    let config = ForceAtlas2Config {
        iterations: ITERATIONS,
        scaling_ratio: SCALING_RATIO,
        gravity: GRAVITY,
        strong_gravity: true,
        barnes_hut: node_count >= BARNES_HUT_THRESHOLD,
        ..ForceAtlas2Config::default()
    };
    let result = matrix
        .force_atlas2(&config)
        .map_err(|error| format!("{error:?}"))?;

    let coordinates = (0..node_count)
        .map(|i| {
            let point = result.point(i);
            [
                point.first().copied().unwrap_or(0.0),
                point.get(1).copied().unwrap_or(0.0),
            ]
        })
        .collect();
    Ok(coordinates)
}

/// A deterministic layout placing nodes evenly on a unit circle.
fn circle_layout(node_count: usize) -> Vec<[f64; 2]> {
    if node_count == 1 {
        return vec![[0.0, 0.0]];
    }
    (0..node_count)
        .map(|i| {
            let angle = core::f64::consts::TAU * (i as f64) / (node_count as f64);
            [angle.cos(), angle.sin()]
        })
        .collect()
}
