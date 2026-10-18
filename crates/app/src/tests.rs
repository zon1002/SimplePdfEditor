// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Fufeng Project

//! 不開視窗驅動整個主視窗的自動測試（egui_kittest）：以按鈕名稱點擊，與使用者操作相同。

use crate::app::{App, Tool};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use pdfcore::{Mark, MarkType, Pt, Rgba};

fn sample() -> String {
    format!("{}/../../test/sample.pdf", env!("CARGO_MANIFEST_DIR"))
}

fn harness(app: App) -> Harness<'static, App> {
    Harness::builder().with_size([1280.0, 900.0]).build_ui_state(|ui, app: &mut App| app.frame_ui(ui), app)
}

fn opened() -> App {
    let mut app = App::new(0);
    app.open_path(&sample());
    assert!(app.doc.is_some(), "無法開啟範例檔");
    app
}

fn line_mark() -> Mark {
    Mark { kind: MarkType::Line, p1: Pt::new(72.0, 400.0), p2: Pt::new(300.0, 400.0), color: Rgba::rgb(255, 0, 0), ..Default::default() }
}

#[test]
fn opens_in_view_mode_and_blocks_editing() {
    let mut h = harness(opened());
    h.run();
    let app = h.state_mut();
    assert!(!app.editing, "開啟後應為檢視模式");
    // 檢視模式：切換工具、加入註記的動作都無效
    let before = app.tool;
    app.set_tool(Tool::Line);
    assert_eq!(app.tool, before);
    app.model.marks.push(line_mark());
    app.selection = vec![0];
    app.delete_selected();
    assert_eq!(app.model.marks.len(), 1, "檢視模式不可刪除");
    assert!(!app.save_to_path("unused.pdf"), "檢視模式不可存檔");
}

#[test]
fn edit_then_cancel_restores_original() {
    let mut h = harness(opened());
    h.run();
    h.get_by_label("編輯").click();
    h.run();
    assert!(h.state().editing, "按「編輯」後應進入編輯模式");
    assert!(h.state().title.contains("（編輯中）"));

    // 編輯：加入註記（與拖曳畫線相同的動作）
    h.state_mut().set_tool(Tool::Line);
    let m = h.state().new_line_mark(Pt::new(72.0, 400.0), Pt::new(300.0, 400.0));
    h.state_mut().add_mark(m);
    h.run();
    assert!(h.state().model.dirty);

    // 取消編輯：有修改時先確認，選「放棄編輯」後回到原始狀態
    h.get_by_label("取消編輯").click();
    h.run();
    assert!(h.state().editing, "確認前不應放棄");
    h.get_by_label("放棄編輯").click();
    h.run();
    let app = h.state();
    assert!(!app.editing);
    assert!(app.model.marks.is_empty() && !app.model.dirty && app.model.undo_stack.is_empty());
}

#[test]
fn cancel_without_changes_needs_no_confirmation() {
    let mut h = harness(opened());
    h.run();
    h.get_by_label("編輯").click();
    h.run();
    h.get_by_label("取消編輯").click();
    h.run();
    assert!(!h.state().editing);
    assert!(h.state().dialog.is_none());
}

#[test]
fn save_returns_to_view_mode_with_saved_file() {
    let out = format!("{}/../../target/edit-mode-test", env!("CARGO_MANIFEST_DIR"));
    std::fs::create_dir_all(&out).unwrap();
    let out = format!("{out}/saved.pdf");
    let original = std::fs::read(sample()).unwrap();

    let mut h = harness(opened());
    h.run();
    h.get_by_label("編輯").click();
    h.run();
    h.state_mut().add_mark(line_mark());
    assert!(h.state_mut().save_to_path(&out));
    h.run();
    let app = h.state();
    assert!(!app.editing, "存檔後應回到檢視模式");
    assert_eq!(app.doc.as_ref().unwrap().path(), out, "應改為開啟存好的檔案");
    assert!(app.model.marks.is_empty() && !app.model.dirty);
    // 原始檔案不變
    assert_eq!(std::fs::read(sample()).unwrap(), original);
}

// ---------------------------------------------------------------- 矩形、圓角矩形、圓形框

/// 以真正的滑鼠事件在頁面上拖曳（頁面座標 pt）
fn drag(h: &mut Harness<'static, App>, a: Pt, b: Pt, shift: bool) {
    let to_screen = |app: &App, p: Pt| egui::pos2(app.page_origin.x + (p.x * app.zoom) as f32, app.page_origin.y + (p.y * app.zoom) as f32);
    let mods = if shift { egui::Modifiers::SHIFT } else { egui::Modifiers::NONE };
    let (sa, sb) = (to_screen(h.state(), a), to_screen(h.state(), b));
    // 拖曳期間按住 Shift（與使用者按住按鍵相同）
    h.event(egui::Event::ModifiersChanged(mods));
    h.event(egui::Event::PointerMoved(sa));
    h.run();
    h.event(egui::Event::PointerButton { pos: sa, button: egui::PointerButton::Primary, pressed: true, modifiers: mods });
    h.run();
    for i in 1..=5 {
        let p = sa + (sb - sa) * (i as f32 / 5.0);
        h.event(egui::Event::PointerMoved(p));
        h.run();
    }
    h.event(egui::Event::PointerButton { pos: sb, button: egui::PointerButton::Primary, pressed: false, modifiers: mods });
    h.run();
    h.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
    h.run();
}

fn editing_harness() -> Harness<'static, App> {
    let mut h = harness(opened());
    h.run();
    h.get_by_label("編輯").click();
    h.run();
    h
}

#[test]
fn draws_rect_round_rect_and_circle_by_dragging() {
    let mut h = editing_harness();

    // 矩形：工具列按鈕
    h.get_by_label("矩形").click();
    h.run();
    assert_eq!(h.state().tool, Tool::Rect);
    drag(&mut h, Pt::new(100.0, 200.0), Pt::new(250.0, 300.0), false);

    // 圓角矩形：快捷鍵 Q，半徑取工具列設定
    h.state_mut().props.corner_radius = 20.0;
    h.key_press(egui::Key::Q);
    h.run();
    assert_eq!(h.state().tool, Tool::RoundRect);
    drag(&mut h, Pt::new(300.0, 200.0), Pt::new(450.0, 300.0), false);

    // 圓形：Shift 拖曳成正圓
    h.get_by_label("圓形").click();
    h.run();
    assert_eq!(h.state().tool, Tool::Ellipse);
    drag(&mut h, Pt::new(470.0, 200.0), Pt::new(550.0, 250.0), true);

    let marks = &h.state().model.marks;
    assert_eq!(marks.len(), 3, "應新增三個框：{:?}", marks.iter().map(|m| (m.kind, m.radius, pdfcore::marks::shape_rect(m))).collect::<Vec<_>>());
    let r0 = pdfcore::marks::shape_rect(&marks[0]);
    assert_eq!(marks[0].kind, MarkType::Rect);
    assert!((r0.width() - 150.0).abs() < 1.0 && (r0.height() - 100.0).abs() < 1.0, "矩形大小依拖曳：{r0:?}");
    assert_eq!(marks[0].radius, 0.0);
    assert_eq!(marks[1].kind, MarkType::Rect);
    assert_eq!(marks[1].radius, 20.0);
    let r2 = pdfcore::marks::shape_rect(&marks[2]);
    assert_eq!(marks[2].kind, MarkType::Ellipse);
    assert!((r2.width() - r2.height()).abs() < 0.5 && (r2.width() - 80.0).abs() < 1.0, "Shift 應為正圓：{r2:?}");
}

#[test]
fn select_hollow_shape_by_its_outline_and_change_radius() {
    let mut h = editing_harness();
    h.state_mut().set_tool(Tool::RoundRect);
    drag(&mut h, Pt::new(100.0, 200.0), Pt::new(300.0, 350.0), false);
    assert_eq!(h.state().model.marks.len(), 1);

    h.key_press(egui::Key::V);
    h.run();
    // 點框的中間（鏤空）不會選到
    drag(&mut h, Pt::new(200.0, 275.0), Pt::new(200.0, 275.0), false);
    assert!(h.state().selection.is_empty(), "框中間是空的，不應選到");
    // 點外框線可選取，屬性工具列出現「圓角」
    drag(&mut h, Pt::new(200.0, 200.0), Pt::new(200.0, 200.0), false);
    assert_eq!(h.state().selection, vec![0]);
    assert!(h.state().visible_bars().radius);
    h.state_mut().apply_to_selection("radius", |m| {
        m.radius = 40.0;
        true
    });
    assert_eq!(h.state().model.marks[0].radius, 40.0);

    // 存檔：寫成 Square 註解，可重新開啟
    let out = format!("{}/../../target/edit-mode-test/shapes.pdf", env!("CARGO_MANIFEST_DIR"));
    std::fs::create_dir_all(std::path::Path::new(&out).parent().unwrap()).unwrap();
    assert!(h.state_mut().save_to_path(&out));
    let lo = lopdf::Document::load(&out).unwrap();
    let page = *lo.get_pages().get(&1).unwrap();
    let annots = lo.get_dictionary(page).unwrap().get(b"Annots").unwrap().as_array().unwrap().clone();
    let subtypes: Vec<String> = annots
        .iter()
        .filter_map(|a| lo.get_dictionary(a.as_reference().ok()?).ok())
        .filter_map(|d| d.get(b"Subtype").ok()?.as_name().ok().map(|n| String::from_utf8_lossy(n).into_owned()))
        .collect();
    assert!(subtypes.iter().any(|s| s == "Square"), "{subtypes:?}");
}

// ---------------------------------------------------------------- 控制點調整框的大小

/// 畫一個框並以選取工具選取它
fn selected_shape(tool: Tool, a: Pt, b: Pt) -> Harness<'static, App> {
    let mut h = editing_harness();
    h.state_mut().set_tool(tool);
    drag(&mut h, a, b, false);
    h.key_press(egui::Key::V);
    h.run();
    // 點上邊線選取
    let top = Pt::new((a.x + b.x) / 2.0 + 10.0, a.y);
    drag(&mut h, top, top, false);
    assert_eq!(h.state().selection, vec![0], "應選取剛畫的框");
    h
}

fn rect_of(h: &Harness<'static, App>) -> pdfcore::Rect {
    pdfcore::marks::shape_rect(&h.state().model.marks[0])
}

#[test]
fn corner_handle_resizes_both_directions_and_undo_restores() {
    let mut h = selected_shape(Tool::Rect, Pt::new(100.0, 200.0), Pt::new(300.0, 300.0));
    // 右下角控制點往右下拖
    drag(&mut h, Pt::new(300.0, 300.0), Pt::new(380.0, 340.0), false);
    let r = rect_of(&h);
    assert!((r.x0 - 100.0).abs() < 0.5 && (r.y0 - 200.0).abs() < 0.5, "對角應固定：{r:?}");
    assert!((r.x1 - 380.0).abs() < 0.5 && (r.y1 - 340.0).abs() < 0.5, "右下角應跟著拖曳：{r:?}");
    assert_eq!(h.state().selection, vec![0], "調整大小後仍保持選取");
    // 一次拖曳 = 一次復原
    h.state_mut().undo();
    let r = rect_of(&h);
    assert!((r.x1 - 300.0).abs() < 0.5 && (r.y1 - 300.0).abs() < 0.5, "復原後回到原大小：{r:?}");
}

#[test]
fn edge_handle_moves_only_that_side() {
    let mut h = selected_shape(Tool::RoundRect, Pt::new(100.0, 200.0), Pt::new(300.0, 300.0));
    // 左邊中點往左拖；上下不變
    drag(&mut h, Pt::new(100.0, 250.0), Pt::new(60.0, 280.0), false);
    let r = rect_of(&h);
    assert!((r.x0 - 60.0).abs() < 0.5 && (r.x1 - 300.0).abs() < 0.5, "{r:?}");
    assert!((r.y0 - 200.0).abs() < 0.5 && (r.y1 - 300.0).abs() < 0.5, "邊的控制點不應改變高度：{r:?}");
    assert!(h.state().model.marks[0].radius > 0.0, "圓角保留");
}

#[test]
fn shift_corner_keeps_circle_round() {
    let mut h = selected_shape(Tool::Ellipse, Pt::new(100.0, 200.0), Pt::new(200.0, 300.0));
    // 按住 Shift 拖右下角：維持正圓
    drag(&mut h, Pt::new(200.0, 300.0), Pt::new(260.0, 320.0), true);
    let r = rect_of(&h);
    assert!((r.width() - r.height()).abs() < 0.5, "Shift 應維持正圓：{r:?}");
    assert!((r.x0 - 100.0).abs() < 0.5 && (r.y0 - 200.0).abs() < 0.5, "對角固定：{r:?}");
    assert!((r.width() - 160.0).abs() < 0.5, "以較長的一邊為準：{r:?}");
}

#[test]
fn dragging_past_the_opposite_side_flips_instead_of_vanishing() {
    let mut h = selected_shape(Tool::Rect, Pt::new(100.0, 200.0), Pt::new(300.0, 300.0));
    // 右邊中點拖到左邊外面
    drag(&mut h, Pt::new(300.0, 250.0), Pt::new(50.0, 250.0), false);
    let r = rect_of(&h);
    assert!((r.x0 - 50.0).abs() < 0.5 && (r.x1 - 100.0).abs() < 0.5, "{r:?}");
    assert!(r.width() >= 2.0 && r.height() >= 2.0);
}

/// 輸出畫面截圖供人工檢查（需要顯示卡；`cargo test -- --ignored screenshot`）
#[test]
#[ignore]
fn screenshot_resize_handles() {
    let mut h = selected_shape(Tool::RoundRect, Pt::new(100.0, 180.0), Pt::new(330.0, 300.0));
    crate::setup_fonts(&h.ctx);
    h.run();
    let img = h.render().expect("render");
    let out = format!("{}/../../target/ui-shot.png", env!("CARGO_MANIFEST_DIR"));
    img.save(&out).unwrap();
}

// ---------------------------------------------------------------- 左側欄：頁面縮圖與書籤

/// 範例檔加上兩層書籤
fn bookmarked_pdf() -> String {
    let out = format!("{}/../../target/edit-mode-test", env!("CARGO_MANIFEST_DIR"));
    std::fs::create_dir_all(&out).unwrap();
    let out = format!("{out}/bookmarks.pdf");
    let mut lo = lopdf::Document::load(sample()).unwrap();
    let pages: Vec<lopdf::ObjectId> = lo.get_pages().values().copied().collect();
    let ch1 = lo.add_bookmark(lopdf::Bookmark::new("第一章".into(), [0.0; 3], 0, pages[0]), None);
    lo.add_bookmark(lopdf::Bookmark::new("1.1 小節".into(), [0.0; 3], 0, pages[1]), Some(ch1));
    lo.add_bookmark(lopdf::Bookmark::new("第二章".into(), [0.0; 3], 0, pages[2]), None);
    let outline = lo.build_outline().unwrap();
    lo.catalog_mut().unwrap().set("Outlines", lopdf::Object::Reference(outline));
    lo.save(&out).unwrap();
    out
}

fn sidebar_harness(path: &str, tab: crate::sidebar::SidebarTab) -> Harness<'static, App> {
    let mut app = App::new(0);
    app.settings.sidebar = true;
    app.settings.sidebar_tab = tab;
    app.open_path(path);
    let mut h = harness(app);
    h.run();
    h
}

#[test]
fn clicking_a_thumbnail_goes_to_that_page() {
    let mut h = sidebar_harness(&sample(), crate::sidebar::SidebarTab::Pages);
    // 每頁都有縮圖，可直接點選
    for p in 1..=3 {
        h.get_by_label(&format!("第 {p} 頁縮圖"));
    }
    h.get_by_label("第 3 頁縮圖").click();
    h.run();
    assert_eq!(h.state().page, 2);
    h.get_by_label("第 1 頁縮圖").click();
    h.run();
    assert_eq!(h.state().page, 0);
    // 縮圖在背景逐張產生
    h.run_steps(5);
    assert_eq!(h.state().thumbs.len(), 3, "應產生 3 張縮圖");
}

#[test]
fn f4_toggles_the_sidebar() {
    let mut h = sidebar_harness(&sample(), crate::sidebar::SidebarTab::Pages);
    assert!(h.query_by_label("第 1 頁縮圖").is_some());
    h.key_press(egui::Key::F4);
    h.run();
    assert!(!h.state().settings.sidebar);
    assert!(h.query_by_label("第 1 頁縮圖").is_none(), "隱藏後不顯示縮圖");
    h.key_press(egui::Key::F4);
    h.run();
    assert!(h.state().settings.sidebar);
}

#[test]
fn bookmarks_navigate_and_collapse() {
    let path = bookmarked_pdf();
    let mut h = sidebar_harness(&path, crate::sidebar::SidebarTab::Pages);
    // 切到「書籤」分頁
    h.get_by_label("書籤").click();
    h.run();
    h.get_by_label("第二章").click();
    h.run();
    assert_eq!(h.state().page, 2);
    h.get_by_label("1.1 小節").click();
    h.run();
    assert_eq!(h.state().page, 1);
    // 收合「第一章」：子項目隱藏
    h.get_by_label("收合").click();
    h.run();
    assert!(h.query_by_label("1.1 小節").is_none(), "收合後子書籤隱藏");
    h.get_by_label("展開").click();
    h.run();
    assert!(h.query_by_label("1.1 小節").is_some());
}

#[test]
fn file_without_bookmarks_says_so() {
    let mut h = sidebar_harness(&sample(), crate::sidebar::SidebarTab::Bookmarks);
    h.run();
    assert!(h.query_by_label("此 PDF 沒有書籤").is_some());
}

/// 輸出側欄截圖供人工檢查（需要顯示卡；`cargo test -- --ignored screenshot`）
#[test]
#[ignore]
fn screenshot_sidebar() {
    for (tab, name) in [(crate::sidebar::SidebarTab::Pages, "pages"), (crate::sidebar::SidebarTab::Bookmarks, "bookmarks")] {
        let mut h = sidebar_harness(&bookmarked_pdf(), tab);
        crate::setup_fonts(&h.ctx);
        h.run_steps(6);
        h.state_mut().go_to_page(1);
        h.run_steps(3);
        let img = h.render().expect("render");
        img.save(format!("{}/../../target/ui-sidebar-{name}.png", env!("CARGO_MANIFEST_DIR"))).unwrap();
    }
}

// ---------------------------------------------------------------- 文字辨識（OCR）

/// 開發時由 `ocr-packs build` 產生的語言包；沒有時略過 OCR 測試
fn ocr_packs() -> Option<String> {
    let d = format!("{}/../../target/ocr-packs", env!("CARGO_MANIFEST_DIR"));
    if !std::path::Path::new(&d).join("std/pack.json").exists() {
        eprintln!("略過：找不到 target/ocr-packs");
        return None;
    }
    // SAFETY: 只在讀取語言包之前設定
    unsafe { std::env::set_var("SPE_OCR_PACKS", &d) };
    Some(d)
}

/// 模擬掃描檔：範例檔第 1 頁轉成 200 dpi 的圖片，沒有文字層
/// `footer`：再加一行真正的文字（例如 pdfFactory 試用版在每頁底部加的字樣）
fn scanned_pdf_with(name: &str, footer: Option<&str>) -> String {
    use lopdf::{Object, Stream, dictionary};
    let out = format!("{}/../../target/edit-mode-test", env!("CARGO_MANIFEST_DIR"));
    std::fs::create_dir_all(&out).unwrap();
    let out = format!("{out}/{name}.pdf");
    let src = pdfcore::Document::open_file(&sample(), "").unwrap();
    let (w, h) = src.page_size(0);
    let img = src.render(0, 200.0 / 72.0).unwrap();
    let rgb: Vec<u8> = img.rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
    let mut doc = lopdf::Document::with_version("1.7");
    let mut st = Stream::new(
        dictionary! { "Type" => "XObject", "Subtype" => "Image", "Width" => img.width as i64, "Height" => img.height as i64, "ColorSpace" => "DeviceRGB", "BitsPerComponent" => 8 },
        rgb,
    );
    st.compress().unwrap();
    let im = doc.add_object(st);
    let mut ops = format!("q {w} 0 0 {h} 0 0 cm /Im0 Do Q");
    if let Some(t) = footer {
        ops += &format!(" BT /F1 9 Tf 40 12 Td ({t}) Tj ET");
    }
    let content = doc.add_object(Stream::new(dictionary! {}, ops.into_bytes()));
    let font = doc.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica" });
    let pages = doc.new_object_id();
    let page = doc.add_object(dictionary! {
        "Type" => "Page", "Parent" => pages, "MediaBox" => vec![0.into(), 0.into(), Object::Real(w as f32), Object::Real(h as f32)],
        "Contents" => content, "Resources" => dictionary! { "XObject" => dictionary! { "Im0" => im }, "Font" => dictionary! { "F1" => font } },
    });
    doc.objects.insert(pages, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1 }));
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
    doc.trailer.set("Root", catalog);
    doc.save(&out).unwrap();
    out
}

fn scanned_pdf(name: &str) -> String {
    scanned_pdf_with(name, None)
}

fn wait_ocr(h: &mut Harness<'static, App>) {
    let t = std::time::Instant::now();
    while h.state().ocr_job.is_some() {
        assert!(t.elapsed().as_secs() < 120, "辨識逾時");
        std::thread::sleep(std::time::Duration::from_millis(100));
        h.step();
    }
    h.run();
}

#[test]
fn ocr_scanned_page_then_select_and_save_searchable_pdf() {
    if ocr_packs().is_none() {
        return;
    }
    let scan = scanned_pdf("scanned-save");
    let mut app = App::new(0);
    app.open_path(&scan);
    assert!(app.doc.as_ref().unwrap().page_text(0).is_empty(), "掃描檔不應有文字層");
    let mut h = harness(app);
    h.run();
    h.get_by_label("編輯").click();
    h.run();
    h.state_mut().open_ocr_dialog();
    h.run();
    // 對話框列出精度與說明
    h.get_by_label_contains("標準（建議）");
    h.get_by_label("開始辨識").click();
    h.step();
    assert!(h.state().ocr_job.is_some(), "應開始背景辨識");
    wait_ocr(&mut h);

    let app = h.state();
    let ocr: Vec<_> = app.model.marks.iter().filter(|m| m.kind == MarkType::Ocr).collect();
    assert_eq!(ocr.len(), 1, "一頁一個 OCR 結果");
    let all: String = ocr[0].ocr.iter().map(|l| l.text.as_str()).collect::<Vec<_>>().join("\n");
    assert!(all.contains("quick brown fox"), "{all}");
    assert!(all.contains("中文測試"), "{all}");
    assert!(app.model.dirty);

    // 存檔前就能以 OCR 結果選取文字（拖過整頁）
    let doc = app.doc.as_ref().unwrap();
    let (w, hh) = doc.page_size(0);
    let (quads, text) = crate::ocr_ui::select_on_page(doc, &app.model.marks, 0, Pt::new(0.0, 0.0), Pt::new(w, hh));
    assert!(!quads.is_empty() && text.contains("quick brown fox"), "{text}");

    // 另存後：外觀不變、有可搜尋的文字層
    let out = format!("{}/../../target/edit-mode-test/ocr-saved.pdf", env!("CARGO_MANIFEST_DIR"));
    let before = doc.render(0, 1.0).unwrap();
    assert!(h.state_mut().save_to_path(&out));
    let saved = pdfcore::Document::open_file(&out, "").unwrap();
    let t = saved.page_text(0).plain_text();
    assert!(t.contains("quick brown fox") && t.contains("中文測試"), "{t}");
    assert!(!saved.page_text(0).find("lazy dog").is_empty());
    let after = saved.render(0, 1.0).unwrap();
    assert_eq!(before.rgba, after.rgba, "文字層必須看不見");
}

#[test]
fn ocr_is_undone_in_one_step_and_rerun_replaces_page_result() {
    if ocr_packs().is_none() {
        return;
    }
    let mut app = App::new(0);
    app.open_path(&scanned_pdf("scanned-undo"));
    let mut h = harness(app);
    h.run();
    h.get_by_label("編輯").click();
    h.run();
    for _ in 0..2 {
        h.state_mut().open_ocr_dialog();
        h.run();
        assert!(h.state().ocr_dialog.is_some(), "dialog {:?} job {}", h.state().status, h.state().ocr_job.is_some());
        h.get_by_label("開始辨識").click();
        h.step();
        wait_ocr(&mut h);
    }
    let n = |h: &Harness<'static, App>| h.state().model.marks.iter().filter(|m| m.kind == MarkType::Ocr).count();
    assert_eq!(n(&h), 1, "重新辨識應取代同一頁的結果");
    h.state_mut().undo();
    assert_eq!(n(&h), 1, "復原一次回到第一次辨識的結果");
    h.state_mut().undo();
    assert_eq!(n(&h), 0);
}

#[test]
fn skip_pages_that_only_have_existing_text() {
    if ocr_packs().is_none() {
        return;
    }
    let mut h = harness(opened());
    h.run();
    h.get_by_label("編輯").click();
    h.run();
    h.state_mut().open_ocr_dialog();
    h.run();
    h.get_by_label("開始辨識").click();
    h.step();
    wait_ocr(&mut h);
    assert!(h.state().model.marks.is_empty(), "只有原有文字的頁面預設略過");
    assert!(!h.state().model.dirty);
}

#[test]
fn ocr_needs_edit_mode_and_pack_manager_lists_packs() {
    if ocr_packs().is_none() {
        return;
    }
    let mut h = harness(opened());
    h.run();
    h.state_mut().open_ocr_dialog();
    h.run();
    assert!(h.state().ocr_dialog.is_none(), "檢視模式不能辨識");
    h.state_mut().open_pack_manager();
    h.run_steps(4);
    h.get_by_label("OCR 語言管理");
    h.get_by_label_contains("releases/tag/ocr-packs");
    h.get_by_label("從檔案加入…");
    h.get_by_label_contains("日文　高精度");
    h.get_by_label("韓文");
}

#[test]
#[ignore]
fn screenshot_ocr() {
    if ocr_packs().is_none() {
        return;
    }
    let shot = |h: &mut Harness<'static, App>, name: &str| {
        let img = h.render().expect("render");
        img.save(format!("{}/../../target/ui-ocr-{name}.png", env!("CARGO_MANIFEST_DIR"))).unwrap();
    };
    let mut app = App::new(0);
    app.open_path(&scanned_pdf("scanned-shot"));
    let mut h = harness(app);
    crate::setup_fonts(&h.ctx);
    h.run();
    h.get_by_label("編輯").click();
    h.run();
    h.state_mut().open_ocr_dialog();
    h.run_steps(4);
    shot(&mut h, "dialog");
    h.get_by_label("開始辨識").click();
    h.run_steps(3);
    shot(&mut h, "progress");
    wait_ocr(&mut h);
    // 以高亮工具沿著辨識出的文字拖曳
    h.state_mut().set_tool(Tool::Highlight);
    drag(&mut h, Pt::new(60.0, 50.0), Pt::new(400.0, 130.0), false);
    h.run_steps(3);
    shot(&mut h, "selected");
    h.state_mut().open_pack_manager();
    h.run_steps(4);
    shot(&mut h, "packs");
}

#[test]
#[ignore]
fn screenshot_pack_download() {
    // 沒有任何語言包（與剛安裝、未勾選 OCR 相同）：從下載網站實際下載
    // SAFETY: 只在讀取語言包之前設定
    unsafe { std::env::set_var("SPE_OCR_PACKS", format!("{}/../../target/no-packs", env!("CARGO_MANIFEST_DIR"))) };
    let home = format!("{}/../../target/download-test", env!("CARGO_MANIFEST_DIR"));
    let _ = std::fs::remove_dir_all(&home);
    // SAFETY: 同上；下載的語言包裝到 target 下，不動到使用者資料夾
    unsafe { std::env::set_var("LOCALAPPDATA", &home) };
    let mut h = harness(opened());
    crate::setup_fonts(&h.ctx);
    h.run();
    h.state_mut().open_pack_manager();
    h.run_steps(4);
    h.get_by_label_contains("下載（7.9 MB）").click();
    h.step();
    h.step();
    let t = std::time::Instant::now();
    while h.state().packs_ui.downloading() && t.elapsed().as_secs() < 300 {
        std::thread::sleep(std::time::Duration::from_millis(200));
        h.step();
    }
    h.run_steps(4);
    let img = h.render().expect("render");
    img.save(format!("{}/../../target/ui-ocr-download.png", env!("CARGO_MANIFEST_DIR"))).unwrap();
    assert!(ocr::packs::find_installed("ko").is_some() && ocr::packs::find_installed("engine").is_some(), "應已下載並安裝韓文與辨識引擎");
}

/// 回歸：掃描檔（或 CAD 圖面）帶一行真正的頁尾文字時，仍要辨識其他內容，且頁尾不重複
#[test]
fn page_with_footer_text_is_still_recognized_without_duplicating_it() {
    if ocr_packs().is_none() {
        return;
    }
    let footer = "PDF created with pdfFactory Pro trial version www.pdffactory.com";
    let mut app = App::new(0);
    app.open_path(&scanned_pdf_with("scanned-footer", Some(footer)));
    assert!(app.doc.as_ref().unwrap().page_text(0).plain_text().contains("pdfFactory"));
    let mut h = harness(app);
    h.run();
    h.get_by_label("編輯").click();
    h.run();
    h.state_mut().open_ocr_dialog();
    h.run();
    h.get_by_label("開始辨識").click();
    h.step();
    wait_ocr(&mut h);
    let all: String = h.state().model.marks.iter().filter(|m| m.kind == MarkType::Ocr).flat_map(|m| m.ocr.iter()).map(|l| l.text.clone()).collect::<Vec<_>>().join("
");
    assert!(all.contains("quick brown fox") && all.contains("中文測試"), "{all}");
    assert!(!all.to_lowercase().contains("pdffactory"), "原有的頁尾文字不應重複辨識：{all}");
}
