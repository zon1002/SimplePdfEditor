// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Fufeng Project

//! 從 zip 加入語言包（「從檔案加入…」與下載後的安裝相同流程）。需要 target/ocr-packs。

use ocr::packs;

#[test]
fn install_zip_into_user_folder_then_remove() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/ocr-packs");
    if !src.join("ocr-ko-v1.zip").exists() {
        eprintln!("略過：找不到 target/ocr-packs");
        return;
    }
    let home = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/install-test");
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).unwrap();
    // SAFETY: 單一測試、讀取語言包前設定；使用者資料夾改到 target 下
    unsafe {
        std::env::set_var("LOCALAPPDATA", &home);
        std::env::remove_var("SPE_OCR_PACKS");
    }
    assert!(packs::find_installed("ko").is_none());
    for id in ["engine", "ko"] {
        let info = packs::install_zip(&src.join(format!("ocr-{id}-v1.zip")), None).expect("加入");
        assert_eq!(info.id, id);
    }
    let ko = packs::find_installed("ko").expect("應已安裝");
    assert!(ko.user && ko.dir.starts_with(&home));
    assert!(packs::find_installed("engine").is_some());

    // 損壞的 zip 不能加入，也不留下殘檔
    let bad = home.join("bad.zip");
    let mut bytes = std::fs::read(src.join("ocr-latin-v1.zip")).unwrap();
    let mid = bytes.len() / 2;
    bytes[mid] ^= 0xFF;
    std::fs::write(&bad, bytes).unwrap();
    assert!(packs::install_zip(&bad, None).is_err());
    assert!(packs::find_installed("latin").is_none());

    packs::remove(&ko).unwrap();
    assert!(packs::find_installed("ko").is_none());
}
