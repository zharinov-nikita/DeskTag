//! Rasterise the badge "pill" into pixels, then wrap it as a tray HICON or an .ico.
//! Single rasteriser, two consumers. GDI-based; Windows-only.

use anyhow::{anyhow, Result};
use std::ffi::c_void;
use windows::core::w;
use windows::Win32::Foundation::{COLORREF, RECT, SIZE};
use windows::Win32::Graphics::Gdi::*;

// Pill palette — keep in sync with badge.rs (window pill uses the same colors).
const BG: COLORREF = COLORREF(0x0020_2020); // 0x00BBGGRR, dark gray
const TEXT: COLORREF = COLORREF(0x00F0_F0F0); // near white

/// Rasterise a rounded pill with centered `text` into a `size`×`size`, top-down
/// RGBA buffer (4 bytes/px). Alpha is 255 inside the pill, 0 outside.
pub fn rasterize(text: &str, size: u32) -> Result<Vec<u8>> {
    let s = size as i32;
    let radius = (s * 6 / 10).max(2); // strong rounding for a pill look
    unsafe {
        let hdc = CreateCompatibleDC(None);
        if hdc.is_invalid() {
            return Err(anyhow!("CreateCompatibleDC failed"));
        }

        // Top-down 32-bpp DIB we can read back (negative biHeight = top-down).
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: s,
                biHeight: -s,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut c_void = std::ptr::null_mut();
        let dib = CreateDIBSection(hdc, &bmi, DIB_RGB_COLORS, &mut bits, None, 0)
            .map_err(|e| anyhow!("CreateDIBSection: {e:?}"))?;
        let old_bmp = SelectObject(hdc, dib);

        // Transparent everywhere first.
        std::ptr::write_bytes(bits as *mut u8, 0, (s * s * 4) as usize);

        // Pill fill, no outline: NULL_PEN + solid brush.
        let brush = CreateSolidBrush(BG);
        let old_brush = SelectObject(hdc, brush);
        let old_pen = SelectObject(hdc, GetStockObject(NULL_PEN));
        let _ = RoundRect(hdc, 0, 0, s, s, radius, radius);
        SelectObject(hdc, old_pen);
        SelectObject(hdc, old_brush);
        let _ = DeleteObject(brush);

        // Centered text, shrink-to-fit (handles 2-digit numbers).
        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, TEXT);
        let font = make_icon_font(hdc, text, size);
        let old_font = SelectObject(hdc, font);
        let mut rc = RECT { left: 0, top: 0, right: s, bottom: s };
        let mut wtext: Vec<u16> = text.encode_utf16().collect();
        let _ = DrawTextW(
            hdc,
            &mut wtext,
            &mut rc,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOCLIP,
        );
        SelectObject(hdc, old_font);
        let _ = DeleteObject(font);

        // Read pixels (BGRA, top-down).
        let mut bgra = vec![0u8; (s * s * 4) as usize];
        std::ptr::copy_nonoverlapping(bits as *const u8, bgra.as_mut_ptr(), bgra.len());

        SelectObject(hdc, old_bmp);
        let _ = DeleteObject(dib);
        let _ = DeleteDC(hdc);

        // Alpha from region membership; convert BGRA -> RGBA.
        let rgn = CreateRoundRectRgn(0, 0, s + 1, s + 1, radius, radius);
        let mut rgba = vec![0u8; bgra.len()];
        for y in 0..s {
            for x in 0..s {
                let i = ((y * s + x) * 4) as usize;
                let a = if PtInRegion(rgn, x, y).as_bool() { 255u8 } else { 0u8 };
                rgba[i] = bgra[i + 2]; // R
                rgba[i + 1] = bgra[i + 1]; // G
                rgba[i + 2] = bgra[i]; // B
                rgba[i + 3] = a;
            }
        }
        let _ = DeleteObject(rgn);
        Ok(rgba)
    }
}

/// A Segoe UI Semibold font sized to fill the pill, shrunk so `text` fits the width.
unsafe fn make_icon_font(hdc: HDC, text: &str, size: u32) -> HFONT {
    let max_w = size as i32 * 78 / 100;
    let mut h = (size as i32 * 72 / 100).max(6);
    loop {
        let font = CreateFontW(
            -h, 0, 0, 0,
            FW_SEMIBOLD.0 as i32,
            0, 0, 0,
            DEFAULT_CHARSET.0 as u32,
            0, 0,
            CLEARTYPE_QUALITY.0 as u32,
            0,
            w!("Segoe UI"),
        );
        let old = SelectObject(hdc, font);
        let wt: Vec<u16> = text.encode_utf16().collect();
        let mut ext = SIZE::default();
        let _ = GetTextExtentPoint32W(hdc, &wt, &mut ext);
        SelectObject(hdc, old);
        if ext.cx <= max_w || h <= 6 {
            return font;
        }
        let _ = DeleteObject(font);
        h = (h * max_w / ext.cx.max(1)).max(6);
    }
}

#[cfg(test)]
#[cfg(windows)]
mod tests {
    use super::*;

    #[test]
    fn rasterize_pill_has_size_and_alpha() {
        let size = 32u32;
        let rgba = rasterize("1", size).unwrap();
        assert_eq!(rgba.len(), (size * size * 4) as usize);
        // Pill body is opaque somewhere, corners are transparent somewhere.
        assert!(rgba.chunks(4).any(|p| p[3] == 255), "expected opaque pixels");
        assert!(rgba.chunks(4).any(|p| p[3] == 0), "expected transparent corners");
    }
}
