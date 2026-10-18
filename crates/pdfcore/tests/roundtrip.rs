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

/// OCR 文字層：存檔後可擷取文字、位置正確，且頁面外觀完全不變（文字不可見）
#[test]
fn ocr_layer_is_invisible_searchable_text() {
    use pdfcore::{Mark, MarkType, OcrGlyph, OcrTextLine, Pt, Rect};
    let doc = Document::open_file(&sample(), "").unwrap();
    // 在頁面空白處（y 500–520）放一行 OCR 結果：每個字 20×20 pt
    let text = "掃描文字OCR測試";
    let glyphs: Vec<OcrGlyph> = text
        .chars()
        .enumerate()
        .map(|(i, c)| OcrGlyph { text: c.to_string(), quad: Rect::new(100.0 + i as f64 * 20.0, 500.0, 120.0 + i as f64 * 20.0, 520.0).corners() })
        .collect();
    let m = Mark { kind: MarkType::Ocr, page: 0, ocr: vec![OcrTextLine { text: text.into(), confidence: 0.99, glyphs }], ..Default::default() };

    // 存檔前：OCR 結果可選取
    let pt = pdfcore::document::ocr_page_text(&m.ocr, Pt::default(), (612.0, 792.0));
    let (quads, sel) = pdfcore::document::select_in(&pt, Pt::new(101.0, 510.0), Pt::new(279.0, 510.0));
    assert_eq!(sel, text);
    assert_eq!(quads.len(), 1);

    let bytes = pdfcore::save::save_to_bytes(&doc, std::slice::from_ref(&m), &pdfcore::SaveOptions::default()).unwrap();
    let plain = pdfcore::save::save_to_bytes(&doc, &[], &pdfcore::SaveOptions::default()).unwrap();
    let saved = Document::open_bytes(bytes, "ocr.pdf", "").unwrap();
    let base = Document::open_bytes(plain, "plain.pdf", "").unwrap();

    // 頁面外觀與沒有 OCR 層時完全相同
    let (a, b) = (saved.render(0, 1.0).unwrap(), base.render(0, 1.0).unwrap());
    assert_eq!(a.rgba, b.rgba, "OCR 文字層不應改變頁面外觀");

    // 可擷取文字，且選取位置與原本的字相符
    let t = saved.page_text(0);
    assert!(t.plain_text().contains(text), "擷取到：{}", t.plain_text());
    let (quads, sel) = saved.select_text(0, Pt::new(101.0, 510.0), Pt::new(279.0, 510.0));
    assert_eq!(sel.replace(' ', ""), text);
    let r = pdfcore::geom::quad_bounds(&quads[0]);
    // 9 個字 × 20 pt：100–280
    assert!((r.x0 - 100.0).abs() < 0.5 && (r.x1 - 280.0).abs() < 0.5, "{r:?}");
    assert!((r.y0 - 500.0).abs() < 0.5 && (r.y1 - 520.0).abs() < 0.5, "{r:?}");
    std::fs::write(format!("{}/../../target/ocr-layer.pdf", env!("CARGO_MANIFEST_DIR")), saved.bytes().as_slice()).unwrap();
}

/// 直書的 OCR 結果：一欄是一行（複製時沒有每字換行、不混入隔壁欄），
/// 選取的四邊形沿著書寫方向，底線畫在整欄的一側而不是最後一個字底下
#[test]
fn vertical_ocr_column_selects_as_one_line_along_the_column() {
    use pdfcore::{OcrGlyph, OcrTextLine, Pt};
    // 由上往下書寫：四邊形 ul, ur, lr, ll 依文字方向（ul→ur 朝下，「上方」在右側）
    let column = |text: &str, x: f64| {
        let glyphs = text
            .chars()
            .enumerate()
            .map(|(i, c)| {
                let y = 100.0 + i as f64 * 20.0;
                OcrGlyph { text: c.to_string(), quad: [Pt::new(x + 20.0, y), Pt::new(x + 20.0, y + 20.0), Pt::new(x, y + 20.0), Pt::new(x, y)] }
            })
            .collect();
        OcrTextLine { text: text.into(), confidence: 0.99, glyphs }
    };
    let lines = vec![column("豐鈺北屯太原段", 100.0), column("店舖住宅新建工程", 75.0)];
    let pt = pdfcore::document::ocr_page_text(&lines, Pt::default(), (612.0, 792.0));
    // 第一欄由第一個字拖到最後一個字
    let (quads, sel) = pdfcore::document::select_in(&pt, Pt::new(110.0, 110.0), Pt::new(110.0, 230.0));
    assert_eq!(sel, "豐鈺北屯太原段");
    assert_eq!(quads.len(), 1, "一欄一個四邊形");
    let q = quads[0];
    // 底邊（ll → lr）沿著整欄（x = 100 那一側，長 140）
    assert!((q[3].x - 100.0).abs() < 0.5 && (q[2].x - 100.0).abs() < 0.5, "{q:?}");
    assert!(((q[2].y - q[3].y).abs() - 140.0).abs() < 0.5, "{q:?}");
    // 整頁：兩欄各一行，由右而左
    assert_eq!(pt.plain_text(), "豐鈺北屯太原段\n店舖住宅新建工程");
}
