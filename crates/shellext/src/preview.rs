// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Fufeng Project
// ============================== 預覽處理常式 ==============================

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::OnceLock;
use std::sync::atomic::Ordering;

use pdfcore::{Document, OpenError};
use windows::Win32::Foundation::{
    COLORREF, E_FAIL, E_INVALIDARG, E_NOTIMPL, E_POINTER, ERROR_CLASS_ALREADY_EXISTS, GetLastError, HWND, LPARAM,
    LRESULT, RECT, S_FALSE, S_OK, SIZE, WPARAM,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateCompatibleBitmap, CreateCompatibleDC,
    CreateFontW, CreateSolidBrush, DEFAULT_CHARSET, DT_CENTER, DT_SINGLELINE, DT_VCENTER, DeleteDC, DeleteObject,
    DrawTextW, EndPaint, FW_NORMAL, FillRect, GetTextExtentPoint32W, HBITMAP, HDC, HFONT, InvalidateRect,
    LOGFONTW, OUT_DEFAULT_PRECIS, PAINTSTRUCT, SRCCOPY, SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::System::Com::IStream;
use windows::Win32::UI::Controls::{SetScrollInfo, SetScrollPos};
use windows::Win32::System::Ole::{IObjectWithSite, IObjectWithSite_Impl, IOleWindow, IOleWindow_Impl};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetFocus, SetFocus, VK_DOWN, VK_END, VK_HOME, VK_NEXT, VK_PRIOR, VK_SPACE, VK_UP,
};
use windows::Win32::UI::Shell::PropertiesSystem::{IInitializeWithStream, IInitializeWithStream_Impl};
use windows::Win32::UI::Shell::{
    IPreviewHandler, IPreviewHandler_Impl, IPreviewHandlerFrame, IPreviewHandlerVisuals, IPreviewHandlerVisuals_Impl,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DLGC_WANTARROWS, DLGC_WANTCHARS, DefWindowProcW,
    DestroyWindow, GWLP_USERDATA, GetClientRect, GetScrollInfo, GetWindowLongPtrW, IDC_ARROW, LoadCursorW, MSG,
    RegisterClassExW, SB_BOTTOM, SB_LINEDOWN, SB_LINEUP, SB_PAGEDOWN, SB_PAGEUP, SB_THUMBPOSITION, SB_THUMBTRACK,
    SB_TOP, SB_VERT, SCROLLINFO, SIF_ALL, SIF_PAGE, SIF_POS, SIF_RANGE, SW_SHOW, SWP_NOACTIVATE, SWP_NOZORDER,
    SetParent, SetWindowLongPtrW, SetWindowPos, ShowWindow, UnregisterClassW,
    WINDOW_EX_STYLE, WM_ERASEBKGND, WM_GETDLGCODE, WM_KEYDOWN, WM_LBUTTONDOWN, WM_MOUSEWHEEL, WM_NCCREATE,
    WM_NCDESTROY, WM_PAINT, WM_PRINTCLIENT, WM_SIZE, WM_VSCROLL, WNDCLASSEXW, WS_CHILD, WS_CLIPCHILDREN,
    WS_CLIPSIBLINGS, WS_VSCROLL,
};
use windows::core::{BOOL, Error, GUID, IUnknown, Interface, PCWSTR, Ref, Result, implement, w};

use crate::common::{guard, image_to_bitmap, read_stream, render_page};
use crate::{OBJECTS, module};

const WINDOW_CLASS: PCWSTR = w!("SimplePdfEditor.PreviewWindow");
const MARGIN: i32 = 12;
const GAP: i32 = 12;
const WHEEL_DELTA: i32 = 120;

const fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF(r as u32 | (g as u32) << 8 | (b as u32) << 16)
}
const DEFAULT_BG: COLORREF = rgb(0x52, 0x56, 0x5C);
const DEFAULT_FG: COLORREF = rgb(0xF0, 0xF0, 0xF0);

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

fn ui_font(height: i32) -> HFONT {
    unsafe {
        CreateFontW(
            height,
            0,
            0,
            0,
            FW_NORMAL.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            0,
            w!("Microsoft JhengHei UI"),
        )
    }
}

// ============================== 預覽視窗 ==============================
// 頁面依視窗寬度等比縮放後由上而下排列；只渲染畫面上看得到的頁面並快取。

enum DocState {
    Ready(Document),
    NeedsPassword,
    Failed,
}

struct CachedPage {
    width: i32,
    bmp: HBITMAP, // 渲染失敗時為 null（同樣快取，避免反覆重試）
    size: (i32, i32),
}

struct PreviewWindow {
    hwnd: Cell<HWND>,
    doc: DocState,
    sizes: Vec<(f64, f64)>, // 各頁大小（pt，已含旋轉）
    cache: RefCell<BTreeMap<usize, CachedPage>>,
    scroll: Cell<i32>,
    bg: Cell<COLORREF>,
    fg: Cell<COLORREF>,
}

impl Drop for PreviewWindow {
    fn drop(&mut self) {
        self.clear_cache();
    }
}

impl PreviewWindow {
    fn new(doc: DocState) -> Self {
        let sizes = match &doc {
            DocState::Ready(d) => (0..d.page_count()).map(|i| d.page_size(i)).collect(),
            _ => Vec::new(),
        };
        Self {
            hwnd: Cell::new(HWND::default()),
            doc,
            sizes,
            cache: RefCell::new(BTreeMap::new()),
            scroll: Cell::new(0),
            bg: Cell::new(DEFAULT_BG),
            fg: Cell::new(DEFAULT_FG),
        }
    }

    fn register_class() -> bool {
        static REGISTERED: OnceLock<bool> = OnceLock::new();
        *REGISTERED.get_or_init(|| unsafe {
            let wc = WNDCLASSEXW {
                cbSize: size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(Self::proc),
                hInstance: module(),
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                lpszClassName: WINDOW_CLASS,
                ..Default::default()
            };
            if RegisterClassExW(&wc) != 0 {
                return true;
            }
            // DLL 曾被卸載後重新載入：舊的視窗類別仍指向舊位址，先取消登錄再重新登錄
            if GetLastError() == ERROR_CLASS_ALREADY_EXISTS {
                let _ = UnregisterClassW(WINDOW_CLASS, Some(module()));
                return RegisterClassExW(&wc) != 0;
            }
            false
        })
    }

    fn create(parent: HWND, rc: &RECT, this: *const PreviewWindow) -> Result<HWND> {
        if !Self::register_class() {
            return Err(Error::from_thread());
        }
        unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                WINDOW_CLASS,
                w!(""),
                WS_CHILD | WS_VSCROLL | WS_CLIPSIBLINGS | WS_CLIPCHILDREN,
                rc.left,
                rc.top,
                rc.right - rc.left,
                rc.bottom - rc.top,
                Some(parent),
                None,
                Some(module()),
                Some(this as *const c_void),
            )
        }
    }

    fn set_colors(&self, bg: COLORREF, fg: COLORREF) {
        self.bg.set(bg);
        self.fg.set(fg);
        let hwnd = self.hwnd.get();
        if !hwnd.is_invalid() {
            unsafe {
                let _ = InvalidateRect(Some(hwnd), None, true);
            }
        }
    }

    unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        // 視窗程序中的 panic 不可越過 FFI 邊界
        catch_unwind(AssertUnwindSafe(|| unsafe {
            if msg == WM_NCCREATE {
                let cs = lp.0 as *const CREATESTRUCTW;
                let this = (*cs).lpCreateParams as *const PreviewWindow;
                if !this.is_null() {
                    (*this).hwnd.set(hwnd);
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, this as isize);
                }
            }
            let this = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const PreviewWindow;
            if this.is_null() { DefWindowProcW(hwnd, msg, wp, lp) } else { (*this).handle(msg, wp, lp) }
        }))
        .unwrap_or(LRESULT(0))
    }

    fn handle(&self, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        let hwnd = self.hwnd.get();
        unsafe {
            match msg {
                WM_SIZE => {
                    self.clear_cache();
                    self.update_scroll();
                    let _ = InvalidateRect(Some(hwnd), None, true);
                    return LRESULT(0);
                }
                WM_ERASEBKGND => return LRESULT(1),
                WM_PAINT => {
                    let mut ps = PAINTSTRUCT::default();
                    let hdc = BeginPaint(hwnd, &mut ps);
                    self.paint(hdc);
                    let _ = EndPaint(hwnd, &ps);
                    return LRESULT(0);
                }
                WM_PRINTCLIENT => {
                    // 供測試程式離屏擷取畫面
                    self.paint(HDC(wp.0 as *mut c_void));
                    return LRESULT(0);
                }
                WM_VSCROLL => {
                    self.on_scroll((wp.0 & 0xFFFF) as i32);
                    return LRESULT(0);
                }
                WM_MOUSEWHEEL => {
                    let delta = ((wp.0 >> 16) & 0xFFFF) as u16 as i16 as i32;
                    self.scroll_by(-delta * 120 / WHEEL_DELTA);
                    return LRESULT(0);
                }
                WM_LBUTTONDOWN => {
                    let _ = SetFocus(Some(hwnd));
                    return LRESULT(0);
                }
                WM_KEYDOWN => {
                    let key = wp.0 as u16;
                    let page = self.client_height() - 40;
                    match key {
                        k if k == VK_DOWN.0 => self.scroll_by(60),
                        k if k == VK_UP.0 => self.scroll_by(-60),
                        k if k == VK_NEXT.0 || k == VK_SPACE.0 => self.scroll_by(page),
                        k if k == VK_PRIOR.0 => self.scroll_by(-page),
                        k if k == VK_HOME.0 => self.scroll_to(0),
                        k if k == VK_END.0 => self.scroll_to(self.content_height()),
                        _ => return DefWindowProcW(hwnd, msg, wp, lp),
                    }
                    return LRESULT(0);
                }
                WM_GETDLGCODE => return LRESULT((DLGC_WANTARROWS | DLGC_WANTCHARS) as isize),
                WM_NCDESTROY => {
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                    self.hwnd.set(HWND::default());
                }
                _ => {}
            }
            DefWindowProcW(hwnd, msg, wp, lp)
        }
    }

    fn client_rect(&self) -> RECT {
        let mut r = RECT::default();
        unsafe {
            let _ = GetClientRect(self.hwnd.get(), &mut r);
        }
        r
    }
    fn client_width(&self) -> i32 {
        self.client_rect().right
    }
    fn client_height(&self) -> i32 {
        self.client_rect().bottom
    }
    fn page_width_px(&self) -> i32 {
        (self.client_width() - 2 * MARGIN).max(40)
    }

    fn page_height_px(&self, i: usize, width: i32) -> i32 {
        let (w, h) = self.sizes[i];
        (h * f64::from(width) / w + 0.5) as i32
    }

    fn content_height(&self) -> i32 {
        let width = self.page_width_px();
        let mut h = MARGIN;
        for i in 0..self.sizes.len() {
            h += self.page_height_px(i, width) + GAP;
        }
        h + MARGIN - GAP
    }

    fn update_scroll(&self) {
        let max = if self.can_render() { self.content_height() } else { 0 };
        let page = self.client_height();
        let mut si = SCROLLINFO {
            cbSize: size_of::<SCROLLINFO>() as u32,
            fMask: SIF_RANGE | SIF_PAGE | SIF_POS,
            nMin: 0,
            nMax: max,
            nPage: page.max(0) as u32,
            ..Default::default()
        };
        let pos = self.scroll.get().min(max - page + 1).max(0);
        self.scroll.set(pos);
        si.nPos = pos;
        unsafe {
            SetScrollInfo(self.hwnd.get(), SB_VERT, &si, true);
        }
    }

    fn scroll_to(&self, y: i32) {
        let max_y = (self.content_height() - self.client_height()).max(0);
        let y = y.min(max_y).max(0);
        if y == self.scroll.get() {
            return;
        }
        self.scroll.set(y);
        unsafe {
            SetScrollPos(self.hwnd.get(), SB_VERT, y, true);
            let _ = InvalidateRect(Some(self.hwnd.get()), None, false);
        }
    }

    fn scroll_by(&self, dy: i32) {
        self.scroll_to(self.scroll.get() + dy);
    }

    fn on_scroll(&self, code: i32) {
        let mut si = SCROLLINFO { cbSize: size_of::<SCROLLINFO>() as u32, fMask: SIF_ALL, ..Default::default() };
        unsafe {
            let _ = GetScrollInfo(self.hwnd.get(), SB_VERT, &mut si);
        }
        match code {
            c if c == SB_LINEUP.0 => self.scroll_by(-40),
            c if c == SB_LINEDOWN.0 => self.scroll_by(40),
            c if c == SB_PAGEUP.0 => self.scroll_by(-(si.nPage as i32)),
            c if c == SB_PAGEDOWN.0 => self.scroll_by(si.nPage as i32),
            c if c == SB_THUMBTRACK.0 || c == SB_THUMBPOSITION.0 => self.scroll_to(si.nTrackPos),
            c if c == SB_TOP.0 => self.scroll_to(0),
            c if c == SB_BOTTOM.0 => self.scroll_to(self.content_height()),
            _ => {}
        }
    }

    fn can_render(&self) -> bool {
        matches!(self.doc, DocState::Ready(_)) && !self.sizes.is_empty()
    }

    fn clear_cache(&self) {
        for (_, c) in std::mem::take(&mut *self.cache.borrow_mut()) {
            if !c.bmp.is_invalid() {
                unsafe {
                    let _ = DeleteObject(c.bmp.into());
                }
            }
        }
    }

    fn page_bitmap(&self, i: usize, width: i32) -> Option<(HBITMAP, (i32, i32))> {
        let DocState::Ready(doc) = &self.doc else { return None };
        let mut cache = self.cache.borrow_mut();
        if let Some(c) = cache.get(&i) {
            if c.width == width {
                return (!c.bmp.is_invalid()).then_some((c.bmp, c.size));
            }
            if !c.bmp.is_invalid() {
                unsafe {
                    let _ = DeleteObject(c.bmp.into());
                }
            }
            cache.remove(&i);
        }
        let img = render_page(doc, i, f64::from(width) / self.sizes[i].0);
        let bmp = img.as_ref().and_then(image_to_bitmap).unwrap_or_default();
        let size = img.map_or((0, 0), |m| (m.width as i32, m.height as i32));
        cache.insert(i, CachedPage { width, bmp, size });
        (!bmp.is_invalid()).then_some((bmp, size))
    }

    fn paint(&self, hdc: HDC) {
        let rc = self.client_rect();
        unsafe {
            // 雙緩衝避免捲動閃爍
            let mem = CreateCompatibleDC(Some(hdc));
            let back = CreateCompatibleBitmap(hdc, rc.right, rc.bottom);
            let old_back = SelectObject(mem, back.into());
            let bg_brush = CreateSolidBrush(self.bg.get());
            FillRect(mem, &rc, bg_brush);
            let _ = DeleteObject(bg_brush.into());

            if !self.can_render() {
                let msg = if matches!(self.doc, DocState::NeedsPassword) {
                    "此 PDF 已加密，需要密碼才能預覽。"
                } else {
                    "無法預覽此 PDF 檔案。"
                };
                SetBkMode(mem, TRANSPARENT);
                SetTextColor(mem, self.fg.get());
                let font = ui_font(-16);
                let old_font = SelectObject(mem, font.into());
                let mut text = wide(msg);
                let mut r = rc;
                DrawTextW(mem, &mut text, &mut r, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
                SelectObject(mem, old_font);
                let _ = DeleteObject(font.into());
            } else {
                let page_dc = CreateCompatibleDC(Some(hdc));
                let width = self.page_width_px();
                let mut y = MARGIN - self.scroll.get();
                let (mut first, mut last) = (-1isize, -1isize);
                for i in 0..self.sizes.len() {
                    let h = self.page_height_px(i, width);
                    if y + h >= 0 && y < rc.bottom {
                        if first < 0 {
                            first = i as isize;
                        }
                        last = i as isize;
                        if let Some((bmp, (cx, cy))) = self.page_bitmap(i, width) {
                            // 頁面陰影
                            let shadow = RECT { left: MARGIN + 3, top: y + 3, right: MARGIN + cx + 3, bottom: y + cy + 3 };
                            let sb = CreateSolidBrush(rgb(0, 0, 0));
                            FillRect(mem, &shadow, sb);
                            let _ = DeleteObject(sb.into());
                            let old = SelectObject(page_dc, bmp.into());
                            let _ = BitBlt(mem, MARGIN, y, cx, cy, Some(page_dc), 0, 0, SRCCOPY);
                            SelectObject(page_dc, old);
                        }
                    }
                    y += h + GAP;
                }
                let _ = DeleteDC(page_dc);

                // 只保留畫面附近（前後各 2 頁）的快取，控制記憶體用量
                self.cache.borrow_mut().retain(|&i, c| {
                    let keep = !(first >= 0 && ((i as isize) < first - 2 || (i as isize) > last + 2));
                    if !keep && !c.bmp.is_invalid() {
                        let _ = DeleteObject(c.bmp.into());
                    }
                    keep
                });

                // 右上角頁碼
                if first >= 0 {
                    let label = wide(&format!(" {} / {} ", first + 1, self.sizes.len()));
                    let font = ui_font(-13);
                    let old_font = SelectObject(mem, font.into());
                    let mut ts = SIZE::default();
                    let _ = GetTextExtentPoint32W(mem, &label, &mut ts);
                    let mut bx = RECT { left: rc.right - ts.cx - 20, top: 8, right: rc.right - 12, bottom: 8 + ts.cy + 4 };
                    let bb = CreateSolidBrush(rgb(0x33, 0x33, 0x33));
                    FillRect(mem, &bx, bb);
                    let _ = DeleteObject(bb.into());
                    SetBkMode(mem, TRANSPARENT);
                    SetTextColor(mem, rgb(0xFF, 0xFF, 0xFF));
                    let mut text = label;
                    DrawTextW(mem, &mut text, &mut bx, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
                    SelectObject(mem, old_font);
                    let _ = DeleteObject(font.into());
                }
            }

            let _ = BitBlt(hdc, 0, 0, rc.right, rc.bottom, Some(mem), 0, 0, SRCCOPY);
            SelectObject(mem, old_back);
            let _ = DeleteObject(back.into());
            let _ = DeleteDC(mem);
        }
    }
}

// ============================== COM 物件 ==============================

struct HandlerState {
    data: Vec<u8>,
    view: Option<Box<PreviewWindow>>,
    parent: HWND,
    hwnd: HWND,
    rect: RECT,
    site: Option<IUnknown>,
    bg: COLORREF,
    fg: COLORREF,
}

#[implement(IPreviewHandler, IPreviewHandlerVisuals, IInitializeWithStream, IObjectWithSite, IOleWindow)]
pub struct PdfPreviewHandler {
    st: RefCell<HandlerState>,
}

impl PdfPreviewHandler {
    pub fn new() -> Self {
        OBJECTS.fetch_add(1, Ordering::SeqCst);
        Self {
            st: RefCell::new(HandlerState {
                data: Vec::new(),
                view: None,
                parent: HWND::default(),
                hwnd: HWND::default(),
                rect: RECT::default(),
                site: None,
                bg: DEFAULT_BG,
                fg: DEFAULT_FG,
            }),
        }
    }

    fn unload(&self) {
        // 先取出再呼叫 Win32，避免重入時 RefCell 衝突
        let (hwnd, view) = {
            let mut st = self.st.borrow_mut();
            st.data = Vec::new();
            (std::mem::take(&mut st.hwnd), st.view.take())
        };
        if !hwnd.is_invalid() {
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
        }
        drop(view);
    }

    fn move_window(&self) {
        let (hwnd, rc) = {
            let st = self.st.borrow();
            (st.hwnd, st.rect)
        };
        if !hwnd.is_invalid() {
            unsafe {
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    rc.left,
                    rc.top,
                    rc.right - rc.left,
                    rc.bottom - rc.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
        }
    }

    fn apply_colors(&self) {
        let st = self.st.borrow();
        if let Some(v) = &st.view {
            v.set_colors(st.bg, st.fg);
        }
    }
}

impl Drop for PdfPreviewHandler {
    fn drop(&mut self) {
        self.unload();
        OBJECTS.fetch_sub(1, Ordering::SeqCst);
    }
}

impl IInitializeWithStream_Impl for PdfPreviewHandler_Impl {
    // 先把檔案讀進記憶體，DoPreview 時再解析
    fn Initialize(&self, stream: Ref<IStream>, _mode: u32) -> Result<()> {
        guard(|| {
            let stream = stream.as_ref().ok_or(E_INVALIDARG)?;
            let data = read_stream(stream).ok_or(E_FAIL)?;
            self.st.borrow_mut().data = data;
            Ok(())
        })
    }
}

impl IPreviewHandler_Impl for PdfPreviewHandler_Impl {
    fn SetWindow(&self, parent: HWND, prc: *const RECT) -> Result<()> {
        guard(|| unsafe {
            if parent.is_invalid() || prc.is_null() {
                return Err(E_INVALIDARG.into());
            }
            let hwnd = {
                let mut st = self.st.borrow_mut();
                st.parent = parent;
                st.rect = *prc;
                st.hwnd
            };
            if !hwnd.is_invalid() {
                let _ = SetParent(hwnd, Some(parent));
                self.move_window();
            }
            Ok(())
        })
    }

    fn SetRect(&self, prc: *const RECT) -> Result<()> {
        guard(|| unsafe {
            if prc.is_null() {
                return Err(E_INVALIDARG.into());
            }
            self.st.borrow_mut().rect = *prc;
            self.move_window();
            Ok(())
        })
    }

    fn DoPreview(&self) -> Result<()> {
        guard(|| {
            let (data, parent, rect, bg, fg) = {
                let mut st = self.st.borrow_mut();
                if !st.hwnd.is_invalid() || st.parent.is_invalid() {
                    return Err(E_FAIL.into());
                }
                (std::mem::take(&mut st.data), st.parent, st.rect, st.bg, st.fg)
            };
            // 開啟失敗時視窗會顯示「無法預覽」
            let doc = match Document::open_bytes(data, "", "") {
                Ok(d) => DocState::Ready(d),
                Err(OpenError::NeedsPassword) => DocState::NeedsPassword,
                Err(_) => DocState::Failed,
            };
            let view = Box::new(PreviewWindow::new(doc));
            view.set_colors(bg, fg);
            let hwnd = PreviewWindow::create(parent, &rect, &*view)?;
            {
                let mut st = self.st.borrow_mut();
                st.view = Some(view);
                st.hwnd = hwnd;
            }
            unsafe {
                let _ = ShowWindow(hwnd, SW_SHOW);
            }
            Ok(())
        })
    }

    fn Unload(&self) -> Result<()> {
        guard(|| {
            self.unload();
            Ok(())
        })
    }

    fn SetFocus(&self) -> Result<()> {
        guard(|| {
            let hwnd = self.st.borrow().hwnd;
            if hwnd.is_invalid() {
                return Err(Error::from_hresult(S_FALSE));
            }
            unsafe {
                let _ = SetFocus(Some(hwnd));
            }
            Ok(())
        })
    }

    fn QueryFocus(&self) -> Result<HWND> {
        guard(|| unsafe {
            let hwnd = GetFocus();
            if hwnd.is_invalid() {
                let e = GetLastError();
                if e.0 != 0 {
                    return Err(e.to_hresult().into());
                }
            }
            Ok(hwnd)
        })
    }

    // 未處理的快速鍵交還給檔案總管
    fn TranslateAccelerator(&self, msg: *const MSG) -> Result<()> {
        guard(|| unsafe {
            let site = self.st.borrow().site.clone();
            let mut hr = S_FALSE;
            if let Some(frame) = site.and_then(|s| s.cast::<IPreviewHandlerFrame>().ok()) {
                // 直接呼叫 vtable 以保留 S_FALSE（已處理 / 未處理）
                hr = (frame.vtable().TranslateAccelerator)(frame.as_raw(), msg);
            }
            if hr == S_OK { Ok(()) } else { Err(Error::from_hresult(hr)) }
        })
    }
}

// 跟隨檔案總管的深色／淺色配色
impl IPreviewHandlerVisuals_Impl for PdfPreviewHandler_Impl {
    fn SetBackgroundColor(&self, color: COLORREF) -> Result<()> {
        guard(|| {
            self.st.borrow_mut().bg = color;
            self.apply_colors();
            Ok(())
        })
    }

    fn SetFont(&self, _plf: *const LOGFONTW) -> Result<()> {
        Ok(())
    }

    fn SetTextColor(&self, color: COLORREF) -> Result<()> {
        guard(|| {
            self.st.borrow_mut().fg = color;
            self.apply_colors();
            Ok(())
        })
    }
}

impl IObjectWithSite_Impl for PdfPreviewHandler_Impl {
    fn SetSite(&self, site: Ref<IUnknown>) -> Result<()> {
        let old = std::mem::replace(&mut self.st.borrow_mut().site, site.cloned());
        drop(old);
        Ok(())
    }

    fn GetSite(&self, riid: *const GUID, ppv: *mut *mut c_void) -> Result<()> {
        guard(|| unsafe {
            if ppv.is_null() {
                return Err(E_POINTER.into());
            }
            *ppv = std::ptr::null_mut();
            let site = self.st.borrow().site.clone();
            match site {
                Some(s) => s.query(riid, ppv).ok(),
                None => Err(E_FAIL.into()),
            }
        })
    }
}

impl IOleWindow_Impl for PdfPreviewHandler_Impl {
    fn GetWindow(&self) -> Result<HWND> {
        Ok(self.st.borrow().parent)
    }

    fn ContextSensitiveHelp(&self, _enter: BOOL) -> Result<()> {
        Err(E_NOTIMPL.into())
    }
}
