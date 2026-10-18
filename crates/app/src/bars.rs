// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Fufeng Project

//! 選單、主工具列、屬性工具列、狀態列與快捷鍵。

use crate::app::{App, Tool};
use crate::dialogs::Pending;
use crate::icons::{IconId, IconStyle, render_icon};
use crate::ruler::RulerUnit;
use crate::settings::ButtonMode;
use egui::{Color32, Key, KeyboardShortcut, Modifiers, Response, RichText, Sense, Ui, vec2};
use pdfcore::{MarkType, Rgba};

const ICON: f32 = 22.0;

/// 色票
const PRESETS: [Rgba; 9] = [
    Rgba::rgb(0xFF, 0xEB, 0x3B),
    Rgba::rgb(0x8B, 0xE8, 0x7A),
    Rgba::rgb(0x7F, 0xDB, 0xFF),
    Rgba::rgb(0xFF, 0x9E, 0xCB),
    Rgba::rgb(0xFF, 0xB7, 0x4D),
    Rgba::rgb(0xE5, 0x39, 0x35),
    Rgba::rgb(0x1E, 0x63, 0xD6),
    Rgba::rgb(0x2E, 0x7D, 0x32),
    Rgba::rgb(0x1A, 0x1A, 0x1A),
];

fn to32(c: Rgba) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
}

fn from32(c: Color32) -> Rgba {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    Rgba::rgba(r, g, b, a)
}

/// 工具列上的一個動作
#[derive(Clone, Copy, PartialEq)]
enum Act {
    Open,
    Edit,
    Save,
    Export,
    Print,
    Preview,
    Quit,
    Undo,
    Redo,
    Delete,
    Copy,
    Prev,
    Next,
    ZoomIn,
    ZoomOut,
    Fit,
    Rulers,
    Sidebar,
    SidebarTab(crate::sidebar::SidebarTab),
    Help,
    About,
    Tool(Tool),
    Ocr,
    OcrPacks,
}

fn shortcut(m: Modifiers, k: Key) -> KeyboardShortcut {
    KeyboardShortcut::new(m, k)
}

impl App {
    fn icon(&mut self, ctx: &egui::Context, id: IconId, style: IconStyle) -> egui::TextureHandle {
        let dark = ctx.theme() == egui::Theme::Dark;
        let px = (ICON * ctx.pixels_per_point()).round() as u32;
        let key = (id, style.to_index(), dark, px);
        if let Some(t) = self.icon_tex.get(&key) {
            return t.clone();
        }
        let (size, rgba) = render_icon(id, style, dark, px).unwrap_or((1, vec![0; 4]));
        let ci = egui::ColorImage::from_rgba_unmultiplied([size as usize, size as usize], &rgba);
        let t = ctx.load_texture("icon", ci, egui::TextureOptions::LINEAR);
        self.icon_tex.insert(key, t.clone());
        t
    }

    /// 工具列按鈕：依設定顯示圖示與文字
    fn tool_button(&mut self, ui: &mut Ui, icon: IconId, text: &str, selected: bool, enabled: bool, tip: &str) -> Response {
        let mode = self.settings.button_mode;
        let tex = self.icon(ui.ctx(), icon, self.settings.icon_style);
        let font = egui::FontId::proportional(13.0);
        let galley = ui.painter().layout_no_wrap(text.to_string(), font, ui.visuals().text_color());
        let tw = galley.size();
        let pad = 5.0;
        let size = match mode {
            ButtonMode::TextUnderIcon => vec2(ICON.max(tw.x) + 2.0 * pad, ICON + tw.y + 2.0 + 2.0 * pad - 2.0),
            ButtonMode::TextBesideIcon => vec2(ICON + 4.0 + tw.x + 2.0 * pad, ICON + 2.0 * pad - 2.0),
            ButtonMode::IconOnly => vec2(ICON + 2.0 * pad, ICON + 2.0 * pad - 2.0),
            ButtonMode::TextOnly => vec2(tw.x + 2.0 * pad + 4.0, tw.y + 2.0 * pad),
        };
        let sense = if enabled { Sense::click() } else { Sense::hover() };
        let (rect, resp) = ui.allocate_exact_size(size, sense);
        // 無障礙資訊：讓螢幕閱讀器（與自動測試）能以名稱找到按鈕
        resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, selected, text));
        let vis = ui.visuals();
        if selected {
            ui.painter().rect(rect, 4.0, vis.selection.bg_fill.gamma_multiply(0.45), egui::Stroke::new(1.0, vis.selection.bg_fill), egui::StrokeKind::Inside);
        } else if enabled && resp.hovered() {
            ui.painter().rect_filled(rect, 4.0, vis.widgets.hovered.weak_bg_fill);
        }
        let tint = if enabled { Color32::WHITE } else { Color32::from_white_alpha(90) };
        let text_color = if enabled { vis.text_color() } else { vis.weak_text_color() };
        let icon_rect = |min: egui::Pos2| egui::Rect::from_min_size(min, vec2(ICON, ICON));
        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        match mode {
            ButtonMode::TextUnderIcon => {
                let ir = icon_rect(egui::pos2(rect.center().x - ICON / 2.0, rect.top() + pad - 1.0));
                ui.painter().image(tex.id(), ir, uv, tint);
                ui.painter().galley(egui::pos2(rect.center().x - tw.x / 2.0, ir.bottom() + 1.0), galley, text_color);
            }
            ButtonMode::TextBesideIcon => {
                let ir = icon_rect(egui::pos2(rect.left() + pad, rect.center().y - ICON / 2.0));
                ui.painter().image(tex.id(), ir, uv, tint);
                ui.painter().galley(egui::pos2(ir.right() + 4.0, rect.center().y - tw.y / 2.0), galley, text_color);
            }
            ButtonMode::IconOnly => {
                ui.painter().image(tex.id(), icon_rect(rect.center() - vec2(ICON / 2.0, ICON / 2.0)), uv, tint);
            }
            ButtonMode::TextOnly => {
                ui.painter().galley(rect.center() - tw / 2.0, galley, text_color);
            }
        }
        if tip.is_empty() { resp } else { resp.on_hover_text(tip) }
    }

    fn act(&mut self, ctx: &egui::Context, a: Act) {
        let open = self.is_open();
        match a {
            Act::Open => self.request(Pending::OpenDialog),
            Act::Edit if open => self.toggle_edit(),
            Act::Save if open => {
                self.save_as();
            }
            Act::Export if open => self.open_export_dialog(),
            Act::Print if open => self.print(),
            Act::Preview if open => self.open_preview(),
            Act::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Act::Undo => self.undo(),
            Act::Redo => self.redo(),
            Act::Delete if open => self.delete_selected(),
            Act::Copy if open => self.copy_text(ctx),
            Act::Prev if open => self.go_to_page(self.page.saturating_sub(1)),
            Act::Next if open => self.go_to_page(self.page + 1),
            Act::ZoomIn if open => self.set_zoom(self.zoom * 1.2),
            Act::ZoomOut if open => self.set_zoom(self.zoom / 1.2),
            Act::Fit if open => self.fit_width(),
            Act::Rulers => {
                self.settings.rulers = !self.settings.rulers;
                self.settings.save();
            }
            Act::Sidebar => {
                self.settings.sidebar = !self.settings.sidebar;
                self.settings.save();
            }
            Act::SidebarTab(t) => {
                self.settings.sidebar = true;
                self.settings.sidebar_tab = t;
                self.settings.save();
            }
            Act::Help => self.help.open = true,
            Act::About => self.about_open = true,
            Act::Tool(t) if open => self.set_tool(t),
            Act::Ocr if open => self.open_ocr_dialog(),
            Act::OcrPacks => self.open_pack_manager(),
            _ => {}
        }
    }

    /// 快捷鍵（對話框開啟時不處理；文字輸入中不處理單鍵）
    pub fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if self.dialog.is_some() || self.print_job.is_some() || self.ocr_dialog.is_some() || self.ocr_job.is_some() {
            return;
        }
        let c = Modifiers::COMMAND;
        let cs = Modifiers::COMMAND | Modifiers::SHIFT;
        let combos: [(KeyboardShortcut, Act); 19] = [
            (shortcut(Modifiers::NONE, Key::F4), Act::Sidebar),
            (shortcut(cs, Key::P), Act::Preview),
            (shortcut(cs, Key::Z), Act::Redo),
            (shortcut(cs, Key::S), Act::Save),
            (shortcut(c, Key::O), Act::Open),
            (shortcut(c, Key::S), Act::Save),
            (shortcut(c, Key::E), Act::Export),
            (shortcut(c, Key::P), Act::Print),
            (shortcut(c, Key::Z), Act::Undo),
            (shortcut(c, Key::Y), Act::Redo),
            (shortcut(c, Key::C), Act::Copy),
            (shortcut(c, Key::Plus), Act::ZoomIn),
            (shortcut(c, Key::Equals), Act::ZoomIn),
            (shortcut(c, Key::Minus), Act::ZoomOut),
            (shortcut(c, Key::Num0), Act::Fit),
            (shortcut(c, Key::R), Act::Rulers),
            (shortcut(Modifiers::NONE, Key::F1), Act::Help),
            (shortcut(Modifiers::NONE, Key::PageUp), Act::Prev),
            (shortcut(Modifiers::NONE, Key::PageDown), Act::Next),
        ];
        let typing = ctx.egui_wants_keyboard_input();
        let mut todo = Vec::new();
        ctx.input_mut(|i| {
            for (sc, a) in &combos {
                // 輸入框中的 Ctrl+Z / Ctrl+C 交給輸入框自己
                if typing && matches!(a, Act::Undo | Act::Redo | Act::Copy) {
                    continue;
                }
                if i.consume_shortcut(sc) {
                    todo.push(*a);
                }
            }
            if !typing {
                for t in Tool::ALL {
                    if i.consume_key(Modifiers::NONE, t.key()) {
                        todo.push(Act::Tool(t));
                    }
                }
                if i.consume_key(Modifiers::NONE, Key::Delete) || i.consume_key(Modifiers::NONE, Key::Backspace) {
                    todo.push(Act::Delete);
                }
            }
        });
        for a in todo {
            self.act(ctx, a);
        }
    }

    pub fn show_menu(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let open = self.is_open();
        let editing = open && self.editing;
        let mut todo: Option<Act> = None;
        egui::MenuBar::new().ui(ui, |ui| {
            let item = |ui: &mut Ui, text: &str, sc: &str, enabled: bool, a: Act, todo: &mut Option<Act>| {
                let b = egui::Button::new(text).shortcut_text(sc);
                if ui.add_enabled(enabled, b).clicked() {
                    *todo = Some(a);
                }
            };
            ui.menu_button("檔案(F)", |ui| {
                item(ui, "開啟…", "Ctrl+O", true, Act::Open, &mut todo);
                item(ui, "另存新檔…", "Ctrl+S", editing, Act::Save, &mut todo);
                item(ui, "匯出頁面 / 加密…", "Ctrl+E", open, Act::Export, &mut todo);
                ui.separator();
                item(ui, "列印…", "Ctrl+P", open, Act::Print, &mut todo);
                item(ui, "列印預覽…", "Ctrl+Shift+P", open, Act::Preview, &mut todo);
                ui.separator();
                item(ui, "離開", "", true, Act::Quit, &mut todo);
            });
            ui.menu_button("編輯(E)", |ui| {
                item(ui, if self.editing { "取消編輯" } else { "編輯" }, "", open, Act::Edit, &mut todo);
                ui.separator();
                item(ui, "復原", "Ctrl+Z", editing && !self.model.undo_stack.is_empty(), Act::Undo, &mut todo);
                item(ui, "取消復原", "Ctrl+Y", editing && !self.model.redo_stack.is_empty(), Act::Redo, &mut todo);
                ui.separator();
                item(ui, "刪除選取的註記", "Delete", editing, Act::Delete, &mut todo);
                item(ui, "複製最後選取的文字", "Ctrl+C", open, Act::Copy, &mut todo);
            });
            ui.menu_button("檢視(V)", |ui| {
                item(ui, "放大", "Ctrl++", open, Act::ZoomIn, &mut todo);
                item(ui, "縮小", "Ctrl+-", open, Act::ZoomOut, &mut todo);
                item(ui, "符合寬度", "Ctrl+0", open, Act::Fit, &mut todo);
                ui.separator();
                item(ui, "上一頁", "PgUp", open, Act::Prev, &mut todo);
                item(ui, "下一頁", "PgDn", open, Act::Next, &mut todo);
                ui.separator();
                let mut side = self.settings.sidebar;
                if ui.checkbox(&mut side, "顯示側邊欄（頁面／書籤）　F4").clicked() {
                    todo = Some(Act::Sidebar);
                }
                use crate::sidebar::SidebarTab;
                if ui.radio(self.settings.sidebar && self.settings.sidebar_tab == SidebarTab::Pages, "　頁面縮圖").clicked() {
                    todo = Some(Act::SidebarTab(SidebarTab::Pages));
                }
                if ui.radio(self.settings.sidebar && self.settings.sidebar_tab == SidebarTab::Bookmarks, "　書籤").clicked() {
                    todo = Some(Act::SidebarTab(SidebarTab::Bookmarks));
                }
                ui.separator();
                let mut rulers = self.settings.rulers;
                if ui.checkbox(&mut rulers, "顯示尺規　Ctrl+R").clicked() {
                    todo = Some(Act::Rulers);
                }
                ui.menu_button("尺規單位", |ui| {
                    for u in RulerUnit::ALL {
                        if ui.radio(self.settings.ruler_unit == u, u.name()).clicked() {
                            self.settings.ruler_unit = u;
                            self.settings.save();
                        }
                    }
                });
                ui.separator();
                ui.menu_button("工具列圖示", |ui| {
                    for st in IconStyle::ALL {
                        let tex = self.icon(&ctx, IconId::Highlight, st);
                        let img = egui::Image::new((tex.id(), vec2(18.0, 18.0)));
                        let mark = if self.settings.icon_style == st { "● " } else { "　 " };
                        if ui.add(egui::Button::image_and_text(img, format!("{mark}{}", st.name()))).clicked() {
                            self.settings.icon_style = st;
                            self.settings.save();
                        }
                    }
                    ui.separator();
                    for m in ButtonMode::ALL {
                        if ui.radio(self.settings.button_mode == m, m.name()).clicked() {
                            self.settings.button_mode = m;
                            self.settings.save();
                        }
                    }
                });
            });
            ui.menu_button("工具(T)", |ui| {
                for t in Tool::ALL {
                    let b = egui::Button::new(t.name()).shortcut_text(t.key_name()).selected(self.tool == t);
                    if ui.add_enabled(editing, b).clicked() {
                        todo = Some(Act::Tool(t));
                    }
                }
                ui.separator();
                item(ui, "文字辨識 (OCR)…", "", editing, Act::Ocr, &mut todo);
                item(ui, "OCR 語言管理…", "", true, Act::OcrPacks, &mut todo);
            });
            ui.menu_button("說明(H)", |ui| {
                item(ui, "使用說明", "F1", true, Act::Help, &mut todo);
                ui.separator();
                item(ui, "關於…", "", true, Act::About, &mut todo);
            });
        });
        if let Some(a) = todo {
            self.act(&ctx, a);
        }
    }

    pub fn show_main_toolbar(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let open = self.is_open();
        let editing = open && self.editing;
        let mut todo: Option<Act> = None;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let b = |s: &mut App, ui: &mut Ui, icon, text: &str, enabled, a: Act, tip: &str, todo: &mut Option<Act>| {
                if s.tool_button(ui, icon, text, false, enabled, tip).clicked() {
                    *todo = Some(a);
                }
            };
            b(self, ui, IconId::Open, "開啟", true, Act::Open, "開啟… (Ctrl+O)", &mut todo);
            b(self, ui, IconId::Save, "另存", editing, Act::Save, "另存新檔… (Ctrl+S)\n存檔後回到檢視狀態", &mut todo);
            b(self, ui, IconId::Export, "匯出加密", open, Act::Export, "匯出頁面 / 加密… (Ctrl+E)", &mut todo);
            b(self, ui, IconId::Print, "列印", open, Act::Print, "列印 (Ctrl+P)\n含所有註記，與另存新檔的結果相同", &mut todo);
            ui.separator();
            // 編輯／取消編輯：按下編輯後才可改動；取消編輯則放棄所有修改，回到原始檔案
            let (label, tip) = if self.editing {
                ("取消編輯", "取消編輯\n放棄所有尚未儲存的修改，回到原始檔案")
            } else {
                ("編輯", "編輯\n開始加入高亮、底線、文字、線條等註記（原始檔案不會被改動）")
            };
            if self.tool_button(ui, IconId::Edit, label, editing, open, tip).clicked() {
                todo = Some(Act::Edit);
            }
            for t in Tool::ALL {
                let tip = format!("{} ({})\n{}", t.name(), t.key_name(), t.hint());
                if self.tool_button(ui, t.icon(), t.name(), self.tool == t && editing, editing, &tip).clicked() {
                    todo = Some(Act::Tool(t));
                }
            }
            ui.separator();
            let (can_undo, can_redo) = (editing && !self.model.undo_stack.is_empty(), editing && !self.model.redo_stack.is_empty());
            b(self, ui, IconId::Undo, "復原", can_undo, Act::Undo, "復原 (Ctrl+Z)", &mut todo);
            b(self, ui, IconId::Redo, "取消復原", can_redo, Act::Redo, "取消復原 (Ctrl+Y)", &mut todo);
            ui.separator();
            b(self, ui, IconId::Prev, "上一頁", open, Act::Prev, "上一頁 (PgUp)", &mut todo);
            let n = self.page_count();
            let mut p = self.page + 1;
            let r = ui.add_enabled(open, egui::DragValue::new(&mut p).range(1..=n.max(1)).speed(0.1));
            if r.changed() && p >= 1 {
                self.go_to_page(p - 1);
            }
            ui.label(format!(" / {n} "));
            b(self, ui, IconId::Next, "下一頁", open, Act::Next, "下一頁 (PgDown)", &mut todo);
            ui.separator();
            b(self, ui, IconId::ZoomOut, "縮小", open, Act::ZoomOut, "縮小 (Ctrl + 滾輪)", &mut todo);
            ui.add_sized([44.0, 20.0], egui::Label::new(format!("{}%", (self.zoom * 100.0).round())));
            b(self, ui, IconId::ZoomIn, "放大", open, Act::ZoomIn, "放大 (Ctrl + 滾輪)", &mut todo);
            b(self, ui, IconId::FitWidth, "符合寬度", open, Act::Fit, "符合寬度 (Ctrl+0)", &mut todo);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                b(self, ui, IconId::Help, "說明", true, Act::Help, "使用說明 (F1)", &mut todo);
            });
        });
        if let Some(a) = todo {
            self.act(&ctx, a);
        }
    }

    fn swatch(ui: &mut Ui, c: Rgba, tip: &str) -> Response {
        let (rect, resp) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::click());
        let r = rect.shrink(1.5);
        ui.painter().rect(r, 3.0, to32(c), egui::Stroke::new(1.0, Color32::from_black_alpha(120)), egui::StrokeKind::Inside);
        if resp.hovered() {
            ui.painter().rect_stroke(rect, 3.0, egui::Stroke::new(1.0, ui.visuals().selection.bg_fill), egui::StrokeKind::Inside);
        }
        resp.on_hover_text(tip)
    }

    /// 屬性工具列：顏色、文字樣式、線條樣式（只顯示目前用得到的部分）
    pub fn show_props_toolbar(&mut self, ui: &mut Ui) {
        let bars = self.visible_bars();
        let (show_text, show_line, arrow) = (bars.text, bars.stroke, bars.arrow);
        // 檢視模式：屬性工具列停用
        if !self.editing {
            ui.disable();
        }
        ui.horizontal_wrapped(|ui| {
            // 固定高度：切換工具時頁面不會跳動
            ui.set_min_height(24.0);
            ui.label(" 顏色 ");
            // 目前顏色（選取註記時顯示第一個註記的顏色）
            let cur = match (self.tool, self.selection.first().and_then(|&i| self.model.marks.get(i))) {
                (Tool::Select, Some(m)) => m.color,
                _ => self.tool_color(),
            };
            let mut c = to32(cur);
            let r = egui::color_picker::color_edit_button_srgba(ui, &mut c, egui::color_picker::Alpha::Opaque).on_hover_text("自訂顏色…");
            if r.changed() {
                self.set_current_color(from32(c));
            }
            ui.label("自訂");
            for p in PRESETS {
                if Self::swatch(ui, p, &p.hex()).clicked() {
                    self.set_current_color(p);
                }
            }

            if show_text {
                ui.separator();
                ui.label(" 字型 ");
                let label = self.current_font().map(|f| f.label.clone()).unwrap_or_else(|| "（無字型）".into());
                let mut pick: Option<usize> = None;
                let mut other = false;
                egui::ComboBox::from_id_salt("font").selected_text(label).width(140.0).show_ui(ui, |ui| {
                    for (i, f) in self.fonts.iter().enumerate() {
                        if ui.selectable_label(self.props.font == i, &f.label).clicked() {
                            pick = Some(i);
                        }
                    }
                    if ui.selectable_label(false, "其他字型檔…").clicked() {
                        other = true;
                    }
                });
                if let Some(i) = pick {
                    self.set_font(i);
                }
                if other {
                    self.pick_font_file();
                }
                ui.label(" 大小 ");
                let mut v = self.props.font_size;
                if ui.add(egui::DragValue::new(&mut v).range(4.0..=200.0).speed(0.2).fixed_decimals(1).suffix(" pt")).changed() {
                    self.props.font_size = v;
                    self.apply_to_selection("fontSize", |m| set_if(m, MarkType::Text, |m| &mut m.font_size, v));
                }
                ui.separator();
                // 外框與背景：勾選框 + 顏色按鈕（+ 外框粗細）；選了顏色就順便勾選
                let mut on = self.props.text_border;
                if ui.checkbox(&mut on, "外框").changed() {
                    self.props.text_border = on;
                    self.apply_to_selection("border", |m| set_bool(m, |m| &mut m.text_border, on));
                }
                let mut bc = to32(self.props.border_color);
                if egui::color_picker::color_edit_button_srgba(ui, &mut bc, egui::color_picker::Alpha::OnlyBlend).on_hover_text("外框顏色").changed() {
                    let c = from32(bc);
                    self.props.border_color = c;
                    self.props.text_border = true;
                    self.apply_to_selection("borderColor", |m| {
                        if m.kind != MarkType::Text {
                            return false;
                        }
                        let ch = m.border_color != c || !m.text_border;
                        m.border_color = c;
                        m.text_border = true;
                        ch
                    });
                }
                let mut bw = self.props.border_width;
                if ui.add(egui::DragValue::new(&mut bw).range(0.25..=10.0).speed(0.05).fixed_decimals(2).suffix(" pt")).on_hover_text("外框粗細").changed() {
                    self.props.border_width = bw;
                    self.apply_to_selection("borderWidth", |m| set_if(m, MarkType::Text, |m| &mut m.border_width, bw));
                }
                ui.add_space(6.0);
                let mut fill = self.props.text_fill;
                if ui.checkbox(&mut fill, "背景").changed() {
                    self.props.text_fill = fill;
                    self.apply_to_selection("fill", |m| set_bool(m, |m| &mut m.text_fill, fill));
                }
                let mut fc = to32(self.props.fill_color);
                if egui::color_picker::color_edit_button_srgba(ui, &mut fc, egui::color_picker::Alpha::OnlyBlend).on_hover_text("背景顏色（可設定透明度）").changed() {
                    let c = from32(fc);
                    self.props.fill_color = c;
                    self.props.text_fill = true;
                    self.apply_to_selection("fillColor", |m| {
                        if m.kind != MarkType::Text {
                            return false;
                        }
                        let ch = m.fill_color != c || !m.text_fill;
                        m.fill_color = c;
                        m.text_fill = true;
                        ch
                    });
                }
            }

            if show_line {
                ui.separator();
                ui.label(" 線寬 ");
                let mut w = self.props.line_width;
                if ui.add(egui::DragValue::new(&mut w).range(0.5..=20.0).speed(0.1).fixed_decimals(1).suffix(" pt")).changed() {
                    self.props.line_width = w;
                    self.apply_to_selection("width", |m| set_line(m, |m| &mut m.width, w));
                }
                if bars.radius {
                    ui.label(" 圓角 ").on_hover_text("圓角矩形四角的圓弧半徑（0 = 直角）");
                    let mut r = self.props.corner_radius;
                    if ui.add(egui::DragValue::new(&mut r).range(0.0..=200.0).speed(0.3).fixed_decimals(1).suffix(" pt")).on_hover_text("圓角矩形四角的圓弧半徑（0 = 直角）").changed() {
                        self.props.corner_radius = r;
                        self.apply_to_selection("radius", |m| set_if(m, MarkType::Rect, |m| &mut m.radius, r));
                    }
                }
                if bars.line { ui.add_enabled_ui(arrow, |ui| {
                    ui.label(" 箭頭 ").on_hover_text("箭頭長度（寬度為長度的一半）");
                    let mut a = self.props.arrow_size;
                    if ui.add(egui::DragValue::new(&mut a).range(2.0..=120.0).speed(0.3).fixed_decimals(1).suffix(" pt")).on_hover_text("箭頭長度（寬度為長度的一半）").changed() {
                        self.props.arrow_size = a;
                        self.apply_to_selection("arrow", |m| set_if(m, MarkType::Arrow, |m| &mut m.arrow_size, a));
                    }
                    let mut both = self.props.both_ends;
                    egui::ComboBox::from_id_salt("ends").selected_text(if both { "↔ 雙向" } else { "→ 單向" }).width(80.0).show_ui(ui, |ui| {
                        ui.selectable_value(&mut both, false, "→ 單向");
                        ui.selectable_value(&mut both, true, "↔ 雙向");
                    });
                    if both != self.props.both_ends {
                        self.props.both_ends = both;
                        self.apply_to_selection("ends", |m| {
                            if m.kind != MarkType::Arrow || m.both_ends == both {
                                return false;
                            }
                            m.both_ends = both;
                            true
                        });
                    }
                }); }
                ui.label(" 線型 ");
                let mut dashed = self.props.dashed;
                egui::ComboBox::from_id_salt("style").selected_text(if dashed { "╍╍ 虛線" } else { "━━ 實線" }).width(80.0).show_ui(ui, |ui| {
                    ui.selectable_value(&mut dashed, false, "━━ 實線");
                    ui.selectable_value(&mut dashed, true, "╍╍ 虛線");
                });
                if dashed != self.props.dashed {
                    self.props.dashed = dashed;
                    self.apply_to_selection("dashed", |m| set_line_bool(m, |m| &mut m.dashed, dashed));
                }
                ui.add_enabled_ui(self.props.dashed, |ui| {
                    ui.label(" 虛線 ").on_hover_text("每段虛線的長度");
                    let mut d = self.props.dash_len;
                    if ui.add(egui::DragValue::new(&mut d).range(0.5..=100.0).speed(0.1).fixed_decimals(1).suffix(" pt")).on_hover_text("每段虛線的長度").changed() {
                        self.props.dash_len = d;
                        self.apply_to_selection("dash", |m| set_line(m, |m| &mut m.dash_len, d));
                    }
                    ui.label(" 間隔 ").on_hover_text("虛線之間的空隙");
                    let mut g = self.props.gap_len;
                    if ui.add(egui::DragValue::new(&mut g).range(0.5..=100.0).speed(0.1).fixed_decimals(1).suffix(" pt")).on_hover_text("虛線之間的空隙").changed() {
                        self.props.gap_len = g;
                        self.apply_to_selection("gap", |m| set_line(m, |m| &mut m.gap_len, g));
                    }
                });
            }
        });
    }

    pub fn show_status(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            if let Some((msg, until)) = &self.status {
                if self.now < *until {
                    ui.label(msg.as_str());
                    ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64((until - self.now).max(0.05)));
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let hint = if !self.is_open() || self.editing { self.tool.hint() } else { "檢視模式：按「編輯」開始加入註記" };
                ui.label(RichText::new(hint).color(ui.visuals().weak_text_color()));
                ui.add_space(12.0);
                let coord = match self.cursor_pt {
                    Some((p, true)) if self.is_open() => {
                        let u = self.settings.ruler_unit;
                        let k = u.points();
                        let digits = if u == RulerUnit::Inch { 2 } else { 1 };
                        format!("X {:.*}  Y {:.*} {}", digits, p.x / k, digits, p.y / k, u.suffix())
                    }
                    _ => String::new(),
                };
                ui.add_sized([170.0, 18.0], egui::Label::new(coord));
            });
        });
    }
}

fn set_if(m: &mut pdfcore::Mark, kind: MarkType, field: impl Fn(&mut pdfcore::Mark) -> &mut f64, v: f64) -> bool {
    if m.kind != kind {
        return false;
    }
    let f = field(m);
    let changed = *f != v;
    *f = v;
    changed
}

fn set_bool(m: &mut pdfcore::Mark, field: impl Fn(&mut pdfcore::Mark) -> &mut bool, v: bool) -> bool {
    if m.kind != MarkType::Text {
        return false;
    }
    let f = field(m);
    let changed = *f != v;
    *f = v;
    changed
}

fn set_line(m: &mut pdfcore::Mark, field: impl Fn(&mut pdfcore::Mark) -> &mut f64, v: f64) -> bool {
    if !m.is_stroked() {
        return false;
    }
    let f = field(m);
    let changed = *f != v;
    *f = v;
    changed
}

fn set_line_bool(m: &mut pdfcore::Mark, field: impl Fn(&mut pdfcore::Mark) -> &mut bool, v: bool) -> bool {
    if !m.is_stroked() {
        return false;
    }
    let f = field(m);
    let changed = *f != v;
    *f = v;
    changed
}
