//! Canvas rendering of the similarity graph.
//!
//! Drawing is done imperatively against the 2D canvas context via `web-sys`.
//! Layout coordinates are fit to the canvas with a pannable, zoomable view
//! transform. Nodes are coloured by connected component for now; richer
//! colouring schemes are layered on later.

use wasm_bindgen::JsCast;

use crate::similarity::SimilarityGraph;

/// Pan and zoom state for the graph view.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewTransform {
    /// Horizontal pan offset, in screen pixels.
    pub pan_x: f64,
    /// Vertical pan offset, in screen pixels.
    pub pan_y: f64,
    /// Zoom multiplier applied on top of the fit-to-canvas scale.
    pub zoom: f64,
}

impl Default for ViewTransform {
    fn default() -> Self {
        Self {
            pan_x: 0.0,
            pan_y: 0.0,
            zoom: 1.0,
        }
    }
}

/// Qualitative colour palette, seeded with the app accents. Ordered so the
/// first dozen entries (which cover the common case of a handful of groups) are
/// maximally distinct hues; later entries fill in second shades. Twenty colours
/// combined with [`SHAPE_COUNT`] marker shapes give `lcm(20, 6) = 60` distinct
/// node identities before any colour-and-shape pair repeats.
const PALETTE: [&str; 20] = [
    "#205e8c", // blue
    "#c1432b", // red
    "#3c8c52", // green
    "#8a4fa3", // purple
    "#d2811e", // amber
    "#2f8f8f", // teal
    "#b8417a", // magenta
    "#7d8c2a", // olive
    "#9c5a2c", // brown
    "#cf5a36", // coral
    "#a23a5e", // raspberry
    "#2aa18c", // emerald
    "#5a5fb0", // indigo
    "#3f7fb5", // steel blue
    "#6fa023", // leaf green
    "#4fa37a", // mint
    "#b34a3a", // rust
    "#7a5cc0", // violet
    "#9a7d1c", // dark gold
    "#c24f9a", // orchid
];

/// Returns a stable palette colour for a group index.
#[must_use]
pub fn palette_color(index: usize) -> &'static str {
    PALETTE[index % PALETTE.len()]
}

/// Anchor colours for the continuous heatmap gradient (dark teal to coral).
const HEAT_STOPS: [(u8, u8, u8); 4] = [
    (38, 70, 83),
    (42, 157, 143),
    (233, 196, 106),
    (231, 111, 81),
];

/// Maps a normalised value `t` in `[0, 1]` to a heatmap colour string.
#[must_use]
pub fn heat_color(t: f64) -> String {
    let t = t.clamp(0.0, 1.0);
    let segments = HEAT_STOPS.len() - 1;
    let scaled = t * segments as f64;
    let lower = (scaled.floor() as usize).min(segments - 1);
    let frac = scaled - lower as f64;
    let (r0, g0, b0) = HEAT_STOPS[lower];
    let (r1, g1, b1) = HEAT_STOPS[lower + 1];
    let lerp = |a: u8, b: u8| (f64::from(a) + (f64::from(b) - f64::from(a)) * frac).round() as u8;
    format!("rgb({}, {}, {})", lerp(r0, r1), lerp(g0, g1), lerp(b0, b1))
}

/// The canvas element id the renderer draws into.
pub const CANVAS_ID: &str = "graph-canvas";

/// Node radius in CSS pixels for the current zoom.
fn node_radius(view: ViewTransform) -> f64 {
    (5.0 * view.zoom).clamp(2.5, 9.0)
}

/// Maps layout coordinates to screen pixels under a view transform.
///
/// The bounding box is taken from a fixed reference layout, so dragging a node
/// does not rescale or recentre the whole view.
pub struct Projection {
    center_x: f64,
    center_y: f64,
    scale: f64,
    screen_x: f64,
    screen_y: f64,
}

impl Projection {
    /// Builds a projection fitting `reference` into a `width` x `height` area.
    #[must_use]
    pub fn new(reference: &[[f64; 2]], view: ViewTransform, width: f64, height: f64) -> Self {
        let mut min_x = f64::MAX;
        let mut max_x = f64::MIN;
        let mut min_y = f64::MAX;
        let mut max_y = f64::MIN;
        for &[x, y] in reference {
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
        }
        if !min_x.is_finite() {
            min_x = 0.0;
            max_x = 0.0;
            min_y = 0.0;
            max_y = 0.0;
        }
        let span = (max_x - min_x).max(max_y - min_y).max(1e-9);
        let base = (width.min(height) * 0.82) / span;
        Self {
            center_x: (min_x + max_x) / 2.0,
            center_y: (min_y + max_y) / 2.0,
            scale: base * view.zoom,
            screen_x: width / 2.0 + view.pan_x,
            screen_y: height / 2.0 + view.pan_y,
        }
    }

    /// Projects a world coordinate to screen pixels.
    #[must_use]
    pub fn to_screen(&self, point: [f64; 2]) -> (f64, f64) {
        (
            self.screen_x + (point[0] - self.center_x) * self.scale,
            self.screen_y + (point[1] - self.center_y) * self.scale,
        )
    }

    /// Inversely maps screen pixels to a world coordinate.
    #[must_use]
    pub fn to_world(&self, screen_x: f64, screen_y: f64) -> [f64; 2] {
        [
            self.center_x + (screen_x - self.screen_x) / self.scale,
            self.center_y + (screen_y - self.screen_y) / self.scale,
        ]
    }
}

/// Reads the CSS (layout) size of the graph canvas, if present.
#[must_use]
pub fn canvas_size() -> Option<(f64, f64)> {
    let canvas = web_sys::window()?
        .document()?
        .get_element_by_id(CANVAS_ID)?
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .ok()?;
    Some((
        f64::from(canvas.client_width().max(1)),
        f64::from(canvas.client_height().max(1)),
    ))
}

/// Returns the node nearest to `(screen_x, screen_y)` within its hit radius.
///
/// `reference` fixes the projection; `positions` holds the live (possibly
/// dragged) node coordinates.
#[must_use]
pub fn hit_test(
    reference: &[[f64; 2]],
    positions: &[[f64; 2]],
    view: ViewTransform,
    width: f64,
    height: f64,
    screen_x: f64,
    screen_y: f64,
) -> Option<usize> {
    let projection = Projection::new(reference, view, width, height);
    let tolerance = node_radius(view) + 4.0;
    let mut best: Option<(usize, f64)> = None;
    for (index, &point) in positions.iter().enumerate() {
        let (px, py) = projection.to_screen(point);
        let distance = ((px - screen_x).powi(2) + (py - screen_y).powi(2)).sqrt();
        if distance <= tolerance && best.is_none_or(|(_, closest)| distance < closest) {
            best = Some((index, distance));
        }
    }
    best.map(|(index, _)| index)
}

/// Perpendicular distance from point `(px, py)` to segment `a`-`b`.
fn point_segment_distance(px: f64, py: f64, ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    let dx = bx - ax;
    let dy = by - ay;
    let length_squared = dx * dx + dy * dy;
    if length_squared <= 1e-9 {
        return (px - ax).hypot(py - ay);
    }
    let t = (((px - ax) * dx + (py - ay) * dy) / length_squared).clamp(0.0, 1.0);
    let cx = ax + t * dx;
    let cy = ay + t * dy;
    (px - cx).hypot(py - cy)
}

/// Returns the index of the edge nearest to `(screen_x, screen_y)` within the
/// edge hit tolerance, if any. Endpoints use the live `positions`.
#[must_use]
pub fn edge_hit_test(
    graph: &SimilarityGraph,
    positions: &[[f64; 2]],
    view: ViewTransform,
    width: f64,
    height: f64,
    screen_x: f64,
    screen_y: f64,
) -> Option<usize> {
    let projection = Projection::new(&graph.coordinates, view, width, height);
    let tolerance = 5.0;
    let mut best: Option<(usize, f64)> = None;
    for (index, &(u, v, _score)) in graph.edges.iter().enumerate() {
        let (Some(&a), Some(&b)) = (positions.get(u), positions.get(v)) else {
            continue;
        };
        let (ax, ay) = projection.to_screen(a);
        let (bx, by) = projection.to_screen(b);
        let distance = point_segment_distance(screen_x, screen_y, ax, ay, bx, by);
        if distance <= tolerance && best.is_none_or(|(_, closest)| distance < closest) {
            best = Some((index, distance));
        }
    }
    best.map(|(index, _)| index)
}

/// Number of distinct node marker shapes used to encode categorical groups.
pub const SHAPE_COUNT: usize = 6;

/// Traces a marker shape (selected by `shape % SHAPE_COUNT`) as a path centred
/// at `(x, y)` with radius `r`, then fills and strokes it with the current
/// context styles.
fn draw_marker(context: &web_sys::CanvasRenderingContext2d, shape: usize, x: f64, y: f64, r: f64) {
    context.begin_path();
    match shape % SHAPE_COUNT {
        1 => {
            // Square.
            context.rect(x - r, y - r, 2.0 * r, 2.0 * r);
        }
        2 => {
            // Triangle pointing up.
            let half = r * 0.95;
            context.move_to(x, y - r);
            context.line_to(x + half, y + r * 0.7);
            context.line_to(x - half, y + r * 0.7);
            context.close_path();
        }
        3 => {
            // Diamond.
            context.move_to(x, y - r);
            context.line_to(x + r, y);
            context.line_to(x, y + r);
            context.line_to(x - r, y);
            context.close_path();
        }
        4 => {
            // Triangle pointing down.
            let half = r * 0.95;
            context.move_to(x, y + r);
            context.line_to(x + half, y - r * 0.7);
            context.line_to(x - half, y - r * 0.7);
            context.close_path();
        }
        5 => {
            // Hexagon with a vertex at the top.
            for corner in 0..6 {
                let angle =
                    core::f64::consts::FRAC_PI_3 * f64::from(corner) - core::f64::consts::FRAC_PI_2;
                let px = x + r * angle.cos();
                let py = y + r * angle.sin();
                if corner == 0 {
                    context.move_to(px, py);
                } else {
                    context.line_to(px, py);
                }
            }
            context.close_path();
        }
        _ => {
            // Circle.
            let _ = context.arc(x, y, r, 0.0, core::f64::consts::TAU);
        }
    }
    context.fill();
    context.stroke();
}

/// Draws the similarity graph into the `<canvas>` identified by [`CANVAS_ID`].
///
/// `positions` gives the live (possibly dragged) node coordinates; the
/// projection's fit comes from `graph.coordinates`, so dragging stays stable.
/// `colors` is the fill per node and `highlight` rings the active node. When
/// `categorical` is set, `groups` selects a marker shape per node and edges
/// within a group are drawn in that group's colour. `highlight_edge` draws the
/// focused edge (its two endpoint indices) with a bold accent stroke.
#[allow(clippy::too_many_arguments)]
pub fn draw_graph(
    graph: &SimilarityGraph,
    positions: &[[f64; 2]],
    view: ViewTransform,
    colors: &[String],
    groups: &[usize],
    categorical: bool,
    highlight: Option<usize>,
    highlight_edge: Option<(usize, usize)>,
) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    let Some(element) = document.get_element_by_id(CANVAS_ID) else {
        return;
    };
    let Ok(canvas) = element.dyn_into::<web_sys::HtmlCanvasElement>() else {
        return;
    };

    // Size the backing store in device pixels so the plot stays crisp on
    // HiDPI/retina displays, while drawing in CSS-pixel coordinates.
    let dpr = window.device_pixel_ratio().max(1.0);
    let css_width = canvas.client_width().max(1);
    let css_height = canvas.client_height().max(1);
    let backing_width = (f64::from(css_width) * dpr).round() as u32;
    let backing_height = (f64::from(css_height) * dpr).round() as u32;
    if canvas.width() != backing_width {
        canvas.set_width(backing_width);
    }
    if canvas.height() != backing_height {
        canvas.set_height(backing_height);
    }

    let Ok(Some(object)) = canvas.get_context("2d") else {
        return;
    };
    let Ok(context) = object.dyn_into::<web_sys::CanvasRenderingContext2d>() else {
        return;
    };

    // Map CSS pixels to device pixels for all subsequent drawing.
    let _ = context.set_transform(dpr, 0.0, 0.0, dpr, 0.0, 0.0);

    let width = f64::from(css_width);
    let height = f64::from(css_height);
    context.set_fill_style_str("#fbf8f2");
    context.fill_rect(0.0, 0.0, width, height);

    if positions.is_empty() {
        return;
    }

    let projection = Projection::new(&graph.coordinates, view, width, height);
    let screen_of = |index: usize| {
        positions
            .get(index)
            .map(|&point| projection.to_screen(point))
    };

    // Edges first, so nodes sit on top. Edges within one group are tinted with
    // that group's colour to reinforce community membership; the rest are grey.
    context.set_line_width(1.0);
    for &(u, v, score) in &graph.edges {
        let (Some(a), Some(b)) = (screen_of(u), screen_of(v)) else {
            continue;
        };
        let same_group = categorical && groups.get(u).is_some() && groups.get(u) == groups.get(v);
        if same_group {
            context.set_stroke_style_str(colors.get(u).map_or("#627077", String::as_str));
            context.set_global_alpha((0.3 + 0.6 * score).clamp(0.1, 0.95));
        } else {
            context.set_stroke_style_str("#627077");
            context.set_global_alpha((0.1 + 0.45 * score).clamp(0.05, 0.6));
        }
        context.begin_path();
        context.move_to(a.0, a.1);
        context.line_to(b.0, b.1);
        context.stroke();
    }
    context.set_global_alpha(1.0);

    // The focused edge, drawn bold so the selection is clear. Endpoint markers
    // are painted afterwards, so the line reads as a link between two nodes.
    if let Some((u, v)) = highlight_edge {
        if let (Some(a), Some(b)) = (screen_of(u), screen_of(v)) {
            context.set_stroke_style_str("#9d4133");
            context.set_line_width(3.0);
            context.begin_path();
            context.move_to(a.0, a.1);
            context.line_to(b.0, b.1);
            context.stroke();
            context.set_line_width(1.0);
        }
    }

    // Nodes, shaped by group (categorical schemes) and filled by colour.
    let radius = node_radius(view);
    context.set_line_width(1.0);
    context.set_stroke_style_str("#15232b");
    for index in 0..positions.len() {
        let Some((x, y)) = screen_of(index) else {
            continue;
        };
        let color = colors.get(index).map_or("#205e8c", String::as_str);
        let shape = if categorical {
            groups.get(index).copied().unwrap_or(0)
        } else {
            0
        };
        context.set_fill_style_str(color);
        draw_marker(&context, shape, x, y, radius);
    }

    // Highlight ring on the active node.
    if let Some((x, y)) = highlight.and_then(screen_of) {
        context.set_line_width(2.5);
        context.set_stroke_style_str("#15232b");
        context.begin_path();
        let _ = context.arc(x, y, radius + 3.0, 0.0, core::f64::consts::TAU);
        context.stroke();
    }
}
