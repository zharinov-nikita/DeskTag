//! Win32 layered, click-through, always-on-top "pill" badge.
//! Painted with GDI; shaped with a rounded-rect region; uniform alpha.

use anyhow::{anyhow, Result};
use std::cell::{Cell, RefCell};
use windows::core::w;
use windows::Win32::Foundation::{BOOL, COLORREF, HWND, LPARAM, LRESULT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::theme::{Palette, Theme};

/// Posted (from the listener thread) when the current desktop or its name changes.
pub const WM_APP_DESKTOP_CHANGED: u32 = WM_APP + 1;

/// Posted by the shell when the tray icon is interacted with.
const WM_APP_TRAY: u32 = WM_APP + 2;
const TRAY_UID: u32 = 1;
const MENU_QUIT: usize = 1001;
const TIMER_TOPMOST: usize = 1;
const TIMER_POLL: usize = 2;

// COLORREF is 0x00BBGGRR.
const ALPHA: u8 = 220;

thread_local! {
    static LABEL: RefCell<String> = RefCell::new(String::from("Desktop ?"));
    /// The tray HICON we currently own (null = none / using a shared fallback).
    static CURRENT_TRAY_ICON: Cell<HICON> = const { Cell::new(HICON(std::ptr::null_mut())) };
    /// The system theme we last painted with. Updated on WM_SETTINGCHANGE.
    static CURRENT_THEME: Cell<Theme> = const { Cell::new(Theme::Dark) };
}

/// Create and show the badge window. Returns its HWND.
pub fn create(initial: &str) -> Result<HWND> {
    LABEL.with(|l| *l.borrow_mut() = initial.to_string());
    CURRENT_THEME.with(|t| t.set(crate::theme::detect()));
    unsafe {
        let hinstance = GetModuleHandleW(None).map_err(|e| anyhow!("GetModuleHandleW: {e:?}"))?;
        let class_name = w!("DeskTagBadgeClass");

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
        // of Alt-Tab while it is being pinned. We do NOT set WS_EX_TOOLWINDOW at
        // creation: a tool window (or a WS_EX_NOACTIVATE window) is not given an
        // application view by the shell, and winvd::pin_window needs that view to
        // pin the badge to every desktop. So the order is: create plain + owned,
        // pin (which registers the app view — and that app view re-introduces the
        // badge into Alt-Tab despite ownership), THEN add WS_EX_TOOLWINDOW via
        // hide_from_alt_tab. The pinned app view survives the late style change,
        // but the badge drops back out of the switcher.
        let owner = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class_name,
            w!("DeskTag-owner"),
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

        // WS_EX_TOOLWINDOW is added later (post-pin; see hide_from_alt_tab and the
        // owner comment above), not here. WS_EX_NOACTIVATE is never used: the badge
        // already never activates — it is shown with SW_SHOWNOACTIVATE, moved with
        // SWP_NOACTIVATE, and WS_EX_TRANSPARENT passes clicks through.
        let ex_style = WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST;

        let hwnd = CreateWindowExW(
            ex_style,
            class_name,
            w!("DeskTag"),
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
        SetTimer(hwnd, TIMER_TOPMOST, 2000, None);
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

/// Drop the badge out of the Alt-Tab switcher by adding WS_EX_TOOLWINDOW.
///
/// Call this AFTER the window has been pinned to all desktops: pinning registers
/// a shell application view, and an app-view window shows up in Alt-Tab even when
/// it is owned. WS_EX_TOOLWINDOW removes it from the switcher; the already-pinned
/// app view survives the late style change, so the badge stays on every desktop.
pub fn hide_from_alt_tab(hwnd: HWND) {
    unsafe {
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex | WS_EX_TOOLWINDOW.0 as isize);
        // Commit the style change so the frame/switcher state is refreshed.
        let _ = SetWindowPos(
            hwnd,
            HWND::default(),
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}

/// Start a 750ms poll that re-reads the label. Use only as a fallback when the
/// event listener could not start.
pub fn start_fallback_poll(hwnd: HWND) {
    unsafe {
        SetTimer(hwnd, TIMER_POLL, 750, None);
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

/// The palette for the theme we currently believe is active (UI thread only).
fn current_palette() -> Palette {
    Palette::for_theme(CURRENT_THEME.with(|t| t.get()))
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

/// Current desktop number ("1", "2", …) for the tray icon. Falls back to "?".
fn current_desktop_number() -> String {
    match crate::desktop::current_index_and_name() {
        Ok((index0, _)) => (index0 + 1).to_string(),
        Err(_) => "?".to_string(),
    }
}

/// Square tray icon size for this session (16, or 32 on HiDPI shells).
fn tray_icon_size() -> u32 {
    let px = unsafe { GetSystemMetrics(SM_CXSMICON) };
    if px <= 0 {
        16
    } else {
        px as u32
    }
}

/// Install a tray icon (dynamic desktop-number pill) whose context menu offers Quit.
pub fn install_tray(hwnd: HWND) {
    unsafe {
        let size = tray_icon_size();
        let hicon = match crate::icon::make_tray_hicon(&current_desktop_number(), size) {
            Ok(h) => {
                CURRENT_TRAY_ICON.with(|c| c.set(h));
                h
            }
            Err(_) => LoadIconW(None, IDI_APPLICATION).unwrap_or_default(),
        };
        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_UID,
            uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
            uCallbackMessage: WM_APP_TRAY,
            hIcon: hicon,
            ..Default::default()
        };
        // szTip: copy "DeskTag" into the fixed [u16; 128] buffer.
        for (i, c) in "DeskTag".encode_utf16().enumerate() {
            nid.szTip[i] = c;
        }
        let _ = Shell_NotifyIconW(NIM_ADD, &nid);
    }
}

/// Rebuild the tray icon for the current desktop number and push it to the shell.
/// Destroys the previously-owned icon. No-op on rasterise failure (keeps current).
fn update_tray_icon(hwnd: HWND) {
    unsafe {
        let size = tray_icon_size();
        let new = match crate::icon::make_tray_hicon(&current_desktop_number(), size) {
            Ok(h) => h,
            Err(_) => return,
        };
        let nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_UID,
            uFlags: NIF_ICON,
            hIcon: new,
            ..Default::default()
        };
        let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
        let old = CURRENT_TRAY_ICON.with(|c| c.replace(new));
        if !old.is_invalid() {
            let _ = DestroyIcon(old);
        }
    }
}

unsafe fn remove_tray(hwnd: HWND) {
    let nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_UID,
        ..Default::default()
    };
    let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
    let old = CURRENT_TRAY_ICON.with(|c| c.replace(HICON(std::ptr::null_mut())));
    if !old.is_invalid() {
        let _ = DestroyIcon(old);
    }
}

unsafe fn show_tray_menu(hwnd: HWND) {
    let menu = CreatePopupMenu().unwrap_or_default();
    let _ = AppendMenuW(menu, MF_STRING, MENU_QUIT, w!("Quit"));
    let mut pt = windows::Win32::Foundation::POINT::default();
    let _ = GetCursorPos(&mut pt);
    // Required so the menu closes when focus is lost.
    let _ = SetForegroundWindow(hwnd);
    let _ = TrackPopupMenu(menu, TPM_RIGHTBUTTON, pt.x, pt.y, 0, hwnd, None);
    let _ = DestroyMenu(menu);
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rc = RECT::default();
                let _ = GetClientRect(hwnd, &mut rc);

                let p = current_palette();

                let brush = CreateSolidBrush(p.bg);
                FillRect(hdc, &rc, brush);
                let _ = DeleteObject(brush);

                let font = make_font(hwnd);
                let old = SelectObject(hdc, font);
                SetBkMode(hdc, TRANSPARENT);
                SetTextColor(hdc, p.text);
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

                // Hairline border: frame the rounded region from the inside so
                // the (DPI-scaled) 1px line is not clipped by the window region.
                let radius = scale(hwnd, 16);
                let rgn = CreateRoundRectRgn(0, 0, rc.right + 1, rc.bottom + 1, radius, radius);
                let border_brush = CreateSolidBrush(p.border);
                let t = scale(hwnd, 1).max(1);
                let _ = FrameRgn(hdc, rgn, border_brush, t, t);
                let _ = DeleteObject(border_brush);
                let _ = DeleteObject(rgn);

                let _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            WM_APP_DESKTOP_CHANGED => {
                if let Ok(text) = crate::desktop::current_label() {
                    apply_label(hwnd, &text);
                }
                update_tray_icon(hwnd);
                LRESULT(0)
            }
            WM_APP_TRAY => {
                // lParam low word holds the mouse event.
                let event = (lparam.0 as u32) & 0xFFFF;
                if event == WM_RBUTTONUP || event == WM_LBUTTONUP {
                    show_tray_menu(hwnd);
                }
                LRESULT(0)
            }
            WM_COMMAND => {
                if (wparam.0 & 0xFFFF) == MENU_QUIT {
                    remove_tray(hwnd);
                    PostQuitMessage(0);
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                remove_tray(hwnd);
                PostQuitMessage(0);
                LRESULT(0)
            }
            WM_TIMER => {
                match wparam.0 {
                    TIMER_TOPMOST => reassert_topmost(hwnd),
                    TIMER_POLL => {
                        if let Ok(text) = crate::desktop::current_label() {
                            // apply_label is cheap and idempotent; only repaints on change of size.
                            apply_label(hwnd, &text);
                        }
                        update_tray_icon(hwnd);
                    }
                    _ => {}
                }
                LRESULT(0)
            }
            WM_DPICHANGED => {
                resize_and_position(hwnd);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}
