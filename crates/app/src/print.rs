// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Fufeng Project

//! 列印（Win32 GDI）：系統列印對話框、逐頁點陣輸出到印表機。
//!
//! 把含註記的文件（與另存新檔相同的結果）逐頁繪到印表機。
//! 頁面比紙張大時等比縮小到可列印範圍，否則以實際大小置中；橫向頁面在直式紙張上自動旋轉。

use pdfcore::{Document, Image};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use windows::Win32::Foundation::{GlobalFree, HGLOBAL, HWND};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, DEVMODEW, DIB_RGB_COLORS, DeleteDC, GetDeviceCaps, HALFTONE, HDC, HORZRES, LOGPIXELSX, PHYSICALHEIGHT,
    PHYSICALOFFSETX, PHYSICALOFFSETY, PHYSICALWIDTH, SRCCOPY, SetStretchBltMode, StretchDIBits, VERTRES,
};
use windows::Win32::Storage::Xps::{AbortDoc, DOCINFOW, EndDoc, EndPage, StartDocW, StartPage};
use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};
use windows::Win32::UI::Controls::Dialogs::{
    PD_COLLATE, PD_CURRENTPAGE, PD_NOSELECTION, PD_PAGENUMS, PD_RESULT_PRINT, PD_RETURNDC, PD_RETURNDEFAULT, PRINTDLGEXW, PRINTPAGERANGE, PrintDlgExW,
    START_PAGE_GENERAL,
};
use windows::core::PCWSTR;

/// DMCOLOR_MONOCHROME
const DM_MONOCHROME: i16 = 1;

/// 印表機裝置內容（可跨執行緒傳遞：列印在背景執行緒進行）
pub struct PrinterDc(HDC);
// SAFETY: HDC 只是控制代碼；建立後只由列印執行緒使用，結束時刪除
unsafe impl Send for PrinterDc {}

impl Drop for PrinterDc {
    fn drop(&mut self) {
        // SAFETY: 由 PrintDlgExW 建立、只刪除一次
        unsafe {
            let _ = DeleteDC(self.0);
        }
    }
}

pub struct PrintSetup {
    dc: PrinterDc,
    /// 0 起算，已依份數與自動分頁展開
    pub sequence: Vec<usize>,
    pub gray: bool,
}

fn free(h: HGLOBAL) {
    if !h.is_invalid() {
        // SAFETY: 由對話框配置的記憶體，用完釋放
        unsafe {
            let _ = GlobalFree(Some(h));
        }
    }
}

/// DEVMODE 是否設定為黑白列印
fn devmode_is_gray(h: HGLOBAL) -> bool {
    if h.is_invalid() {
        return false;
    }
    // SAFETY: hDevMode 為對話框傳回的 DEVMODEW 記憶體
    unsafe {
        let p = GlobalLock(h) as *const DEVMODEW;
        if p.is_null() {
            return false;
        }
        let gray = (*p).dmColor.0 == DM_MONOCHROME;
        let _ = GlobalUnlock(h);
        gray
    }
}

/// 顯示系統列印對話框；使用者按下「列印」時回傳設定
pub fn print_dialog(owner: isize, page_count: usize, current_page: usize) -> Option<PrintSetup> {
    if page_count == 0 {
        return None;
    }
    let mut ranges = [PRINTPAGERANGE { nFromPage: 1, nToPage: page_count as u32 }; 10];
    let mut pd = PRINTDLGEXW {
        lStructSize: std::mem::size_of::<PRINTDLGEXW>() as u32,
        hwndOwner: HWND(owner as *mut _),
        Flags: PD_RETURNDC | PD_NOSELECTION,
        nMaxPageRanges: ranges.len() as u32,
        nPageRanges: 0,
        lpPageRanges: ranges.as_mut_ptr(),
        nMinPage: 1,
        nMaxPage: page_count as u32,
        nCopies: 1,
        nStartPage: START_PAGE_GENERAL,
        ..Default::default()
    };
    // SAFETY: 結構已正確初始化；頁碼範圍陣列在呼叫期間有效
    let ok = unsafe { PrintDlgExW(&mut pd) }.is_ok();
    let gray = devmode_is_gray(pd.hDevMode);
    free(pd.hDevMode);
    free(pd.hDevNames);
    if !ok || pd.dwResultAction != PD_RESULT_PRINT || pd.hDC.is_invalid() {
        if !pd.hDC.is_invalid() {
            drop(PrinterDc(pd.hDC));
        }
        return None;
    }
    let dc = PrinterDc(pd.hDC);

    let mut pages: Vec<usize> = Vec::new();
    if pd.Flags.contains(PD_PAGENUMS) {
        for r in ranges.iter().take(pd.nPageRanges as usize) {
            let (a, b) = (r.nFromPage.min(r.nToPage).max(1) as usize, r.nFromPage.max(r.nToPage).min(page_count as u32) as usize);
            for p in a..=b {
                pages.push(p - 1);
            }
        }
    } else if pd.Flags.contains(PD_CURRENTPAGE) {
        pages.push(current_page.min(page_count - 1));
    } else {
        pages.extend(0..page_count);
    }
    // 驅動程式不處理多份時自行重複（依「自動分頁」決定順序）
    let copies = pd.nCopies.clamp(1, 999) as usize;
    let sequence: Vec<usize> = if pd.Flags.contains(PD_COLLATE) {
        (0..copies).flat_map(|_| pages.iter().copied()).collect()
    } else {
        pages.iter().flat_map(|&p| std::iter::repeat_n(p, copies)).collect()
    };
    if sequence.is_empty() {
        return None;
    }
    Some(PrintSetup { dc, sequence, gray })
}

/// 紙張資訊（英吋）：整張紙與可列印範圍
#[derive(Clone, Copy, Debug)]
pub struct Paper {
    pub width: f64,
    pub height: f64,
    pub printable: [f64; 4],
    pub dpi: f64,
}

impl Paper {
    /// A4、四周 0.25 吋不可列印（找不到印表機時使用）
    pub fn a4() -> Paper {
        let (w, h) = (210.0 / 25.4, 297.0 / 25.4);
        Paper { width: w, height: h, printable: [0.25, 0.25, w - 0.25, h - 0.25], dpi: 600.0 }
    }
    pub fn rotated(&self) -> Paper {
        let [x0, y0, x1, y1] = self.printable;
        Paper { width: self.height, height: self.width, printable: [y0, self.width - x1, y1, self.width - x0], dpi: self.dpi }
    }
}

fn paper_of(dc: HDC) -> Option<Paper> {
    // SAFETY: dc 為有效的印表機裝置內容
    let cap = |i| unsafe { GetDeviceCaps(Some(dc), i) } as f64;
    let dpi = cap(LOGPIXELSX);
    if dpi <= 0.0 {
        return None;
    }
    let (pw, ph) = (cap(PHYSICALWIDTH) / dpi, cap(PHYSICALHEIGHT) / dpi);
    let (ox, oy) = (cap(PHYSICALOFFSETX) / dpi, cap(PHYSICALOFFSETY) / dpi);
    let (hw, vh) = (cap(HORZRES) / dpi, cap(VERTRES) / dpi);
    if pw <= 0.0 || ph <= 0.0 {
        return None;
    }
    Some(Paper { width: pw, height: ph, printable: [ox, oy, ox + hw, oy + vh], dpi })
}

/// 預設印表機的紙張（列印預覽用）
pub fn default_paper() -> Option<Paper> {
    let mut pd = PRINTDLGEXW {
        lStructSize: std::mem::size_of::<PRINTDLGEXW>() as u32,
        Flags: PD_RETURNDEFAULT | PD_RETURNDC,
        nStartPage: START_PAGE_GENERAL,
        ..Default::default()
    };
    // SAFETY: PD_RETURNDEFAULT 不顯示對話框，只傳回預設印表機
    let ok = unsafe { PrintDlgExW(&mut pd) }.is_ok();
    free(pd.hDevMode);
    free(pd.hDevNames);
    if !ok || pd.hDC.is_invalid() {
        return None;
    }
    let dc = PrinterDc(pd.hDC);
    paper_of(dc.0)
}

/// 頁面在紙張上的擺放（與實際列印相同的規則）：回傳 (是否旋轉, 可列印範圍內的目標矩形 [x, y, w, h])，單位同 area
pub fn place_page(page_w: f64, page_h: f64, area_w: f64, area_h: f64, units_per_pt: f64) -> (bool, [f64; 4]) {
    let rotate = (page_w > page_h) != (area_w > area_h);
    let (avail_w, avail_h) = if rotate { (area_h, area_w) } else { (area_w, area_h) };
    let scale = units_per_pt.min((avail_w / page_w).min(avail_h / page_h));
    let (w, h) = (page_w * scale, page_h * scale);
    if rotate {
        // 旋轉 90°（順時針）後置中
        (true, [(area_w - h) / 2.0, (area_h - w) / 2.0, h, w])
    } else {
        (false, [(avail_w - w) / 2.0, (avail_h - h) / 2.0, w, h])
    }
}

/// 影像順時針旋轉 90°
pub fn rotate_cw(img: &Image) -> Image {
    let (w, h) = (img.width as usize, img.height as usize);
    let mut out = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let (nx, ny) = (h - 1 - y, x);
            let s = (y * w + x) * 4;
            let d = (ny * h + nx) * 4;
            out[d..d + 4].copy_from_slice(&img.rgba[s..s + 4]);
        }
    }
    Image { width: img.height, height: img.width, rgba: out }
}

/// 轉成灰階
pub fn to_gray(img: &mut Image) {
    for px in img.rgba.chunks_exact_mut(4) {
        let y = (px[0] as u32 * 299 + px[1] as u32 * 587 + px[2] as u32 * 114) / 1000;
        px[0] = y as u8;
        px[1] = y as u8;
        px[2] = y as u8;
    }
}

/// 在背景執行緒執行：開啟含註記的 PDF（另存結果），逐頁送到印表機。
/// `progress` 回報已完成頁數；`cancel` 設為 true 時中止列印。
pub fn run_print_job(setup: PrintSetup, pdf: Vec<u8>, doc_name: String, progress: Arc<AtomicUsize>, cancel: Arc<AtomicBool>) -> Result<bool, String> {
    let doc = Document::open_bytes(pdf, &doc_name, "").map_err(|e| format!("準備列印失敗：{e}"))?;
    let dc = setup.dc.0;
    // SAFETY: dc 為有效的印表機裝置內容
    let cap = |i| unsafe { GetDeviceCaps(Some(dc), i) } as f64;
    let res = cap(LOGPIXELSX).max(72.0);
    let (area_w, area_h) = (cap(HORZRES), cap(VERTRES));
    let render_dpi = res.min(300.0);

    let name: Vec<u16> = doc_name.encode_utf16().chain(std::iter::once(0)).collect();
    let di = DOCINFOW { cbSize: std::mem::size_of::<DOCINFOW>() as i32, lpszDocName: PCWSTR(name.as_ptr()), ..Default::default() };
    // SAFETY: 文件名稱字串在呼叫期間有效
    if unsafe { StartDocW(dc, &di) } <= 0 {
        return Err("無法開始列印（印表機無法使用？）".into());
    }
    for (i, &page) in setup.sequence.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            // SAFETY: 已開始的列印工作
            unsafe { AbortDoc(dc) };
            return Ok(false);
        }
        // SAFETY: 同上
        unsafe { StartPage(dc) };
        let (pw, ph) = doc.page_size(page);
        let actual = res / 72.0;
        let (rotate, target) = place_page(pw, ph, area_w, area_h, actual);
        let scale = if rotate { target[3] / pw } else { target[2] / pw };
        if let Some(mut img) = doc.render(page, render_dpi / 72.0 * (scale / actual)) {
            if setup.gray {
                to_gray(&mut img);
            }
            if rotate {
                img = rotate_cw(&img);
            }
            blit(dc, &img, target);
        }
        // SAFETY: 同上
        unsafe { EndPage(dc) };
        progress.store(i + 1, Ordering::Relaxed);
    }
    // SAFETY: 同上
    unsafe { EndDoc(dc) };
    Ok(true)
}

/// RGBA 影像縮放繪到 dc 的 target 矩形
fn blit(dc: HDC, img: &Image, target: [f64; 4]) {
    let mut bgra = img.rgba.clone();
    for px in bgra.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: img.width as i32,
            biHeight: -(img.height as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    // SAFETY: 影像緩衝區大小與 BITMAPINFO 一致
    unsafe {
        SetStretchBltMode(dc, HALFTONE);
        StretchDIBits(
            dc,
            target[0].round() as i32,
            target[1].round() as i32,
            target[2].round() as i32,
            target[3].round() as i32,
            0,
            0,
            img.width as i32,
            img.height as i32,
            Some(bgra.as_ptr() as *const _),
            &bmi,
            DIB_RGB_COLORS,
            SRCCOPY,
        );
    }
}
