use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use rustle_lang::{DrawCommand, RenderMode, ShapeData, ShapeDesc, origin_offset};

use crate::atlas::AtlasData;
use crate::instance::{LineVertex, PolygonVertex, SdfInstance, TextVertex};

#[derive(Clone)]
pub struct PreparedFrame {
    pub(crate) sdf_instances: Vec<SdfInstance>,
    pub(crate) line_vertices: Vec<LineVertex>,
    pub(crate) line_indices: Vec<u32>,
    pub(crate) polygon_vertices: Vec<PolygonVertex>,
    pub(crate) polygon_indices: Vec<u32>,
    pub(crate) text_vertices: Vec<TextVertex>,
    pub(crate) text_indices: Vec<u32>,
}

/// Per-shape cached GPU data, keyed by a content hash of the ShapeData.
#[derive(Clone)]
enum CachedShape {
    Sdf(SdfInstance),
    Line { verts: Vec<LineVertex>, indices: Vec<u32> },
    PolyFill { verts: Vec<PolygonVertex>, indices: Vec<u32> },
    PolyOutline { verts: Vec<LineVertex>, indices: Vec<u32> },
    Text { verts: Vec<TextVertex>, indices: Vec<u32> },
    None,
}

fn hash_shape(data: &ShapeData) -> u64 {
    let mut h = DefaultHasher::new();
    // Hash the raw bytes of all f64 fields via to_bits for determinism
    // Desc
    match &data.desc {
        ShapeDesc::Circle { center, radius } => {
            0u8.hash(&mut h);
            center.0.to_bits().hash(&mut h);
            center.1.to_bits().hash(&mut h);
            radius.to_bits().hash(&mut h);
        }
        ShapeDesc::Rect { center, size, origin } => {
            1u8.hash(&mut h);
            center.0.to_bits().hash(&mut h);
            center.1.to_bits().hash(&mut h);
            size.0.to_bits().hash(&mut h);
            size.1.to_bits().hash(&mut h);
            (*origin as u8).hash(&mut h);
        }
        ShapeDesc::Line { from, to } => {
            2u8.hash(&mut h);
            from.0.to_bits().hash(&mut h);
            from.1.to_bits().hash(&mut h);
            to.0.to_bits().hash(&mut h);
            to.1.to_bits().hash(&mut h);
        }
        ShapeDesc::Polygon(pts) => {
            3u8.hash(&mut h);
            pts.len().hash(&mut h);
            for p in pts {
                p.0.to_bits().hash(&mut h);
                p.1.to_bits().hash(&mut h);
            }
        }
        ShapeDesc::Text { pos, content, size } => {
            4u8.hash(&mut h);
            pos.0.to_bits().hash(&mut h);
            pos.1.to_bits().hash(&mut h);
            content.hash(&mut h);
            size.to_bits().hash(&mut h);
        }
        _ => { 255u8.hash(&mut h); }
    }
    // Color
    for c in &data.color {
        c.to_bits().hash(&mut h);
    }
    // Render mode
    match &data.render_mode {
        RenderMode::Fill => 0u8.hash(&mut h),
        RenderMode::Outline => 1u8.hash(&mut h),
        RenderMode::Stroke(w) => { 2u8.hash(&mut h); w.to_bits().hash(&mut h); }
        RenderMode::Sdf => 3u8.hash(&mut h),
        _ => 255u8.hash(&mut h),
    }
    // Transforms
    data.transforms.len().hash(&mut h);
    for t in &data.transforms {
        t.tx.to_bits().hash(&mut h);
        t.ty.to_bits().hash(&mut h);
        t.sx.to_bits().hash(&mut h);
        t.sy.to_bits().hash(&mut h);
        t.angle.to_bits().hash(&mut h);
    }
    // CoordMeta
    data.coord_meta.px_width.to_bits().hash(&mut h);
    data.coord_meta.px_height.to_bits().hash(&mut h);
    (data.coord_meta.origin as u8).hash(&mut h);

    h.finish()
}

/// Per-shape cache with per-buffer-type dirty tracking.
/// Only reassembles buffer types that contain changed shapes.
pub struct ShapeCache {
    hashes: Vec<u64>,
    shapes: Vec<CachedShape>,
    last_frame: PreparedFrame,
    sdf_dirty: bool,
    line_dirty: bool,
    poly_dirty: bool,
    text_dirty: bool,
}

impl ShapeCache {
    pub fn new() -> Self {
        Self {
            hashes: Vec::new(),
            shapes: Vec::new(),
            last_frame: PreparedFrame {
                sdf_instances: Vec::new(),
                line_vertices: Vec::new(),
                line_indices: Vec::new(),
                polygon_vertices: Vec::new(),
                polygon_indices: Vec::new(),
                text_vertices: Vec::new(),
                text_indices: Vec::new(),
            },
            sdf_dirty: false,
            line_dirty: false,
            poly_dirty: false,
            text_dirty: false,
        }
    }
}

fn shape_buffer_type(shape: &CachedShape) -> u8 {
    match shape {
        CachedShape::Sdf(_) => 0,
        CachedShape::Line { .. } | CachedShape::PolyOutline { .. } => 1,
        CachedShape::PolyFill { .. } => 2,
        CachedShape::Text { .. } => 3,
        CachedShape::None => 255,
    }
}

#[must_use]
pub fn prepare(commands: &[DrawCommand], atlas: &AtlasData) -> PreparedFrame {
    prepare_cached(commands, atlas, None).0
}

/// Returns (frame, changed). If `changed` is false, the frame is identical to last call
/// and GPU buffers don't need re-uploading.
pub fn prepare_cached(
    commands: &[DrawCommand],
    atlas: &AtlasData,
    cache: Option<&mut ShapeCache>,
) -> (PreparedFrame, bool) {
    let draw_shapes: Vec<&ShapeData> = commands.iter().filter_map(|cmd| {
        if let DrawCommand::DrawShape(data) = cmd { Some(data) } else { None }
    }).collect();

    let Some(cache) = cache else {
        let shapes: Vec<CachedShape> = draw_shapes.iter()
            .map(|data| prepare_single(data, atlas))
            .collect();
        return (assemble_frame(&shapes), true);
    };

    let count = draw_shapes.len();
    let size_changed = count != cache.hashes.len();

    if size_changed {
        // Mark all buffer types dirty when shape count changes
        cache.sdf_dirty = true;
        cache.line_dirty = true;
        cache.poly_dirty = true;
        cache.text_dirty = true;
        cache.shapes.resize(count, CachedShape::None);
        cache.hashes.resize(count, 0);
    }

    for (i, data) in draw_shapes.iter().enumerate() {
        let h = hash_shape(data);
        if cache.hashes[i] != h {
            let old_buf = if i < cache.shapes.len() { shape_buffer_type(&cache.shapes[i]) } else { 255 };
            cache.hashes[i] = h;
            cache.shapes[i] = prepare_single(data, atlas);
            let new_buf = shape_buffer_type(&cache.shapes[i]);
            // Mark both old and new buffer types as dirty
            mark_dirty(cache, old_buf);
            mark_dirty(cache, new_buf);
        }
    }
    cache.shapes.truncate(count);
    cache.hashes.truncate(count);

    let any_dirty = cache.sdf_dirty || cache.line_dirty || cache.poly_dirty || cache.text_dirty;
    if !any_dirty {
        return (cache.last_frame.clone(), false);
    }

    // Only reassemble dirty buffer types
    if cache.sdf_dirty {
        cache.last_frame.sdf_instances.clear();
        for shape in &cache.shapes {
            if let CachedShape::Sdf(inst) = shape {
                cache.last_frame.sdf_instances.push(*inst);
            }
        }
        cache.sdf_dirty = false;
    }

    if cache.line_dirty {
        cache.last_frame.line_vertices.clear();
        cache.last_frame.line_indices.clear();
        for shape in &cache.shapes {
            match shape {
                CachedShape::Line { verts, indices } | CachedShape::PolyOutline { verts, indices } => {
                    let base = cache.last_frame.line_vertices.len() as u32;
                    cache.last_frame.line_vertices.extend_from_slice(verts);
                    cache.last_frame.line_indices.extend(indices.iter().map(|i| i + base));
                }
                _ => {}
            }
        }
        cache.line_dirty = false;
    }

    if cache.poly_dirty {
        cache.last_frame.polygon_vertices.clear();
        cache.last_frame.polygon_indices.clear();
        for shape in &cache.shapes {
            if let CachedShape::PolyFill { verts, indices } = shape {
                let base = cache.last_frame.polygon_vertices.len() as u32;
                cache.last_frame.polygon_vertices.extend_from_slice(verts);
                cache.last_frame.polygon_indices.extend(indices.iter().map(|i| i + base));
            }
        }
        cache.poly_dirty = false;
    }

    if cache.text_dirty {
        cache.last_frame.text_vertices.clear();
        cache.last_frame.text_indices.clear();
        for shape in &cache.shapes {
            if let CachedShape::Text { verts, indices } = shape {
                let base = cache.last_frame.text_vertices.len() as u32;
                cache.last_frame.text_vertices.extend_from_slice(verts);
                cache.last_frame.text_indices.extend(indices.iter().map(|i| i + base));
            }
        }
        cache.text_dirty = false;
    }

    (cache.last_frame.clone(), true)
}

fn mark_dirty(cache: &mut ShapeCache, buf_type: u8) {
    match buf_type {
        0 => cache.sdf_dirty = true,
        1 => cache.line_dirty = true,
        2 => cache.poly_dirty = true,
        3 => cache.text_dirty = true,
        _ => {}
    }
}

fn prepare_single(data: &ShapeData, atlas: &AtlasData) -> CachedShape {
    match &data.desc {
        ShapeDesc::Circle { .. } => {
            match build_circle(data) {
                Some(inst) => CachedShape::Sdf(inst),
                None => CachedShape::None,
            }
        }
        ShapeDesc::Rect { .. } => {
            match build_rect(data) {
                Some(inst) => CachedShape::Sdf(inst),
                None => CachedShape::None,
            }
        }
        ShapeDesc::Line { .. } => {
            let mut verts = Vec::new();
            let mut indices = Vec::new();
            build_line(data, &mut verts, &mut indices);
            CachedShape::Line { verts, indices }
        }
        ShapeDesc::Polygon(pts) => match &data.render_mode {
            RenderMode::Outline | RenderMode::Stroke(_) => {
                let mut verts = Vec::new();
                let mut indices = Vec::new();
                build_polygon_outline(data, pts, &mut verts, &mut indices);
                CachedShape::PolyOutline { verts, indices }
            }
            _ => {
                let mut verts = Vec::new();
                let mut indices = Vec::new();
                build_polygon_fill(data, pts, &mut verts, &mut indices);
                CachedShape::PolyFill { verts, indices }
            }
        },
        ShapeDesc::Text { pos, content, size } => {
            let mut verts = Vec::new();
            let mut indices = Vec::new();
            build_text(data, *pos, content, *size, atlas, &mut verts, &mut indices);
            CachedShape::Text { verts, indices }
        }
        _ => CachedShape::None,
    }
}

fn assemble_frame(shapes: &[CachedShape]) -> PreparedFrame {
    let mut sdf_instances = Vec::new();
    let mut line_vertices = Vec::new();
    let mut line_indices = Vec::new();
    let mut polygon_vertices = Vec::new();
    let mut polygon_indices = Vec::new();
    let mut text_vertices = Vec::new();
    let mut text_indices = Vec::new();

    for shape in shapes {
        match shape {
            CachedShape::Sdf(inst) => {
                sdf_instances.push(*inst);
            }
            CachedShape::Line { verts, indices } => {
                let base = line_vertices.len() as u32;
                line_vertices.extend_from_slice(verts);
                line_indices.extend(indices.iter().map(|i| i + base));
            }
            CachedShape::PolyFill { verts, indices } => {
                let base = polygon_vertices.len() as u32;
                polygon_vertices.extend_from_slice(verts);
                polygon_indices.extend(indices.iter().map(|i| i + base));
            }
            CachedShape::PolyOutline { verts, indices } => {
                let base = line_vertices.len() as u32;
                line_vertices.extend_from_slice(verts);
                line_indices.extend(indices.iter().map(|i| i + base));
            }
            CachedShape::Text { verts, indices } => {
                let base = text_vertices.len() as u32;
                text_vertices.extend_from_slice(verts);
                text_indices.extend(indices.iter().map(|i| i + base));
            }
            CachedShape::None => {}
        }
    }

    PreparedFrame {
        sdf_instances,
        line_vertices,
        line_indices,
        polygon_vertices,
        polygon_indices,
        text_vertices,
        text_indices,
    }
}

// ─── Coordinate helpers ──────────────────────────────────────────────────────

fn color_f32(data: &ShapeData) -> [f32; 4] {
    [
        data.color[0] as f32,
        data.color[1] as f32,
        data.color[2] as f32,
        data.color[3] as f32,
    ]
}

/// Accumulate translation and scale from transforms.
/// Returns (tx, ty) in user coords and (sx, sy) scale factors.
fn accumulate_transforms(data: &ShapeData) -> (f64, f64, f64, f64) {
    let mut tx = 0.0;
    let mut ty = 0.0;
    let mut sx = 1.0;
    let mut sy = 1.0;
    for t in &data.transforms {
        tx += t.tx;
        ty += t.ty;
        sx *= t.sx;
        sy *= t.sy;
    }
    (tx, ty, sx, sy)
}

/// Convert a user-coordinate point (with transforms applied) to NDC.
fn to_ndc(data: &ShapeData, x: f64, y: f64) -> (f64, f64) {
    let meta = &data.coord_meta;
    (meta.x_to_ndc(x), meta.y_to_ndc(y))
}

/// Apply transforms to a point relative to a pivot, return in user coords.
fn apply_transform(x: f64, y: f64, pivot_x: f64, pivot_y: f64, tx: f64, ty: f64, sx: f64, sy: f64) -> (f64, f64) {
    let rx = pivot_x + (x - pivot_x) * sx + tx;
    let ry = pivot_y + (y - pivot_y) * sy + ty;
    (rx, ry)
}

// ─── Render mode → SDF params ────────────────────────────────────────────────

#[expect(clippy::match_same_arms, reason = "non_exhaustive enum requires wildcard arm")]
fn render_mode_code(mode: &RenderMode) -> f32 {
    match mode {
        RenderMode::Fill => 0.0,
        RenderMode::Outline => 1.0,
        RenderMode::Stroke(_) => 2.0,
        RenderMode::Sdf => 3.0,
        _ => 0.0,
    }
}

fn stroke_width_px(mode: &RenderMode) -> f32 {
    match mode {
        RenderMode::Stroke(w) => *w as f32,
        _ => 0.0,
    }
}

// ─── Circle (SDF) ────────────────────────────────────────────────────────────

fn build_circle(data: &ShapeData) -> Option<SdfInstance> {
    let ShapeDesc::Circle { center, radius } = &data.desc else {
        return None;
    };
    let meta = &data.coord_meta;
    let (tx, ty, sx, sy) = accumulate_transforms(data);

    let cx = center.0 + tx;
    let cy = center.1 + ty;
    let (ndc_x, ndc_y) = to_ndc(data, cx, cy);
    let r_ndc_x = meta.w_to_ndc(*radius) * sx;
    let r_ndc_y = meta.h_to_ndc(*radius) * sy;

    Some(SdfInstance {
        center: [ndc_x as f32, ndc_y as f32],
        size: [r_ndc_x as f32, r_ndc_y as f32],
        color: color_f32(data),
        shape_params: [
            0.0,
            render_mode_code(&data.render_mode),
            stroke_width_px(&data.render_mode),
            0.0,
        ],
    })
}

// ─── Rect (SDF) ──────────────────────────────────────────────────────────────

fn build_rect(data: &ShapeData) -> Option<SdfInstance> {
    let ShapeDesc::Rect { center, size, origin } = &data.desc else {
        return None;
    };
    let meta = &data.coord_meta;
    let (tx, ty, sx, sy) = accumulate_transforms(data);

    let hw_ndc = meta.w_to_ndc(size.0 / 2.0) * sx;
    let hh_ndc = meta.h_to_ndc(size.1 / 2.0) * sy;
    let (off_x, off_y) = origin_offset(origin, hw_ndc, hh_ndc);

    let cx = center.0 + tx;
    let cy = center.1 + ty;
    let (ndc_x, ndc_y) = to_ndc(data, cx, cy);

    Some(SdfInstance {
        center: [(ndc_x + off_x) as f32, (ndc_y + off_y) as f32],
        size: [hw_ndc as f32, hh_ndc as f32],
        color: color_f32(data),
        shape_params: [
            1.0,
            render_mode_code(&data.render_mode),
            stroke_width_px(&data.render_mode),
            0.0,
        ],
    })
}

// ─── Line (geometry quad) ────────────────────────────────────────────────────

fn line_thickness(data: &ShapeData) -> f64 {
    match &data.render_mode {
        RenderMode::Stroke(w) => *w,
        _ => 1.0,
    }
}

fn build_line(data: &ShapeData, vertices: &mut Vec<LineVertex>, indices: &mut Vec<u32>) {
    let ShapeDesc::Line { from, to } = &data.desc else {
        return;
    };
    let (tx, ty, sx, sy) = accumulate_transforms(data);
    let pivot_x = (from.0 + to.0) / 2.0;
    let pivot_y = (from.1 + to.1) / 2.0;

    let (ax, ay) = apply_transform(from.0, from.1, pivot_x, pivot_y, tx, ty, sx, sy);
    let (bx, by) = apply_transform(to.0, to.1, pivot_x, pivot_y, tx, ty, sx, sy);

    let thickness = line_thickness(data);
    let color = color_f32(data);

    emit_line_quad(data, ax, ay, bx, by, thickness, color, vertices, indices);
}

/// Emit a single line-segment quad (4 vertices, 6 indices).
/// Endpoints are in user coordinates; perpendicular is computed in user space
/// (which is pixel-isotropic), then vertices are converted to NDC.
fn emit_line_quad(
    data: &ShapeData,
    ax: f64, ay: f64,
    bx: f64, by: f64,
    thickness: f64,
    color: [f32; 4],
    vertices: &mut Vec<LineVertex>,
    indices: &mut Vec<u32>,
) {
    let dx = bx - ax;
    let dy = by - ay;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-12 {
        return;
    }

    let half_t = thickness / 2.0;
    let aa_pad = 1.5;
    let expand = half_t + aa_pad;

    // Perpendicular unit vector in user/pixel space (isotropic)
    let px = -dy / len;
    let py = dx / len;

    // Four corners: offset endpoints by perpendicular * expand
    let corners = [
        (ax - px * expand, ay - py * expand), // v0: start, left edge
        (ax + px * expand, ay + py * expand), // v1: start, right edge
        (bx - px * expand, by - py * expand), // v2: end, left edge
        (bx + px * expand, by + py * expand), // v3: end, right edge
    ];
    let dists = [
        -(expand as f32),
        expand as f32,
        -(expand as f32),
        expand as f32,
    ];

    let base = vertices.len() as u32;
    for (i, &(cx, cy)) in corners.iter().enumerate() {
        let (nx, ny) = to_ndc(data, cx, cy);
        vertices.push(LineVertex {
            position: [nx as f32, ny as f32],
            color,
            edge_dist: dists[i],
            half_thickness: half_t as f32,
        });
    }

    indices.extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 1, base + 3]);
}

// ─── Polygon (fill) ──────────────────────────────────────────────────────────

fn build_polygon_fill(
    data: &ShapeData,
    pts: &[(f64, f64)],
    vertices: &mut Vec<PolygonVertex>,
    indices: &mut Vec<u32>,
) {
    if pts.len() < 3 {
        return;
    }

    let (tx, ty, sx, sy) = accumulate_transforms(data);
    let color = color_f32(data);

    let centroid_x: f64 = pts.iter().map(|p| p.0).sum::<f64>() / pts.len() as f64;
    let centroid_y: f64 = pts.iter().map(|p| p.1).sum::<f64>() / pts.len() as f64;

    let base = vertices.len() as u32;

    for &(px, py) in pts {
        let (rx, ry) = apply_transform(px, py, centroid_x, centroid_y, tx, ty, sx, sy);
        let (nx, ny) = to_ndc(data, rx, ry);
        vertices.push(PolygonVertex {
            position: [nx as f32, ny as f32],
            color,
        });
    }

    let n = pts.len() as u32;
    for i in 1..n - 1 {
        indices.push(base);
        indices.push(base + i);
        indices.push(base + i + 1);
    }
}

// ─── Polygon (outline) ──────────────────────────────────────────────────────

fn build_polygon_outline(
    data: &ShapeData,
    pts: &[(f64, f64)],
    line_vertices: &mut Vec<LineVertex>,
    line_indices: &mut Vec<u32>,
) {
    if pts.len() < 2 {
        return;
    }

    let (tx, ty, sx, sy) = accumulate_transforms(data);
    let color = color_f32(data);
    let thickness = line_thickness(data);

    let centroid_x: f64 = pts.iter().map(|p| p.0).sum::<f64>() / pts.len() as f64;
    let centroid_y: f64 = pts.iter().map(|p| p.1).sum::<f64>() / pts.len() as f64;

    let n = pts.len();
    for i in 0..n {
        let j = (i + 1) % n;
        let (ax, ay) = apply_transform(pts[i].0, pts[i].1, centroid_x, centroid_y, tx, ty, sx, sy);
        let (bx, by) = apply_transform(pts[j].0, pts[j].1, centroid_x, centroid_y, tx, ty, sx, sy);

        emit_line_quad(data, ax, ay, bx, by, thickness, color, line_vertices, line_indices);
    }
}

// ─── Text (MSDF) ─────────────────────────────────────────────────────────────

fn build_text(
    data: &ShapeData,
    pos: (f64, f64),
    content: &str,
    font_size: f64,
    atlas: &AtlasData,
    vertices: &mut Vec<TextVertex>,
    indices: &mut Vec<u32>,
) {
    let (tx, ty, _sx, _sy) = accumulate_transforms(data);
    let color = color_f32(data);
    let atlas_w = atlas.width as f32;
    let atlas_h = atlas.height as f32;

    // px_range: how many screen pixels the distance field covers.
    // distanceRange is the range in atlas pixels; scale by (rendered size / atlas glyph size).
    // Atlas glyphs are ~1em tall; em = advance height. With distance_range=16 on a 2048 atlas,
    // typical glyph is ~40-60px tall in atlas. A simple heuristic:
    // px_range = distance_range * (font_size / atlas_glyph_size)
    // We approximate atlas_glyph_size from a reference glyph's atlas bounds height.
    let reference_atlas_height = atlas.glyphs.values()
        .find_map(|g| g.atlas_bounds.as_ref().map(|b| (b.top - b.bottom).abs()))
        .unwrap_or(40.0);
    let px_range = atlas.distance_range * (font_size as f32 / reference_atlas_height);

    let y_sign = if data.coord_meta.origin.is_y_down() { -1.0 } else { 1.0 };

    // Vertically center text around pos.y (like rects/circles use center).
    // Compute mid-point of a reference glyph in em-space and offset the baseline.
    let line_mid_em = atlas.glyphs.get(&'A')
        .or_else(|| atlas.glyphs.get(&'H'))
        .and_then(|g| g.plane_bounds.as_ref())
        .map(|b| f64::from((b.top + b.bottom) / 2.0))
        .unwrap_or(0.35);

    let mut cursor_x = pos.0 + tx;
    let cursor_y = pos.1 + ty - y_sign * line_mid_em * font_size;

    for ch in content.chars() {
        let Some(glyph) = atlas.glyphs.get(&ch) else {
            cursor_x += 0.6 * font_size;
            continue;
        };

        if let (Some(plane), Some(ab)) = (&glyph.plane_bounds, &glyph.atlas_bounds) {
            let x0 = cursor_x + plane.left as f64 * font_size;
            let x1 = cursor_x + plane.right as f64 * font_size;
            let y0 = cursor_y + y_sign * plane.top as f64 * font_size;
            let y1 = cursor_y + y_sign * plane.bottom as f64 * font_size;

            // UV coordinates from atlas bounds (pixel coords → [0,1])
            let u0 = ab.left / atlas_w;
            let u1 = ab.right / atlas_w;
            // Atlas yOrigin is "bottom": bottom of atlas texture is y=0 in atlas coords.
            // In texture UV space, v=0 is top, v=1 is bottom, so we flip.
            let v0 = 1.0 - ab.top / atlas_h;    // top of glyph in atlas → lower v
            let v1 = 1.0 - ab.bottom / atlas_h; // bottom of glyph in atlas → higher v

            let (ndc_x0, ndc_y0) = to_ndc(data, x0, y0);
            let (ndc_x1, ndc_y1) = to_ndc(data, x1, y1);

            let base = vertices.len() as u32;
            vertices.push(TextVertex { position: [ndc_x0 as f32, ndc_y0 as f32], uv: [u0, v0], color, px_range });
            vertices.push(TextVertex { position: [ndc_x1 as f32, ndc_y0 as f32], uv: [u1, v0], color, px_range });
            vertices.push(TextVertex { position: [ndc_x0 as f32, ndc_y1 as f32], uv: [u0, v1], color, px_range });
            vertices.push(TextVertex { position: [ndc_x1 as f32, ndc_y1 as f32], uv: [u1, v1], color, px_range });

            indices.extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 1, base + 3]);
        }

        cursor_x += glyph.advance as f64 * font_size;
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rustle_lang::{CoordMeta, Origin};
    use std::collections::HashMap;

    fn meta_800x600() -> CoordMeta {
        CoordMeta {
            px_width: 800.0,
            px_height: 600.0,
            origin: Origin::Center,
        }
    }

    fn dummy_atlas() -> AtlasData {
        AtlasData {
            width: 1,
            height: 1,
            distance_range: 16.0,
            glyphs: HashMap::new(),
        }
    }

    #[test]
    fn circle_at_origin_ndc() {
        let data = ShapeData {
            desc: ShapeDesc::Circle {
                center: (0.0, 0.0),
                radius: 50.0,
            },
            render_mode: RenderMode::Fill,
            coord_meta: meta_800x600(),
            transforms: Vec::new(),
            color: [1.0, 0.0, 0.0, 1.0],
        };
        let cmds = vec![DrawCommand::DrawShape(data)];
        let atlas = dummy_atlas();
        let frame = prepare(&cmds, &atlas);

        assert_eq!(frame.sdf_instances.len(), 1);
        let inst = &frame.sdf_instances[0];
        assert!((inst.center[0]).abs() < 1e-6);
        assert!((inst.center[1]).abs() < 1e-6);
        let expected_w = 2.0 * 50.0 / 800.0;
        let expected_h = 2.0 * 50.0 / 600.0;
        assert!((f64::from(inst.size[0]) - expected_w).abs() < 1e-5);
        assert!((f64::from(inst.size[1]) - expected_h).abs() < 1e-5);
    }

    #[test]
    fn polygon_four_points_fill() {
        let pts = vec![(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)];
        let data = ShapeData {
            desc: ShapeDesc::Polygon(pts),
            render_mode: RenderMode::Fill,
            coord_meta: meta_800x600(),
            transforms: Vec::new(),
            color: [0.0, 1.0, 0.0, 1.0],
        };
        let cmds = vec![DrawCommand::DrawShape(data)];
        let atlas = dummy_atlas();
        let frame = prepare(&cmds, &atlas);

        assert_eq!(frame.polygon_vertices.len(), 4);
        assert_eq!(frame.polygon_indices.len(), 6);
        assert!(frame.line_vertices.is_empty());
    }

    #[test]
    fn polygon_outline_produces_line_geometry() {
        let pts = vec![(0.0, 0.0), (100.0, 0.0), (100.0, 100.0)];
        let data = ShapeData {
            desc: ShapeDesc::Polygon(pts),
            render_mode: RenderMode::Outline,
            coord_meta: meta_800x600(),
            transforms: Vec::new(),
            color: [1.0, 1.0, 1.0, 1.0],
        };
        let cmds = vec![DrawCommand::DrawShape(data)];
        let atlas = dummy_atlas();
        let frame = prepare(&cmds, &atlas);

        assert!(frame.polygon_vertices.is_empty());
        // 3 edges × 4 vertices = 12
        assert_eq!(frame.line_vertices.len(), 12);
        // 3 edges × 6 indices = 18
        assert_eq!(frame.line_indices.len(), 18);
    }

    #[test]
    fn line_produces_quad() {
        let data = ShapeData {
            desc: ShapeDesc::Line {
                from: (0.0, 0.0),
                to: (100.0, 0.0),
            },
            render_mode: RenderMode::Fill,
            coord_meta: meta_800x600(),
            transforms: Vec::new(),
            color: [1.0, 1.0, 1.0, 1.0],
        };
        let cmds = vec![DrawCommand::DrawShape(data)];
        let atlas = dummy_atlas();
        let frame = prepare(&cmds, &atlas);

        assert_eq!(frame.line_vertices.len(), 4);
        assert_eq!(frame.line_indices.len(), 6);
        assert!(frame.sdf_instances.is_empty());
    }

    // ─── Text (MSDF) tests ───────────────────────────────────────────────────

    fn atlas_with_glyphs() -> AtlasData {
        use crate::atlas::{GlyphInfo, Bounds};
        let mut glyphs = HashMap::new();
        glyphs.insert('A', GlyphInfo {
            advance: 0.6,
            plane_bounds: Some(Bounds { left: -0.25, bottom: -0.29, right: 0.85, top: 0.82 }),
            atlas_bounds: Some(Bounds { left: 1.5, bottom: 1793.5, right: 126.5, top: 1918.5 }),
        });
        glyphs.insert('B', GlyphInfo {
            advance: 0.6,
            plane_bounds: Some(Bounds { left: -0.25, bottom: -0.29, right: 0.85, top: 0.82 }),
            atlas_bounds: Some(Bounds { left: 129.5, bottom: 1793.5, right: 254.5, top: 1918.5 }),
        });
        glyphs.insert(' ', GlyphInfo {
            advance: 0.6,
            plane_bounds: None,
            atlas_bounds: None,
        });
        AtlasData {
            width: 2048,
            height: 2048,
            distance_range: 16.0,
            glyphs,
        }
    }

    fn text_shape(content: &str, pos: (f64, f64), size: f64) -> ShapeData {
        ShapeData {
            desc: ShapeDesc::Text { pos, content: content.to_string(), size },
            render_mode: RenderMode::Fill,
            coord_meta: meta_800x600(),
            transforms: Vec::new(),
            color: [1.0, 1.0, 1.0, 1.0],
        }
    }

    #[test]
    fn text_empty_string_produces_no_geometry() {
        let data = text_shape("", (0.0, 0.0), 24.0);
        let cmds = vec![DrawCommand::DrawShape(data)];
        let atlas = atlas_with_glyphs();
        let frame = prepare(&cmds, &atlas);

        assert!(frame.text_vertices.is_empty());
        assert!(frame.text_indices.is_empty());
    }

    #[test]
    fn text_single_char_produces_one_quad() {
        let data = text_shape("A", (0.0, 0.0), 24.0);
        let cmds = vec![DrawCommand::DrawShape(data)];
        let atlas = atlas_with_glyphs();
        let frame = prepare(&cmds, &atlas);

        assert_eq!(frame.text_vertices.len(), 4);
        assert_eq!(frame.text_indices.len(), 6);
    }

    #[test]
    fn text_two_chars_produce_two_quads() {
        let data = text_shape("AB", (0.0, 0.0), 24.0);
        let cmds = vec![DrawCommand::DrawShape(data)];
        let atlas = atlas_with_glyphs();
        let frame = prepare(&cmds, &atlas);

        assert_eq!(frame.text_vertices.len(), 8);
        assert_eq!(frame.text_indices.len(), 12);
    }

    #[test]
    fn text_space_produces_no_quad_but_advances() {
        let data = text_shape("A A", (0.0, 0.0), 24.0);
        let cmds = vec![DrawCommand::DrawShape(data)];
        let atlas = atlas_with_glyphs();
        let frame = prepare(&cmds, &atlas);

        // Space has no bounds → no quad, only 2 visible chars
        assert_eq!(frame.text_vertices.len(), 8);
        assert_eq!(frame.text_indices.len(), 12);

        // Second 'A' should be further right than first 'A' due to space advance
        let first_a_x = frame.text_vertices[0].position[0];
        let second_a_x = frame.text_vertices[4].position[0];
        assert!(second_a_x > first_a_x, "second A should be to the right of first A");
    }

    #[test]
    fn text_unknown_char_skipped_with_default_advance() {
        let data = text_shape("A🦀A", (0.0, 0.0), 24.0);
        let cmds = vec![DrawCommand::DrawShape(data)];
        let atlas = atlas_with_glyphs();
        let frame = prepare(&cmds, &atlas);

        // Only two 'A' chars have quads; the emoji is skipped
        assert_eq!(frame.text_vertices.len(), 8);
        assert_eq!(frame.text_indices.len(), 12);
    }

    #[test]
    fn text_indices_are_valid() {
        let data = text_shape("AB", (0.0, 0.0), 24.0);
        let cmds = vec![DrawCommand::DrawShape(data)];
        let atlas = atlas_with_glyphs();
        let frame = prepare(&cmds, &atlas);

        let max_vertex = frame.text_vertices.len() as u32;
        for &idx in &frame.text_indices {
            assert!(idx < max_vertex, "index {idx} out of bounds (max {max_vertex})");
        }
        // First quad: 0,1,2,2,1,3  Second quad: 4,5,6,6,5,7
        assert_eq!(&frame.text_indices[..6], &[0, 1, 2, 2, 1, 3]);
        assert_eq!(&frame.text_indices[6..], &[4, 5, 6, 6, 5, 7]);
    }

    #[test]
    fn text_uvs_are_in_unit_range() {
        let data = text_shape("A", (0.0, 0.0), 24.0);
        let cmds = vec![DrawCommand::DrawShape(data)];
        let atlas = atlas_with_glyphs();
        let frame = prepare(&cmds, &atlas);

        for v in &frame.text_vertices {
            assert!(v.uv[0] >= 0.0 && v.uv[0] <= 1.0, "u={} out of [0,1]", v.uv[0]);
            assert!(v.uv[1] >= 0.0 && v.uv[1] <= 1.0, "v={} out of [0,1]", v.uv[1]);
        }
    }

    #[test]
    fn text_color_propagated() {
        let mut data = text_shape("A", (0.0, 0.0), 24.0);
        data.color = [1.0, 0.0, 0.5, 0.8];
        let cmds = vec![DrawCommand::DrawShape(data)];
        let atlas = atlas_with_glyphs();
        let frame = prepare(&cmds, &atlas);

        for v in &frame.text_vertices {
            assert_eq!(v.color, [1.0, 0.0, 0.5, 0.8]);
        }
    }

    #[test]
    fn text_px_range_positive() {
        let data = text_shape("A", (0.0, 0.0), 24.0);
        let cmds = vec![DrawCommand::DrawShape(data)];
        let atlas = atlas_with_glyphs();
        let frame = prepare(&cmds, &atlas);

        for v in &frame.text_vertices {
            assert!(v.px_range > 0.0, "px_range should be positive, got {}", v.px_range);
        }
    }

    #[test]
    fn text_larger_size_produces_larger_quads() {
        let small = text_shape("A", (0.0, 0.0), 12.0);
        let big = text_shape("A", (0.0, 0.0), 48.0);

        let atlas = atlas_with_glyphs();
        let frame_small = prepare(&[DrawCommand::DrawShape(small)], &atlas);
        let frame_big = prepare(&[DrawCommand::DrawShape(big)], &atlas);

        let small_width = (frame_small.text_vertices[1].position[0]
            - frame_small.text_vertices[0].position[0]).abs();
        let big_width = (frame_big.text_vertices[1].position[0]
            - frame_big.text_vertices[0].position[0]).abs();

        assert!(big_width > small_width, "bigger font should produce wider quad");
    }

    #[test]
    fn text_position_offset_affects_ndc() {
        let at_origin = text_shape("A", (0.0, 0.0), 24.0);
        let at_offset = text_shape("A", (100.0, 50.0), 24.0);

        let atlas = atlas_with_glyphs();
        let f1 = prepare(&[DrawCommand::DrawShape(at_origin)], &atlas);
        let f2 = prepare(&[DrawCommand::DrawShape(at_offset)], &atlas);

        // Offset text should have different NDC positions
        assert!((f2.text_vertices[0].position[0] - f1.text_vertices[0].position[0]).abs() > 0.01);
    }

    #[test]
    fn text_does_not_affect_other_buffers() {
        let data = text_shape("AB", (0.0, 0.0), 24.0);
        let cmds = vec![DrawCommand::DrawShape(data)];
        let atlas = atlas_with_glyphs();
        let frame = prepare(&cmds, &atlas);

        assert!(frame.sdf_instances.is_empty());
        assert!(frame.line_vertices.is_empty());
        assert!(frame.line_indices.is_empty());
        assert!(frame.polygon_vertices.is_empty());
        assert!(frame.polygon_indices.is_empty());
    }

    #[test]
    fn text_mixed_with_shapes() {
        let circle = ShapeData {
            desc: ShapeDesc::Circle { center: (0.0, 0.0), radius: 50.0 },
            render_mode: RenderMode::Fill,
            coord_meta: meta_800x600(),
            transforms: Vec::new(),
            color: [1.0, 0.0, 0.0, 1.0],
        };
        let text = text_shape("A", (100.0, 100.0), 24.0);
        let cmds = vec![
            DrawCommand::DrawShape(circle),
            DrawCommand::DrawShape(text),
        ];
        let atlas = atlas_with_glyphs();
        let frame = prepare(&cmds, &atlas);

        assert_eq!(frame.sdf_instances.len(), 1);
        assert_eq!(frame.text_vertices.len(), 4);
        assert_eq!(frame.text_indices.len(), 6);
    }

    #[test]
    fn text_top_left_origin_positions_correctly() {
        let data = ShapeData {
            desc: ShapeDesc::Text { pos: (10.0, 20.0), content: "A".to_string(), size: 24.0 },
            render_mode: RenderMode::Fill,
            coord_meta: CoordMeta {
                px_width: 800.0,
                px_height: 600.0,
                origin: Origin::TopLeft,
            },
            transforms: Vec::new(),
            color: [1.0, 1.0, 1.0, 1.0],
        };
        let cmds = vec![DrawCommand::DrawShape(data)];
        let atlas = atlas_with_glyphs();
        let frame = prepare(&cmds, &atlas);

        // With top-left origin, x=10 should map to near NDC -1,
        // and all x values should be negative (left side of screen)
        for v in &frame.text_vertices {
            assert!(v.position[0] < 0.0, "x should be negative for top-left origin near left edge");
        }
    }
}
