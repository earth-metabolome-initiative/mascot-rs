//! Messages exchanged between the app and the graph-building Web Worker.
//!
//! These are (de)serialized with `serde-wasm-bindgen` and passed via
//! `postMessage`, so they must be plain serde types.

use serde::{Deserialize, Serialize};

use crate::similarity::{GraphParams, SimilarityGraph};

/// A request sent from the app to the worker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkerRequest {
    /// Build a graph from MGF `text` with `params`, tagged by `id`.
    Build {
        /// Correlates the response with this request.
        id: u64,
        /// The raw MGF document.
        text: String,
        /// The graph-construction parameters.
        params: GraphParams,
    },
}

/// A response sent from the worker back to the app.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkerResponse {
    /// The worker has initialised and is ready for requests.
    Ready,
    /// A graph was built successfully for request `id`.
    Built {
        /// The request this answers.
        id: u64,
        /// The built graph.
        graph: SimilarityGraph,
    },
    /// Building the graph for request `id` failed.
    Error {
        /// The request this answers.
        id: u64,
        /// A human-readable error description.
        message: String,
    },
}
