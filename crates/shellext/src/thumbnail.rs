// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Fu Project
// ============================== 縮圖提供者 ==============================

use std::cell::{Cell, RefCell};
use std::sync::atomic::Ordering;

use pdfcore::Document;
use windows::Win32::Foundation::{E_FAIL, E_INVALIDARG, ERROR_ALREADY_INITIALIZED};
use windows::Win32::Graphics::Gdi::HBITMAP;
use windows::Win32::System::Com::IStream;
use windows::Win32::UI::Shell::PropertiesSystem::{IInitializeWithStream, IInitializeWithStream_Impl};
use windows::Win32::UI::Shell::{IThumbnailProvider, IThumbnailProvider_Impl, WTS_ALPHATYPE, WTSAT_RGB};
use windows::core::{HRESULT, Ref, Result, implement};

use crate::OBJECTS;
use crate::common::{guard, image_to_bitmap, read_stream, render_page};

#[implement(IThumbnailProvider, IInitializeWithStream)]
pub struct PdfThumbnailProvider {
    doc: RefCell<Option<Document>>,
    initialized: Cell<bool>,
}

impl PdfThumbnailProvider {
    pub fn new() -> Self {
        OBJECTS.fetch_add(1, Ordering::SeqCst);
        Self { doc: RefCell::new(None), initialized: Cell::new(false) }
    }
}

impl Drop for PdfThumbnailProvider {
    fn drop(&mut self) {
        OBJECTS.fetch_sub(1, Ordering::SeqCst);
    }
}

impl IInitializeWithStream_Impl for PdfThumbnailProvider_Impl {
    fn Initialize(&self, stream: Ref<IStream>, _mode: u32) -> Result<()> {
        guard(|| {
            let stream = stream.as_ref().ok_or(E_INVALIDARG)?;
            if self.initialized.get() {
                return Err(HRESULT::from_win32(ERROR_ALREADY_INITIALIZED.0).into());
            }
            let data = read_stream(stream).ok_or(E_FAIL)?;
            self.initialized.set(true);
            let doc = Document::open_bytes(data, "", "").map_err(|_| E_FAIL)?;
            *self.doc.borrow_mut() = Some(doc);
            Ok(())
        })
    }
}

impl IThumbnailProvider_Impl for PdfThumbnailProvider_Impl {
    // 第 1 頁等比縮放到長邊 = cx
    fn GetThumbnail(&self, cx: u32, phbmp: *mut HBITMAP, alpha: *mut WTS_ALPHATYPE) -> Result<()> {
        guard(|| unsafe {
            if phbmp.is_null() || alpha.is_null() || cx == 0 {
                return Err(E_INVALIDARG.into());
            }
            *phbmp = HBITMAP::default();
            let doc = self.doc.borrow();
            let doc = doc.as_ref().ok_or(E_FAIL)?;
            if doc.page_count() == 0 {
                return Err(E_FAIL.into());
            }
            let (w, h) = doc.page_size(0);
            let scale = f64::from(cx) / w.max(h);
            let bmp = render_page(doc, 0, scale).as_ref().and_then(image_to_bitmap).ok_or(E_FAIL)?;
            *phbmp = bmp;
            *alpha = WTSAT_RGB;
            Ok(())
        })
    }
}
