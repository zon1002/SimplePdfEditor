// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Fufeng Project

//! Windows 剪貼簿：同時放入文字（CF_UNICODETEXT）與圖片（CF_DIB 與 PNG），
//! 貼上時由目的程式選擇需要的格式。

use pdfcore::Image;

#[cfg(test)]
thread_local! {
    /// 測試時不動到系統剪貼簿，改記錄在這裡：(文字, 圖片寬高)
    pub static LAST: std::cell::RefCell<Option<(Option<String>, Option<(u32, u32)>)>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub fn set(text: Option<&str>, image: Option<&Image>) -> Result<(), String> {
    LAST.with(|l| *l.borrow_mut() = Some((text.map(str::to_string), image.map(|i| (i.width, i.height)))));
    Ok(())
}

#[cfg(not(test))]
pub fn set(text: Option<&str>, image: Option<&Image>) -> Result<(), String> {
    set_system(text, image)
}

/// 寫入系統剪貼簿
pub fn set_system(text: Option<&str>, image: Option<&Image>) -> Result<(), String> {
    use windows::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, RegisterClipboardFormatW, SetClipboardData};
    use windows::Win32::System::Ole::{CF_DIB, CF_UNICODETEXT};
    use windows::core::w;

    // 其他程式可能正暫時占用剪貼簿：稍候重試
    let mut opened = false;
    for _ in 0..10 {
        if unsafe { OpenClipboard(None) }.is_ok() {
            opened = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(30));
    }
    if !opened {
        return Err("剪貼簿正由其他程式使用中，請稍後再試".into());
    }
    let result = (|| -> Result<(), String> {
        unsafe { EmptyClipboard() }.map_err(|e| e.to_string())?;
        let put = |format: u32, bytes: &[u8]| -> Result<(), String> {
            let h = global(bytes)?;
            unsafe { SetClipboardData(format, Some(windows::Win32::Foundation::HANDLE(h.0))) }.map_err(|e| e.to_string())?;
            Ok(())
        };
        if let Some(t) = text {
            // Windows 的換行為 CRLF，結尾為 0
            let wide: Vec<u16> = t.replace("\r\n", "\n").replace('\n', "\r\n").encode_utf16().chain([0]).collect();
            let bytes: Vec<u8> = wide.iter().flat_map(|c| c.to_le_bytes()).collect();
            put(CF_UNICODETEXT.0 as u32, &bytes)?;
        }
        if let Some(img) = image {
            put(CF_DIB.0 as u32, &dib(img))?;
            // 許多程式（Office、瀏覽器、通訊軟體）優先使用 PNG
            if let Some(png) = png(img) {
                let fmt = unsafe { RegisterClipboardFormatW(w!("PNG")) };
                if fmt != 0 {
                    put(fmt, &png)?;
                }
            }
        }
        Ok(())
    })();
    let _ = unsafe { CloseClipboard() };
    result
}

/// 配置可移動的全域記憶體並填入資料（成功放入剪貼簿後由系統管理）
fn global(bytes: &[u8]) -> Result<windows::Win32::Foundation::HGLOBAL, String> {
    use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
    unsafe {
        let h = GlobalAlloc(GMEM_MOVEABLE, bytes.len().max(1)).map_err(|e| e.to_string())?;
        let p = GlobalLock(h) as *mut u8;
        if p.is_null() {
            return Err("無法配置記憶體".into());
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len());
        let _ = GlobalUnlock(h);
        Ok(h)
    }
}

/// 與裝置無關的點陣圖：BITMAPINFOHEADER + 由下而上的 24 位元 BGR 列（每列補齊 4 位元組）
pub fn dib(img: &Image) -> Vec<u8> {
    let (w, h) = (img.width as usize, img.height as usize);
    let stride = (w * 3).div_ceil(4) * 4;
    let mut out = Vec::with_capacity(40 + stride * h);
    let le32 = |v: &mut Vec<u8>, x: u32| v.extend_from_slice(&x.to_le_bytes());
    le32(&mut out, 40); // biSize
    le32(&mut out, w as u32); // biWidth
    le32(&mut out, h as u32); // biHeight（正值：由下而上）
    out.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
    out.extend_from_slice(&24u16.to_le_bytes()); // biBitCount
    le32(&mut out, 0); // biCompression = BI_RGB
    le32(&mut out, (stride * h) as u32); // biSizeImage
    le32(&mut out, 11811); // 300 dpi（每公尺像素）
    le32(&mut out, 11811);
    le32(&mut out, 0);
    le32(&mut out, 0);
    for y in (0..h).rev() {
        let row = &img.rgba[y * w * 4..(y + 1) * w * 4];
        for px in row.chunks_exact(4) {
            out.extend_from_slice(&[px[2], px[1], px[0]]);
        }
        out.resize(out.len() + (stride - w * 3), 0);
    }
    out
}

/// PNG 編碼（頁面影像為不透明，直接當作預乘資料）
pub fn png(img: &Image) -> Option<Vec<u8>> {
    let size = tiny_skia::IntSize::from_wh(img.width, img.height)?;
    tiny_skia::Pixmap::from_vec(img.rgba.clone(), size)?.encode_png().ok()
}
