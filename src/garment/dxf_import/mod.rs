//! Bounded ASCII DXF geometry reader, not an AAMA/ASTM garment importer.
//!
//! Model-space LINE, ARC, CIRCLE, LWPOLYLINE and legacy 2D POLYLINE/VERTEX are
//! accepted on XY with the default +Z extrusion. Layers and entity handles are
//! retained with surrounding whitespace trimmed; cut/sew/notch semantics are
//! never inferred or combined. Input must already be decoded UTF-8 text.
//! Non-HEADER/ENTITIES sections are ignored; block insertions are unsupported.
//! Legacy vertices inherit their parent polyline's layer in the returned path.
//! Closed flags add a closing segment; open paths remain open (closure tolerance
//! is zero). Circular segments are sampled to `chord_error_m`, not fitted curves.
//! Unknown/unsupported entities produce issues. Callers MUST show these issues
//! before using partial geometry; parsing alone does not validate a cloth panel.
//! No document, mesh or solver is modified. HEADER units are read from INSUNITS;
//! unitless/unknown drawings require an explicit metres-per-unit override.
//!
//! Independently implemented from Autodesk's DXF reference (consulted 2026-10-26):
//! <https://help.autodesk.com/cloudhelp/2026/ENU/AutoCAD-DXF/files/GUID-A85E8E67-27CD-4C59-BE61-4DC9FADBE74A.htm>
//! <https://help.autodesk.com/cloudhelp/2018/ENU/AutoCAD-DXF/files/GUID-748FC305-F3F2-4F74-825A-61F04D757A50.htm>
//! <https://help.autodesk.com/cloudhelp/2024/ENU/AutoCAD-DXF/files/GUID-0741E831-599E-4CBF-91E1-8ADBCFD6556D.htm>
//! <https://help.autodesk.com/cloudhelp/2024/ENU/AutoCAD-DXF/files/GUID-0B14D8F1-0EBA-44BF-9108-57D8CE614BC8.htm>
pub mod fit;

use glam::DVec2;
use std::f64::consts::{FRAC_PI_2, TAU};

const MAX_BYTES: usize = 16 * 1024 * 1024;
const MAX_POINTS: usize = 1_000_000;
const MAX_PATH_POINTS: usize = 100_000;

#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// Explicit user scale overrides header units and is recorded in the result.
    pub metres_per_unit: Option<f64>,
    pub chord_error_m: f64,
    /// Collapse dense samples toward this many nodes per contour (0 disables fitting).
    pub node_budget: usize,
    /// Fidelity limit for a fitted contour against its source samples, metres.
    pub fit_tolerance_m: f64,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            metres_per_unit: None,
            chord_error_m: 0.0001,
            node_budget: fit::DEFAULT_NODE_BUDGET,
            fit_tolerance_m: 0.0005,
        }
    }
}

#[derive(Debug)]
pub struct Path {
    pub entity: String,
    pub handle: Option<String>,
    pub layer: String,
    /// Piece name from the AAMA-style record inside its block, if any.
    pub piece: Option<String>,
    /// Metres, with a repeated first point only for closed paths.
    pub points: Vec<DVec2>,
    pub closed: bool,
    /// Cubic control points per simplified edge, when fitting ran. Keyed by
    /// the index of the edge start within `fitted_points`; absent means straight.
    pub curves: Vec<(usize, [DVec2; 2])>,
    /// Simplified node positions actually used for panels, when fitting ran.
    pub fitted_points: Option<Vec<DVec2>>,
    /// Sample count before fitting, for the report.
    pub source_nodes: usize,
}

#[derive(Debug)]
pub struct Issue {
    pub entity: String,
    pub handle: Option<String>,
    pub layer: String,
    pub reason: String,
}

#[derive(Debug)]
pub struct Drawing {
    pub paths: Vec<Path>,
    pub issues: Vec<Issue>,
    pub insunits: Option<i32>,
    pub metres_per_unit: f64,
    pub scale_overridden: bool,
    /// SAMPLE SIZE record, when the file carries one.
    pub sample_size: Option<String>,
    /// UNITS record, when the file carries one.
    pub unit_record: Option<String>,
}

#[derive(Clone, Copy)]
struct Pair<'a> {
    code: i32,
    value: &'a str,
}

fn field<'a>(pairs: &[Pair<'a>], code: i32) -> Result<Option<&'a str>, String> {
    let mut values = pairs.iter().filter(|p| p.code == code);
    let first = values.next().map(|p| p.value);
    if values.next().is_some() {
        return Err(format!("Duplicate group {code}"));
    }
    Ok(first)
}
fn number(pairs: &[Pair<'_>], code: i32, default: Option<f64>) -> Result<f64, String> {
    let value = match field(pairs, code)? {
        Some(value) => value
            .parse::<f64>()
            .map_err(|_| format!("Invalid group {code}"))?,
        None => default.ok_or_else(|| format!("Missing group {code}"))?,
    };
    if !value.is_finite() {
        return Err(format!("Nonfinite group {code}"));
    }
    Ok(value)
}
fn integer(pairs: &[Pair<'_>], code: i32, default: i32) -> Result<i32, String> {
    field(pairs, code)?.map_or(Ok(default), |v| {
        v.parse()
            .map_err(|_| format!("Invalid integer group {code}"))
    })
}
fn point(pairs: &[Pair<'_>], x: i32, scale: f64) -> Result<DVec2, String> {
    let p = DVec2::new(number(pairs, x, None)?, number(pairs, x + 10, None)?) * scale;
    if !p.is_finite() {
        return Err("Coordinate overflow after unit conversion".into());
    }
    Ok(p)
}
fn planar(pairs: &[Pair<'_>]) -> Result<(), String> {
    for code in [30, 31, 38, 39] {
        if number(pairs, code, Some(0.))? != 0. {
            return Err("Nonzero Z/elevation/thickness is unsupported".into());
        }
    }
    if number(pairs, 210, Some(0.))? != 0.
        || number(pairs, 220, Some(0.))? != 0.
        || number(pairs, 230, Some(1.))? != 1.
    {
        return Err("Only default +Z extrusion is supported".into());
    }
    if integer(pairs, 67, 0)? != 0 {
        return Err("Paper-space entities are unsupported".into());
    }
    Ok(())
}
fn push(points: &mut Vec<DVec2>, p: DVec2) -> Result<(), String> {
    if !p.is_finite() {
        return Err("Nonfinite geometry".into());
    }
    if points.len() >= MAX_PATH_POINTS {
        return Err("Path point budget exceeded".into());
    }
    points.push(p);
    Ok(())
}
fn arc(
    points: &mut Vec<DVec2>,
    center: DVec2,
    radius: f64,
    start: f64,
    sweep: f64,
    error: f64,
) -> Result<(), String> {
    if !radius.is_finite() || radius <= 0. || !center.is_finite() {
        return Err("Invalid circle radius/center".into());
    }
    let angle = (4. * (error / (2. * radius)).min(1.).sqrt().asin()).min(FRAC_PI_2);
    let count = (sweep.abs() / angle).ceil().max(1.);
    if !count.is_finite() || count > (MAX_PATH_POINTS - points.len()) as f64 {
        return Err("Curve sampling budget exceeded".into());
    }
    let count = count as usize;
    for k in 1..=count {
        let a = start + sweep * k as f64 / count as f64;
        push(points, center + radius * DVec2::new(a.cos(), a.sin()))?;
    }
    Ok(())
}
fn polyline(vertices: &[(DVec2, f64)], closed: bool, error: f64) -> Result<Vec<DVec2>, String> {
    if vertices.len() < 2 {
        return Err("Polyline needs at least two vertices".into());
    }
    let mut out = vec![vertices[0].0];
    let segments = vertices.len() - usize::from(!closed);
    for i in 0..segments {
        let (a, bulge) = vertices[i];
        let b = vertices[(i + 1) % vertices.len()].0;
        let chord = b - a;
        let length = chord.length();
        if !length.is_finite() {
            return Err("Polyline segment length overflow".into());
        }
        if length == 0. && bulge != 0. {
            return Err("Bulged segment has coincident endpoints".into());
        }
        if bulge == 0. || bulge.abs() * length * 0.5 <= error {
            push(&mut out, b)?;
        } else {
            let normal = DVec2::new(-chord.y, chord.x) / length;
            let center = a + chord * 0.5 + normal * (length * 0.25 * (1. / bulge - bulge));
            let radius = length * 0.25 * (bulge.abs() + 1. / bulge.abs());
            let start = (a - center).y.atan2((a - center).x);
            arc(&mut out, center, radius, start, 4. * bulge.atan(), error)?;
            *out.last_mut().unwrap() = b;
        }
    }
    Ok(out)
}

fn geometry(
    kind: &str,
    pairs: &[Pair<'_>],
    vertices: &[Vec<Pair<'_>>],
    scale: f64,
    error: f64,
) -> Result<(Vec<DVec2>, bool), String> {
    planar(pairs)?;
    match kind {
        "LINE" => Ok((
            vec![point(pairs, 10, scale)?, point(pairs, 11, scale)?],
            false,
        )),
        "CIRCLE" | "ARC" => {
            let center = point(pairs, 10, scale)?;
            let radius = number(pairs, 40, None)? * scale;
            let start = if kind == "ARC" {
                number(pairs, 50, None)?.to_radians()
            } else {
                0.
            };
            let sweep = if kind == "ARC" {
                (number(pairs, 51, None)?.to_radians() - start).rem_euclid(TAU)
            } else {
                TAU
            };
            if !start.is_finite() || !sweep.is_finite() || sweep == 0. {
                return Err("Invalid or ambiguous arc angles".into());
            }
            let mut out = vec![];
            push(
                &mut out,
                center + radius * DVec2::new(start.cos(), start.sin()),
            )?;
            arc(&mut out, center, radius, start, sweep, error)?;
            if kind == "CIRCLE" {
                *out.last_mut().unwrap() = out[0];
            }
            Ok((out, kind == "CIRCLE"))
        }
        "LWPOLYLINE" | "POLYLINE" => {
            let flags = integer(pairs, 70, 0)?;
            if flags & !129 != 0 {
                return Err("3D, mesh or fitted polylines are unsupported".into());
            }
            for code in [40, 41, 43] {
                if pairs
                    .iter()
                    .any(|p| p.code == code && p.value.parse::<f64>() != Ok(0.))
                {
                    return Err("Polyline stroke width is unsupported".into());
                }
            }
            let chunks: Vec<Vec<Pair<'_>>>;
            let records = if kind == "LWPOLYLINE" {
                let mut v: Vec<Vec<Pair<'_>>> = vec![];
                for p in pairs {
                    if p.code == 10 {
                        v.push(vec![]);
                    }
                    if matches!(p.code, 10 | 20 | 42) {
                        v.last_mut()
                            .ok_or("Vertex field before its X coordinate")?
                            .push(*p);
                    }
                }
                let count = integer(pairs, 90, -1)?;
                if count < 0 || count as usize != v.len() {
                    return Err("Polyline vertex count mismatch".into());
                }
                chunks = v;
                &chunks
            } else {
                vertices
            };
            let mut values = vec![];
            for v in records {
                planar(v)?;
                if integer(v, 70, 0)? != 0 {
                    return Err("Fitted/3D vertices are unsupported".into());
                }
                if number(v, 40, Some(0.))? != 0. || number(v, 41, Some(0.))? != 0. {
                    return Err("Vertex stroke width is unsupported".into());
                }
                values.push((point(v, 10, scale)?, number(v, 42, Some(0.))?));
            }
            let closed = flags & 1 != 0;
            Ok((polyline(&values, closed, error)?, closed))
        }
        _ => Err("Unsupported entity type".into()),
    }
}


/// One entity as owned group pairs, so a block definition can be replayed
/// many times with different INSERT transforms.
#[derive(Clone, Debug)]
struct RawEntity {
    kind: String,
    pairs: Vec<(i32, String)>,
    vertices: Vec<Vec<(i32, String)>>,
}

fn as_pairs(owned: &[(i32, String)]) -> Vec<Pair<'_>> {
    owned
        .iter()
        .map(|(code, value)| Pair {
            code: *code,
            value: value,
        })
        .collect()
}

/// Parse a bounded slice of group pairs into entities. Shared by ENTITIES and
/// every BLOCK definition, so a block entity is read exactly like a model one.
fn parse_entity_list(pairs: &[Pair<'_>]) -> Result<Vec<RawEntity>, String> {
    let mut out = vec![];
    let mut i = 0;
    while i < pairs.len() {
        if pairs[i].code != 0 {
            return Err("Entity must start with group 0".into());
        }
        let kind = pairs[i].value.to_owned();
        i += 1;
        let start = i;
        while i < pairs.len() && pairs[i].code != 0 {
            i += 1;
        }
        let fields = pairs[start..i]
            .iter()
            .map(|p| (p.code, p.value.to_owned()))
            .collect::<Vec<_>>();
        let mut vertices = vec![];
        if kind == "POLYLINE" {
            while i < pairs.len() && pairs[i].value == "VERTEX" {
                i += 1;
                let start = i;
                while i < pairs.len() && pairs[i].code != 0 {
                    i += 1;
                }
                vertices.push(
                    pairs[start..i]
                        .iter()
                        .map(|p| (p.code, p.value.to_owned()))
                        .collect(),
                );
            }
            if i >= pairs.len() || pairs[i].value != "SEQEND" {
                return Err("POLYLINE missing SEQEND".into());
            }
            i += 1;
            while i < pairs.len() && pairs[i].code != 0 {
                i += 1;
            }
        }
        out.push(RawEntity {
            kind,
            pairs: fields,
            vertices,
        });
    }
    Ok(out)
}

#[derive(Clone, Debug)]
struct Block {
    base: DVec2,
    entities: Vec<RawEntity>,
    /// AAMA-style piece record carried by the block, if the file has one.
    piece: Option<String>,
}

/// Record prefixes the industry writes as TEXT near or inside a piece.
const PIECE_RECORD: &str = "PIECE NAME:";
const SAMPLE_RECORD: &str = "SAMPLE SIZE:";
const UNITS_RECORD: &str = "UNITS:";

fn record_value_of(entity: &RawEntity, prefix: &str) -> Option<String> {
    if entity.kind != "TEXT" {
        return None;
    }
    let text = entity.pairs.iter().find(|(code, _)| *code == 1)?.1.as_str();
    text.trim()
        .strip_prefix(prefix)
        .map(|rest| rest.trim().to_owned())
        .filter(|rest| !rest.is_empty())
}

fn is_record_line(entity: &RawEntity) -> bool {
    record_value_of(entity, PIECE_RECORD).is_some()
        || record_value_of(entity, SAMPLE_RECORD).is_some()
        || record_value_of(entity, UNITS_RECORD).is_some()
}

struct Blocks {
    entries: Vec<(String, Block)>,
    sample_size: Option<String>,
    unit_record: Option<String>,
}

fn parse_blocks(content: &[Pair<'_>], scale: f64) -> Result<Blocks, String> {
    let mut blocks = vec![];
    let mut sample_size = None;
    let mut unit_record = None;
    let mut i = 0;
    while i < content.len() {
        if content[i].code != 0 || content[i].value != "BLOCK" {
            return Err("Expected BLOCK in BLOCKS section".into());
        }
        i += 1;
        let start = i;
        while i < content.len() && content[i].code != 0 {
            i += 1;
        }
        let header = &content[start..i];
        let name = field(header, 2)?.ok_or("BLOCK without a name")?.to_owned();
        let base = point(header, 10, scale)?;
        let body_start = i;
        while i < content.len() && !(content[i].code == 0 && content[i].value == "ENDBLK") {
            i += 1;
        }
        if i == content.len() {
            return Err("BLOCK missing ENDBLK".into());
        }
        let mut entities = parse_entity_list(&content[body_start..i])?;
        let mut piece = None;
        entities.retain(|entity| {
            if let Some(value) = record_value_of(entity, PIECE_RECORD) {
                piece = Some(value);
                return false;
            }
            if let Some(value) = record_value_of(entity, SAMPLE_RECORD) {
                sample_size.get_or_insert(value);
                return false;
            }
            if let Some(value) = record_value_of(entity, UNITS_RECORD) {
                unit_record.get_or_insert(value);
                return false;
            }
            true
        });
        i += 1;
        while i < content.len() && content[i].code != 0 {
            i += 1;
        }
        blocks.push((
            name,
            Block {
                base,
                entities,
                piece,
            },
        ));
    }
    Ok(Blocks {
        entries: blocks,
        sample_size,
        unit_record,
    })
}

/// Uniform-scale, rotation and translation of a block instance.
#[derive(Clone, Copy)]
struct Xform {
    scale: f64,
    cos: f64,
    sin: f64,
    translation: DVec2,
}
impl Xform {
    fn identity() -> Self {
        Self {
            scale: 1.,
            cos: 1.,
            sin: 0.,
            translation: DVec2::ZERO,
        }
    }
    fn apply(&self, p: DVec2) -> DVec2 {
        let x = p.x * self.scale;
        let y = p.y * self.scale;
        DVec2::new(
            x * self.cos - y * self.sin + self.translation.x,
            x * self.sin + y * self.cos + self.translation.y,
        )
    }
    /// self after inner.
    fn compose(&self, inner: &Xform) -> Xform {
        let angle = self.sin.atan2(self.cos) + inner.sin.atan2(inner.cos);
        Xform {
            scale: self.scale * inner.scale,
            cos: angle.cos(),
            sin: angle.sin(),
            translation: self.apply(inner.translation),
        }
    }
}

const MAX_BLOCK_DEPTH: usize = 4;

struct Expansion<'a> {
    blocks: &'a [(String, Block)],
    piece: Option<String>,
    scale: f64,
    error: f64,
    node_budget: usize,
    fit_tolerance_m: f64,
    paths: Vec<Path>,
    issues: Vec<Issue>,
    total: usize,
}

impl Expansion<'_> {
    fn emit(
        &mut self,
        kind: &str,
        fields: &[Pair<'_>],
        vertices: &[Vec<Pair<'_>>],
        xform: &Xform,
        layer: &str,
        handle: Option<String>,
        depth: usize,
    ) {
        match geometry(kind, fields, vertices, self.scale, self.error / xform.scale.abs()) {
            Ok((points, closed)) => {
                let mut path = Path {
                    entity: kind.into(),
                    handle,
                    layer: layer.to_owned(),
                    piece: self.piece.clone(),
                    points: points.into_iter().map(|p| xform.apply(p)).collect(),
                    closed,
                    curves: vec![],
                    fitted_points: None,
                    source_nodes: 0,
                };
                path.source_nodes = path.points.len();
                if self.node_budget > 0 && path.closed && path.points.len() > 8 {
                    // The budget is a goal, never a licence to distort: nodes
                    // are dropped only while every kept edge stays within the
                    // fidelity limit of the source samples. A corner-heavy
                    // contour may keep more nodes than the budget asks for.
                    let mut best: Option<(Vec<DVec2>, Vec<(usize, [DVec2; 2])>)> = None;
                    for step in 0..8u32 {
                        let tolerance = self.fit_tolerance_m * (1u32 << step) as f64;
                        let simplified = fit::simplify(&path.points, tolerance);
                        if simplified.len() < 4 {
                            break;
                        }
                        let edges = fit::fit_edges(&simplified, true);
                        let mut curves = Vec::new();
                        let mut faithful = true;
                        for (i, e) in edges.iter().enumerate() {
                            // Measure the edge against its own span of source
                            // samples only. The full ring contains distant
                            // geometry that every straight edge would "miss".
                            let start_index = simplified
                                .iter()
                                .position(|p| *p == e.start)
                                .unwrap_or(i);
                            let end_index = if i + 1 >= simplified.len() {
                                path.points.len() - 1
                            } else {
                                let end_point = simplified[i + 1];
                                path.points[start_index..]
                                    .iter()
                                    .position(|p| *p == end_point)
                                    .map(|k| start_index + k)
                                    .unwrap_or(start_index + 1)
                            };
                            let span = &path.points[start_index.min(end_index)..=end_index.max(start_index)];
                            let dev = fit::max_deviation(span, std::slice::from_ref(e), 16);
                            if dev > self.fit_tolerance_m {
                                faithful = false;
                                break;
                            }
                            if e.c1 != e.start || e.c2 != e.end {
                                curves.push((i, [e.c1, e.c2]));
                            }
                        }
                        if !faithful {
                            continue;
                        }
                        let count = simplified.len();
                        let candidate = (simplified, curves);
                        match &best {
                            Some((previous, _)) if previous.len() <= count => {}
                            _ => best = Some(candidate),
                        }
                        if count <= self.node_budget {
                            break;
                        }
                    }
                    if let Some((simplified, curves)) = best {
                        path.fitted_points = Some(simplified);
                        path.curves = curves;
                    }
                }
                self.total += path.points.len();
                self.paths.push(path);
            }
            Err(reason) => self.issues.push(Issue {
                entity: kind.into(),
                handle,
                layer: layer.to_owned(),
                reason,
            }),
        }
        let _ = depth;
    }

    /// Expand a block instance. Nested inserts compose their transforms; the
    /// depth cap keeps a self-referencing block from looping.
    fn insert(
        &mut self,
        entities: &[RawEntity],
        xform: &Xform,
        inherited_layer: &str,
        piece: Option<&str>,
        depth: usize,
    ) {
        let previous = self.piece.take();
        if let Some(piece) = piece {
            self.piece = Some(piece.to_owned());
        }
        for raw in entities {
            let pairs = as_pairs(&raw.pairs);
            let layer = field(&pairs, 8)
                .ok()
                .flatten()
                .unwrap_or("0")
                .to_owned();
            let layer = if layer == "0" {
                inherited_layer.to_owned()
            } else {
                layer
            };
            let handle = field(&pairs, 5).ok().flatten().map(str::to_owned);
            if raw.kind == "INSERT" {
                if depth + 1 > MAX_BLOCK_DEPTH {
                    self.issues.push(Issue {
                        entity: "INSERT".into(),
                        handle,
                        layer,
                        reason: format!("Block nesting deeper than {MAX_BLOCK_DEPTH}"),
                    });
                    continue;
                }
                match self.instance(&pairs, xform) {
                    Ok((name, child)) => {
                        let local = self.blocks.iter().find(|(n, _)| *n == name);
                        match local {
                            Some((_, block)) => {
                                let entities = block.entities.clone();
                                let base = block.base;
                                let placed = Xform {
                                    translation: child.translation
                                        - DVec2::new(
                                            base.x * child.scale * child.cos
                                                - base.y * child.scale * child.sin,
                                            base.x * child.scale * child.sin
                                                + base.y * child.scale * child.cos,
                                        ),
                                    ..child
                                };
                                let combined = xform.compose(&placed);
                                let inner = block.piece.clone();
                                self.insert(&entities, &combined, &layer, inner.as_deref(), depth + 1);
                            }
                            None => self.issues.push(Issue {
                                entity: "INSERT".into(),
                                handle,
                                layer,
                                reason: format!("Unknown block {name}"),
                            }),
                        }
                    }
                    Err(reason) => self.issues.push(Issue {
                        entity: "INSERT".into(),
                        handle,
                        layer,
                        reason,
                    }),
                }
                continue;
            }
            let vertices = raw
                .vertices
                .iter()
                .map(|v| as_pairs(v))
                .collect::<Vec<_>>();
            if is_record_line(raw) {
                // A record line is metadata consumed by the reader; it is
                // never a contour and never an unsupported-entity issue.
                continue;
            }
            self.emit(&raw.kind, &pairs, &vertices, xform, &layer, handle, depth);
        }
        self.piece = previous;
    }

    /// Read an INSERT's placement in drawing space, already in metres.
    fn instance(
        &self,
        pairs: &[Pair<'_>],
        parent: &Xform,
    ) -> Result<(String, Xform), String> {
        let name = field(pairs, 2)?.ok_or("INSERT without a block name")?.to_owned();
        let sx = number(pairs, 41, Some(1.))?;
        let sy = number(pairs, 42, Some(sx))?;
        if !sx.is_finite() || !sy.is_finite() || sx == 0. || sy == 0. || (sx - sy).abs() > 1e-9 {
            return Err("Non-uniform or zero INSERT scale is unsupported".into());
        }
        let rotation = number(pairs, 50, Some(0.))?.to_radians();
        let insertion = point(pairs, 10, self.scale)?;
        let local = Xform {
            scale: sx,
            cos: rotation.cos(),
            sin: rotation.sin(),
            translation: insertion,
        };
        Ok((name, parent.compose(&local)))
    }
}

/// Parse into a reviewable geometry report. Structural or scale failures are
/// errors; rejected entities are retained as issues alongside accepted paths.
pub fn read_ascii(text: &str, options: Options) -> Result<Drawing, String> {
    if text.len() > MAX_BYTES || text.contains('\0') {
        return Err("Not bounded text DXF (16 MiB maximum)".into());
    }
    if !options.chord_error_m.is_finite() || options.chord_error_m <= 0. {
        return Err("Chord error must be positive finite metres".into());
    }
    let mut lines = text.trim_start_matches('\u{feff}').lines();
    let mut pairs = vec![];
    while let Some(code) = lines.next() {
        let value = lines.next().ok_or("Incomplete DXF group pair")?;
        let code = code
            .trim()
            .parse::<i32>()
            .map_err(|_| "Invalid DXF group code")?;
        if code != 999 {
            pairs.push(Pair {
                code,
                value: value.trim(),
            });
        }
    }
    let mut i = 0;
    let mut units = None;
    let mut entities: Option<&[Pair<'_>]> = None;
    let mut blocks_section: Option<&[Pair<'_>]> = None;
    let mut eof = false;
    while i < pairs.len() {
        if pairs[i].code == 0 && pairs[i].value == "EOF" {
            eof = true;
            i += 1;
            break;
        }
        if pairs[i].code != 0 || pairs[i].value != "SECTION" {
            return Err("Expected SECTION or EOF".into());
        }
        i += 1;
        let section = pairs
            .get(i)
            .filter(|p| p.code == 2)
            .ok_or("Missing section name")?
            .value;
        i += 1;
        let start = i;
        while i < pairs.len() && !(pairs[i].code == 0 && pairs[i].value == "ENDSEC") {
            i += 1;
        }
        if i == pairs.len() {
            return Err("Unclosed DXF section".into());
        }
        let content = &pairs[start..i];
        if section == "BLOCKS" {
            if blocks_section.replace(content).is_some() {
                return Err("Duplicate BLOCKS section".into());
            }
        } else if section == "ENTITIES" {
            if entities.replace(content).is_some() {
                return Err("Duplicate ENTITIES section".into());
            }
        } else if section == "HEADER" {
            for (j, p) in content.iter().enumerate() {
                if p.code == 9 && p.value == "$INSUNITS" {
                    if units.is_some() {
                        return Err("Duplicate INSUNITS".into());
                    }
                    units = Some(
                        content
                            .get(j + 1)
                            .filter(|p| p.code == 70)
                            .ok_or("Missing INSUNITS value")?
                            .value
                            .parse::<i32>()
                            .map_err(|_| "Invalid INSUNITS")?,
                    );
                }
            }
        }
        i += 1;
    }
    if !eof || i != pairs.len() {
        return Err("Missing EOF or trailing DXF content".into());
    }
    let scale = options
        .metres_per_unit
        .or(match units {
            Some(1) => Some(0.0254),
            Some(2) => Some(0.3048),
            Some(4) => Some(0.001),
            Some(5) => Some(0.01),
            Some(6) => Some(1.),
            _ => None,
        })
        .ok_or("Unknown/unitless scale: supply explicit metres_per_unit")?;
    if !scale.is_finite() || scale <= 0. {
        return Err("Scale must be positive and finite".into());
    }
    let entities = entities.ok_or("Missing ENTITIES section")?;
    let blocks = match blocks_section {
        Some(content) => parse_blocks(content, scale)?,
        None => Blocks {
            entries: vec![],
            sample_size: None,
            unit_record: None,
        },
    };
    let mut result = Drawing {
        paths: vec![],
        issues: vec![],
        insunits: units,
        metres_per_unit: scale,
        scale_overridden: options.metres_per_unit.is_some(),
        sample_size: blocks.sample_size.clone(),
        unit_record: blocks.unit_record.clone(),
    };
    let mut expansion = Expansion {
        blocks: &blocks.entries,
        piece: None,
        scale,
        error: options.chord_error_m,
        node_budget: options.node_budget,
        fit_tolerance_m: options.fit_tolerance_m,
        paths: vec![],
        issues: vec![],
        total: 0,
    };
    let model = parse_entity_list(entities)?;
    // Walk model entities in document order. AAMA record lines are metadata:
    // a PIECE NAME labels every outline until the next record, SAMPLE SIZE and
    // UNITS describe the drawing. None of them becomes a contour or an issue.
    let mut model_sample_size = blocks.sample_size.clone();
    let mut model_unit_record = blocks.unit_record.clone();
    let mut current_piece: Option<String> = None;
    for raw in &model {
        if let Some(value) = record_value_of(raw, SAMPLE_RECORD) {
            model_sample_size.get_or_insert(value);
            continue;
        }
        if let Some(value) = record_value_of(raw, UNITS_RECORD) {
            model_unit_record.get_or_insert(value);
            continue;
        }
        if let Some(value) = record_value_of(raw, PIECE_RECORD) {
            current_piece = Some(value);
            continue;
        }
        // insert() saves and restores the ambient piece around recursion, so
        // the running name must arrive as an argument, not as prior state.
        expansion.insert(
            std::slice::from_ref(raw),
            &Xform::identity(),
            "0",
            current_piece.as_deref(),
            0,
        );
    }
    if expansion.total > MAX_POINTS {
        return Err("Drawing point budget exceeded".into());
    }
    result.paths = expansion.paths;
    result.issues = expansion.issues;
    result.sample_size = model_sample_size;
    result.unit_record = model_unit_record;
    Ok(result)
}
