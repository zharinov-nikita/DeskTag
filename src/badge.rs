//! Win32 layered, click-through, always-on-top "pill" badge.
//! Painted with GDI; shaped with a rounded-rect region; uniform alpha.

use anyhow::{anyhow, Result};
use std::cell::RefCell;
use windows::core::w;
use windows::Win32::Foundation::{BOOL, COLORREF, HWND, LPARAM, LRESULT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::*;

/// Posted (from the listener thread) when the current desktop or its name changes.
pub const WM_APP_DESKTOP_CHANGED: u32 = WM_APP + 1;

// COLORREF is 0x00BBGGRR.
const BG_COLOR: COLORREF = COLORREF(0x0020_2020); // dark gray
const TEXT_COLOR: COLORREF = COLORREF(0x00F0_F0F0); // near white
const ALPHA: u8 = 220;

thread_local! {
    static LABEL: RefCell<String> = RefCell::new(String::from("Desktop ?"));
}

/// Create and show the badge window. Returns its HWND.
pub fn create(initial: &str) -> Result<HWND> {
    LABEL.with(|l| *l.borrow_mut() = initial.to_string());
    unsafe {
        let hinstance = GetModuleHandleW(None).map_err(|e| anyhow!("GetModuleHandleW: {e:?}"))?;
        let class_name = w!("FbvdBadgeClass");

        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(wndproc),
            hInstance: hinstance.into(),
            lpszClassName: class_name,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            ..Default::default()
        };
        if RegisterClassExW(&wc) == 0 {
            return Err(anyhow!("RegisterClassExW failed"));
        }

        // Hidden owner window. Owning the badge keeps it off the taskbar and out
        // of Alt-Tab WITHOUT WS_EX_TOOLWINDOW. This matters: a tool window (or a
        // WS_EX_NOACTIVATE window) is not given an application view by the shell,
        // and winvd::pin_window needs that view — so a tool window can never be
        // pinned to all desktops. Ownership achieves the same hiding while
        // leaving the badge pinnable.
        let owner = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class_name,
            w!("fbvd-owner"),
            WS_POPUP,
            0,
            0,
            0,
            0,
            None,
            None,
            hinstance,
            None,
        )
        .map_err(|e| anyhow!("CreateWindowExW(owner): {e:?}"))?;

        // No WS_EX_TOOLWINDOW / WS_EX_NOACTIVATE (see owner comment above). The
        // badge still never activates: it is shown with SW_SHOWNOACTIVATE, moved
        // with SWP_NOACTIVATE, and WS_EX_TRANSPARENT passes clicks through.
        let ex_style = WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST;

        let hwnd = CreateWindowExW(
            ex_style,
            class_name,
            w!("fbvd"),
            WS_POPUP,
            0,
            0,
            10,
            10,
            owner,
            None,
            hinstance,
            None,
        )
        .map_err(|e| anyhow!("CreateWindowExW: {e:?}"))?;

        // Uniform alpha over the region-clipped window.
        SetLayeredWindowAttributes(hwnd, COLORREF(0), ALPHA, LWA_ALPHA)
            .map_err(|e| anyhow!("SetLayeredWindowAttributes: {e:?}"))?;

        resize_and_position(hwnd);
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        Ok(hwnd)
    }
}

/// Re-assert HWND_TOPMOST (Z-order can be stolen; call on a timer).
pub fn reassert_topmost(hwnd: HWND) {
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
}

/// Replace the label, resize/reposition the pill, and repaint. UI thread only.
pub fn apply_label(hwnd: HWND, text: &str) {
    LABEL.with(|l| *l.borrow_mut() = text.to_string());
    unsafe {
        resize_and_position(hwnd);
        let _ = InvalidateRect(hwnd, None, BOOL(1));
    }
}

fn scale(hwnd: HWND, v: i32) -> i32 {
    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
    v * dpi as i32 / 96
}

unsafe fn make_font(hwnd: HWND) -> HFONT {
    CreateFontW(
        -scale(hwnd, 15),
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
    )
}

unsafe fn measure(hwnd: HWND) -> (i32, i32) {
    let pad_x = scale(hwnd, 14);
    let pad_y = scale(hwnd, 7);
    let hdc = GetDC(hwnd);
    let font = make_font(hwnd);
    let old = SelectObject(hdc, font);
    let mut size = SIZE::default();
    LABEL.with(|l| {
        let text: Vec<u16> = l.borrow().encode_utf16().collect();
        let _ = GetTextExtentPoint32W(hdc, &text, &mut size);
    });
    SelectObject(hdc, old);
    let _ = DeleteObject(font);
    ReleaseDC(hwnd, hdc);
    (size.cx + pad_x * 2, size.cy + pad_y * 2)
}

unsafe fn resize_and_position(hwnd: HWND) {
    let (w, h) = measure(hwnd);
    let screen_w = GetSystemMetrics(SM_CXSCREEN);
    let x = (screen_w - w) / 2;
    let y = scale(hwnd, 8);
    let _ = SetWindowPos(hwnd, HWND_TOPMOST, x, y, w, h, SWP_NOACTIVATE);
    let radius = scale(hwnd, 16);
    let rgn = CreateRoundRectRgn(0, 0, w + 1, h + 1, radius, radius);
    // The window takes ownership of the region; do not delete it here.
    let _ = SetWindowRgn(hwnd, rgn, BOOL(1));
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rc = RECT::default();
                let _ = GetClientRect(hwnd, &mut rc);

                let brush = CreateSolidBrush(BG_COLOR);
                FillRect(hdc, &rc, brush);
                let _ = DeleteObject(brush);

                let font = make_font(hwnd);
                let old = SelectObject(hdc, font);
                SetBkMode(hdc, TRANSPARENT);
                SetTextColor(hdc, TEXT_COLOR);
                LABEL.with(|l| {
                    let mut text: Vec<u16> = l.borrow().encode_utf16().collect();
                    let _ = DrawTextW(
                        hdc,
                        &mut text,
                        &mut rc,
                        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                    );
                });
                SelectObject(hdc, old);
                let _ = DeleteObject(font);

                let _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            WM_APP_DESKTOP_CHANGED => {
                if let Ok(text) = crate::desktop::current_label() {
                    apply_label(hwnd, &text);
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}
