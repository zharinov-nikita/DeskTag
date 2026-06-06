//! System theme (light/dark) detection and the per-theme color palette.
//! `Palette::for_theme` is a pure mapping (unit-tested); `detect` reads the
//! Windows registry. This module is the single source of truth for the badge
//! and tray-icon colors.

use windows::Win32::Foundation::COLORREF;

/// The Windows system-UI theme we render against.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Theme {
    Light,
    Dark,
}

/// Colors for one theme. `COLORREF` is `0x00BBGGRR`; the grays here are
/// symmetric, so byte order does not matter.
#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: COLORREF,
    pub text: COLORREF,
    pub border: COLORREF,
}

impl Palette {
    /// Pure `Theme` -> colors mapping.
    pub fn for_theme(theme: Theme) -> Palette {
        match theme {
            Theme::Dark => Palette {
                bg: COLORREF(0x0020_2020),
                text: COLORREF(0x00F0_F0F0),
                border: COLORREF(0x0050_5050),
            },
            Theme::Light => Palette {
                bg: COLORREF(0x00F0_F0F0),
                text: COLORREF(0x0020_2020),
                border: COLORREF(0x00C8_C8C8),
            },
        }
    }
}

/// Read `HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize`
/// value `SystemUsesLightTheme` (REG_DWORD): 1 => Light, 0 => Dark. On any
/// error or a missing value => Dark (preserve the current look; never panic).
pub fn detect() -> Theme {
    use windows::core::w;
    use windows::Win32::Foundation::ERROR_SUCCESS;
    use windows::Win32::System::Registry::{
        RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD,
    };

    let mut value: u32 = 0;
    let mut size = std::mem::size_of::<u32>() as u32;
    // SAFETY: `value`/`size` are valid for a 4-byte DWORD read; RRF_RT_REG_DWORD
    // restricts the value type so nothing else is written.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("SystemUsesLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut value as *mut u32 as *mut core::ffi::c_void),
            Some(&mut size),
        )
    };
    if status == ERROR_SUCCESS && value == 1 {
        Theme::Light
    } else {
        Theme::Dark
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_palette_matches_legacy_colors() {
        let p = Palette::for_theme(Theme::Dark);
        assert_eq!(p.bg.0, 0x0020_2020);
        assert_eq!(p.text.0, 0x00F0_F0F0);
        assert_eq!(p.border.0, 0x0050_5050);
    }

    #[test]
    fn light_palette_is_inverted() {
        let p = Palette::for_theme(Theme::Light);
        assert_eq!(p.bg.0, 0x00F0_F0F0);
        assert_eq!(p.text.0, 0x0020_2020);
        assert_eq!(p.border.0, 0x00C8_C8C8);
    }

    #[test]
    #[cfg(windows)]
    fn detect_returns_a_theme_without_panicking() {
        assert!(matches!(detect(), Theme::Light | Theme::Dark));
    }
}
