//! CSV, TSV, and Parquet export of the similarity graph as a node list and a
//! weighted edge list.
//!
//! Two tables are produced. The node list has one row per spectrum, keyed by a
//! 0-based `node_id` that matches the indices used in the edge list, with a
//! selectable mix of MGF metadata and graph-derived columns. The edge list has
//! one row per graph edge, with the endpoints referenced by index, feature id,
//! or both, and a selectable subset of the four similarity measures (computed
//! directly via [`crate::similarity::pairwise_similarities`], so they match the
//! graph weights). The pure table builders are unit-tested; [`download_text`]
//! and [`download_bytes`] are the only browser-only pieces.

use std::collections::HashMap;
use std::sync::Arc;

use mascot_rs::prelude::{MascotGenericFormat, Spectrum, SpectrumFloat};
use parquet::data_type::{ByteArray, ByteArrayType, DoubleType, Int64Type};
use parquet::file::properties::WriterProperties;
use parquet::file::writer::SerializedFileWriter;
use parquet::schema::parser::parse_message_type;

use crate::coloring::intensity_entropy;
use crate::similarity::{pairwise_similarities, GraphParams, SimilarityGraph, SimilarityMethod};

/// The field delimiter and associated file metadata for an export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delimiter {
    /// Comma-separated values.
    Comma,
    /// Tab-separated values.
    Tab,
}

impl Delimiter {
    /// The separator character.
    #[must_use]
    pub const fn char(self) -> char {
        match self {
            Self::Comma => ',',
            Self::Tab => '\t',
        }
    }
}

/// A download file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// Comma-separated text.
    Csv,
    /// Tab-separated text.
    Tsv,
    /// Apache Parquet (columnar binary).
    Parquet,
}

impl OutputFormat {
    /// All formats, in display order.
    pub const ALL: [Self; 3] = [Self::Csv, Self::Tsv, Self::Parquet];

    /// A stable identifier used as a list key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Tsv => "tsv",
            Self::Parquet => "parquet",
        }
    }

    /// A short UI label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Csv => "CSV",
            Self::Tsv => "TSV",
            Self::Parquet => "Parquet",
        }
    }

    /// The file extension.
    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Tsv => "tsv",
            Self::Parquet => "parquet",
        }
    }

    /// The MIME type.
    #[must_use]
    pub const fn mime(self) -> &'static str {
        match self {
            Self::Csv => "text/csv",
            Self::Tsv => "text/tab-separated-values",
            Self::Parquet => "application/vnd.apache.parquet",
        }
    }

    /// The text delimiter for the delimited formats, or `None` for Parquet.
    #[must_use]
    pub const fn delimiter(self) -> Option<Delimiter> {
        match self {
            Self::Csv => Some(Delimiter::Comma),
            Self::Tsv => Some(Delimiter::Tab),
            Self::Parquet => None,
        }
    }
}

/// A selectable node-list column. `node_id` (the index) is always emitted first
/// and is not part of this set.
///
/// Columns split into what the app computes (communities, layout, degree,
/// intensity entropy, peak count) and a single `mgf_metadata` column that passes
/// through everything read from the MGF file verbatim. The app does no chemical
/// inference (no formula or structure estimation), so any annotation here came
/// from the file, never from spectrum analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeColumn {
    /// Louvain community label.
    LouvainCommunity,
    /// Leiden community label.
    LeidenCommunity,
    /// Number of fragment peaks.
    PeakCount,
    /// Spectral intensity entropy.
    IntensityEntropy,
    /// Number of incident edges.
    Degree,
    /// Layout x coordinate.
    X,
    /// Layout y coordinate.
    Y,
    /// All metadata read from the MGF, as `KEY=value` pairs.
    MgfMetadata,
}

impl NodeColumn {
    /// All columns, in display order.
    pub const ALL: [Self; 8] = [
        Self::LouvainCommunity,
        Self::LeidenCommunity,
        Self::PeakCount,
        Self::IntensityEntropy,
        Self::Degree,
        Self::X,
        Self::Y,
        Self::MgfMetadata,
    ];

    /// The columns enabled by default (the Core set).
    pub const DEFAULTS: [Self; 5] = [
        Self::LouvainCommunity,
        Self::LeidenCommunity,
        Self::PeakCount,
        Self::IntensityEntropy,
        Self::MgfMetadata,
    ];

    /// The column header (snake_case), also used as a list key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::LouvainCommunity => "louvain_community",
            Self::LeidenCommunity => "leiden_community",
            Self::PeakCount => "peak_count",
            Self::IntensityEntropy => "intensity_entropy",
            Self::Degree => "degree",
            Self::X => "x",
            Self::Y => "y",
            Self::MgfMetadata => "mgf_metadata",
        }
    }

    /// A human-readable label for the config UI.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::LouvainCommunity => "Louvain community",
            Self::LeidenCommunity => "Leiden community",
            Self::PeakCount => "Peak count",
            Self::IntensityEntropy => "Intensity entropy",
            Self::Degree => "Degree",
            Self::X => "Layout x",
            Self::Y => "Layout y",
            Self::MgfMetadata => "MGF metadata",
        }
    }

    /// The Parquet schema fragment for this column (physical type and
    /// nullability). Names match [`Self::id`].
    #[must_use]
    const fn parquet_field(self) -> &'static str {
        match self {
            Self::LouvainCommunity => "required int64 louvain_community;",
            Self::LeidenCommunity => "required int64 leiden_community;",
            Self::PeakCount => "required int64 peak_count;",
            Self::IntensityEntropy => "required double intensity_entropy;",
            Self::Degree => "required int64 degree;",
            Self::X => "required double x;",
            Self::Y => "required double y;",
            Self::MgfMetadata => "optional binary mgf_metadata (UTF8);",
        }
    }

    /// The Parquet column data for this column across all `records`.
    #[must_use]
    fn parquet_data(
        self,
        records: &[MascotGenericFormat],
        graph: &SimilarityGraph,
        degrees: &[usize],
    ) -> ColumnData {
        let count = records.len();
        match self {
            Self::LouvainCommunity => {
                req_i64((0..count).map(|index| as_i64(graph.community_of_node.get(index).copied())))
            }
            Self::LeidenCommunity => {
                req_i64((0..count).map(|index| as_i64(graph.leiden_of_node.get(index).copied())))
            }
            Self::PeakCount => req_i64(records.iter().map(|record| record.len() as i64)),
            Self::IntensityEntropy => req_f64(records.iter().map(intensity_entropy)),
            Self::Degree => req_i64((0..count).map(|index| as_i64(degrees.get(index).copied()))),
            Self::X => req_f64(
                (0..count).map(|index| graph.coordinates.get(index).map_or(0.0, |point| point[0])),
            ),
            Self::Y => req_f64(
                (0..count).map(|index| graph.coordinates.get(index).map_or(0.0, |point| point[1])),
            ),
            Self::MgfMetadata => opt_utf8(
                records
                    .iter()
                    .map(|record| Some(mgf_metadata_string(record))),
            ),
        }
    }

    /// The value of this column for `record` at node `index`, as a string.
    #[must_use]
    fn value(
        self,
        index: usize,
        record: &MascotGenericFormat,
        graph: &SimilarityGraph,
        degree: usize,
    ) -> String {
        match self {
            Self::LouvainCommunity => graph
                .community_of_node
                .get(index)
                .map(|community| community.to_string())
                .unwrap_or_default(),
            Self::LeidenCommunity => graph
                .leiden_of_node
                .get(index)
                .map(|community| community.to_string())
                .unwrap_or_default(),
            Self::PeakCount => record.len().to_string(),
            Self::IntensityEntropy => intensity_entropy(record).to_string(),
            Self::Degree => degree.to_string(),
            Self::X => graph
                .coordinates
                .get(index)
                .map(|point| point[0].to_string())
                .unwrap_or_default(),
            Self::Y => graph
                .coordinates
                .get(index)
                .map(|point| point[1].to_string())
                .unwrap_or_default(),
            Self::MgfMetadata => mgf_metadata_string(record),
        }
    }
}

/// Serializes everything read from the MGF for `record` as `KEY=value` pairs
/// joined by "; ". This is a verbatim passthrough of the file metadata: the app
/// performs no chemical inference, so a formula or structure here came from the
/// file, not from spectrum analysis.
fn mgf_metadata_string(record: &MascotGenericFormat) -> String {
    let metadata = record.metadata();
    let mut parts: Vec<String> = Vec::new();
    if let Some(value) = metadata.feature_id() {
        parts.push(format!("FEATURE_ID={value}"));
    }
    parts.push(format!("PEPMASS={}", record.precursor_mz().to_f64()));
    if let Some(value) = metadata.charge() {
        parts.push(format!("CHARGE={value}"));
    }
    if let Some(value) = metadata.ion_mode() {
        parts.push(format!("IONMODE={}", value.as_str()));
    }
    if let Some(value) = metadata.level() {
        parts.push(format!("MSLEVEL={value}"));
    }
    if let Some(value) = metadata.retention_time() {
        parts.push(format!("RTINSECONDS={value}"));
    }
    if let Some(value) = metadata.source_instrument() {
        parts.push(format!("SOURCE_INSTRUMENT={}", value.as_str()));
    }
    if let Some(value) = metadata.smiles() {
        parts.push(format!("SMILES={value}"));
    }
    if let Some(value) = metadata.formula() {
        parts.push(format!("FORMULA={value}"));
    }
    if let Some(value) = metadata.splash() {
        parts.push(format!("SPLASH={value}"));
    }
    if let Some(value) = metadata.scans() {
        parts.push(format!("SCANS={value}"));
    }
    if let Some(value) = metadata.filename() {
        parts.push(format!("FILENAME={value}"));
    }
    for (key, value) in metadata.arbitrary_metadata() {
        parts.push(format!("{key}={value}"));
    }
    parts.join("; ")
}

/// How the edge list references each endpoint spectrum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointId {
    /// 0-based node index (joins to `node_id` in the node list).
    Index,
    /// MGF feature id.
    FeatureId,
    /// Both the index and the feature id.
    Both,
}

impl EndpointId {
    /// All variants, in display order.
    pub const ALL: [Self; 3] = [Self::Index, Self::FeatureId, Self::Both];

    /// Whether the variant relies on the feature id as a node key.
    #[must_use]
    pub const fn uses_feature_id(self) -> bool {
        matches!(self, Self::FeatureId | Self::Both)
    }

    /// A stable identifier used as a list key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Index => "index",
            Self::FeatureId => "feature-id",
            Self::Both => "both",
        }
    }

    /// A human-readable label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Index => "Index",
            Self::FeatureId => "Feature ID",
            Self::Both => "Both",
        }
    }
}

/// Reports why the feature id cannot be used as a unique node key, if so.
///
/// Returns `None` when every record has a distinct feature id, otherwise a
/// human-readable message naming the problem (missing or shared ids).
#[must_use]
pub fn feature_id_issue(records: &[MascotGenericFormat]) -> Option<String> {
    let mut missing = 0usize;
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for record in records {
        match record.metadata().feature_id() {
            Some(id) => *counts.entry(id).or_insert(0) += 1,
            None => missing += 1,
        }
    }
    let duplicated = counts.values().filter(|&&count| count > 1).count();
    if missing == 0 && duplicated == 0 {
        return None;
    }
    let mut parts = Vec::new();
    if missing > 0 {
        parts.push(format!("{missing} record(s) have no feature id"));
    }
    if duplicated > 0 {
        parts.push(format!(
            "{duplicated} feature id(s) are shared by multiple records"
        ));
    }
    Some(format!(
        "Feature ID is not a unique node key: {}.",
        parts.join(", and ")
    ))
}

/// Per-node incident edge count.
fn node_degrees(graph: &SimilarityGraph) -> Vec<usize> {
    let mut degrees = vec![0usize; graph.node_count];
    for &(u, v, _) in &graph.edges {
        if let Some(degree) = degrees.get_mut(u) {
            *degree += 1;
        }
        if let Some(degree) = degrees.get_mut(v) {
            *degree += 1;
        }
    }
    degrees
}

/// RFC 4180 field quoting: wrap in quotes (doubling interior quotes) when the
/// field contains the delimiter, a quote, or a line break.
#[must_use]
pub fn escape(field: &str, delimiter: char) -> String {
    if field.contains(delimiter)
        || field.contains('"')
        || field.contains('\n')
        || field.contains('\r')
    {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

/// Appends one escaped, delimiter-joined row (terminated by a newline).
fn write_row(out: &mut String, fields: &[String], delimiter: char) {
    for (column, field) in fields.iter().enumerate() {
        if column > 0 {
            out.push(delimiter);
        }
        out.push_str(&escape(field, delimiter));
    }
    out.push('\n');
}

/// The feature id of `records[index]`, or empty when absent.
fn feature_id_of(records: &[MascotGenericFormat], index: usize) -> String {
    records
        .get(index)
        .and_then(|record| record.metadata().feature_id())
        .map(str::to_string)
        .unwrap_or_default()
}

/// Builds the node-list table: `node_id` plus the selected `columns`.
#[must_use]
pub fn build_node_table(
    records: &[MascotGenericFormat],
    graph: &SimilarityGraph,
    columns: &[NodeColumn],
    delimiter: Delimiter,
) -> String {
    let separator = delimiter.char();
    let degrees = node_degrees(graph);
    let mut out = String::new();

    let mut header = Vec::with_capacity(columns.len() + 1);
    header.push("node_id".to_string());
    header.extend(columns.iter().map(|column| column.id().to_string()));
    write_row(&mut out, &header, separator);

    for (index, record) in records.iter().enumerate() {
        let mut row = Vec::with_capacity(columns.len() + 1);
        row.push(index.to_string());
        let degree = degrees.get(index).copied().unwrap_or(0);
        for &column in columns {
            row.push(column.value(index, record, graph, degree));
        }
        write_row(&mut out, &row, separator);
    }
    out
}

/// Builds the weighted edge-list table.
///
/// Endpoint columns depend on `endpoint`; each selected measure in `weights`
/// becomes one column, recomputed per edge so the values match the graph.
#[must_use]
pub fn build_edge_table(
    records: &[MascotGenericFormat],
    graph: &SimilarityGraph,
    params: &GraphParams,
    endpoint: EndpointId,
    weights: &[SimilarityMethod],
    delimiter: Delimiter,
) -> String {
    let separator = delimiter.char();
    let mut out = String::new();

    let mut header = Vec::new();
    match endpoint {
        EndpointId::Index => {
            header.push("source".to_string());
            header.push("target".to_string());
        }
        EndpointId::FeatureId => {
            header.push("source_feature_id".to_string());
            header.push("target_feature_id".to_string());
        }
        EndpointId::Both => {
            header.push("source".to_string());
            header.push("target".to_string());
            header.push("source_feature_id".to_string());
            header.push("target_feature_id".to_string());
        }
    }
    // Snake-case headers to match the node-list columns (`method.id()` is the
    // kebab-case UI key, e.g. "modified-cosine").
    header.extend(weights.iter().map(|method| method.id().replace('-', "_")));
    write_row(&mut out, &header, separator);

    for &(u, v, _) in &graph.edges {
        let mut row = Vec::new();
        match endpoint {
            EndpointId::Index => {
                row.push(u.to_string());
                row.push(v.to_string());
            }
            EndpointId::FeatureId => {
                row.push(feature_id_of(records, u));
                row.push(feature_id_of(records, v));
            }
            EndpointId::Both => {
                row.push(u.to_string());
                row.push(v.to_string());
                row.push(feature_id_of(records, u));
                row.push(feature_id_of(records, v));
            }
        }
        if !weights.is_empty() {
            let scores = match (records.get(u), records.get(v)) {
                (Some(left), Some(right)) => pairwise_similarities(left, right, params),
                _ => Vec::new(),
            };
            for &method in weights {
                let value = scores
                    .iter()
                    .find(|(candidate, _)| *candidate == method)
                    .and_then(|(_, score)| *score);
                row.push(value.map(|score| score.to_string()).unwrap_or_default());
            }
        }
        write_row(&mut out, &row, separator);
    }
    out
}

/// A typed Parquet column: the values to write and, for `optional` fields, the
/// definition levels (1 = present, 0 = null). For `optional` columns `values`
/// holds only the present entries.
enum ColumnData {
    /// A 64-bit integer column.
    Int64 {
        /// Present values.
        values: Vec<i64>,
        /// Definition levels for an optional column, or `None` when required.
        def: Option<Vec<i16>>,
    },
    /// A 64-bit float column.
    Double {
        /// Present values.
        values: Vec<f64>,
        /// Definition levels for an optional column, or `None` when required.
        def: Option<Vec<i16>>,
    },
    /// A UTF-8 byte-array column.
    Bytes {
        /// Present values.
        values: Vec<ByteArray>,
        /// Definition levels for an optional column, or `None` when required.
        def: Option<Vec<i16>>,
    },
}

/// Widens an optional count to a required `i64`, defaulting absent entries to 0.
fn as_i64(value: Option<usize>) -> i64 {
    value.unwrap_or(0) as i64
}

/// Builds an optional UTF-8 column from an iterator of `Option<String>`.
fn opt_utf8<I: Iterator<Item = Option<String>>>(iter: I) -> ColumnData {
    let mut values = Vec::new();
    let mut def = Vec::new();
    for item in iter {
        match item {
            Some(text) => {
                values.push(ByteArray::from(text.into_bytes()));
                def.push(1);
            }
            None => def.push(0),
        }
    }
    ColumnData::Bytes {
        values,
        def: Some(def),
    }
}

/// Builds a required `f64` column from an iterator of `f64`.
fn req_f64<I: Iterator<Item = f64>>(iter: I) -> ColumnData {
    ColumnData::Double {
        values: iter.collect(),
        def: None,
    }
}

/// Builds a required `i64` column from an iterator of `i64`.
fn req_i64<I: Iterator<Item = i64>>(iter: I) -> ColumnData {
    ColumnData::Int64 {
        values: iter.collect(),
        def: None,
    }
}

/// Serializes `columns` (in order) into a single-row-group Parquet file under
/// the given message-type `schema`. Uncompressed, so no codec dependency.
fn write_parquet(schema: &str, columns: &[ColumnData]) -> Result<Vec<u8>, String> {
    let parsed = parse_message_type(schema).map_err(|error| error.to_string())?;
    let properties = Arc::new(WriterProperties::builder().build());
    let mut buffer: Vec<u8> = Vec::new();
    {
        let mut writer = SerializedFileWriter::new(&mut buffer, Arc::new(parsed), properties)
            .map_err(|error| error.to_string())?;
        let mut row_group = writer.next_row_group().map_err(|error| error.to_string())?;
        for column in columns {
            let mut writer = row_group
                .next_column()
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "schema has fewer columns than data".to_string())?;
            match column {
                ColumnData::Int64 { values, def } => {
                    writer
                        .typed::<Int64Type>()
                        .write_batch(values, def.as_deref(), None)
                }
                ColumnData::Double { values, def } => {
                    writer
                        .typed::<DoubleType>()
                        .write_batch(values, def.as_deref(), None)
                }
                ColumnData::Bytes { values, def } => {
                    writer
                        .typed::<ByteArrayType>()
                        .write_batch(values, def.as_deref(), None)
                }
            }
            .map_err(|error| error.to_string())?;
            writer.close().map_err(|error| error.to_string())?;
        }
        row_group.close().map_err(|error| error.to_string())?;
        writer.close().map_err(|error| error.to_string())?;
    }
    Ok(buffer)
}

/// Builds the node list as a Parquet file: `node_id` plus the selected columns.
pub fn build_node_parquet(
    records: &[MascotGenericFormat],
    graph: &SimilarityGraph,
    columns: &[NodeColumn],
) -> Result<Vec<u8>, String> {
    let degrees = node_degrees(graph);
    let mut fields = String::from("required int64 node_id;");
    for column in columns {
        fields.push_str(column.parquet_field());
    }
    let schema = format!("message node {{ {fields} }}");

    let mut data = Vec::with_capacity(columns.len() + 1);
    data.push(ColumnData::Int64 {
        values: (0..records.len() as i64).collect(),
        def: None,
    });
    for &column in columns {
        data.push(column.parquet_data(records, graph, &degrees));
    }
    write_parquet(&schema, &data)
}

/// Builds the weighted edge list as a Parquet file.
pub fn build_edge_parquet(
    records: &[MascotGenericFormat],
    graph: &SimilarityGraph,
    params: &GraphParams,
    endpoint: EndpointId,
    weights: &[SimilarityMethod],
) -> Result<Vec<u8>, String> {
    let mut fields = String::new();
    match endpoint {
        EndpointId::Index => fields.push_str("required int64 source; required int64 target;"),
        EndpointId::FeatureId => fields.push_str(
            "optional binary source_feature_id (UTF8); optional binary target_feature_id (UTF8);",
        ),
        EndpointId::Both => fields.push_str(
            "required int64 source; required int64 target; \
             optional binary source_feature_id (UTF8); optional binary target_feature_id (UTF8);",
        ),
    }
    for method in weights {
        fields.push_str(&format!(
            "optional double {};",
            method.id().replace('-', "_")
        ));
    }
    let schema = format!("message edge {{ {fields} }}");

    let mut source = Vec::new();
    let mut target = Vec::new();
    let mut source_fid = (Vec::<ByteArray>::new(), Vec::<i16>::new());
    let mut target_fid = (Vec::<ByteArray>::new(), Vec::<i16>::new());
    let mut weight_values: Vec<Vec<f64>> = vec![Vec::new(); weights.len()];
    let mut weight_def: Vec<Vec<i16>> = vec![Vec::new(); weights.len()];
    let need_feature_id = endpoint.uses_feature_id();

    for &(u, v, _) in &graph.edges {
        source.push(u as i64);
        target.push(v as i64);
        if need_feature_id {
            push_feature_id(records, u, &mut source_fid);
            push_feature_id(records, v, &mut target_fid);
        }
        if !weights.is_empty() {
            let scores = match (records.get(u), records.get(v)) {
                (Some(left), Some(right)) => pairwise_similarities(left, right, params),
                _ => Vec::new(),
            };
            for (slot, &method) in weights.iter().enumerate() {
                match scores
                    .iter()
                    .find(|(candidate, _)| *candidate == method)
                    .and_then(|(_, score)| *score)
                {
                    Some(value) => {
                        weight_values[slot].push(value);
                        weight_def[slot].push(1);
                    }
                    None => weight_def[slot].push(0),
                }
            }
        }
    }

    let mut data = Vec::new();
    match endpoint {
        EndpointId::Index => {
            data.push(ColumnData::Int64 {
                values: source,
                def: None,
            });
            data.push(ColumnData::Int64 {
                values: target,
                def: None,
            });
        }
        EndpointId::FeatureId => {
            data.push(ColumnData::Bytes {
                values: source_fid.0,
                def: Some(source_fid.1),
            });
            data.push(ColumnData::Bytes {
                values: target_fid.0,
                def: Some(target_fid.1),
            });
        }
        EndpointId::Both => {
            data.push(ColumnData::Int64 {
                values: source,
                def: None,
            });
            data.push(ColumnData::Int64 {
                values: target,
                def: None,
            });
            data.push(ColumnData::Bytes {
                values: source_fid.0,
                def: Some(source_fid.1),
            });
            data.push(ColumnData::Bytes {
                values: target_fid.0,
                def: Some(target_fid.1),
            });
        }
    }
    for (values, def) in weight_values.into_iter().zip(weight_def) {
        data.push(ColumnData::Double {
            values,
            def: Some(def),
        });
    }
    write_parquet(&schema, &data)
}

/// Appends one endpoint's feature id (value and definition level) to a column.
fn push_feature_id(
    records: &[MascotGenericFormat],
    index: usize,
    column: &mut (Vec<ByteArray>, Vec<i16>),
) {
    match records
        .get(index)
        .and_then(|record| record.metadata().feature_id())
    {
        Some(id) => {
            column.0.push(ByteArray::from(id.as_bytes().to_vec()));
            column.1.push(1);
        }
        None => column.1.push(0),
    }
}

/// Saves a `Blob` to disk via a temporary object URL and a hidden `<a download>`.
fn save_blob(filename: &str, blob: &web_sys::Blob) {
    use wasm_bindgen::JsCast;

    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    let Ok(url) = web_sys::Url::create_object_url_with_blob(blob) else {
        return;
    };
    if let Ok(element) = document.create_element("a") {
        if let Ok(anchor) = element.dyn_into::<web_sys::HtmlAnchorElement>() {
            anchor.set_href(&url);
            anchor.set_download(filename);
            anchor.click();
        }
    }
    let _ = web_sys::Url::revoke_object_url(&url);
}

/// Triggers a browser download of text `content` as `filename`.
pub fn download_text(filename: &str, mime: &str, content: &str) {
    use wasm_bindgen::JsValue;

    let parts = js_sys::Array::of1(&JsValue::from_str(content));
    let options = web_sys::BlobPropertyBag::new();
    options.set_type(mime);
    if let Ok(blob) = web_sys::Blob::new_with_str_sequence_and_options(parts.as_ref(), &options) {
        save_blob(filename, &blob);
    }
}

/// Triggers a browser download of binary `bytes` as `filename`.
pub fn download_bytes(filename: &str, mime: &str, bytes: &[u8]) {
    let array = js_sys::Uint8Array::from(bytes);
    let parts = js_sys::Array::of1(array.as_ref());
    let options = web_sys::BlobPropertyBag::new();
    options.set_type(mime);
    if let Ok(blob) =
        web_sys::Blob::new_with_u8_array_sequence_and_options(parts.as_ref(), &options)
    {
        save_blob(filename, &blob);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataset::{parse_mgf, Records};
    use crate::similarity::SimilarityGraph;

    fn record_block(feature_id: &str, pepmass: &str) -> String {
        format!(
            "BEGIN IONS\nFEATURE_ID={feature_id}\nPEPMASS={pepmass}\nCHARGE=1+\nMSLEVEL=2\n\
             100.0 10.0\n150.0 20.0\n200.0 5.0\nEND IONS\n"
        )
    }

    fn two_record_graph() -> (Records, SimilarityGraph) {
        let text = format!(
            "{}{}",
            record_block("1", "200.0"),
            record_block("2", "210.0")
        );
        let (records, _) = parse_mgf(&text);
        let graph = SimilarityGraph {
            node_count: records.as_ref().len(),
            edges: vec![(0, 1, 0.5)],
            component_count: 1,
            community_of_node: vec![0, 0],
            community_count: 1,
            leiden_of_node: vec![0, 1],
            leiden_count: 2,
            coordinates: vec![[0.0, 0.0], [1.0, 1.0]],
        };
        (records, graph)
    }

    #[test]
    fn escape_quotes_only_when_needed() {
        assert_eq!(escape("plain", ','), "plain");
        assert_eq!(escape("a,b", ','), "\"a,b\"");
        assert_eq!(escape("a,b", '\t'), "a,b");
        assert_eq!(escape("a\"b", ','), "\"a\"\"b\"");
        assert_eq!(escape("a\nb", ','), "\"a\nb\"");
    }

    #[test]
    fn node_table_header_and_row_count() {
        let (records, graph) = two_record_graph();
        let records = records.as_ref();
        let columns = [
            NodeColumn::LouvainCommunity,
            NodeColumn::PeakCount,
            NodeColumn::Degree,
        ];
        let table = build_node_table(records, &graph, &columns, Delimiter::Comma);
        let lines: Vec<&str> = table.lines().collect();
        assert_eq!(lines.len(), records.len() + 1);
        assert_eq!(lines[0], "node_id,louvain_community,peak_count,degree");
        // First data row: node_id 0, Louvain community 0, peaks 3, degree 1.
        assert_eq!(lines[1], "0,0,3,1");
    }

    #[test]
    fn node_table_uses_tab_delimiter() {
        let (records, graph) = two_record_graph();
        let table = build_node_table(
            records.as_ref(),
            &graph,
            &[NodeColumn::MgfMetadata],
            Delimiter::Tab,
        );
        assert_eq!(table.lines().next(), Some("node_id\tmgf_metadata"));
    }

    #[test]
    fn mgf_metadata_is_a_passthrough_dump() {
        let (records, _) = two_record_graph();
        let value = mgf_metadata_string(&records.as_ref()[0]);
        // Verbatim file fields, not computed: feature id, precursor m/z, charge.
        assert!(value.contains("FEATURE_ID=1"));
        assert!(value.contains("PEPMASS="));
        assert!(value.contains("CHARGE=1"));
    }

    #[test]
    fn edge_table_columns_and_row_count() {
        let (records, graph) = two_record_graph();
        let params = GraphParams::default();
        let weights = [SimilarityMethod::ModifiedCosine, SimilarityMethod::Entropy];
        let table = build_edge_table(
            records.as_ref(),
            &graph,
            &params,
            EndpointId::Both,
            &weights,
            Delimiter::Comma,
        );
        let lines: Vec<&str> = table.lines().collect();
        assert_eq!(lines.len(), graph.edges.len() + 1);
        // Weight headers are snake_case, matching the node columns.
        assert_eq!(
            lines[0],
            "source,target,source_feature_id,target_feature_id,modified_cosine,entropy"
        );
        // source=0, target=1, feature ids 1 and 2.
        assert!(lines[1].starts_with("0,1,1,2,"));
        // Six columns present.
        assert_eq!(lines[1].split(',').count(), 6);
    }

    #[test]
    fn feature_id_issue_flags_missing_and_duplicates() {
        // Unique ids: no issue.
        let (unique, _) = two_record_graph();
        assert!(feature_id_issue(unique.as_ref()).is_none());

        // Duplicate ids.
        let dup_text = format!(
            "{}{}",
            record_block("1", "200.0"),
            record_block("1", "210.0")
        );
        let (dups, _) = parse_mgf(&dup_text);
        assert!(feature_id_issue(dups.as_ref()).is_some());
    }

    #[test]
    fn node_parquet_round_trips() {
        let (records, graph) = two_record_graph();
        let columns = [
            NodeColumn::LouvainCommunity,
            NodeColumn::PeakCount,
            NodeColumn::IntensityEntropy,
            NodeColumn::MgfMetadata,
        ];
        let bytes = build_node_parquet(records.as_ref(), &graph, &columns).expect("parquet");
        let (rows, cols) = parquet_shape(&bytes, "mascot_node_test.parquet");
        assert_eq!(rows, records.as_ref().len() as i64);
        // node_id plus the four selected columns.
        assert_eq!(cols, columns.len() + 1);
    }

    #[test]
    fn edge_parquet_round_trips() {
        let (records, graph) = two_record_graph();
        let params = GraphParams::default();
        let weights = [SimilarityMethod::ModifiedCosine, SimilarityMethod::Entropy];
        let bytes = build_edge_parquet(
            records.as_ref(),
            &graph,
            &params,
            EndpointId::Both,
            &weights,
        )
        .expect("parquet");
        let (rows, cols) = parquet_shape(&bytes, "mascot_edge_test.parquet");
        assert_eq!(rows, graph.edges.len() as i64);
        // source, target, source_feature_id, target_feature_id, plus two weights.
        assert_eq!(cols, 6);
    }

    /// Reads back a Parquet buffer with the reference reader, returning
    /// `(num_rows, num_columns)`. Proves the file parses, not just that it has
    /// the right magic bytes.
    fn parquet_shape(bytes: &[u8], name: &str) -> (i64, usize) {
        use parquet::file::reader::{FileReader, SerializedFileReader};

        assert_eq!(&bytes[..4], b"PAR1", "missing leading magic");
        assert_eq!(&bytes[bytes.len() - 4..], b"PAR1", "missing trailing magic");
        let path = std::env::temp_dir().join(name);
        std::fs::write(&path, bytes).expect("write temp parquet");
        let file = std::fs::File::open(&path).expect("open temp parquet");
        let reader = SerializedFileReader::new(file).expect("read parquet");
        let metadata = reader.metadata().file_metadata();
        let shape = (metadata.num_rows(), metadata.schema_descr().num_columns());
        let _ = std::fs::remove_file(&path);
        shape
    }
}
