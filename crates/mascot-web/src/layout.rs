//! 2D layout of the similarity graph via classical MDS over graph distances.
//!
//! Edge weights are cosine-style distances (`1 - score`). All-pairs shortest
//! paths over the (sparse) k-nearest-neighbour graph give a distance matrix
//! that classical MDS projects to two dimensions. This reflects graph topology
//! and consumes the sparse graph we already built, rather than a dense
//! all-pairs similarity matrix. MDS is O(n^3), so it suits the small-to-medium
//! datasets this view targets; a deterministic circular layout is used as a
//! fallback when MDS does not apply or fails.

use geometric_traits::impls::{PaddedMatrix2D, ValuedCSR2D};
use geometric_traits::prelude::*;
use geometric_traits::traits::{DenseValuedMatrix, EdgesBuilder};

use crate::similarity::Edge;

/// Weighted sparse matrix backing the shortest-path computation.
type WeightedCsr = ValuedCSR2D<usize, usize, usize, f64>;

/// Computes 2D coordinates for each node.
///
/// Uses classical MDS over shortest-path graph distances, falling back to a
/// deterministic circular layout for trivial graphs (fewer than three nodes)
/// or if MDS fails.
#[must_use]
pub fn mds_layout(node_count: usize, edges: &[Edge]) -> Vec<[f64; 2]> {
    if node_count == 0 {
        return Vec::new();
    }
    if node_count < 3 {
        return circle_layout(node_count);
    }
    try_mds_layout(node_count, edges).unwrap_or_else(|_| circle_layout(node_count))
}

/// Attempts a classical-MDS layout, returning an error describing any failure.
fn try_mds_layout(node_count: usize, edges: &[Edge]) -> Result<Vec<[f64; 2]>, String> {
    let distances = shortest_path_distances(node_count, edges)?;
    let inner: WeightedCsr = GenericEdgesBuilder::<_, WeightedCsr>::default()
        .expected_number_of_edges(0)
        .expected_shape((node_count, node_count))
        .edges(core::iter::empty())
        .build()
        .map_err(|error| format!("{error:?}"))?;
    let n = node_count;
    let padded = PaddedMatrix2D::new(
        inner,
        Box::new(move |coords: (usize, usize)| distances[coords.0 * n + coords.1])
            as Box<dyn Fn((usize, usize)) -> f64>,
    )
    .map_err(|error| format!("{error:?}"))?;

    let config = MdsConfig {
        dimensions: 2,
        ..MdsConfig::default()
    };
    let result = padded
        .classical_mds(&config)
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

/// Builds a dense, flat (row-major) distance matrix from graph shortest paths.
///
/// Pairs in different connected components (unreachable) are filled with 1.5x
/// the largest finite distance, pushing components apart while keeping every
/// entry finite for MDS.
fn shortest_path_distances(node_count: usize, edges: &[Edge]) -> Result<Vec<f64>, String> {
    let mut weighted: Vec<(usize, usize, f64)> = Vec::with_capacity(edges.len() * 2);
    for &(u, v, score) in edges {
        let distance = (1.0 - score).max(0.0);
        weighted.push((u, v, distance));
        weighted.push((v, u, distance));
    }
    weighted.sort_unstable_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));

    let csr: WeightedCsr = GenericEdgesBuilder::<_, WeightedCsr>::default()
        .expected_number_of_edges(weighted.len())
        .expected_shape((node_count, node_count))
        .edges(weighted.into_iter())
        .build()
        .map_err(|error| format!("{error:?}"))?;
    let paths = csr
        .pairwise_dijkstra()
        .map_err(|error| format!("{error:?}"))?;

    let mut max_finite = 0.0_f64;
    for i in 0..node_count {
        for j in 0..node_count {
            if let Some(distance) = paths.value((i, j)) {
                if distance.is_finite() && distance > max_finite {
                    max_finite = distance;
                }
            }
        }
    }
    let fill = if max_finite > 0.0 {
        max_finite * 1.5
    } else {
        1.0
    };

    let mut flat = vec![0.0_f64; node_count * node_count];
    for i in 0..node_count {
        for j in 0..node_count {
            flat[i * node_count + j] = if i == j {
                0.0
            } else {
                paths
                    .value((i, j))
                    .filter(|distance| distance.is_finite())
                    .unwrap_or(fill)
            };
        }
    }
    Ok(flat)
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
