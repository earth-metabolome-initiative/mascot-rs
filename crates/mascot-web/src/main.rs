//! Dioxus web app to visualize MGF spectra as a spectral similarity graph.

mod coloring;
mod dataset;
mod export;
mod layout;
mod render;
mod similarity;

use dioxus::html::{FileData, HasFileData};
use dioxus::prelude::*;
use dioxus_free_icons::icons::fa_brands_icons::FaGithub;
use dioxus_free_icons::icons::fa_solid_icons::{
    FaBolt, FaChartArea, FaChartColumn, FaCircleNodes, FaDownload, FaFlask, FaFolderOpen,
    FaHashtag, FaLayerGroup, FaMicroscope, FaPlusMinus, FaRightLeft, FaShuffle, FaUserGroup,
    FaUsers, FaVial, FaWaveSquare, FaWeightHanging, FaXmark,
};
use dioxus_free_icons::Icon;
use mascot_rs::prelude::{MascotGenericFormat, Spectrum, SpectrumFloat};

use crate::coloring::ColorScheme;
use crate::dataset::DatasetState;
use crate::similarity::{build_graph, GraphParams, SimilarityGraph, SimilarityMethod};

/// Renders the icon for a similarity method, with the given fill colour.
fn similarity_icon(method: SimilarityMethod, fill: String) -> Element {
    match method {
        SimilarityMethod::Cosine => {
            rsx! { Icon { width: 14, height: 14, fill, icon: FaWaveSquare } }
        }
        SimilarityMethod::ModifiedCosine => {
            rsx! { Icon { width: 14, height: 14, fill, icon: FaRightLeft } }
        }
        SimilarityMethod::Entropy => {
            rsx! { Icon { width: 14, height: 14, fill, icon: FaChartColumn } }
        }
        SimilarityMethod::ModifiedEntropy => {
            rsx! { Icon { width: 14, height: 14, fill, icon: FaShuffle } }
        }
    }
}

/// Renders a small SVG legend marker matching the canvas node shape for a
/// group index, filled with `color`.
fn node_marker_svg(group: usize, color: String) -> Element {
    let stroke = "#15232b";
    let center = 7.0_f64;
    let radius = 5.0_f64;
    let corner = center - radius;
    let side = 2.0 * radius;
    match group % render::SHAPE_COUNT {
        1 => rsx! {
            svg { class: "legend-marker", width: "14", height: "14", view_box: "0 0 14 14",
                rect {
                    x: "{corner}",
                    y: "{corner}",
                    width: "{side}",
                    height: "{side}",
                    fill: "{color}",
                    stroke,
                    stroke_width: "1",
                }
            }
        },
        2 => {
            let apex_y = center - radius;
            let base_y = center + radius * 0.7;
            let half = radius * 0.95;
            let left = center - half;
            let right = center + half;
            let points = format!("{center},{apex_y} {right},{base_y} {left},{base_y}");
            rsx! {
                svg { class: "legend-marker", width: "14", height: "14", view_box: "0 0 14 14",
                    polygon { points: "{points}", fill: "{color}", stroke, stroke_width: "1" }
                }
            }
        }
        3 => {
            let top = center - radius;
            let bottom = center + radius;
            let left = center - radius;
            let right = center + radius;
            let points =
                format!("{center},{top} {right},{center} {center},{bottom} {left},{center}");
            rsx! {
                svg { class: "legend-marker", width: "14", height: "14", view_box: "0 0 14 14",
                    polygon { points: "{points}", fill: "{color}", stroke, stroke_width: "1" }
                }
            }
        }
        4 => {
            let apex_y = center + radius;
            let base_y = center - radius * 0.7;
            let half = radius * 0.95;
            let left = center - half;
            let right = center + half;
            let points = format!("{center},{apex_y} {right},{base_y} {left},{base_y}");
            rsx! {
                svg { class: "legend-marker", width: "14", height: "14", view_box: "0 0 14 14",
                    polygon { points: "{points}", fill: "{color}", stroke, stroke_width: "1" }
                }
            }
        }
        5 => {
            let points = (0..6)
                .map(|corner| {
                    let angle = std::f64::consts::FRAC_PI_3 * f64::from(corner)
                        - std::f64::consts::FRAC_PI_2;
                    format!(
                        "{:.2},{:.2}",
                        center + radius * angle.cos(),
                        center + radius * angle.sin()
                    )
                })
                .collect::<Vec<_>>()
                .join(" ");
            rsx! {
                svg { class: "legend-marker", width: "14", height: "14", view_box: "0 0 14 14",
                    polygon { points: "{points}", fill: "{color}", stroke, stroke_width: "1" }
                }
            }
        }
        _ => rsx! {
            svg { class: "legend-marker", width: "14", height: "14", view_box: "0 0 14 14",
                circle { cx: "{center}", cy: "{center}", r: "{radius}", fill: "{color}", stroke, stroke_width: "1" }
            }
        },
    }
}

/// Renders the icon for a colour scheme, with the given fill colour.
fn scheme_icon(scheme: ColorScheme, fill: String) -> Element {
    match scheme {
        ColorScheme::Community => rsx! { Icon { width: 14, height: 14, fill, icon: FaUsers } },
        ColorScheme::LeidenCommunity => {
            rsx! { Icon { width: 14, height: 14, fill, icon: FaUserGroup } }
        }
        ColorScheme::IonMode => rsx! { Icon { width: 14, height: 14, fill, icon: FaPlusMinus } },
        ColorScheme::Charge => rsx! { Icon { width: 14, height: 14, fill, icon: FaBolt } },
        ColorScheme::MsLevel => rsx! { Icon { width: 14, height: 14, fill, icon: FaLayerGroup } },
        ColorScheme::Instrument => {
            rsx! { Icon { width: 14, height: 14, fill, icon: FaMicroscope } }
        }
        ColorScheme::IntensityEntropy => {
            rsx! { Icon { width: 14, height: 14, fill, icon: FaChartArea } }
        }
        ColorScheme::PeakCount => rsx! { Icon { width: 14, height: 14, fill, icon: FaHashtag } },
        ColorScheme::PrecursorMz => {
            rsx! { Icon { width: 14, height: 14, fill, icon: FaWeightHanging } }
        }
    }
}

/// State of the computed similarity graph.
enum GraphState {
    /// No graph has been built yet.
    None,
    /// A graph was built successfully.
    Built(SimilarityGraph),
    /// Graph construction failed.
    Error(String),
}

/// The current pointer interaction on the graph canvas.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Drag {
    /// No drag in progress.
    None,
    /// Panning the view; holds the last pointer position.
    Pan {
        /// Last pointer x, in canvas CSS pixels.
        x: f64,
        /// Last pointer y, in canvas CSS pixels.
        y: f64,
    },
    /// Dragging a node, identified by its index.
    Node {
        /// Index of the node being dragged.
        index: usize,
    },
}

/// A hovered edge: its similarity score and the cursor position for the tooltip.
#[derive(Debug, Clone, Copy, PartialEq)]
struct EdgeHover {
    /// Similarity score of the hovered edge.
    score: f64,
    /// Cursor x in canvas CSS pixels.
    x: f64,
    /// Cursor y in canvas CSS pixels.
    y: f64,
}

/// The hovered and selected node, shared between the graph and the info panel.
///
/// Both fields share the type `Signal<Option<usize>>`, so they are grouped in
/// one context value rather than provided separately.
#[derive(Clone, Copy)]
struct NodeFocus {
    /// The clicked (pinned) node, if any.
    selected: Signal<Option<usize>>,
    /// The hovered node, if any.
    hovered: Signal<Option<usize>>,
}

impl NodeFocus {
    /// The node to show details for: the selection takes priority over hover.
    fn active(self) -> Option<usize> {
        (*self.selected.read()).or(*self.hovered.read())
    }
}

/// The hovered and selected edge, shared between the graph and the edge panel.
///
/// Each field is the index of an edge in `SimilarityGraph::edges`. Indices are
/// only valid for the current built graph and are reset when a new graph is
/// built (see the reset effect in `GraphCanvas`).
#[derive(Clone, Copy)]
struct EdgeFocus {
    /// The clicked (pinned) edge, if any.
    selected: Signal<Option<usize>>,
    /// The hovered edge, if any.
    hovered: Signal<Option<usize>>,
}

impl EdgeFocus {
    /// The edge to show details for: the selection takes priority over hover.
    fn active(self) -> Option<usize> {
        (*self.selected.read()).or(*self.hovered.read())
    }
}

/// A bundled example MGF (a SIRIUS-derived DBGI feature list) so the app can be
/// explored without uploading a file.
const EXAMPLE_MGF: &str = include_str!("../assets/example.mgf");

/// Display name for the bundled example.
const EXAMPLE_NAME: &str = "example.mgf";

// Register the stylesheet so `dx` writes a `<link>` into the static `<head>` at
// build time (`with_static_head`) with a preload hint (`with_preload`). Loading
// the CSS before the WASM app mounts avoids a flash of unstyled content; a
// runtime `document::Stylesheet` would only inject the link after first paint.
const _: Asset = asset!(
    "/assets/main.css",
    AssetOptions::css()
        .with_static_head(true)
        .with_preload(true)
);

fn main() {
    dioxus::launch(App);
}

/// Root application component.
#[component]
fn App() -> Element {
    // Shared app state lives in context so child components can read and
    // update it without threading non-`PartialEq` signals through props.
    use_context_provider(|| Signal::new(DatasetState::Empty));
    use_context_provider(|| Signal::new(GraphParams::default()));
    use_context_provider(|| Signal::new(GraphState::None));
    use_context_provider(|| Signal::new(ColorScheme::LeidenCommunity));
    use_context_provider(|| NodeFocus {
        selected: Signal::new(None),
        hovered: Signal::new(None),
    });
    use_context_provider(|| EdgeFocus {
        selected: Signal::new(None),
        hovered: Signal::new(None),
    });
    // Before a dataset is loaded, centre the hero and drop area in the viewport;
    // once content appears, fall back to the normal top-aligned flow.
    let dataset = use_context::<Signal<DatasetState>>();
    let landing = !matches!(&*dataset.read(), DatasetState::Loaded { .. });
    rsx! {
        div { class: if landing { "page landing" } else { "page" },
            header { class: "hero",
                p { class: "eyebrow", "Earth Metabolome Initiative" }
                div { class: "hero-title-row",
                    h1 {
                        span { class: "hero-rust-suffix", "Spectral" }
                        " similarity graph"
                    }
                    a {
                        class: "github-badge",
                        href: "https://github.com/LucaCappelletti94/mascot-rs",
                        target: "_blank",
                        rel: "noopener noreferrer",
                        title: "View the source code on GitHub",
                        aria_label: "View the source code on GitHub",
                        Icon { width: 16, height: 16, icon: FaGithub }
                        span { "GitHub" }
                    }
                }
                p { class: "hero-copy",
                    "Drop an "
                    a {
                        href: "http://www.matrixscience.com/help/data_file_help.html",
                        target: "_blank",
                        rel: "noopener noreferrer",
                        "MGF"
                    }
                    " file to build a spectral similarity graph. Each spectrum is a node, and edges connect spectra whose fragments match under the measure you pick: "
                    a {
                        href: "https://doi.org/10.1016/1044-0305(94)87009-8",
                        target: "_blank",
                        rel: "noopener noreferrer",
                        "cosine"
                    }
                    ", "
                    a {
                        href: "https://doi.org/10.1038/nbt.3597",
                        target: "_blank",
                        rel: "noopener noreferrer",
                        "modified cosine"
                    }
                    ", "
                    a {
                        href: "https://doi.org/10.1038/s41592-021-01331-z",
                        target: "_blank",
                        rel: "noopener noreferrer",
                        "spectral entropy"
                    }
                    ", or its modified variant. Neighbours come from a "
                    a {
                        href: "https://doi.org/10.1038/s41592-023-02012-9",
                        target: "_blank",
                        rel: "noopener noreferrer",
                        "flash-entropy-inspired"
                    }
                    " index, keeping the top-k links above a score threshold. The graph is laid out with "
                    a {
                        href: "https://doi.org/10.1371/journal.pone.0098679",
                        target: "_blank",
                        rel: "noopener noreferrer",
                        "ForceAtlas2"
                    }
                    " and split into communities by "
                    a {
                        href: "https://doi.org/10.1088/1742-5468/2008/10/P10008",
                        target: "_blank",
                        rel: "noopener noreferrer",
                        "Louvain"
                    }
                    " and "
                    a {
                        href: "https://doi.org/10.1038/s41598-019-41695-z",
                        target: "_blank",
                        rel: "noopener noreferrer",
                        "Leiden"
                    }
                    ", which you can map onto node colour alongside metadata such as precursor m/z, charge, and ion mode. Duplicate spectra are flagged by their "
                    a {
                        href: "https://doi.org/10.1038/nbt.3689",
                        target: "_blank",
                        rel: "noopener noreferrer",
                        "SPLASH"
                    }
                    ". Everything runs in your browser through "
                    a {
                        href: "https://webassembly.org/",
                        target: "_blank",
                        rel: "noopener noreferrer",
                        "WebAssembly"
                    }
                    " with no upload, ensuring complete data privacy, and you can export the node and edge tables, with all four scores, as CSV, TSV, or Parquet."
                }
            }
            LoaderPanel {}
            DatasetView {}
            GraphControls {}
            GraphCanvas {}
        }
        InfoPanel {}
        EdgePanel {}
    }
}

/// The MGF loading panel, shown only until a dataset is loaded.
#[component]
fn LoaderPanel() -> Element {
    let dataset = use_context::<Signal<DatasetState>>();
    if matches!(&*dataset.read(), DatasetState::Loaded { .. }) {
        return rsx! {};
    }
    rsx! {
        div { class: "loader-wrap",
            div { class: "panel drop-panel",
                DropZone {}
            }
        }
    }
}

/// A drag-and-drop target with a file-input fallback.
#[component]
fn DropZone() -> Element {
    let state = use_context::<Signal<DatasetState>>();
    let graph = use_context::<Signal<GraphState>>();
    let mut hovering = use_signal(|| false);
    rsx! {
        div {
            class: if hovering() { "drop-zone hovering" } else { "drop-zone" },
            ondragover: move |evt| {
                evt.prevent_default();
                hovering.set(true);
            },
            ondragleave: move |_| hovering.set(false),
            ondrop: move |evt| {
                evt.prevent_default();
                hovering.set(false);
                load_from_files(state, graph, evt.files());
            },
            p { "Drag and drop an .mgf file here" }
            div { class: "drop-actions",
                label {
                    class: "file-button",
                    title: "Open a file picker to load an MGF file from your computer.",
                    Icon { width: 16, height: 16, icon: FaFolderOpen }
                    span { "Choose MGF file" }
                    input {
                        class: "file-input",
                        r#type: "file",
                        accept: ".mgf,.txt",
                        aria_label: "Load an MGF file from your computer",
                        onchange: move |evt| load_from_files(state, graph, evt.files()),
                    }
                }
                button {
                    r#type: "button",
                    class: "button-green",
                    title: "Load a bundled example MGF (a SIRIUS-derived feature list) without uploading a file.",
                    aria_label: "Load the bundled example MGF file",
                    onclick: move |_| load_dataset(state, graph, EXAMPLE_NAME.to_string(), EXAMPLE_MGF),
                    Icon { width: 16, height: 16, icon: FaFlask }
                    span { "Load example" }
                }
            }
        }
    }
}

/// Renders the current dataset state below the drop zone.
#[component]
fn DatasetView() -> Element {
    let state = use_context::<Signal<DatasetState>>();
    let rendered = match &*state.read() {
        DatasetState::Empty => rsx! {},
        DatasetState::Loading { name } => rsx! {
            div { class: "panel",
                p { "Loading {name}..." }
            }
        },
        DatasetState::Failed { name, error } => rsx! {
            div { class: "panel",
                p { class: "error", "Failed to load {name}: {error}" }
            }
        },
        DatasetState::Loaded { .. } => rsx! {},
    };
    rendered
}

/// Controls for choosing the similarity measure and edge thresholds, plus the
/// build trigger. Renders nothing until a dataset is loaded.
#[component]
fn GraphControls() -> Element {
    let dataset = use_context::<Signal<DatasetState>>();
    let mut params = use_context::<Signal<GraphParams>>();
    let graph = use_context::<Signal<GraphState>>();
    let focus = use_context::<NodeFocus>();
    // Rebuild the graph (debounced) whenever the dataset or parameters change.
    // `use_resource` cancels the in-flight future when a tracked dependency
    // changes, so the leading delay coalesces rapid edits into a single build.
    // `params()` and `dataset` are read before the first `.await` so they are
    // tracked. Colour-scheme changes are handled by the renderer, not here.
    let build = use_resource(move || async move {
        let parameters = params();
        let _ = dataset.read();
        gloo_timers::future::TimeoutFuture::new(200).await;
        dataset
            .peek()
            .records()
            .map(|records| build_graph(records.as_ref(), &parameters))
    });

    // Reflect the build outcome into the shared graph state.
    use_effect(move || {
        let mut graph = graph;
        match &*build.read() {
            Some(Some(Ok(built))) => graph.set(GraphState::Built(built.clone())),
            Some(Some(Err(error))) => graph.set(GraphState::Error(error.clone())),
            _ => {}
        }
    });

    let loaded = dataset.read();
    let DatasetState::Loaded { name, summary, .. } = &*loaded else {
        return rsx! {};
    };
    let name = name.clone();
    let summary = summary.clone();
    drop(loaded);
    let current = params();
    let on_reset = move |_| {
        let mut selected = focus.selected;
        let mut hovered = focus.hovered;
        let mut graph = graph;
        let mut dataset = dataset;
        selected.set(None);
        hovered.set(None);
        graph.set(GraphState::None);
        dataset.set(DatasetState::Empty);
    };

    rsx! {
        section { class: "panel",
            div { class: "dataset-bar",
                span {
                    class: "dataset-name",
                    title: "The loaded MGF file.",
                    aria_label: "Loaded MGF file: {name}",
                    Icon { width: 18, height: 18, fill: "#b6792f", icon: FaVial }
                    span { "{name}" }
                }
                span {
                    class: "meta-pill",
                    title: "Number of spectra (records) parsed from the MGF file.",
                    strong { "{summary.count}" }
                    " records"
                }
                if summary.skipped > 0 {
                    span {
                        class: "meta-pill warn",
                        title: "Malformed ion blocks skipped during parsing.",
                        strong { "{summary.skipped}" }
                        " skipped"
                    }
                }
                span {
                    class: if summary.duplicate_splash > 0 { "meta-pill warn" } else { "meta-pill" },
                    title: "Records whose SPLASH identifier matches an earlier record (likely duplicate spectra).",
                    strong { "{summary.duplicate_splash}" }
                    " duplicate SPLASH"
                }
                span {
                    class: if summary.shared_pepmass > 0 { "meta-pill warn" } else { "meta-pill" },
                    title: "Records whose precursor m/z (pepmass) matches an earlier record.",
                    strong { "{summary.shared_pepmass}" }
                    " shared m/z"
                }
                button {
                    r#type: "button",
                    class: "reset-button",
                    title: "Clear this dataset and load another MGF.",
                    aria_label: "Clear the dataset and return to the loading screen",
                    onclick: on_reset,
                    Icon { width: 16, height: 16, icon: FaXmark }
                }
            }
            div { class: "controls",
                div { class: "field field-wide",
                    label { title: "The spectral similarity measure used to score and weight edges between spectra.", "Similarity" }
                    div { class: "segmented", role: "group", aria_label: "Spectral similarity measure",
                        for method in SimilarityMethod::ALL {
                            button {
                                key: "{method.id()}",
                                r#type: "button",
                                class: if method == current.method { "segment active" } else { "segment" },
                                style: "--accent: {method.accent()};",
                                aria_pressed: if method == current.method { "true" } else { "false" },
                                title: method.description(),
                                aria_label: method.description(),
                                onclick: move |_| params.with_mut(|p| p.method = method),
                                {similarity_icon(method, if method == current.method { "#ffffff".to_string() } else { method.accent().to_string() })}
                                span { "{method.label()}" }
                            }
                        }
                    }
                }
                div { class: "field",
                    label { title: "Mass tolerance in daltons for matching fragment peaks. Larger values match peaks more loosely and merge nearby ones.", "m/z tolerance (Da)" }
                    input {
                        r#type: "number",
                        min: "0",
                        step: "0.001",
                        title: "Mass tolerance in daltons for matching fragment peaks. Larger values match peaks more loosely and merge nearby ones.",
                        aria_label: "m/z matching tolerance in daltons. Larger values match peaks more loosely.",
                        value: "{current.mz_tolerance}",
                        onchange: move |evt| {
                            if let Ok(value) = evt.value().parse::<f64>() {
                                params.with_mut(|p| p.mz_tolerance = value.max(0.0));
                            }
                        },
                    }
                }
                div { class: "field",
                    label { title: "Exponent applied to peak m/z when weighting matches. 0 ignores m/z; higher values give more weight to high-m/z fragments.", "m/z power" }
                    input {
                        r#type: "number",
                        min: "0",
                        step: "0.1",
                        title: "Exponent applied to peak m/z when weighting matches. 0 ignores m/z; higher values give more weight to high-m/z fragments.",
                        aria_label: "m/z weighting exponent. 0 ignores m/z; higher values emphasise high-m/z fragments.",
                        value: "{current.mz_power}",
                        onchange: move |evt| {
                            if let Ok(value) = evt.value().parse::<f64>() {
                                params.with_mut(|p| p.mz_power = value.max(0.0));
                            }
                        },
                    }
                }
                div { class: "field",
                    label { title: "Exponent applied to peak intensity. A small intensity exponent increases the importance of smaller peaks; 1 uses raw intensity.", "Intensity power" }
                    input {
                        r#type: "number",
                        min: "0",
                        step: "0.1",
                        title: "Exponent applied to peak intensity. A small intensity exponent increases the importance of smaller peaks; 1 uses raw intensity.",
                        aria_label: "Intensity weighting exponent. A small value increases the importance of smaller peaks.",
                        value: "{current.intensity_power}",
                        onchange: move |evt| {
                            if let Ok(value) = evt.value().parse::<f64>() {
                                params.with_mut(|p| p.intensity_power = value.max(0.0));
                            }
                        },
                    }
                }
                div { class: "field",
                    label { title: "How many of the most similar neighbours each spectrum links to. Higher values produce a denser graph.", "Top-k neighbours" }
                    input {
                        r#type: "number",
                        min: "1",
                        step: "1",
                        title: "How many of the most similar neighbours each spectrum links to. Higher values produce a denser graph.",
                        aria_label: "Number of nearest neighbours per spectrum. Higher values produce a denser graph.",
                        value: "{current.top_k}",
                        onchange: move |evt| {
                            if let Ok(value) = evt.value().parse::<usize>() {
                                params.with_mut(|p| p.top_k = value.max(1));
                            }
                        },
                    }
                }
                div { class: "field",
                    label { title: "Minimum similarity (0 to 1) an edge must reach to be kept. Higher values keep only strong links.", "Min score" }
                    input {
                        r#type: "number",
                        min: "0",
                        max: "1",
                        step: "0.01",
                        title: "Minimum similarity (0 to 1) an edge must reach to be kept. Higher values keep only strong links.",
                        aria_label: "Minimum similarity score for an edge. Higher values keep only strong links.",
                        value: "{current.min_score}",
                        onchange: move |evt| {
                            if let Ok(value) = evt.value().parse::<f64>() {
                                params.with_mut(|p| p.min_score = value.clamp(0.0, 1.0));
                            }
                        },
                    }
                }
            }
            if let GraphState::Error(error) = &*graph.read() {
                p { class: "error", "Graph build failed: {error}" }
            }
        }
    }
}

/// Derived colouring for the current graph, recomputed only when the graph,
/// colour scheme, or dataset changes.
#[derive(Clone, PartialEq)]
struct GraphColoring {
    /// Colour schemes that actually distinguish nodes.
    available: Vec<ColorScheme>,
    /// The scheme used to colour (the selection, or the first available one if
    /// the selection has become trivial).
    effective: ColorScheme,
    /// Per-node colours, groups, and the legend.
    coloring: coloring::Coloring,
}

/// Pannable, zoomable canvas view of the built similarity graph, with a
/// colour-scheme selector and a legend.
#[component]
fn GraphCanvas() -> Element {
    let graph = use_context::<Signal<GraphState>>();
    let dataset = use_context::<Signal<DatasetState>>();
    let mut scheme = use_context::<Signal<ColorScheme>>();
    let focus = use_context::<NodeFocus>();
    let edge_focus = use_context::<EdgeFocus>();
    let mut selected = focus.selected;
    let mut hovered = focus.hovered;
    let mut edge_selected = edge_focus.selected;
    let mut edge_hovered = edge_focus.hovered;
    let mut view = use_signal(render::ViewTransform::default);
    // Live node positions (mutated by dragging) and the fixed reference layout
    // that anchors the projection so dragging does not rescale the view.
    let mut positions = use_signal(Vec::<[f64; 2]>::new);
    let mut reference = use_signal(Vec::<[f64; 2]>::new);
    let mut drag = use_signal(|| Drag::None);
    let mut hovered_edge = use_signal(|| None::<EdgeHover>);

    // Derive the available schemes, effective scheme, and colouring once per
    // (graph, scheme, dataset) change; shared by the body and the redraw effect.
    let coloring_memo = use_memo(move || -> Option<GraphColoring> {
        let graph_ref = graph.read();
        let GraphState::Built(built) = &*graph_ref else {
            return None;
        };
        let current = scheme();
        let dataset_ref = dataset.read();
        let records: &[MascotGenericFormat] = dataset_ref.records().map_or(&[], AsRef::as_ref);
        let available: Vec<ColorScheme> = ColorScheme::ALL
            .into_iter()
            .filter(|candidate| coloring::is_informative(*candidate, built, records))
            .collect();
        let effective = if available.contains(&current) {
            current
        } else {
            available.first().copied().unwrap_or(current)
        };
        let coloring = coloring::compute(effective, built, records);
        Some(GraphColoring {
            available,
            effective,
            coloring,
        })
    });

    // Reset positions, view, and selection whenever a new graph is built.
    use_effect(move || {
        let graph_ref = graph.read();
        view.set(render::ViewTransform::default());
        selected.set(None);
        hovered.set(None);
        edge_selected.set(None);
        edge_hovered.set(None);
        if let GraphState::Built(built) = &*graph_ref {
            positions.set(built.coordinates.clone());
            reference.set(built.coordinates.clone());
        } else {
            positions.set(Vec::new());
            reference.set(Vec::new());
        }
    });

    // Redraw whenever the graph, colouring, view, positions, or focus change.
    use_effect(move || {
        let transform = view();
        let highlight = focus.active();
        let active_edge = edge_focus.active();
        let positions_snapshot = positions();
        let memo = coloring_memo.read();
        let graph_ref = graph.read();
        if let (GraphState::Built(built), Some(view_coloring)) = (&*graph_ref, memo.as_ref()) {
            let highlight_edge = active_edge
                .and_then(|index| built.edges.get(index))
                .map(|&(u, v, _)| (u, v));
            render::draw_graph(
                built,
                &positions_snapshot,
                transform,
                &view_coloring.coloring.colors,
                &view_coloring.coloring.groups,
                view_coloring.coloring.categorical,
                highlight,
                highlight_edge,
            );
        }
    });

    let graph_ref = graph.read();
    let GraphState::Built(built) = &*graph_ref else {
        return rsx! {};
    };
    let memo = coloring_memo.read();
    let Some(view_coloring) = memo.as_ref() else {
        return rsx! {};
    };
    let effective_scheme = view_coloring.effective;
    let legend_markup = match &view_coloring.coloring.legend {
        coloring::Legend::Categorical(entries) => {
            let shown = entries.len().min(24);
            let extra = entries.len().saturating_sub(shown);
            rsx! {
                div { class: "legend",
                    for (index , (label , color)) in entries.iter().take(shown).enumerate() {
                        div {
                            key: "{label}",
                            class: "legend-item",
                            title: "Nodes coloured as {label}.",
                            {node_marker_svg(index, color.clone())}
                            span { class: "legend-label", "{label}" }
                        }
                    }
                    if extra > 0 {
                        div { class: "legend-item",
                            span { class: "legend-label", "+{extra} more" }
                        }
                    }
                }
            }
        }
        coloring::Legend::Continuous { min, max } => {
            let stops: Vec<String> = (0..=8)
                .map(|step| render::heat_color(f64::from(step) / 8.0))
                .collect();
            let bar_style = format!(
                "background: linear-gradient(to right, {});",
                stops.join(", ")
            );
            rsx! {
                div { class: "legend legend-continuous",
                    span { class: "legend-label", "{min:.2}" }
                    span { class: "legend-bar", style: "{bar_style}" }
                    span { class: "legend-label", "{max:.2}" }
                }
            }
        }
    };

    rsx! {
        section { class: "panel",
            div { class: "view-head",
                h2 { class: "controls-title",
                    Icon { width: 18, height: 18, fill: "#38755a", icon: FaCircleNodes }
                    span { "Graph view" }
                }
                div { class: "view-head-right",
                    div { class: "graph-stats-inline",
                        span {
                            class: "meta-pill",
                            title: "Spectra in the graph (one node per record).",
                            strong { "{built.node_count}" }
                            " nodes"
                        }
                        span {
                            class: "meta-pill",
                            title: "Similarity links kept between spectra.",
                            strong { "{built.edges.len()}" }
                            " edges"
                        }
                        span {
                            class: "meta-pill",
                            title: "Connected components: groups of spectra linked by any chain of edges.",
                            strong { "{built.component_count}" }
                            " components"
                        }
                        span {
                            class: "meta-pill",
                            title: "Number of communities found by the Louvain algorithm.",
                            strong { "{built.community_count}" }
                            " Louvain"
                        }
                        span {
                            class: "meta-pill",
                            title: "Number of communities found by the Leiden algorithm.",
                            strong { "{built.leiden_count}" }
                            " Leiden"
                        }
                    }
                    ExportControl {}
                }
            }
            div { class: "field field-wide",
                label { title: "Choose which property maps to node colour: graph structure, spectrum metadata, or a per-spectrum heatmap.", "Colour by" }
                div { class: "segmented", role: "group", aria_label: "Node colouring scheme",
                    for option in view_coloring.available.iter().copied() {
                        button {
                            key: "{option.id()}",
                            r#type: "button",
                            class: if option == effective_scheme { "segment active" } else { "segment" },
                            style: "--accent: {option.accent()};",
                            aria_pressed: if option == effective_scheme { "true" } else { "false" },
                            title: option.description(),
                            aria_label: option.description(),
                            onclick: move |_| scheme.set(option),
                            {scheme_icon(option, if option == effective_scheme { "#ffffff".to_string() } else { option.accent().to_string() })}
                            span { "{option.label()}" }
                        }
                    }
                }
            }
            p { class: "view-hint", "Hover or click a node for details, drag a node to move it, drag the background to pan, scroll to zoom." }
            div { class: "graph-canvas-wrap",
            canvas {
                id: render::CANVAS_ID,
                class: "graph-canvas",
                onmousedown: move |evt| {
                    let point = evt.element_coordinates();
                    let Some((width, height)) = render::canvas_size() else {
                        return;
                    };
                    hovered_edge.set(None);
                    let hit = render::hit_test(
                        reference.peek().as_slice(),
                        positions.peek().as_slice(),
                        *view.peek(),
                        width,
                        height,
                        point.x,
                        point.y,
                    );
                    match hit {
                        Some(index) => {
                            selected.set(Some(index));
                            edge_selected.set(None);
                            drag.set(Drag::Node { index });
                        }
                        None => {
                            // No node under the cursor: pin an edge if one is
                            // hit, otherwise clear and start panning.
                            let edge_index = if let GraphState::Built(built) = &*graph.peek() {
                                render::edge_hit_test(
                                    built,
                                    positions.peek().as_slice(),
                                    *view.peek(),
                                    width,
                                    height,
                                    point.x,
                                    point.y,
                                )
                            } else {
                                None
                            };
                            match edge_index {
                                Some(index) => {
                                    edge_selected.set(Some(index));
                                    selected.set(None);
                                    drag.set(Drag::None);
                                }
                                None => {
                                    selected.set(None);
                                    edge_selected.set(None);
                                    drag.set(Drag::Pan { x: point.x, y: point.y });
                                }
                            }
                        }
                    }
                },
                onmousemove: move |evt| {
                    let point = evt.element_coordinates();
                    let Some((width, height)) = render::canvas_size() else {
                        return;
                    };
                    let mode = *drag.peek();
                    match mode {
                        Drag::Node { index } => {
                            let world = {
                                let anchor = reference.peek();
                                let projection = render::Projection::new(
                                    anchor.as_slice(),
                                    *view.peek(),
                                    width,
                                    height,
                                );
                                projection.to_world(point.x, point.y)
                            };
                            positions.with_mut(|pos| {
                                if let Some(slot) = pos.get_mut(index) {
                                    *slot = world;
                                }
                            });
                        }
                        Drag::Pan { x, y } => {
                            view.with_mut(|transform| {
                                transform.pan_x += point.x - x;
                                transform.pan_y += point.y - y;
                            });
                            drag.set(Drag::Pan { x: point.x, y: point.y });
                        }
                        Drag::None => {
                            let node_hit = render::hit_test(
                                reference.peek().as_slice(),
                                positions.peek().as_slice(),
                                *view.peek(),
                                width,
                                height,
                                point.x,
                                point.y,
                            );
                            if *hovered.peek() != node_hit {
                                hovered.set(node_hit);
                            }
                            // Nodes take precedence; only probe edges off a node.
                            let edge_index = if node_hit.is_some() {
                                None
                            } else if let GraphState::Built(built) = &*graph.peek() {
                                render::edge_hit_test(
                                    built,
                                    positions.peek().as_slice(),
                                    *view.peek(),
                                    width,
                                    height,
                                    point.x,
                                    point.y,
                                )
                            } else {
                                None
                            };
                            // Cursor-following tooltip carries the live position.
                            let tooltip = edge_index.and_then(|index| {
                                if let GraphState::Built(built) = &*graph.peek() {
                                    built.edges.get(index).map(|&(_, _, score)| EdgeHover {
                                        score,
                                        x: point.x,
                                        y: point.y,
                                    })
                                } else {
                                    None
                                }
                            });
                            hovered_edge.set(tooltip);
                            // The mirror panel and canvas highlight track the index.
                            if *edge_hovered.peek() != edge_index {
                                edge_hovered.set(edge_index);
                            }
                        }
                    }
                },
                onmouseup: move |_| drag.set(Drag::None),
                onmouseleave: move |_| {
                    drag.set(Drag::None);
                    hovered.set(None);
                    hovered_edge.set(None);
                    edge_hovered.set(None);
                },
                onwheel: move |evt| {
                    evt.prevent_default();
                    let factor = if evt.delta().strip_units().y < 0.0 { 1.1 } else { 0.9 };
                    view.with_mut(|transform| {
                        transform.zoom = (transform.zoom * factor).clamp(1.0, 50.0);
                    });
                },
            }
            EdgeTooltip { hover: hovered_edge }
            }
            {legend_markup}
        }
    }
}

/// A cursor-following tooltip showing the similarity of the hovered edge.
///
/// Isolated into its own component so edge hover only re-renders this, not the
/// whole graph panel.
#[component]
fn EdgeTooltip(hover: Signal<Option<EdgeHover>>) -> Element {
    let Some(edge) = hover() else {
        return rsx! {};
    };
    rsx! {
        div {
            class: "edge-tooltip",
            style: "left: {edge.x}px; top: {edge.y}px;",
            "similarity {edge.score:.3}"
        }
    }
}

/// Right-side panel showing details and a spectrum plot for the active node.
///
/// The active node is the selection if pinned, otherwise the hovered node.
#[component]
fn InfoPanel() -> Element {
    let focus = use_context::<NodeFocus>();
    let dataset = use_context::<Signal<DatasetState>>();
    let mut selected = focus.selected;
    let mut hovered = focus.hovered;

    let Some(index) = focus.active() else {
        return rsx! {};
    };
    let dataset_ref = dataset.read();
    let Some(record) = dataset_ref
        .records()
        .and_then(|records| records.as_ref().get(index))
    else {
        return rsx! {};
    };

    let title = spectrum_title(record, index);
    let pinned = selected.read().is_some();
    let rows = node_detail_rows(record);
    let peaks = record_peaks(record);

    rsx! {
        aside { class: "node-panel",
            div { class: "node-panel-head",
                h3 { "{title}" }
                button {
                    r#type: "button",
                    class: "node-panel-close",
                    title: "Close the details panel",
                    aria_label: "Close the spectrum details panel",
                    onclick: move |_| {
                        selected.set(None);
                        hovered.set(None);
                    },
                    "\u{00d7}"
                }
            }
            if !pinned {
                p { class: "node-panel-hint", "Click the node to keep this open." }
            }
            div { class: "spectrum-wrap", SpectrumPlot { peaks } }
            dl { class: "kv",
                for (label , value) in rows {
                    dt { "{label}" }
                    dd { "{value}" }
                }
            }
        }
    }
}

/// A short display title for a spectrum: its feature id, or `Spectrum #index`.
fn spectrum_title(record: &MascotGenericFormat, index: usize) -> String {
    record
        .metadata()
        .feature_id()
        .map_or_else(|| format!("Spectrum #{index}"), str::to_string)
}

/// Collects a record's peaks as `(m/z, intensity)` pairs at f64 precision.
fn record_peaks(record: &MascotGenericFormat) -> Vec<(f64, f64)> {
    record
        .peaks()
        .map(|(mz, intensity)| (mz.to_f64(), intensity.to_f64()))
        .collect()
}

/// Right-side panel comparing the two spectra joined by the active edge as a
/// mirror (butterfly) plot. The node panel takes precedence when a node is
/// active, so the two panels never overlap.
///
/// The active edge is the selection if pinned, otherwise the hovered edge.
#[component]
fn EdgePanel() -> Element {
    let node_focus = use_context::<NodeFocus>();
    // A node panel, when shown, occupies the same space; give it precedence.
    if node_focus.active().is_some() {
        return rsx! {};
    }
    let edge_focus = use_context::<EdgeFocus>();
    let graph = use_context::<Signal<GraphState>>();
    let dataset = use_context::<Signal<DatasetState>>();
    let params = use_context::<Signal<GraphParams>>();
    let mut edge_selected = edge_focus.selected;
    let mut edge_hovered = edge_focus.hovered;

    let Some(edge_index) = edge_focus.active() else {
        return rsx! {};
    };
    let graph_ref = graph.read();
    let GraphState::Built(built) = &*graph_ref else {
        return rsx! {};
    };
    let Some(&(top_node, bottom_node, score)) = built.edges.get(edge_index) else {
        return rsx! {};
    };
    let dataset_ref = dataset.read();
    let Some(records) = dataset_ref.records().map(AsRef::as_ref) else {
        return rsx! {};
    };
    let (Some(top_record), Some(bottom_record)) = (records.get(top_node), records.get(bottom_node))
    else {
        return rsx! {};
    };

    let top_label = spectrum_title(top_record, top_node);
    let bottom_label = spectrum_title(bottom_record, bottom_node);
    let top_peaks = record_peaks(top_record);
    let bottom_peaks = record_peaks(bottom_record);
    let parameters = params();
    let tolerance = parameters.mz_tolerance;
    let active = parameters.method;
    let pinned = edge_selected.read().is_some();

    // Every similarity measure for this pair, not just the one that built the
    // graph. The active method's row shows the exact edge weight so the panel
    // agrees with the graph and the hover tooltip.
    let sim_rows: Vec<(SimilarityMethod, String, bool)> =
        crate::similarity::pairwise_similarities(top_record, bottom_record, &parameters)
            .into_iter()
            .map(|(method, value)| {
                let is_active = method == active;
                let text = if is_active {
                    format!("{score:.4}")
                } else {
                    value.map_or_else(|| "n/a".to_string(), |score| format!("{score:.4}"))
                };
                (method, text, is_active)
            })
            .collect();

    let top_mz = top_record.precursor_mz().to_f64();
    let bottom_mz = bottom_record.precursor_mz().to_f64();
    let rows = [
        ("Top precursor m/z", format!("{top_mz:.4}")),
        ("Bottom precursor m/z", format!("{bottom_mz:.4}")),
        (
            "Precursor delta",
            format!("{:.4}", (top_mz - bottom_mz).abs()),
        ),
    ];

    rsx! {
        aside { class: "node-panel edge-panel",
            div { class: "node-panel-head",
                h3 { "{top_label} vs {bottom_label}" }
                button {
                    r#type: "button",
                    class: "node-panel-close",
                    title: "Close the edge comparison panel",
                    aria_label: "Close the spectral comparison panel",
                    onclick: move |_| {
                        edge_selected.set(None);
                        edge_hovered.set(None);
                    },
                    "\u{00d7}"
                }
            }
            if !pinned {
                p { class: "node-panel-hint", "Click the edge to keep this open." }
            }
            div { class: "spectrum-wrap",
                MirrorSpectrumPlot {
                    top: top_peaks,
                    bottom: bottom_peaks,
                    tolerance,
                    top_label,
                    bottom_label,
                }
            }
            div { class: "edge-similarities",
                h4 { class: "edge-sim-title", "All similarity measures" }
                for (method , text , is_active) in sim_rows {
                    div {
                        key: "{method.id()}",
                        class: if is_active { "edge-sim-row active" } else { "edge-sim-row" },
                        title: method.description(),
                        span { class: "edge-sim-name",
                            {similarity_icon(method, if is_active { method.accent().to_string() } else { "#627077".to_string() })}
                            span { "{method.label()}" }
                        }
                        span { class: "edge-sim-value", "{text}" }
                    }
                }
            }
            dl { class: "kv",
                for (label , value) in rows {
                    dt { "{label}" }
                    dd { "{value}" }
                }
            }
        }
    }
}

/// Builds the label/value detail rows for a spectrum.
fn node_detail_rows(record: &MascotGenericFormat) -> Vec<(&'static str, String)> {
    let metadata = record.metadata();
    let mut rows = vec![
        (
            "Precursor m/z",
            format!("{:.4}", record.precursor_mz().to_f64()),
        ),
        ("Peaks", record.len().to_string()),
        (
            "Intensity entropy",
            format!("{:.3}", coloring::intensity_entropy(record)),
        ),
    ];
    if let Some(level) = metadata.level() {
        rows.push(("MS level", level.to_string()));
    }
    if let Some(charge) = metadata.charge() {
        rows.push(("Charge", charge.to_string()));
    }
    if let Some(mode) = metadata.ion_mode() {
        rows.push(("Ion mode", mode.as_str().to_string()));
    }
    if let Some(retention_time) = metadata.retention_time() {
        rows.push(("Retention time", format!("{retention_time:.2}")));
    }
    if let Some(instrument) = metadata.source_instrument() {
        rows.push(("Instrument", instrument.as_str().to_string()));
    }
    if let Some(smiles) = metadata.smiles() {
        rows.push(("SMILES", smiles.to_string()));
    }
    if let Some(splash) = metadata.splash() {
        rows.push(("SPLASH", splash.to_string()));
    }
    rows
}

/// Renders a spectrum as an SVG stick plot (m/z on x, relative intensity on y),
/// with a hover readout of the m/z and intensity of the peak under the cursor.
#[component]
fn SpectrumPlot(peaks: Vec<(f64, f64)>) -> Element {
    let mut hovered = use_signal(|| None::<usize>);
    if peaks.is_empty() {
        return rsx! {
            p { class: "node-panel-hint", "No peaks to display." }
        };
    }

    let width = 320.0_f64;
    let height = 150.0_f64;
    let pad_left = 6.0_f64;
    let pad_right = 6.0_f64;
    let pad_top = 8.0_f64;
    let pad_bottom = 18.0_f64;
    let min_mz = peaks
        .iter()
        .map(|peak| peak.0)
        .fold(f64::INFINITY, f64::min);
    let max_mz = peaks
        .iter()
        .map(|peak| peak.0)
        .fold(f64::NEG_INFINITY, f64::max);
    let max_intensity = peaks
        .iter()
        .map(|peak| peak.1)
        .fold(0.0_f64, f64::max)
        .max(f64::EPSILON);
    let span_mz = (max_mz - min_mz).max(1e-6);
    let plot_width = width - pad_left - pad_right;
    let plot_height = height - pad_top - pad_bottom;
    let baseline = height - pad_bottom;
    // (index, x, top) per peak.
    let sticks: Vec<(usize, f64, f64)> = peaks
        .iter()
        .enumerate()
        .map(|(index, &(mz, intensity))| {
            let x = pad_left + (mz - min_mz) / span_mz * plot_width;
            let top = baseline - (intensity / max_intensity) * plot_height;
            (index, x, top)
        })
        .collect();

    let current = hovered();
    let readout = match current.and_then(|index| peaks.get(index).copied()) {
        Some((mz, intensity)) => rsx! {
            span { class: "spectrum-value", "m/z {mz:.4}, intensity {intensity:.4}" }
        },
        None => rsx! {
            span { class: "node-panel-hint", "Hover a peak to read its m/z and intensity." }
        },
    };

    rsx! {
        svg {
            class: "spectrum",
            view_box: "0 0 {width} {height}",
            width: "100%",
            onmouseleave: move |_| hovered.set(None),
            line {
                x1: "{pad_left}",
                y1: "{baseline}",
                x2: "{width - pad_right}",
                y2: "{baseline}",
                stroke: "#9aa0b0",
                stroke_width: "1",
            }
            for (index , x , top) in sticks.iter().copied() {
                line {
                    x1: "{x}",
                    y1: "{top}",
                    x2: "{x}",
                    y2: "{baseline}",
                    stroke: if current == Some(index) { "#9d4133" } else { "#205e8c" },
                    stroke_width: if current == Some(index) { "2" } else { "1" },
                }
            }
            for (index , x , _top) in sticks.iter().copied() {
                line {
                    x1: "{x}",
                    y1: "{pad_top}",
                    x2: "{x}",
                    y2: "{baseline}",
                    stroke: "rgba(0,0,0,0)",
                    stroke_width: "6",
                    style: "pointer-events: all;",
                    onmouseenter: move |_| hovered.set(Some(index)),
                }
            }
            text { x: "{pad_left}", y: "{height - 4.0}", font_size: "9", fill: "#627077", "{min_mz:.1}" }
            text {
                x: "{width - pad_right}",
                y: "{height - 4.0}",
                font_size: "9",
                fill: "#627077",
                text_anchor: "end",
                "{max_mz:.1}"
            }
        }
        div { class: "spectrum-readout", {readout} }
    }
}

/// Renders two spectra as a mirror (butterfly) plot: the `top` spectrum's peaks
/// rise above a shared m/z axis and the `bottom` spectrum's peaks hang below it.
///
/// Each spectrum is scaled to its own most intense peak, so the comparison is of
/// relative intensities. Fragment peaks that match a peak in the other spectrum
/// within `tolerance` daltons are drawn in the match colour, which is the visual
/// shorthand for why the two spectra are considered similar.
#[component]
fn MirrorSpectrumPlot(
    top: Vec<(f64, f64)>,
    bottom: Vec<(f64, f64)>,
    tolerance: f64,
    top_label: String,
    bottom_label: String,
) -> Element {
    // The hovered peak, as (is_top, peak index).
    let mut hovered = use_signal(|| None::<(bool, usize)>);
    if top.is_empty() && bottom.is_empty() {
        return rsx! {
            p { class: "node-panel-hint", "No peaks to display." }
        };
    }

    let width = 340.0_f64;
    let height = 220.0_f64;
    let pad_left = 6.0_f64;
    let pad_right = 6.0_f64;
    let pad_top = 12.0_f64;
    let pad_bottom = 22.0_f64;
    let center_y = (pad_top + (height - pad_bottom)) / 2.0;
    // Each half spans from the centre axis to its padding edge.
    let half = center_y - pad_top;
    let plot_width = width - pad_left - pad_right;

    let min_mz = top
        .iter()
        .chain(bottom.iter())
        .map(|peak| peak.0)
        .fold(f64::INFINITY, f64::min);
    let max_mz = top
        .iter()
        .chain(bottom.iter())
        .map(|peak| peak.0)
        .fold(f64::NEG_INFINITY, f64::max);
    let span_mz = (max_mz - min_mz).max(1e-6);
    let max_top = top
        .iter()
        .map(|peak| peak.1)
        .fold(0.0_f64, f64::max)
        .max(f64::EPSILON);
    let max_bottom = bottom
        .iter()
        .map(|peak| peak.1)
        .fold(0.0_f64, f64::max)
        .max(f64::EPSILON);

    let matches = |mz: f64, other: &[(f64, f64)]| {
        other
            .iter()
            .any(|&(other_mz, _)| (mz - other_mz).abs() <= tolerance)
    };

    // Per peak: (index, x, y at the stick tip, matched).
    let sticks_top: Vec<(usize, f64, f64, bool)> = top
        .iter()
        .enumerate()
        .map(|(index, &(mz, intensity))| {
            let x = pad_left + (mz - min_mz) / span_mz * plot_width;
            let tip = center_y - (intensity / max_top) * half;
            (index, x, tip, matches(mz, &bottom))
        })
        .collect();
    let sticks_bottom: Vec<(usize, f64, f64, bool)> = bottom
        .iter()
        .enumerate()
        .map(|(index, &(mz, intensity))| {
            let x = pad_left + (mz - min_mz) / span_mz * plot_width;
            let tip = center_y + (intensity / max_bottom) * half;
            (index, x, tip, matches(mz, &top))
        })
        .collect();
    let match_count = sticks_top.iter().filter(|stick| stick.3).count();

    let top_color = "#205e8c";
    let bottom_color = "#38755a";
    let match_color = "#9d4133";

    let current = hovered();
    let readout = match current.and_then(|(is_top, index)| {
        let source = if is_top { &top } else { &bottom };
        source
            .get(index)
            .copied()
            .map(|(mz, intensity)| (is_top, mz, intensity))
    }) {
        Some((is_top, mz, intensity)) => {
            let side = if is_top { "top" } else { "bottom" };
            rsx! {
                span { class: "spectrum-value", "{side}: m/z {mz:.4}, intensity {intensity:.4}" }
            }
        }
        None => rsx! {
            span { class: "node-panel-hint", "Hover a peak to read its m/z and intensity." }
        },
    };

    rsx! {
        svg {
            class: "spectrum",
            view_box: "0 0 {width} {height}",
            width: "100%",
            onmouseleave: move |_| hovered.set(None),
            line {
                x1: "{pad_left}",
                y1: "{center_y}",
                x2: "{width - pad_right}",
                y2: "{center_y}",
                stroke: "#9aa0b0",
                stroke_width: "1",
            }
            for (index , x , tip , matched) in sticks_top.iter().copied() {
                line {
                    x1: "{x}",
                    y1: "{center_y}",
                    x2: "{x}",
                    y2: "{tip}",
                    stroke: if matched { match_color } else { top_color },
                    stroke_width: if current == Some((true, index)) { "3" } else if matched { "2" } else { "1" },
                }
            }
            for (index , x , tip , matched) in sticks_bottom.iter().copied() {
                line {
                    x1: "{x}",
                    y1: "{center_y}",
                    x2: "{x}",
                    y2: "{tip}",
                    stroke: if matched { match_color } else { bottom_color },
                    stroke_width: if current == Some((false, index)) { "3" } else if matched { "2" } else { "1" },
                }
            }
            for (index , x , _tip , _matched) in sticks_top.iter().copied() {
                line {
                    x1: "{x}",
                    y1: "{pad_top}",
                    x2: "{x}",
                    y2: "{center_y}",
                    stroke: "rgba(0,0,0,0)",
                    stroke_width: "6",
                    style: "pointer-events: all;",
                    onmouseenter: move |_| hovered.set(Some((true, index))),
                }
            }
            for (index , x , _tip , _matched) in sticks_bottom.iter().copied() {
                line {
                    x1: "{x}",
                    y1: "{center_y}",
                    x2: "{x}",
                    y2: "{height - pad_bottom}",
                    stroke: "rgba(0,0,0,0)",
                    stroke_width: "6",
                    style: "pointer-events: all;",
                    onmouseenter: move |_| hovered.set(Some((false, index))),
                }
            }
            text { x: "{pad_left}", y: "{height - 6.0}", font_size: "9", fill: "#627077", "{min_mz:.1}" }
            text {
                x: "{width - pad_right}",
                y: "{height - 6.0}",
                font_size: "9",
                fill: "#627077",
                text_anchor: "end",
                "{max_mz:.1}"
            }
        }
        div { class: "spectrum-readout", {readout} }
        div { class: "mirror-legend",
            div { class: "mirror-legend-item",
                span { class: "mirror-swatch", style: "background: {top_color};" }
                span { class: "legend-label", "{top_label}" }
            }
            div { class: "mirror-legend-item",
                span { class: "mirror-swatch", style: "background: {bottom_color};" }
                span { class: "legend-label", "{bottom_label}" }
            }
            div { class: "mirror-legend-item",
                span { class: "mirror-swatch", style: "background: {match_color};" }
                span { class: "legend-label", "{match_count} matched" }
            }
        }
    }
}

/// The dataset name without its file extension, for building export filenames.
fn file_base(name: &str) -> &str {
    name.rsplit_once('.').map_or(name, |(stem, _)| stem)
}

/// Tabs in the export dialog.
#[derive(Clone, Copy, PartialEq)]
enum ExportTab {
    /// File format selection.
    Format,
    /// Node-list columns.
    Nodes,
    /// Edge-list options.
    Edges,
}

impl ExportTab {
    /// All tabs, in display order.
    const ALL: [Self; 3] = [Self::Format, Self::Nodes, Self::Edges];

    /// A stable identifier used as a list key.
    const fn id(self) -> &'static str {
        match self {
            Self::Format => "format",
            Self::Nodes => "nodes",
            Self::Edges => "edges",
        }
    }

    /// A human-readable tab label.
    const fn label(self) -> &'static str {
        match self {
            Self::Format => "Format",
            Self::Nodes => "Nodes",
            Self::Edges => "Edges",
        }
    }
}

/// The graph view's "Export" button and its configuration dialog.
///
/// The dialog is a self-contained modal with tabs for the file format, node
/// columns, and edge options, plus the two download actions. Its settings only
/// affect the downloaded files, never the graph or any other panel. Rendered
/// inside the graph view, so it only appears once a graph is built.
#[component]
fn ExportControl() -> Element {
    let dataset = use_context::<Signal<DatasetState>>();
    let graph = use_context::<Signal<GraphState>>();
    let params = use_context::<Signal<GraphParams>>();

    let mut open = use_signal(|| false);
    let mut tab = use_signal(|| ExportTab::Format);
    let mut format = use_signal(|| export::OutputFormat::Csv);
    let mut node_columns = use_signal(|| export::NodeColumn::DEFAULTS.to_vec());
    let mut endpoint = use_signal(|| export::EndpointId::Index);
    let mut weights = use_signal(|| SimilarityMethod::ALL.to_vec());

    let dataset_ref = dataset.read();
    let DatasetState::Loaded { records, .. } = &*dataset_ref else {
        return rsx! {};
    };
    let graph_ref = graph.read();
    let GraphState::Built(built) = &*graph_ref else {
        return rsx! {};
    };

    let feature_issue = export::feature_id_issue(records.as_ref());
    let endpoint_value = endpoint();
    let blocked_edges = endpoint_value.uses_feature_id() && feature_issue.is_some();
    let node_count = built.node_count;
    let edge_count = built.edges.len();
    let active_tab = tab();
    drop(graph_ref);
    drop(dataset_ref);

    let on_download_nodes = move |_| {
        let dataset_ref = dataset.read();
        let DatasetState::Loaded { name, records, .. } = &*dataset_ref else {
            return;
        };
        let graph_ref = graph.read();
        let GraphState::Built(built) = &*graph_ref else {
            return;
        };
        let output = format();
        let columns: Vec<export::NodeColumn> = export::NodeColumn::ALL
            .into_iter()
            .filter(|column| node_columns.read().contains(column))
            .collect();
        let filename = format!("{}-nodes.{}", file_base(name), output.extension());
        match output.delimiter() {
            Some(delimiter) => {
                let table = export::build_node_table(records.as_ref(), built, &columns, delimiter);
                export::download_text(&filename, output.mime(), &table);
            }
            None => {
                if let Ok(bytes) = export::build_node_parquet(records.as_ref(), built, &columns) {
                    export::download_bytes(&filename, output.mime(), &bytes);
                }
            }
        }
    };

    let on_download_edges = move |_| {
        let dataset_ref = dataset.read();
        let DatasetState::Loaded { name, records, .. } = &*dataset_ref else {
            return;
        };
        let graph_ref = graph.read();
        let GraphState::Built(built) = &*graph_ref else {
            return;
        };
        let output = format();
        let parameters = params();
        let measures: Vec<SimilarityMethod> = SimilarityMethod::ALL
            .into_iter()
            .filter(|method| weights.read().contains(method))
            .collect();
        let filename = format!("{}-edges.{}", file_base(name), output.extension());
        match output.delimiter() {
            Some(delimiter) => {
                let table = export::build_edge_table(
                    records.as_ref(),
                    built,
                    &parameters,
                    endpoint(),
                    &measures,
                    delimiter,
                );
                export::download_text(&filename, output.mime(), &table);
            }
            None => {
                if let Ok(bytes) = export::build_edge_parquet(
                    records.as_ref(),
                    built,
                    &parameters,
                    endpoint(),
                    &measures,
                ) {
                    export::download_bytes(&filename, output.mime(), &bytes);
                }
            }
        }
    };

    rsx! {
        button {
            r#type: "button",
            class: "export-open",
            title: "Export the graph as node and edge tables.",
            aria_label: "Open the export options",
            onclick: move |_| open.set(true),
            Icon { width: 15, height: 15, icon: FaDownload }
            span { "Export" }
        }
        if open() {
            div {
                class: "modal-overlay",
                onclick: move |_| open.set(false),
                div {
                    class: "modal",
                    role: "dialog",
                    aria_modal: "true",
                    aria_label: "Export options",
                    onclick: move |evt| evt.stop_propagation(),
                    div { class: "modal-head",
                        h3 { "Export graph" }
                        button {
                            r#type: "button",
                            class: "node-panel-close",
                            title: "Close",
                            aria_label: "Close the export dialog",
                            onclick: move |_| open.set(false),
                            "\u{00d7}"
                        }
                    }
                    p { class: "export-note",
                        "These options only affect the files you download. They do not change the graph or any other panel."
                    }
                    div { class: "export-tabs", role: "tablist",
                        for option in ExportTab::ALL {
                            button {
                                key: "{option.id()}",
                                r#type: "button",
                                class: if option == active_tab { "export-tab active" } else { "export-tab" },
                                role: "tab",
                                aria_selected: if option == active_tab { "true" } else { "false" },
                                onclick: move |_| tab.set(option),
                                "{option.label()}"
                            }
                        }
                    }
                    div { class: "export-tab-body",
                        {match active_tab {
                            ExportTab::Format => rsx! {
                                div { class: "field field-wide",
                                    label { "File format" }
                                    div { class: "segmented", role: "group", aria_label: "Export file format",
                                        for option in export::OutputFormat::ALL {
                                            button {
                                                key: "{option.id()}",
                                                r#type: "button",
                                                class: if option == format() { "segment active" } else { "segment" },
                                                aria_pressed: if option == format() { "true" } else { "false" },
                                                onclick: move |_| format.set(option),
                                                span { "{option.label()}" }
                                            }
                                        }
                                    }
                                    p { class: "view-hint",
                                        "CSV and TSV are plain-text tables. Parquet is a compact, typed columnar file for data tools."
                                    }
                                }
                            },
                            ExportTab::Nodes => rsx! {
                                div { class: "field field-wide",
                                    label { "Columns" }
                                    div { class: "export-grid",
                                        span {
                                            class: "export-check",
                                            title: "Always included as the join key for the edge list.",
                                            input { r#type: "checkbox", checked: true, disabled: true, aria_label: "node_id (always included)" }
                                            span { "node_id" }
                                        }
                                        for column in export::NodeColumn::ALL {
                                            label {
                                                key: "{column.id()}",
                                                class: "export-check",
                                                input {
                                                    r#type: "checkbox",
                                                    checked: node_columns.read().contains(&column),
                                                    aria_label: "{column.label()}",
                                                    onchange: move |_| {
                                                        node_columns
                                                            .with_mut(|list| {
                                                                if let Some(position)
                                                                    = list.iter().position(|candidate| *candidate == column)
                                                                {
                                                                    list.remove(position);
                                                                } else {
                                                                    list.push(column);
                                                                }
                                                            });
                                                    },
                                                }
                                                span { "{column.label()}" }
                                            }
                                        }
                                    }
                                    p { class: "view-hint",
                                        "node_id is always included as the join key. mgf_metadata is the raw MGF header passed through verbatim; the rest are computed by the app."
                                    }
                                }
                            },
                            ExportTab::Edges => rsx! {
                                div { class: "field field-wide",
                                    label { "Endpoint identifier" }
                                    div { class: "segmented", role: "group", aria_label: "How edges reference their endpoints",
                                        for option in export::EndpointId::ALL {
                                            button {
                                                key: "{option.id()}",
                                                r#type: "button",
                                                class: if option == endpoint_value { "segment active" } else { "segment" },
                                                aria_pressed: if option == endpoint_value { "true" } else { "false" },
                                                onclick: move |_| endpoint.set(option),
                                                span { "{option.label()}" }
                                            }
                                        }
                                    }
                                }
                                div { class: "field field-wide",
                                    label { "Weight columns" }
                                    div { class: "export-grid",
                                        for method in SimilarityMethod::ALL {
                                            label {
                                                key: "{method.id()}",
                                                class: "export-check",
                                                title: method.description(),
                                                input {
                                                    r#type: "checkbox",
                                                    checked: weights.read().contains(&method),
                                                    aria_label: "{method.label()}",
                                                    onchange: move |_| {
                                                        weights
                                                            .with_mut(|list| {
                                                                if let Some(position)
                                                                    = list.iter().position(|candidate| *candidate == method)
                                                                {
                                                                    list.remove(position);
                                                                } else {
                                                                    list.push(method);
                                                                }
                                                            });
                                                    },
                                                }
                                                span { "{method.label()}" }
                                            }
                                        }
                                    }
                                    if blocked_edges {
                                        if let Some(message) = feature_issue.clone() {
                                            p { class: "error",
                                                "{message} Switch the endpoint identifier to Index, or give every record a unique feature id, to export edges."
                                            }
                                        }
                                    }
                                }
                            },
                        }}
                    }
                    div { class: "export-actions",
                        button {
                            r#type: "button",
                            class: "button-green",
                            title: "Download the node list ({node_count} rows).",
                            aria_label: "Download the node list",
                            onclick: on_download_nodes,
                            Icon { width: 16, height: 16, icon: FaDownload }
                            span { "Download nodes ({node_count})" }
                        }
                        button {
                            r#type: "button",
                            class: "button-green",
                            disabled: blocked_edges,
                            title: "Download the weighted edge list ({edge_count} rows).",
                            aria_label: "Download the weighted edge list",
                            onclick: on_download_edges,
                            Icon { width: 16, height: 16, icon: FaDownload }
                            span { "Download edges ({edge_count})" }
                        }
                    }
                }
            }
        }
    }
}

/// Parses MGF `text` into the loaded dataset state.
///
/// Resets any previously built graph, whose node indices would no longer match
/// the new dataset.
fn load_dataset(
    mut state: Signal<DatasetState>,
    mut graph: Signal<GraphState>,
    name: String,
    text: &str,
) {
    graph.set(GraphState::None);
    let (records, skipped) = dataset::parse_mgf(text);
    let summary = dataset::summarize(&records, skipped);
    state.set(DatasetState::Loaded {
        name,
        records,
        summary,
    });
}

/// Reads the first dropped file asynchronously, then parses it.
fn load_from_files(
    mut state: Signal<DatasetState>,
    mut graph: Signal<GraphState>,
    files: Vec<FileData>,
) {
    let Some(file) = files.into_iter().next() else {
        return;
    };
    graph.set(GraphState::None);
    let name = file.name();
    state.set(DatasetState::Loading { name: name.clone() });
    spawn(async move {
        let mut state = state;
        match file.read_string().await {
            Ok(text) => load_dataset(state, graph, name, &text),
            Err(error) => {
                state.set(DatasetState::Failed {
                    name,
                    error: error.to_string(),
                });
            }
        }
    });
}
