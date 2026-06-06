//! Rasterise the badge "pill" into pixels, then wrap it as a tray HICON or an .ico.
//! Single rasteriser, two consumers. GDI-based; Windows-only.

use anyhow::{anyhow, Result};
use std::ffi::c_void;
use windows::core::w;
use windows::Win32::Foundation::{COLORREF, RECT, SIZE};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::{CreateIconIndirect, HICON, ICONINFO};

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
        let mut rc = RECT {
            left: 0,
            top: 0,
            right: s,
            bottom: s,
        };
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
                let a = if PtInRegion(rgn, x, y).as_bool() {
                    255u8
                } else {
                    0u8
                };
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
            -h,
            0,
            0,
            0,
            FW_SEMIBOLD.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET.0 as u32,
            0,
            0,
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

/// Build a tray-sized HICON showing `text`. The caller owns it and must
/// `DestroyIcon` it when it is replaced or on teardown.
pub fn make_tray_hicon(text: &str, size: u32) -> Result<HICON> {
    let rgba = rasterize(text, size)?;
    let s = size as i32;
    unsafe {
        // Color bitmap: 32-bpp top-down DIB filled from RGBA (stored as BGRA).
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
        let screen = GetDC(None);
        let mut bits: *mut c_void = std::ptr::null_mut();
        let color = CreateDIBSection(screen, &bmi, DIB_RGB_COLORS, &mut bits, None, 0)
            .map_err(|e| anyhow!("CreateDIBSection(color): {e:?}"))?;
        ReleaseDC(None, screen);

        let dst = std::slice::from_raw_parts_mut(bits as *mut u8, rgba.len());
        for i in (0..rgba.len()).step_by(4) {
            dst[i] = rgba[i + 2]; // B
            dst[i + 1] = rgba[i + 1]; // G
            dst[i + 2] = rgba[i]; // R
            dst[i + 3] = rgba[i + 3]; // A
        }

        // 1-bpp AND mask: 1 = transparent (alpha 0), 0 = opaque.
        // CreateBitmap rows are WORD-aligned.
        let stride = (((s + 15) / 16) * 2) as usize;
        let mut maskbits = vec![0u8; stride * s as usize];
        for y in 0..s as usize {
            for x in 0..s as usize {
                if rgba[(y * s as usize + x) * 4 + 3] == 0 {
                    maskbits[y * stride + (x >> 3)] |= 0x80 >> (x & 7);
                }
            }
        }
        let mask = CreateBitmap(s, s, 1, 1, Some(maskbits.as_ptr() as *const c_void));

        let ii = ICONINFO {
            fIcon: true.into(),
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask,
            hbmColor: color,
        };
        let icon = CreateIconIndirect(&ii).map_err(|e| anyhow!("CreateIconIndirect: {e:?}"))?;
        // CreateIconIndirect copies the bitmaps; free our originals.
        let _ = DeleteObject(color);
        let _ = DeleteObject(mask);
        Ok(icon)
    }
}

/// Encode `text` as a multi-size .ico (16/32/48/256, BMP entries) and write `path`.
pub fn write_ico(path: &str, text: &str) -> Result<()> {
    let bytes = encode_ico(text, &[16, 32, 48, 256])?;
    std::fs::write(path, bytes).map_err(|e| anyhow!("write {path}: {e}"))
}

/// Assemble an ICONDIR + entries + BMP images. No PNG; BMP entries only.
fn encode_ico(text: &str, sizes: &[u32]) -> Result<Vec<u8>> {
    let images: Vec<Vec<u8>> = sizes
        .iter()
        .map(|&sz| bmp_icon_image(text, sz))
        .collect::<Result<_>>()?;

    let count = sizes.len() as u16;
    let mut out = Vec::new();
    out.extend_from_slice(&0u16.to_le_bytes()); // reserved
    out.extend_from_slice(&1u16.to_le_bytes()); // type = icon
    out.extend_from_slice(&count.to_le_bytes());

    let mut offset = 6 + 16 * count as u32; // data starts after the directory
    for (i, &sz) in sizes.iter().enumerate() {
        let dim = if sz >= 256 { 0u8 } else { sz as u8 };
        let len = images[i].len() as u32;
        out.push(dim); // width
        out.push(dim); // height
        out.push(0); // color count
        out.push(0); // reserved
        out.extend_from_slice(&1u16.to_le_bytes()); // planes
        out.extend_from_slice(&32u16.to_le_bytes()); // bit count
        out.extend_from_slice(&len.to_le_bytes()); // bytes in resource
        out.extend_from_slice(&offset.to_le_bytes()); // image offset
        offset += len;
    }
    for img in &images {
        out.extend_from_slice(img);
    }
    Ok(out)
}

/// One BMP-format icon image: BITMAPINFOHEADER (height doubled for XOR+AND),
/// 32-bpp BGRA XOR (bottom-up), then a 1-bpp AND mask (bottom-up, 4-byte rows).
fn bmp_icon_image(text: &str, size: u32) -> Result<Vec<u8>> {
    let rgba = rasterize(text, size)?; // top-down RGBA
    let s = size as usize;
    let mut out = Vec::new();

    out.extend_from_slice(&40u32.to_le_bytes()); // biSize
    out.extend_from_slice(&(size as i32).to_le_bytes()); // biWidth
    out.extend_from_slice(&((size * 2) as i32).to_le_bytes()); // biHeight = 2x
    out.extend_from_slice(&1u16.to_le_bytes()); // planes
    out.extend_from_slice(&32u16.to_le_bytes()); // bit count
    out.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
    out.extend_from_slice(&0u32.to_le_bytes()); // biSizeImage (0 allowed)
    out.extend_from_slice(&0i32.to_le_bytes()); // x ppm
    out.extend_from_slice(&0i32.to_le_bytes()); // y ppm
    out.extend_from_slice(&0u32.to_le_bytes()); // clr used
    out.extend_from_slice(&0u32.to_le_bytes()); // clr important

    // XOR: BGRA, bottom-up.
    for y in (0..s).rev() {
        for x in 0..s {
            let i = (y * s + x) * 4;
            out.push(rgba[i + 2]); // B
            out.push(rgba[i + 1]); // G
            out.push(rgba[i]); // R
            out.push(rgba[i + 3]); // A
        }
    }
    // AND mask: 1 bpp, bottom-up, rows padded to 4 bytes. 1 = transparent.
    let stride = s.div_ceil(32) * 4;
    for y in (0..s).rev() {
        let mut row = vec![0u8; stride];
        for x in 0..s {
            if rgba[(y * s + x) * 4 + 3] == 0 {
                row[x >> 3] |= 0x80 >> (x & 7);
            }
        }
        out.extend_from_slice(&row);
    }
    Ok(out)
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
        assert!(
            rgba.chunks(4).any(|p| p[3] == 255),
            "expected opaque pixels"
        );
        assert!(
            rgba.chunks(4).any(|p| p[3] == 0),
            "expected transparent corners"
        );
    }

    #[test]
    fn make_tray_hicon_returns_icon() {
        let icon = make_tray_hicon("2", 32).unwrap();
        assert!(!icon.is_invalid());
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::DestroyIcon(icon);
        }
    }

    #[test]
    fn encode_ico_has_valid_header() {
        let sizes = [16u32, 32, 48, 256];
        let bytes = encode_ico("D", &sizes).unwrap();
        // ICONDIR magic: reserved=0, type=1 (icon).
        assert_eq!(&bytes[0..4], &[0, 0, 1, 0]);
        // Entry count.
        assert_eq!(u16::from_le_bytes([bytes[4], bytes[5]]), sizes.len() as u16);
        // First entry width byte (offset 6) = 16.
        assert_eq!(bytes[6], 16);
        // 256 is encoded as 0 in the last entry's width byte.
        let last = 6 + 16 * (sizes.len() - 1);
        assert_eq!(bytes[last], 0);
    }
}
