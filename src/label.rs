//! Pure formatting of the badge text. OS-independent; the only unit-tested unit.

/// Build the badge text from a 0-based desktop index and its (possibly empty) name.
///
/// - Empty/whitespace name -> `"Desktop {n}"` where `n = index + 1`.
/// - Otherwise            -> `"{n} · {name}"` (trimmed).
pub fn format_label(index0: u32, name: &str) -> String {
    let n = index0 + 1;
    let name = name.trim();
    if name.is_empty() {
        format!("Desktop {n}")
    } else {
        format!("{n} · {name}")
    }
}

#[cfg(test)]
mod tests {
    use super::format_label;

    #[test]
    fn unnamed_desktop_shows_number() {
        assert_eq!(format_label(0, ""), "Desktop 1");
        assert_eq!(format_label(3, "   "), "Desktop 4");
    }

    #[test]
    fn named_desktop_shows_number_and_name() {
        assert_eq!(format_label(0, "auth"), "1 · auth");
        assert_eq!(format_label(2, "  ui  "), "3 · ui");
    }
}
