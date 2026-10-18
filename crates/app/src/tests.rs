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
