// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Fufeng Project
// 簡易 PDF 編輯器 — Windows 檔案總管整合
//   * 預覽處理常式（IPreviewHandler）：檔案總管預覽窗格中可捲動瀏覽所有頁面
//   * 縮圖提供者（IThumbnailProvider）：以第 1 頁作為縮圖
// 由安裝程式寫入登錄（見 SimplePdfEditor_Setup.iss），本 DLL 只負責 COM 物件本身。

// 函式庫名稱（SimplePdfPreview）與 COM 匯出函式名稱沿用 Windows 慣例
#![allow(non_snake_case)]

mod common;
mod preview;
mod thumbnail;

use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicI32, AtomicPtr, Ordering};

use windows::Win32::Foundation::{
    CLASS_E_CLASSNOTAVAILABLE, CLASS_E_NOAGGREGATION, E_FAIL, E_POINTER, HINSTANCE, S_FALSE, S_OK,
};
use windows::Win32::System::Com::{IClassFactory, IClassFactory_Impl};
use windows::Win32::System::SystemServices::DLL_PROCESS_ATTACH;
use windows::core::{BOOL, GUID, HRESULT, IUnknown, Interface, Ref, Result, implement};

use common::guard;
use preview::PdfPreviewHandler;
use thumbnail::PdfThumbnailProvider;

// {9E089D14-B74E-4DDD-A2B7-D1821458115C}
pub const CLSID_PDF_PREVIEW_HANDLER: GUID = GUID::from_u128(0x9E089D14_B74E_4DDD_A2B7_D1821458115C);
// {7D86EEC3-4516-48CB-90C9-35EC10436737}
pub const CLSID_PDF_THUMBNAIL_PROVIDER: GUID = GUID::from_u128(0x7D86EEC3_4516_48CB_90C9_35EC10436737);

static MODULE: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
pub(crate) static OBJECTS: AtomicI32 = AtomicI32::new(0);
static LOCKS: AtomicI32 = AtomicI32::new(0);

pub(crate) fn module() -> HINSTANCE {
    HINSTANCE(MODULE.load(Ordering::Relaxed))
}

// ============================== Class factory ==============================

#[derive(Clone, Copy)]
enum Kind {
    Preview,
    Thumbnail,
}

#[implement(IClassFactory)]
struct ClassFactory {
    kind: Kind,
}

impl IClassFactory_Impl for ClassFactory_Impl {
    fn CreateInstance(&self, outer: Ref<IUnknown>, riid: *const GUID, ppv: *mut *mut c_void) -> Result<()> {
        guard(|| unsafe {
            if ppv.is_null() || riid.is_null() {
                return Err(E_POINTER.into());
            }
            *ppv = std::ptr::null_mut();
            if !outer.is_null() {
                return Err(CLASS_E_NOAGGREGATION.into());
            }
            let obj: IUnknown = match self.kind {
                Kind::Preview => PdfPreviewHandler::new().into(),
                Kind::Thumbnail => PdfThumbnailProvider::new().into(),
            };
            obj.query(riid, ppv).ok()
        })
    }

    fn LockServer(&self, lock: BOOL) -> Result<()> {
        if lock.as_bool() {
            LOCKS.fetch_add(1, Ordering::SeqCst);
        } else {
            LOCKS.fetch_sub(1, Ordering::SeqCst);
        }
        Ok(())
    }
}

// ============================== DLL 匯出 ==============================

#[unsafe(no_mangle)]
pub extern "system" fn DllMain(instance: HINSTANCE, reason: u32, _reserved: *mut c_void) -> BOOL {
    if reason == DLL_PROCESS_ATTACH {
        MODULE.store(instance.0, Ordering::Relaxed);
    }
    BOOL(1)
}

/// # Safety
/// 由 COM 呼叫；指標參數須符合 DllGetClassObject 規範。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllGetClassObject(clsid: *const GUID, riid: *const GUID, ppv: *mut *mut c_void) -> HRESULT {
    catch_unwind(AssertUnwindSafe(|| unsafe {
        if ppv.is_null() {
            return E_POINTER;
        }
        *ppv = std::ptr::null_mut();
        if clsid.is_null() || riid.is_null() {
            return E_POINTER;
        }
        let kind = match *clsid {
            CLSID_PDF_PREVIEW_HANDLER => Kind::Preview,
            CLSID_PDF_THUMBNAIL_PROVIDER => Kind::Thumbnail,
            _ => return CLASS_E_CLASSNOTAVAILABLE,
        };
        let factory: IClassFactory = ClassFactory { kind }.into();
        factory.query(riid, ppv)
    }))
    .unwrap_or(E_FAIL)
}

#[unsafe(no_mangle)]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    if OBJECTS.load(Ordering::SeqCst) == 0 && LOCKS.load(Ordering::SeqCst) == 0 { S_OK } else { S_FALSE }
}
