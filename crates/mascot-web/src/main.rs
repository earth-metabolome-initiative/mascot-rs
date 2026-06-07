//! Dioxus web app to visualize MGF spectra as a spectral similarity graph.

mod coloring;
mod dataset;
mod layout;
mod render;
mod similarity;

use dioxus::html::{FileData, HasFileData};
use dioxus::prelude::*;
use dioxus_free_icons::icons::fa_solid_icons::{
    FaBolt, FaChartArea, FaChartColumn, FaCircleNodes, FaFlask, FaFolderOpen, FaHashtag,
    FaLayerGroup, FaMicroscope, FaPlusMinus, FaRightLeft, FaShareNodes, FaShuffle, FaUsers, FaVial,
    FaWaveSquare, FaWeightHanging,
};
use dioxus_free_icons::Icon;

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

/// Renders the icon for a colour scheme, with the given fill colour.
fn scheme_icon(scheme: ColorScheme, fill: String) -> Element {
    match scheme {
        ColorScheme::Component => {
            rsx! { Icon { width: 14, height: 14, fill, icon: FaCircleNodes } }
        }
        ColorScheme::Community => rsx! { Icon { width: 14, height: 14, fill, icon: FaUsers } },
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
use mascot_rs::prelude::{MascotGenericFormat, Spectrum, SpectrumFloat};

use crate::coloring::ColorScheme;
use crate::dataset::{Breakdown, DatasetState, DatasetSummary};
use crate::similarity::{build_graph, GraphParams, SimilarityGraph, SimilarityMethod};

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

/// App stylesheet, adopting the Earth Metabolome Initiative SPLASH app theme
/// (palette, typography, pill controls, and panel cards) from
/// <https://splash.earthmetabolome.org/>.
const CSS: &str = r#"
@import url('https://fonts.googleapis.com/css2?family=IBM+Plex+Mono:wght@400;500&family=IBM+Plex+Sans:wght@400;500;600;700&family=Syne:wght@500;700;800&display=swap');
:root {
    --bg: #f6f0e6; --surface: rgba(255,252,247,.94); --surface-strong: rgba(255,255,255,.98);
    --line: rgba(20,34,41,.1); --line-strong: rgba(20,34,41,.17);
    --text: #15232b; --muted: #627077;
    --blue: #205e8c; --rust: #dea584; --green: #38755a; --red: #9d4133;
    --shadow: 0 18px 48px rgba(27,44,54,.08);
}
* { box-sizing: border-box; }
body { min-height: 100vh; margin: 0; color: var(--text); background: #fbf8f2; font-family: "IBM Plex Sans", "Avenir Next", sans-serif; }
.page { width: min(1180px, 100vw - 2rem); margin: 0 auto; padding: 1.5rem 0 3rem; }
.hero { margin-bottom: 1.25rem; }
.eyebrow { margin: 0; color: var(--muted); font-family: "IBM Plex Mono", monospace; font-size: .75rem; letter-spacing: .14em; text-transform: uppercase; }
.hero h1 { margin: .35rem 0 0; font-family: Syne, "IBM Plex Sans", sans-serif; font-size: clamp(2.4rem, 6vw, 4rem); line-height: .96; }
.hero-rust-suffix { color: var(--rust); }
.hero-copy { max-width: 46rem; margin: .7rem 0 0; color: var(--muted); line-height: 1.6; }
.panel { margin-bottom: 1.25rem; padding: 1.25rem; border: 1px solid var(--line); border-radius: 1.2rem; background: var(--surface); box-shadow: var(--shadow); }
.drop-zone { padding: 2rem; text-align: center; border: 2px dashed var(--line-strong); border-radius: 1rem; background: var(--surface-strong); transition: border-color .15s, background .15s; }
.drop-zone.hovering { border-color: var(--blue); background: rgba(32,94,140,.06); }
.drop-zone p { margin: 0 0 .85rem; color: var(--muted); }
.file-button { position: relative; overflow: hidden; display: inline-flex; align-items: center; justify-content: center; gap: .5rem; min-height: 2.35rem; padding: .45rem 1rem; color: #fff; background: var(--blue); border: 1px solid rgba(32,94,140,.24); border-radius: 999px; cursor: pointer; transition: background .14s, transform .14s; }
.file-button:hover { background: #184f78; transform: translateY(-1px); }
.file-input { position: absolute; inset: 0; opacity: 0; cursor: pointer; }
.drop-actions { display: flex; flex-wrap: wrap; gap: .75rem; justify-content: center; }
.button-green { display: inline-flex; align-items: center; justify-content: center; gap: .5rem; min-height: 2.35rem; padding: .45rem 1rem; color: #fff; background: var(--green); border: 1px solid rgba(56,117,90,.3); border-radius: 999px; font: inherit; cursor: pointer; transition: background .14s, transform .14s; }
.button-green:hover { background: #2b6048; transform: translateY(-1px); }
.summary { display: grid; gap: 1rem; }
.summary h2 { display: flex; align-items: center; gap: .5rem; margin: 0; font-size: 1.25rem; font-weight: 700; }
.cards { display: flex; flex-wrap: wrap; gap: .65rem; }
.card { min-width: 7rem; padding: .75rem 1rem; border: 1px solid var(--line); border-radius: 1rem; background: var(--surface-strong); }
.card .num { font-size: 1.6rem; font-weight: 700; }
.card .lbl { color: var(--muted); font-family: "IBM Plex Mono", monospace; font-size: .78rem; letter-spacing: .04em; }
.breakdowns { display: grid; grid-template-columns: repeat(auto-fit, minmax(13rem, 1fr)); gap: 1rem; }
.breakdown { overflow: hidden; border: 1px solid var(--line); border-radius: 1rem; background: rgba(255,253,250,.74); }
.breakdown h3 { margin: 0; padding: .7rem .85rem; color: var(--muted); font-family: "IBM Plex Mono", monospace; font-size: .72rem; letter-spacing: .12em; text-transform: uppercase; border-bottom: 1px solid var(--line); }
.breakdown table { width: 100%; border-collapse: collapse; font-size: .9rem; }
.breakdown td { padding: .5rem .85rem; border-bottom: 1px solid rgba(20,34,41,.06); }
.breakdown tr:last-child td { border-bottom: 0; }
.breakdown td.count { text-align: right; color: var(--muted); font-family: "IBM Plex Mono", monospace; }
.controls-title { display: flex; align-items: center; gap: .5rem; margin: 0 0 1rem; font-size: 1.25rem; font-weight: 700; }
.controls { display: grid; grid-template-columns: repeat(auto-fit, minmax(11rem, 1fr)); gap: 1rem; align-items: end; }
.field { display: grid; gap: .35rem; }
.field label { color: var(--muted); font-family: "IBM Plex Mono", monospace; font-size: .72rem; letter-spacing: .08em; text-transform: uppercase; }
.field select, .field input { width: 100%; padding: .5rem .65rem; color: var(--text); background: var(--surface-strong); border: 1px solid var(--line-strong); border-radius: .7rem; font: inherit; outline: none; }
.field select:focus, .field input:focus { border-color: rgba(23,112,168,.4); box-shadow: 0 0 0 4px rgba(23,112,168,.08); }
.build-row { display: flex; gap: .75rem; align-items: center; margin-top: 1rem; }
.button-primary { display: inline-flex; align-items: center; justify-content: center; min-height: 2.35rem; padding: .45rem 1.1rem; color: #fff; background: var(--blue); border: 1px solid rgba(32,94,140,.24); border-radius: 999px; font: inherit; cursor: pointer; transition: background .14s, transform .14s; }
.button-primary:hover { background: #184f78; transform: translateY(-1px); }
.graph-stats { margin-top: 1rem; }
.field-wide { grid-column: 1 / -1; }
.segmented { display: flex; flex-wrap: wrap; gap: .4rem; }
.segment { display: inline-flex; align-items: center; gap: .4rem; padding: .4rem .85rem; color: var(--text); background: var(--surface-strong); border: 1px solid var(--line-strong); border-radius: 999px; font: inherit; cursor: pointer; transition: background .14s, color .14s, border-color .14s; }
.segment:hover { color: var(--accent, var(--blue)); border-color: var(--accent, var(--blue)); }
.segment.active { color: #fff; background: var(--accent, var(--blue)); border-color: var(--accent, var(--blue)); }
.segment.active:hover { color: #fff; }
.view-hint { margin: 0 0 .75rem; color: var(--muted); font-size: .9rem; }
.legend { display: flex; flex-wrap: wrap; gap: .5rem .9rem; margin-top: .85rem; }
.legend-item { display: inline-flex; align-items: center; gap: .4rem; color: var(--muted); font-size: .82rem; }
.legend-swatch { display: inline-block; width: .8rem; height: .8rem; border-radius: 999px; border: 1px solid rgba(20,34,41,.25); }
.legend-label { font-family: "IBM Plex Mono", monospace; }
.legend-continuous { align-items: center; gap: .6rem; }
.legend-bar { display: inline-block; width: 12rem; max-width: 50vw; height: .7rem; border-radius: 999px; border: 1px solid rgba(20,34,41,.2); }
.node-panel { position: fixed; top: 0; right: 0; width: 360px; max-width: 92vw; height: 100vh; overflow-y: auto; padding: 1.25rem; background: var(--surface-strong); border-left: 1px solid var(--line); box-shadow: -16px 0 48px rgba(27,44,54,.14); z-index: 60; }
.node-panel-head { display: flex; align-items: center; justify-content: space-between; gap: .5rem; }
.node-panel-head h3 { margin: 0; font-size: 1.05rem; overflow-wrap: anywhere; }
.node-panel-close { display: inline-flex; align-items: center; justify-content: center; width: 1.9rem; height: 1.9rem; border: 1px solid var(--line-strong); border-radius: 999px; background: var(--surface); color: var(--text); font: inherit; cursor: pointer; }
.node-panel-close:hover { color: var(--red); border-color: var(--red); }
.node-panel-hint { margin: .35rem 0 0; color: var(--muted); font-size: .82rem; }
.spectrum-wrap { margin: .9rem 0; padding: .4rem; border: 1px solid var(--line); border-radius: .75rem; background: #fbf8f2; }
.spectrum { display: block; width: 100%; height: auto; }
.spectrum-readout { margin-top: .4rem; min-height: 1.1rem; font-family: "IBM Plex Mono", monospace; font-size: .8rem; }
.spectrum-value { color: var(--text); }
.kv { display: grid; grid-template-columns: auto 1fr; gap: .3rem .8rem; margin: 0; font-size: .88rem; }
.kv dt { color: var(--muted); font-family: "IBM Plex Mono", monospace; }
.kv dd { margin: 0; overflow-wrap: anywhere; }
.graph-canvas-wrap { position: relative; }
.edge-tooltip { position: absolute; transform: translate(-50%, -150%); padding: .2rem .5rem; background: var(--text); color: #fff; border-radius: .4rem; font-family: "IBM Plex Mono", monospace; font-size: .72rem; white-space: nowrap; pointer-events: none; z-index: 5; }
.graph-canvas { display: block; width: 100%; height: 540px; border: 1px solid var(--line); border-radius: 1rem; background: #fbf8f2; cursor: grab; touch-action: none; }
.graph-canvas:active { cursor: grabbing; }
.error { color: var(--red); }
"#;

/// A bundled example MGF (a SIRIUS-derived DBGI feature list) so the app can be
/// explored without uploading a file.
const EXAMPLE_MGF: &str = include_str!("../assets/example.mgf");

/// Display name for the bundled example.
const EXAMPLE_NAME: &str = "example.mgf";

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
    use_context_provider(|| Signal::new(ColorScheme::Component));
    use_context_provider(|| NodeFocus {
        selected: Signal::new(None),
        hovered: Signal::new(None),
    });
    rsx! {
        document::Title { "MGF similarity graph" }
        document::Style { {CSS} }
        div { class: "page",
            header { class: "hero",
                p { class: "eyebrow", "Earth Metabolome Initiative" }
                h1 {
                    "mascot"
                    span { class: "hero-rust-suffix", "-rs" }
                    " similarity graph"
                }
                p { class: "hero-copy",
                    "Drop an MGF file to build a spectral similarity graph from its records."
                }
            }
            div { class: "panel",
                DropZone {}
            }
            DatasetView {}
            GraphControls {}
            GraphCanvas {}
        }
        InfoPanel {}
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
        DatasetState::Loaded { name, summary, .. } => rsx! {
            SummaryView { name: name.clone(), summary: summary.clone() }
        },
    };
    rendered
}

/// Renders a parsed dataset summary.
#[component]
fn SummaryView(name: String, summary: DatasetSummary) -> Element {
    rsx! {
        section { class: "panel summary",
            h2 {
                Icon { width: 18, height: 18, fill: "#b6792f", icon: FaVial }
                span { "{name}" }
            }
            div { class: "cards",
                Stat { num: summary.count, label: "records" }
                Stat { num: summary.skipped, label: "skipped" }
                Stat { num: summary.with_smiles, label: "with SMILES" }
                Stat { num: summary.with_formula, label: "with formula" }
            }
            div { class: "breakdowns",
                BreakdownTable { title: "MS level", items: summary.ms_levels.clone() }
                BreakdownTable { title: "Ion mode", items: summary.ion_modes.clone() }
                BreakdownTable { title: "Charge", items: summary.charges.clone() }
                BreakdownTable { title: "Instrument", items: summary.instruments.clone() }
            }
        }
    }
}

/// A single headline statistic card.
#[component]
fn Stat(num: usize, label: String) -> Element {
    rsx! {
        div { class: "card",
            div { class: "num", "{num}" }
            div { class: "lbl", "{label}" }
        }
    }
}

/// A titled table of `(label, count)` rows.
#[component]
fn BreakdownTable(title: String, items: Breakdown) -> Element {
    rsx! {
        div { class: "breakdown",
            h3 { "{title}" }
            table {
                tbody {
                    for (label , count) in items {
                        tr {
                            td { "{label}" }
                            td { class: "count", "{count}" }
                        }
                    }
                }
            }
        }
    }
}

/// Controls for choosing the similarity measure and edge thresholds, plus the
/// build trigger. Renders nothing until a dataset is loaded.
#[component]
fn GraphControls() -> Element {
    let dataset = use_context::<Signal<DatasetState>>();
    let mut params = use_context::<Signal<GraphParams>>();
    let graph = use_context::<Signal<GraphState>>();
    let mut build_generation = use_signal(|| 0_u64);

    // Rebuild automatically when the dataset or parameters change, debounced so
    // quick successive changes coalesce into a single build. Colour-scheme
    // changes are handled by the renderer and do not rebuild the graph.
    use_effect(move || {
        let _ = params();
        let loaded = dataset.read().records().is_some();
        if !loaded {
            return;
        }
        let generation = build_generation.peek().wrapping_add(1);
        build_generation.set(generation);
        spawn(async move {
            gloo_timers::future::TimeoutFuture::new(200).await;
            if *build_generation.peek() != generation {
                return; // superseded by a newer change during the debounce window
            }
            let parameters = *params.peek();
            let outcome = dataset
                .peek()
                .records()
                .map(|records| build_graph(records.as_ref(), &parameters));
            if *build_generation.peek() != generation {
                return;
            }
            let mut graph = graph;
            match outcome {
                Some(Ok(built)) => graph.set(GraphState::Built(built)),
                Some(Err(error)) => graph.set(GraphState::Error(error)),
                None => {}
            }
        });
    });

    if dataset.read().records().is_none() {
        return rsx! {};
    }
    let current = params();

    rsx! {
        section { class: "panel",
            h2 { class: "controls-title",
                Icon { width: 18, height: 18, fill: "#205e8c", icon: FaShareNodes }
                span { "Similarity graph" }
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
            GraphResultView {}
        }
    }
}

/// Renders the result of the most recent graph build.
#[component]
fn GraphResultView() -> Element {
    let graph = use_context::<Signal<GraphState>>();
    let rendered = match &*graph.read() {
        GraphState::None => rsx! {},
        GraphState::Error(error) => rsx! {
            p { class: "error", "Graph build failed: {error}" }
        },
        GraphState::Built(built) => rsx! {
            div { class: "cards graph-stats",
                Stat { num: built.node_count, label: "nodes" }
                Stat { num: built.edges.len(), label: "edges" }
                Stat { num: built.component_count, label: "components" }
                Stat { num: built.community_count, label: "communities" }
            }
        },
    };
    rendered
}

/// Pannable, zoomable canvas view of the built similarity graph, with a
/// colour-scheme selector and a legend.
#[component]
fn GraphCanvas() -> Element {
    let graph = use_context::<Signal<GraphState>>();
    let dataset = use_context::<Signal<DatasetState>>();
    let mut scheme = use_context::<Signal<ColorScheme>>();
    let focus = use_context::<NodeFocus>();
    let mut selected = focus.selected;
    let mut hovered = focus.hovered;
    let mut view = use_signal(render::ViewTransform::default);
    // Live node positions (mutated by dragging) and the fixed reference layout
    // that anchors the projection so dragging does not rescale the view.
    let mut positions = use_signal(Vec::<[f64; 2]>::new);
    let mut reference = use_signal(Vec::<[f64; 2]>::new);
    let mut drag = use_signal(|| Drag::None);
    let mut hovered_edge = use_signal(|| None::<EdgeHover>);

    // Reset positions, view, and selection whenever a new graph is built.
    use_effect(move || {
        let graph_ref = graph.read();
        view.set(render::ViewTransform::default());
        selected.set(None);
        hovered.set(None);
        if let GraphState::Built(built) = &*graph_ref {
            positions.set(built.coordinates.clone());
            reference.set(built.coordinates.clone());
        } else {
            positions.set(Vec::new());
            reference.set(Vec::new());
        }
    });

    // Redraw whenever the graph, colours, view, positions, or focus change.
    use_effect(move || {
        let transform = view();
        let active = scheme();
        let highlight = focus.active();
        let positions_snapshot = positions();
        let graph_ref = graph.read();
        if let GraphState::Built(built) = &*graph_ref {
            let dataset_ref = dataset.read();
            let records: &[MascotGenericFormat] = dataset_ref.records().map_or(&[], AsRef::as_ref);
            let coloring = coloring::compute(active, built, records);
            render::draw_graph(
                built,
                &positions_snapshot,
                transform,
                &coloring.colors,
                highlight,
            );
        }
    });

    // If the active scheme became trivial (all nodes one group), switch to the
    // first scheme that still distinguishes nodes.
    use_effect(move || {
        let graph_ref = graph.read();
        if let GraphState::Built(built) = &*graph_ref {
            let dataset_ref = dataset.read();
            let records: &[MascotGenericFormat] = dataset_ref.records().map_or(&[], AsRef::as_ref);
            if !coloring::is_informative(*scheme.peek(), built, records) {
                if let Some(first) = ColorScheme::ALL
                    .into_iter()
                    .find(|candidate| coloring::is_informative(*candidate, built, records))
                {
                    scheme.set(first);
                }
            }
        }
    });

    let graph_ref = graph.read();
    let GraphState::Built(built) = &*graph_ref else {
        return rsx! {};
    };
    let current_scheme = scheme();
    let dataset_ref = dataset.read();
    let records: &[MascotGenericFormat] = dataset_ref.records().map_or(&[], AsRef::as_ref);
    // Only offer colour schemes that actually distinguish nodes; a scheme whose
    // values are all equal (e.g. a single charge or ion mode) is dropped.
    let available: Vec<ColorScheme> = ColorScheme::ALL
        .into_iter()
        .filter(|candidate| coloring::is_informative(*candidate, built, records))
        .collect();
    let effective_scheme = if available.contains(&current_scheme) {
        current_scheme
    } else {
        available.first().copied().unwrap_or(current_scheme)
    };
    let legend_markup = match coloring::compute(effective_scheme, built, records).legend {
        coloring::Legend::Categorical(entries) => {
            let shown = entries.len().min(24);
            let extra = entries.len().saturating_sub(shown);
            rsx! {
                div { class: "legend",
                    for (label , color) in entries.into_iter().take(shown) {
                        div { class: "legend-item",
                            span { class: "legend-swatch", style: "background: {color};" }
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
            h2 { class: "controls-title",
                Icon { width: 18, height: 18, fill: "#38755a", icon: FaCircleNodes }
                span { "Graph view" }
            }
            div { class: "field field-wide",
                label { title: "Choose which property maps to node colour: graph structure, spectrum metadata, or a per-spectrum heatmap.", "Colour by" }
                div { class: "segmented", role: "group", aria_label: "Node colouring scheme",
                    for option in available {
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
                            drag.set(Drag::Node { index });
                        }
                        None => {
                            selected.set(None);
                            drag.set(Drag::Pan { x: point.x, y: point.y });
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
                            let edge = if node_hit.is_some() {
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
                                .and_then(|index| built.edges.get(index).map(|&(_, _, score)| score))
                                .map(|score| EdgeHover { score, x: point.x, y: point.y })
                            } else {
                                None
                            };
                            hovered_edge.set(edge);
                        }
                    }
                },
                onmouseup: move |_| drag.set(Drag::None),
                onmouseleave: move |_| {
                    drag.set(Drag::None);
                    hovered.set(None);
                    hovered_edge.set(None);
                },
                onwheel: move |evt| {
                    evt.prevent_default();
                    let factor = if evt.delta().strip_units().y < 0.0 { 1.1 } else { 0.9 };
                    view.with_mut(|transform| {
                        transform.zoom = (transform.zoom * factor).clamp(1.0, 50.0);
                    });
                },
            }
            if let Some(edge) = hovered_edge() {
                div {
                    class: "edge-tooltip",
                    style: "left: {edge.x}px; top: {edge.y}px;",
                    "similarity {edge.score:.3}"
                }
            }
            }
            {legend_markup}
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

    let title = record
        .metadata()
        .feature_id()
        .map_or_else(|| format!("Spectrum #{index}"), str::to_string);
    let pinned = selected.read().is_some();
    let rows = node_detail_rows(record);
    let peaks: Vec<(f64, f64)> = record
        .peaks()
        .map(|(mz, intensity)| (mz.to_f64(), intensity.to_f64()))
        .collect();

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
