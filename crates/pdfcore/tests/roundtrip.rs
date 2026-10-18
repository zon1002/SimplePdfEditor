// SPDX-License-Identifier: MIT OR Apache-2.0
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

#[test]
fn reads_bookmarks_with_levels_and_pages() {
    let mut lo = lopdf::Document::load(sample()).unwrap();
    let pages: Vec<lopdf::ObjectId> = lo.get_pages().values().copied().collect();
    let ch1 = lo.add_bookmark(lopdf::Bookmark::new("第一章".into(), [0.0; 3], 0, pages[0]), None);
    lo.add_bookmark(lopdf::Bookmark::new("1.1 小節".into(), [0.0; 3], 0, pages[1]), Some(ch1));
    lo.add_bookmark(lopdf::Bookmark::new("Chapter 2".into(), [0.0; 3], 0, pages[2]), None);
    let outline = lo.build_outline().unwrap();
    lo.catalog_mut().unwrap().set("Outlines", lopdf::Object::Reference(outline));
    let mut bytes = Vec::new();
    lo.save_to(&mut bytes).unwrap();

    let doc = Document::open_bytes(bytes, "bookmarks.pdf", "").unwrap();
    let items: Vec<(usize, String, usize)> = doc.outline().iter().map(|o| (o.level, o.title.clone(), o.page)).collect();
    assert_eq!(items, vec![(1, "第一章".to_string(), 0), (2, "1.1 小節".to_string(), 1), (1, "Chapter 2".to_string(), 2)]);

    // 沒有書籤的檔案：空清單
    let plain = Document::open_file(&sample(), "").unwrap();
    assert!(plain.outline().is_empty());
}

/// 只指定字型名稱、未嵌入字型的中文 PDF（台灣薪資、報表系統常見）：必須畫出字，且可擷取文字
#[test]
fn non_embedded_cjk_font_is_drawn_with_system_font() {
    use lopdf::{Object, Stream, dictionary};
    let mut doc = lopdf::Document::with_version("1.4");
    let pages_id = doc.new_object_id();
    let cid_font = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "CIDFontType0",
        "BaseFont" => "MSung-Light",
        "CIDSystemInfo" => dictionary! { "Registry" => Object::string_literal("Adobe"), "Ordering" => Object::string_literal("CNS1"), "Supplement" => 0 },
        "FontDescriptor" => dictionary! { "Type" => "FontDescriptor", "FontName" => "MSung-Light", "Flags" => 6, "FontBBox" => vec![0.into(), (-200).into(), 1000.into(), 900.into()], "ItalicAngle" => 0, "Ascent" => 880, "Descent" => -120, "CapHeight" => 880, "StemV" => 93 },
        "DW" => 1000,
    });
    let font = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type0",
        "BaseFont" => "MSung-Light-UniCNS-UCS2-H",
        "Encoding" => "UniCNS-UCS2-H",
        "DescendantFonts" => vec![Object::Reference(cid_font)],
    });
    // 「員工編號」的 UCS-2 編碼
    let text: Vec<u8> = "員工編號".encode_utf16().flat_map(|u| u.to_be_bytes()).collect();
    let hex: String = text.iter().map(|b| format!("{b:02X}")).collect();
    let content = doc.add_object(Stream::new(dictionary! {}, format!("BT /F1 36 Tf 50 700 Td <{hex}> Tj ET").into_bytes()));
    let page = doc.add_object(dictionary! {
        "Type" => "Page", "Parent" => pages_id, "MediaBox" => vec![0.into(), 0.into(), 400.into(), 800.into()],
        "Contents" => content, "Resources" => dictionary! { "Font" => dictionary! { "F1" => font } },
    });
    doc.objects.insert(pages_id, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1 }));
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog);
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();

    let d = Document::open_bytes(bytes, "msung.pdf", "").unwrap();
    let img = d.render(0, 1.0).unwrap();
    // 文字所在的區域（上方 60–120 pt）要有足夠的深色像素
    let w = img.width as usize;
    let dark = (60..120usize)
        .flat_map(|y| (40..220usize).map(move |x| (x, y)))
        .filter(|&(x, y)| img.rgba[(y * w + x) * 4] < 128)
        .count();
    assert!(dark > 400, "未嵌入的中文字應以系統字型畫出（深色像素 {dark}）");
    assert!(d.page_text(0).plain_text().contains("員工編號"), "文字擷取：{}", d.page_text(0).plain_text());
}
