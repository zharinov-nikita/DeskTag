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
}
