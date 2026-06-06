//! Persist the chosen badge position to `%APPDATA%\DeskTag\config`.
//! Thin IO over `position::{parse, format}`. Never panics.

use crate::position::{self, Position};
use std::path::PathBuf;

/// `%APPDATA%\DeskTag\config`, or `None` if `APPDATA` is unset.
fn config_path() -> Option<PathBuf> {
    let appdata = std::env::var_os("APPDATA")?;
    let mut p = PathBuf::from(appdata);
    p.push("DeskTag");
    p.push("config");
    Some(p)
}

/// Load the saved position. Missing file / unreadable / malformed → default.
pub fn load() -> Position {
    load_from(config_path())
}

fn load_from(path: Option<PathBuf>) -> Position {
    let Some(path) = path else {
        return Position::default();
    };
    match std::fs::read_to_string(&path) {
        Ok(s) => position::parse(&s),
        Err(_) => Position::default(),
    }
}

/// Save the position. Best-effort: errors are logged, never propagated.
pub fn save(pos: &Position) {
    if let Err(e) = save_to(config_path(), pos) {
        eprintln!("config save failed: {e}");
    }
}

fn save_to(path: Option<PathBuf>, pos: &Position) -> std::io::Result<()> {
    let path = path
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "APPDATA not set"))?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&path, position::format(pos))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_load_roundtrip_via_temp() {
        let dir = std::env::temp_dir().join(format!("desktag-cfg-{}", std::process::id()));
        let path = dir.join("config");
        let _ = std::fs::remove_dir_all(&dir);
        let p = Position::Custom { x: 42, y: 99 };
        save_to(Some(path.clone()), &p).expect("save");
        assert_eq!(load_from(Some(path)), p);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_missing_file_is_default() {
        let path = std::env::temp_dir().join("desktag-cfg-nonexistent/config");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
        assert_eq!(load_from(Some(path)), Position::default());
    }

    #[test]
    fn load_none_path_is_default() {
        assert_eq!(load_from(None), Position::default());
    }
}
