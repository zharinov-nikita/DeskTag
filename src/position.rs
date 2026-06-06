//! Pure, OS-independent badge-position model: anchor presets, custom
//! coordinates, on-disk (de)serialization, and geometry to turn a position
//! into a top-left origin. Unit-tested with no Win32 dependency, like
//! `label.rs` and `edit.rs`.

/// A plain rectangle (left, top, right, bottom). OS-independent — NOT the Win32
/// `RECT` — so this module builds and tests on any platform.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    pub fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Rect { left, top, right, bottom }
    }
    fn width(&self) -> i32 {
        self.right - self.left
    }
    fn height(&self) -> i32 {
        self.bottom - self.top
    }
}

/// Nine standard anchor positions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    TopLeft,
    TopCenter,
    TopRight,
    MidLeft,
    Center,
    MidRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

impl Anchor {
    /// All nine in reading order — used to build the tray submenu.
    pub const ALL: [Anchor; 9] = [
        Anchor::TopLeft,
        Anchor::TopCenter,
        Anchor::TopRight,
        Anchor::MidLeft,
        Anchor::Center,
        Anchor::MidRight,
        Anchor::BottomLeft,
        Anchor::BottomCenter,
        Anchor::BottomRight,
    ];

    /// Stable token written to the config file.
    pub fn token(self) -> &'static str {
        match self {
            Anchor::TopLeft => "top-left",
            Anchor::TopCenter => "top-center",
            Anchor::TopRight => "top-right",
            Anchor::MidLeft => "mid-left",
            Anchor::Center => "center",
            Anchor::MidRight => "mid-right",
            Anchor::BottomLeft => "bottom-left",
            Anchor::BottomCenter => "bottom-center",
            Anchor::BottomRight => "bottom-right",
        }
    }

    fn from_token(s: &str) -> Option<Anchor> {
        Anchor::ALL.into_iter().find(|a| a.token() == s)
    }

    /// Human label for the tray menu.
    pub fn label(self) -> &'static str {
        match self {
            Anchor::TopLeft => "Top left",
            Anchor::TopCenter => "Top center",
            Anchor::TopRight => "Top right",
            Anchor::MidLeft => "Middle left",
            Anchor::Center => "Center",
            Anchor::MidRight => "Middle right",
            Anchor::BottomLeft => "Bottom left",
            Anchor::BottomCenter => "Bottom center",
            Anchor::BottomRight => "Bottom right",
        }
    }
}

/// Where the badge sits: a named anchor, or an absolute custom point on the
/// virtual screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Position {
    Anchor(Anchor),
    Custom { x: i32, y: i32 },
}

impl Default for Position {
    fn default() -> Self {
        Position::Anchor(Anchor::TopCenter)
    }
}

/// Serialize to the `key=value` config body.
pub fn format(pos: &Position) -> String {
    match pos {
        Position::Anchor(a) => format!("mode=anchor\nanchor={}\n", a.token()),
        Position::Custom { x, y } => format!("mode=custom\nx={x}\ny={y}\n"),
    }
}

/// Parse the config body. Any missing/garbage field falls back to the default.
pub fn parse(s: &str) -> Position {
    let mut mode = None;
    let mut anchor = None;
    let mut x = None;
    let mut y = None;
    for line in s.lines() {
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        match k.trim() {
            "mode" => mode = Some(v.trim().to_string()),
            "anchor" => anchor = Anchor::from_token(v.trim()),
            "x" => x = v.trim().parse::<i32>().ok(),
            "y" => y = v.trim().parse::<i32>().ok(),
            _ => {}
        }
    }
    match mode.as_deref() {
        Some("anchor") => anchor.map_or_else(Position::default, Position::Anchor),
        Some("custom") => match (x, y) {
            (Some(x), Some(y)) => Position::Custom { x, y },
            _ => Position::default(),
        },
        _ => Position::default(),
    }
}

/// Top-left origin for `anchor` inside `work`, for a pill of `size`
/// (width, height), inset by `margin` at the edges.
pub fn anchor_origin(anchor: Anchor, work: Rect, size: (i32, i32), margin: i32) -> (i32, i32) {
    let (w, h) = size;
    let left = work.left + margin;
    let right = work.right - w - margin;
    let cx = work.left + (work.width() - w) / 2;
    let top = work.top + margin;
    let bottom = work.bottom - h - margin;
    let cy = work.top + (work.height() - h) / 2;
    match anchor {
        Anchor::TopLeft => (left, top),
        Anchor::TopCenter => (cx, top),
        Anchor::TopRight => (right, top),
        Anchor::MidLeft => (left, cy),
        Anchor::Center => (cx, cy),
        Anchor::MidRight => (right, cy),
        Anchor::BottomLeft => (left, bottom),
        Anchor::BottomCenter => (cx, bottom),
        Anchor::BottomRight => (right, bottom),
    }
}

/// Push `pos` so a pill of `size` stays fully inside `bounds`. If the pill is
/// larger than `bounds` on an axis, pin to the top/left edge of that axis.
pub fn clamp(pos: (i32, i32), bounds: Rect, size: (i32, i32)) -> (i32, i32) {
    let (x, y) = pos;
    let (w, h) = size;
    // .max(edge) handles a pill wider/taller than bounds: max < min would panic
    // in i32::clamp, so floor the upper bound at the near edge.
    let max_x = (bounds.right - w).max(bounds.left);
    let max_y = (bounds.bottom - h).max(bounds.top);
    (x.clamp(bounds.left, max_x), y.clamp(bounds.top, max_y))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_top_center() {
        assert_eq!(Position::default(), Position::Anchor(Anchor::TopCenter));
    }

    #[test]
    fn roundtrip_all_anchors() {
        for a in Anchor::ALL {
            let p = Position::Anchor(a);
            assert_eq!(parse(&format(&p)), p, "anchor {a:?}");
        }
    }

    #[test]
    fn roundtrip_custom() {
        let p = Position::Custom { x: 1820, y: 12 };
        assert_eq!(parse(&format(&p)), p);
    }

    #[test]
    fn roundtrip_custom_negative() {
        let p = Position::Custom { x: -100, y: -5 };
        assert_eq!(parse(&format(&p)), p);
    }

    #[test]
    fn parse_empty_is_default() {
        assert_eq!(parse(""), Position::default());
    }

    #[test]
    fn parse_garbage_is_default() {
        assert_eq!(parse("hello world\n???"), Position::default());
    }

    #[test]
    fn parse_unknown_anchor_is_default() {
        assert_eq!(
            parse("mode=anchor\nanchor=middle-of-nowhere"),
            Position::default()
        );
    }

    #[test]
    fn parse_custom_missing_coord_is_default() {
        assert_eq!(parse("mode=custom\nx=10"), Position::default());
    }

    #[test]
    fn parse_custom_nonnumeric_is_default() {
        assert_eq!(parse("mode=custom\nx=abc\ny=def"), Position::default());
    }

    fn work() -> Rect {
        Rect::new(0, 0, 1000, 600)
    }
    const SIZE: (i32, i32) = (100, 40);
    const M: i32 = 10;

    #[test]
    fn anchor_corners() {
        assert_eq!(anchor_origin(Anchor::TopLeft, work(), SIZE, M), (10, 10));
        assert_eq!(anchor_origin(Anchor::TopRight, work(), SIZE, M), (890, 10));
        assert_eq!(anchor_origin(Anchor::BottomLeft, work(), SIZE, M), (10, 550));
        assert_eq!(anchor_origin(Anchor::BottomRight, work(), SIZE, M), (890, 550));
    }

    #[test]
    fn anchor_edges_and_center() {
        assert_eq!(anchor_origin(Anchor::TopCenter, work(), SIZE, M), (450, 10));
        assert_eq!(anchor_origin(Anchor::BottomCenter, work(), SIZE, M), (450, 550));
        assert_eq!(anchor_origin(Anchor::MidLeft, work(), SIZE, M), (10, 280));
        assert_eq!(anchor_origin(Anchor::MidRight, work(), SIZE, M), (890, 280));
        assert_eq!(anchor_origin(Anchor::Center, work(), SIZE, M), (450, 280));
    }

    #[test]
    fn anchor_respects_work_offset() {
        // Work area not at the origin (taskbar inset / secondary monitor).
        let w = Rect::new(100, 50, 1100, 650);
        assert_eq!(anchor_origin(Anchor::TopLeft, w, SIZE, M), (110, 60));
        assert_eq!(anchor_origin(Anchor::BottomRight, w, SIZE, M), (990, 600));
    }

    #[test]
    fn clamp_inside_unchanged() {
        assert_eq!(clamp((500, 300), work(), SIZE), (500, 300));
    }

    #[test]
    fn clamp_pushes_in() {
        assert_eq!(clamp((-50, -50), work(), SIZE), (0, 0));
        assert_eq!(clamp((5000, 5000), work(), SIZE), (900, 560));
    }

    #[test]
    fn clamp_respects_bounds_offset() {
        let b = Rect::new(100, 50, 1100, 650);
        assert_eq!(clamp((0, 0), b, SIZE), (100, 50));
    }

    #[test]
    fn clamp_pill_larger_than_bounds_pins_topleft() {
        let tiny = Rect::new(0, 0, 50, 20);
        assert_eq!(clamp((10, 10), tiny, (100, 40)), (0, 0));
    }
}
