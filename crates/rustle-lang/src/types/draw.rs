/// Origin point — used both for per-shape anchoring and canvas coordinate origin.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Origin {
    Center,
    TopLeft, TopRight,
    BottomLeft, BottomRight,
    Top, Bottom, Left, Right,
}

impl Default for Origin {
    fn default() -> Self { Self::Center }
}

impl Origin {
    /// Canvas origins where y=0 is at the top and y increases downward (screen convention).
    pub fn is_y_down(self) -> bool {
        matches!(self, Origin::TopLeft | Origin::TopRight | Origin::Top)
    }
}

/// Coordinate conversion parameters — snapshotted from the interpreter
/// into every DrawCommand so the renderer has full context.
#[derive(Debug, Clone)]
pub struct CoordMeta {
    /// Canvas width in pixels. 0.0 = not set (identity / NDC pass-through).
    pub px_width:  f64,
    /// Canvas height in pixels. 0.0 = not set.
    pub px_height: f64,
    /// Canvas origin — where (0,0) maps on screen, and y-axis direction.
    /// Center = (0,0) is screen center, y-up.
    /// TopLeft = (0,0) is top-left corner, y-down (screen/pixel convention).
    pub origin: Origin,
}

impl Default for CoordMeta {
    fn default() -> Self {
        Self { px_width: 0.0, px_height: 0.0, origin: Origin::Center }
    }
}

impl CoordMeta {
    /// Convert a position x coordinate to NDC [-1, 1].
    ///
    /// x=0 placement:
    ///   Left-edge origins  (TopLeft, BottomLeft, Left)  → NDC x = -1
    ///   Right-edge origins (TopRight, BottomRight, Right) → NDC x = +1
    ///   Center-x origins   (Center, Top, Bottom)          → NDC x = 0
    pub fn x_to_ndc(&self, x: f64) -> f64 {
        if self.px_width > 0.0 {
            match self.origin {
                Origin::Center | Origin::Top | Origin::Bottom
                    => 2.0 * x / self.px_width,
                Origin::TopLeft | Origin::BottomLeft | Origin::Left
                    => 2.0 * x / self.px_width - 1.0,
                Origin::TopRight | Origin::BottomRight | Origin::Right
                    => 1.0 - 2.0 * x / self.px_width,
            }
        } else {
            x
        }
    }

    /// Convert a position y coordinate to NDC [-1, 1].
    ///
    /// y=0 placement and direction:
    ///   y-down (Top*)  → y=0 at screen top (NDC +1), y increases downward
    ///   y-up   (Bot*)  → y=0 at screen bottom (NDC -1), y increases upward
    ///   center (Center, Left, Right) → y=0 at screen center, y-up
    pub fn y_to_ndc(&self, y: f64) -> f64 {
        if self.px_height > 0.0 {
            match self.origin {
                Origin::Center | Origin::Left | Origin::Right
                    => 2.0 * y / self.px_height,
                Origin::BottomLeft | Origin::BottomRight | Origin::Bottom
                    => 2.0 * y / self.px_height - 1.0,
                Origin::TopLeft | Origin::TopRight | Origin::Top
                    => 1.0 - 2.0 * y / self.px_height,
            }
        } else {
            y
        }
    }

    /// Convert a width/x-extent to NDC scale (no position bias).
    pub fn w_to_ndc(&self, w: f64) -> f64 {
        if self.px_width > 0.0 { 2.0 * w / self.px_width } else { w }
    }

    /// Convert a height/y-extent to NDC scale (no position bias, always positive).
    pub fn h_to_ndc(&self, h: f64) -> f64 {
        if self.px_height > 0.0 { 2.0 * h / self.px_height } else { h }
    }

    /// Convert a y-direction translation delta to NDC (respects y-axis direction).
    pub fn dy_to_ndc(&self, dy: f64) -> f64 {
        let scale = if self.px_height > 0.0 { 2.0 / self.px_height } else { 1.0 };
        if self.origin.is_y_down() { -dy * scale } else { dy * scale }
    }

    /// Convert a user-space x to screen pixels (0 = left edge of canvas).
    pub fn x_to_screen_px(&self, x: f64) -> f64 {
        if self.px_width > 0.0 {
            match self.origin {
                Origin::Center | Origin::Top | Origin::Bottom
                    => self.px_width / 2.0 + x,
                Origin::TopLeft | Origin::BottomLeft | Origin::Left
                    => x,
                Origin::TopRight | Origin::BottomRight | Origin::Right
                    => self.px_width - x,
            }
        } else {
            x
        }
    }

    /// Convert a user-space y to screen pixels (0 = top edge of canvas, y-down).
    pub fn y_to_screen_px(&self, y: f64) -> f64 {
        if self.px_height > 0.0 {
            match self.origin {
                Origin::TopLeft | Origin::TopRight | Origin::Top
                    => y,                                        // already screen px
                Origin::Center | Origin::Left | Origin::Right
                    => self.px_height / 2.0 - y,                // y-up from center
                Origin::BottomLeft | Origin::BottomRight | Origin::Bottom
                    => self.px_height - y,                       // y-up from bottom
            }
        } else {
            y
        }
    }
}

// ─── Shape description ────────────────────────────────────────────────────────

/// Semantic shape — not pre-tessellated. The renderer converts coords
/// to NDC and computes actual geometry at draw time.
#[derive(Debug, Clone)]
pub enum ShapeDesc {
    Circle { center: (f64, f64), radius: f64 },
    Rect   { center: (f64, f64), size: (f64, f64), origin: Origin },
    Line   { from: (f64, f64), to: (f64, f64) },
    Polygon(Vec<(f64, f64)>),
}

impl ShapeDesc {
    /// Stored anchor point — the literal first argument to the shape constructor.
    /// This is the reference point for `.in()` offsets.
    pub fn anchor(&self) -> (f64, f64) {
        match self {
            Self::Circle { center, .. } => *center,
            Self::Rect   { center, .. } => *center,
            Self::Line   { from, .. }   => *from,
            Self::Polygon(pts) => pts.first().copied().unwrap_or((0.0, 0.0)),
        }
    }
}

/// Offset from stored origin anchor to visual center, in NDC y-up space.
/// Used by the tessellator after converting anchor and half-size to NDC.
pub fn origin_offset(origin: &Origin, hw: f64, hh: f64) -> (f64, f64) {
    match origin {
        Origin::Center      => ( 0.0,  0.0),
        Origin::TopLeft     => ( hw,  -hh),
        Origin::TopRight    => (-hw,  -hh),
        Origin::BottomLeft  => ( hw,   hh),
        Origin::BottomRight => (-hw,   hh),
        Origin::Top         => ( 0.0, -hh),
        Origin::Bottom      => ( 0.0,  hh),
        Origin::Left        => ( hw,   0.0),
        Origin::Right       => (-hw,   0.0),
    }
}

// ─── Transform ────────────────────────────────────────────────────────────────

/// Transform stored alongside a shape. The tessellator applies it in NDC space
/// after coordinate conversion.
#[derive(Debug, Clone)]
pub struct TransformData {
    pub tx:    f64,
    pub ty:    f64,
    pub sx:    f64,
    pub sy:    f64,
    pub angle: f64,
}

impl Default for TransformData {
    fn default() -> Self {
        Self { tx: 0.0, ty: 0.0, sx: 1.0, sy: 1.0, angle: 0.0 }
    }
}

// ─── Render mode ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum RenderMode { Sdf, Fill, Outline, Stroke(f64) }

impl Default for RenderMode {
    fn default() -> Self { Self::Sdf }
}

// ─── Shape data ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ShapeData {
    pub desc:        ShapeDesc,
    pub render_mode: RenderMode,
    pub coord_meta:  CoordMeta,
    pub transforms:  Vec<TransformData>,
}

impl ShapeData {
    pub fn new(desc: ShapeDesc, render_mode: RenderMode, coord_meta: CoordMeta) -> Self {
        Self { desc, render_mode, coord_meta, transforms: Vec::new() }
    }
}

// ─── Draw command ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum DrawCommand {
    DrawShape(ShapeData),
}
