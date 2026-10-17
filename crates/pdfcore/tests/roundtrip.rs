// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Fufeng Project

use pdfcore::Document;

fn sample() -> String {
    format!("{}/../../test/sample.pdf", env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn selftest_saves_marks_and_encrypted_export() {
    let out = format!("{}/../../target/selftest-out", env!("CARGO_MANIFEST_DIR"));
    let log = pdfcore::selftest::run(&sample(), &out).expect("selftest");
    println!("{log}");

    // 完整檔：可重新開啟，頁數不變，含註解
    let full = Document::open_file(&format!("{out}/full.pdf"), "").expect("open full");
    let orig = Document::open_file(&sample(), "").unwrap();
    assert_eq!(full.page_count(), orig.page_count());
    let raw = std::fs::read(format!("{out}/full.pdf")).unwrap();
    let lo = lopdf::Document::load_mem(&raw).unwrap();
    let page1 = *lo.get_pages().get(&1).unwrap();
    let annots = lo.get_dictionary(page1).unwrap().get(b"Annots").unwrap().as_array().unwrap().len();
    assert!(annots >= 7, "annots = {annots}");
    // 新增的文字可被擷取（ToUnicode 正確）
    let text = full.page_text(0).plain_text();
    assert!(text.contains("新增文字測試"), "text: {text}");

    // 加密檔：沒有密碼打不開，開啟密碼與權限密碼都可開啟，只有 2 頁（或 1 頁）
    let enc = format!("{out}/pages_enc.pdf");
    assert!(matches!(Document::open_file(&enc, ""), Err(pdfcore::OpenError::NeedsPassword)));
    let e = Document::open_file(&enc, "1234").expect("user pw");
    assert_eq!(e.page_count(), orig.page_count().min(3).min(2).max(1).min(if orig.page_count() >= 3 { 2 } else { 1 }));
    Document::open_file(&enc, "owner").expect("owner pw");
}
