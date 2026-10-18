// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Fufeng Project
// 外掛測試：不寫入登錄，直接 LoadLibrary 取得 COM 物件，
// 將縮圖與預覽畫面（含捲動後）存成 BMP。
//   shellext_test <SimplePdfPreview.dll> <file.pdf> <輸出前綴>

use std::ffi::{OsStr, OsString, c_void};
use std::os::windows::ffi::OsStrExt;
use std::process::exit;

use windows::Win32::Foundation::{FreeLibrary, GetLastError, HMODULE, HWND, LPARAM, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleBitmap, CreateCompatibleDC, DIB_RGB_COLORS,
    DeleteDC, DeleteObject, GetDC, GetDIBits, GetObjectW, HBITMAP, ReleaseDC, SelectObject,
};
use windows::Win32::System::Com::{
    COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize, IClassFactory, IStream, STGM_READ, STGM_SHARE_DENY_NONE,
};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use windows::Win32::UI::Shell::PropertiesSystem::IInitializeWithStream;
use windows::Win32::UI::Shell::{IPreviewHandler, IThumbnailProvider, SHCreateStreamOnFileEx, WTS_ALPHATYPE};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, GW_CHILD, GetClientRect, GetWindow, PRF_CLIENT, SendMessageW, WM_MOUSEWHEEL,
    WM_PRINTCLIENT, WS_EX_TOOLWINDOW, WS_POPUP,
};
use windows::core::{GUID, HRESULT, IUnknown, Interface, PCWSTR, Result, s, w};

const CLSID_PDF_PREVIEW_HANDLER: GUID = GUID::from_u128(0x9E089D14_B74E_4DDD_A2B7_D1821458115C);
const CLSID_PDF_THUMBNAIL_PROVIDER: GUID = GUID::from_u128(0x7D86EEC3_4516_48CB_90C9_35EC10436737);

type GetClassObjectFn = unsafe extern "system" fn(*const GUID, *const GUID, *mut *mut c_void) -> HRESULT;
type CanUnloadFn = unsafe extern "system" fn() -> HRESULT;

fn wide(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain(Some(0)).collect()
}

fn with_suffix(prefix: &OsStr, suffix: &str) -> OsString {
    let mut p = prefix.to_os_string();
    p.push(suffix);
    p
}

fn hr_of(r: &Result<()>) -> HRESULT {
    match r {
        Ok(()) => HRESULT(0),
        Err(e) => e.code(),
    }
}

fn save_bmp(bmp: HBITMAP, path: &OsStr) -> bool {
    unsafe {
        let mut bm = BITMAP::default();
        GetObjectW(bmp.into(), size_of::<BITMAP>() as i32, Some(&mut bm as *mut BITMAP as *mut c_void));
        let (w, h) = (bm.bmWidth, bm.bmHeight);
        if w <= 0 || h <= 0 {
            return false;
        }
        let mut bi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = vec![0u8; w as usize * h as usize * 4];
        let dc = GetDC(None);
        GetDIBits(dc, bmp, 0, h as u32, Some(pixels.as_mut_ptr().cast()), &mut bi, DIB_RGB_COLORS);
        ReleaseDC(None, dc);

        // BITMAPFILEHEADER（14 位元組）+ BITMAPINFOHEADER（40 位元組）+ 像素
        let off = 14u32 + 40;
        let mut out = Vec::with_capacity(off as usize + pixels.len());
        out.extend_from_slice(b"BM");
        out.extend_from_slice(&(off + pixels.len() as u32).to_le_bytes());
        out.extend_from_slice(&[0u8; 4]);
        out.extend_from_slice(&off.to_le_bytes());
        out.extend_from_slice(&40u32.to_le_bytes());
        out.extend_from_slice(&w.to_le_bytes());
        out.extend_from_slice(&(-h).to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        out.extend_from_slice(&[0u8; 24]); // BI_RGB、影像大小、解析度、色盤皆為 0
        out.extend_from_slice(&pixels);
        std::fs::write(path, out).is_ok()
    }
}

fn capture(hwnd: HWND, path: &OsStr) -> bool {
    unsafe {
        let mut rc = RECT::default();
        let _ = GetClientRect(hwnd, &mut rc);
        let screen = GetDC(None);
        let mem = CreateCompatibleDC(Some(screen));
        let bmp = CreateCompatibleBitmap(screen, rc.right, rc.bottom);
        let old = SelectObject(mem, bmp.into());
        SendMessageW(hwnd, WM_PRINTCLIENT, Some(WPARAM(mem.0 as usize)), Some(LPARAM(PRF_CLIENT as isize)));
        SelectObject(mem, old);
        let ok = save_bmp(bmp, path);
        let _ = DeleteObject(bmp.into());
        let _ = DeleteDC(mem);
        ReleaseDC(None, screen);
        ok
    }
}

fn main() {
    let args: Vec<OsString> = std::env::args_os().collect();
    if args.len() < 4 {
        exit(2);
    }
    let prefix = args[3].as_os_str();
    let dll_path = wide(&args[1]);
    let pdf_path = wide(&args[2]);

    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let dll: HMODULE = match LoadLibraryW(PCWSTR(dll_path.as_ptr())) {
            Ok(h) => h,
            Err(_) => {
                println!("LoadLibrary failed {}", GetLastError().0);
                exit(3);
            }
        };
        let Some(proc) = GetProcAddress(dll, s!("DllGetClassObject")) else { exit(4) };
        let get_class: GetClassObjectFn = std::mem::transmute(proc);

        let factory = |clsid: &GUID| -> Result<IClassFactory> {
            let mut p = std::ptr::null_mut();
            get_class(clsid, &IClassFactory::IID, &mut p).ok()?;
            Ok(IClassFactory::from_raw(p))
        };
        let open_stream = || -> Result<IStream> {
            SHCreateStreamOnFileEx(PCWSTR(pdf_path.as_ptr()), (STGM_READ | STGM_SHARE_DENY_NONE).0, 0, false, None)
        };

        // ---- 縮圖 ----
        {
            let mut bmp = HBITMAP::default();
            let r = (|| -> Result<()> {
                let cf = factory(&CLSID_PDF_THUMBNAIL_PROVIDER)?;
                let tp: IThumbnailProvider = cf.CreateInstance(None::<&IUnknown>)?;
                let init: IInitializeWithStream = tp.cast()?;
                let stream = open_stream()?;
                init.Initialize(&stream, STGM_READ.0)?;
                let mut alpha = WTS_ALPHATYPE::default();
                tp.GetThumbnail(256, &mut bmp, &mut alpha)
            })();
            println!("thumbnail hr=0x{:08X}", hr_of(&r).0 as u32);
            if !bmp.is_invalid() {
                save_bmp(bmp, &with_suffix(prefix, "_thumb.bmp"));
                let _ = DeleteObject(bmp.into());
            }
        }

        // ---- 預覽 ----
        {
            // 畫面外的隱藏宿主視窗（不顯示、不搶焦點）
            let host = CreateWindowExW(
                WS_EX_TOOLWINDOW,
                w!("STATIC"),
                w!("host"),
                WS_POPUP,
                -4000,
                -4000,
                520,
                700,
                None,
                None,
                GetModuleHandleW(None).ok().map(Into::into),
                None,
            )
            .unwrap_or_default();
            let mut ph: Option<IPreviewHandler> = None;
            let r = (|| -> Result<()> {
                let cf = factory(&CLSID_PDF_PREVIEW_HANDLER)?;
                let p: IPreviewHandler = cf.CreateInstance(None::<&IUnknown>)?;
                ph = Some(p.clone());
                let init: IInitializeWithStream = p.cast()?;
                let stream = open_stream()?;
                init.Initialize(&stream, STGM_READ.0)?;
                let rc = RECT { left: 0, top: 0, right: 520, bottom: 700 };
                p.SetWindow(host, &rc)?;
                p.DoPreview()
            })();
            println!("preview hr=0x{:08X}", hr_of(&r).0 as u32);
            if let Ok(view) = GetWindow(host, GW_CHILD) {
                capture(view, &with_suffix(prefix, "_preview1.bmp"));
                // 往下捲
                let wheel = WPARAM(((-120i16 as u16 as usize) << 16) as usize);
                for _ in 0..6 {
                    SendMessageW(view, WM_MOUSEWHEEL, Some(wheel), Some(LPARAM(0)));
                }
                capture(view, &with_suffix(prefix, "_preview2.bmp"));
                // 縮小窗格
                if let Some(p) = &ph {
                    let rc = RECT { left: 0, top: 0, right: 300, bottom: 500 };
                    let _ = p.SetRect(&rc);
                }
                capture(view, &with_suffix(prefix, "_preview3.bmp"));
            }
            if let Some(p) = ph.take() {
                let _ = p.Unload();
            }
            let _ = DestroyWindow(host);
        }

        let can_unload = GetProcAddress(dll, s!("DllCanUnloadNow"))
            .map(|f| std::mem::transmute::<_, CanUnloadFn>(f)().0 == 0)
            .unwrap_or(false);
        println!("DllCanUnloadNow={}", if can_unload { "S_OK" } else { "S_FALSE" });
        let _ = FreeLibrary(dll);
        CoUninitialize();
    }
}
