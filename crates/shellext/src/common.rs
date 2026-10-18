// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Fufeng Project
// 預覽與縮圖共用：讀取 IStream、PDF 渲染結果轉 HBITMAP、防止 panic 越過 COM 邊界

use std::panic::{AssertUnwindSafe, catch_unwind};

use pdfcore::{Document, Image};
use windows::Win32::Foundation::E_FAIL;
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateDIBSection, DIB_RGB_COLORS, DeleteObject, HBITMAP,
};
use windows::Win32::System::Com::{IStream, STATFLAG_NONAME, STATSTG, STREAM_SEEK_SET};
use windows::core::Result;

/// 超過 512 MB 不預覽
pub const MAX_FILE_SIZE: usize = 512 << 20;

/// 執行 COM 方法本體；發生 panic 時回傳 E_FAIL
pub fn guard<T>(f: impl FnOnce() -> Result<T>) -> Result<T> {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or_else(|_| Err(E_FAIL.into()))
}

/// 從 IStream 讀入全部內容
pub fn read_stream(stream: &IStream) -> Option<Vec<u8>> {
    unsafe {
        let mut st = STATSTG::default();
        if stream.Stat(&mut st, STATFLAG_NONAME).is_ok() && st.cbSize > MAX_FILE_SIZE as u64 {
            return None;
        }
        let _ = stream.Seek(0, STREAM_SEEK_SET, None);
        let mut out = Vec::new();
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let mut got = 0u32;
            let hr = stream.Read(buf.as_mut_ptr().cast(), buf.len() as u32, Some(&mut got));
            if got > 0 {
                out.extend_from_slice(&buf[..got as usize]);
            }
            if hr.is_err() || got == 0 || out.len() > MAX_FILE_SIZE {
                break;
            }
        }
        (!out.is_empty() && out.len() <= MAX_FILE_SIZE).then_some(out)
    }
}

/// 渲染單頁（渲染器的 panic 也攔下來）
pub fn render_page(doc: &Document, page: usize, scale: f64) -> Option<Image> {
    catch_unwind(AssertUnwindSafe(|| doc.render(page, scale))).ok().flatten()
}

/// RGBA 影像轉為 32 位元由上而下的 DIB（BGRA）；失敗回傳 None
pub fn image_to_bitmap(img: &Image) -> Option<HBITMAP> {
    let (w, h) = (img.width as usize, img.height as usize);
    if w == 0 || h == 0 || img.rgba.len() < w * h * 4 {
        return None;
    }
    let bi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w as i32,
            biHeight: -(h as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    unsafe {
        let mut bits = std::ptr::null_mut();
        let bmp = CreateDIBSection(None, &bi, DIB_RGB_COLORS, &mut bits, None, 0).ok()?;
        if bits.is_null() {
            let _ = DeleteObject(bmp.into());
            return None;
        }
        let dst = std::slice::from_raw_parts_mut(bits.cast::<u8>(), w * h * 4);
        for (d, s) in dst.chunks_exact_mut(4).zip(img.rgba.chunks_exact(4)) {
            d[0] = s[2];
            d[1] = s[1];
            d[2] = s[0];
            d[3] = s[3];
        }
        Some(bmp)
    }
}
