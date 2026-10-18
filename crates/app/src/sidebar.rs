// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Fufeng Project

//! 左側欄：所有頁面的縮圖與書籤（PDF 大綱），點選即跳到該頁。

use crate::app::App;
use egui::{Color32, RichText, Sense, Stroke, StrokeKind, Ui, vec2};

/// 縮圖寬度上限（介面點）
const THUMB_MAX_W: f32 = 90.0;
/// 每個畫面最多新產生幾張縮圖，避免一次渲染整份文件造成卡頓
const THUMBS_PER_FRAME: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SidebarTab {
    Pages,
    Bookmarks,
}

impl SidebarTab {
    pub fn key(self) -> &'static str {
        match self {
            SidebarTab::Pages => "pages",
            SidebarTab::Bookmarks => "bookmarks",
        }
    }
    pub fn from_key(s: &str) -> Self {
        if s == "bookmarks" { SidebarTab::Bookmarks } else { SidebarTab::Pages }
    }
}

impl App {
    pub fn show_sidebar(&mut self, ui: &mut Ui) {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            for (tab, label) in [(SidebarTab::Pages, "頁面"), (SidebarTab::Bookmarks, "書籤")] {
                if ui.selectable_label(self.settings.sidebar_tab == tab, RichText::new(label).size(14.0)).clicked() {
                    self.settings.sidebar_tab = tab;
                    self.settings.save();
                }
            }
        });
        ui.separator();
        if self.doc.is_none() {
            ui.label(RichText::new("尚未開啟文件").color(ui.visuals().weak_text_color()));
            return;
        }
        match self.settings.sidebar_tab {
            SidebarTab::Pages => self.show_thumbnails(ui),
            SidebarTab::Bookmarks => self.show_bookmarks(ui),
        }
    }

    fn show_thumbnails(&mut self, ui: &mut Ui) {
        let Some(doc) = &self.doc else { return };
        let n = doc.page_count();
        let sizes: Vec<(f64, f64)> = (0..n).map(|p| doc.page_size(p)).collect();
        let ctx = ui.ctx().clone();
        let ppp = ctx.pixels_per_point();
        // 目前頁改變時（換頁、拖放開啟等），側欄自動捲到該頁
        let follow = self.thumb_follow != Some(self.page);
        self.thumb_follow = Some(self.page);
        let mut rendered = 0;
        let mut goto = None;
        egui::ScrollArea::vertical().id_salt("thumbs").auto_shrink([false, false]).show(ui, |ui| {
            // 縮圖寬度為側欄寬度的一半，置中顯示
            let w = ((ui.available_width() - 16.0) / 2.0).clamp(30.0, THUMB_MAX_W);
            ui.vertical_centered(|ui| {
                for (p, &(pw, ph)) in sizes.iter().enumerate() {
                    let h = (w as f64 * ph / pw.max(1.0)) as f32;
                    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click());
                    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, format!("第 {} 頁縮圖", p + 1)));
                    let current = p == self.page;
                    if ui.is_rect_visible(rect) {
                        let painter = ui.painter();
                        painter.rect_filled(rect.translate(vec2(2.0, 2.0)), 0.0, Color32::from_black_alpha(60));
                        painter.rect_filled(rect, 0.0, Color32::WHITE);
                        if !self.thumbs.contains_key(&p) && rendered < THUMBS_PER_FRAME {
                            rendered += 1;
                            let scale = (w * ppp) as f64 / pw.max(1.0);
                            if let Some(img) = self.doc.as_ref().and_then(|d| d.render(p, scale)) {
                                let ci = egui::ColorImage::from_rgba_unmultiplied([img.width as usize, img.height as usize], &img.rgba);
                                self.thumbs.insert(p, ctx.load_texture("thumb", ci, egui::TextureOptions::LINEAR));
                            }
                        }
                        if let Some(tex) = self.thumbs.get(&p) {
                            ui.painter().image(tex.id(), rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
                        }
                        let vis = ui.visuals();
                        if current {
                            ui.painter().rect_stroke(rect.expand(2.0), 2.0, Stroke::new(3.0, vis.selection.bg_fill), StrokeKind::Outside);
                        } else if resp.hovered() {
                            ui.painter().rect_stroke(rect.expand(1.0), 2.0, Stroke::new(1.5, vis.widgets.hovered.fg_stroke.color), StrokeKind::Outside);
                        }
                    }
                    if current && follow {
                        resp.scroll_to_me(Some(egui::Align::Center));
                    }
                    let label = RichText::new(format!("{}", p + 1)).size(12.0);
                    ui.label(if current { label.strong() } else { label.color(ui.visuals().weak_text_color()) });
                    if resp.on_hover_text(format!("第 {} 頁", p + 1)).clicked() {
                        goto = Some(p);
                    }
                    ui.add_space(6.0);
                }
            });
        });
        if rendered >= THUMBS_PER_FRAME {
            // 還有縮圖沒畫完：下一個畫面繼續
            ctx.request_repaint();
        }
        if let Some(p) = goto {
            self.go_to_page(p);
        }
    }

    fn show_bookmarks(&mut self, ui: &mut Ui) {
        let Some(doc) = &self.doc else { return };
        let items = doc.outline();
        if items.is_empty() {
            ui.label(RichText::new("此 PDF 沒有書籤").color(ui.visuals().weak_text_color()));
            return;
        }
        // 目前頁對應的書籤：頁碼不超過目前頁的最後一個
        let current = items.iter().enumerate().filter(|(_, o)| o.page <= self.page).map(|(i, _)| i).next_back();
        let mut goto = None;
        let mut toggle = None;
        egui::ScrollArea::both().id_salt("bookmarks").auto_shrink([false, false]).show(ui, |ui| {
            // 被收合的上層書籤之下的項目不顯示
            let mut hidden_below: Option<usize> = None;
            for (i, o) in items.iter().enumerate() {
                if let Some(level) = hidden_below {
                    if o.level > level {
                        continue;
                    }
                    hidden_below = None;
                }
                let has_children = items.get(i + 1).is_some_and(|next| next.level > o.level);
                let collapsed = self.collapsed.contains(&i);
                ui.horizontal(|ui| {
                    ui.add_space((o.level.saturating_sub(1) as f32) * 14.0);
                    if has_children {
                        // 展開／收合三角形：自行繪製（介面字型沒有 ▸ ▾ 字形）
                        let (r, resp) = ui.allocate_exact_size(vec2(14.0, 16.0), Sense::click());
                        let name = if collapsed { "展開" } else { "收合" };
                        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name));
                        let color = if resp.hovered() { ui.visuals().strong_text_color() } else { ui.visuals().text_color() };
                        let c = r.center();
                        let pts = if collapsed {
                            vec![c + vec2(-3.0, -5.0), c + vec2(4.0, 0.0), c + vec2(-3.0, 5.0)]
                        } else {
                            vec![c + vec2(-5.0, -3.0), c + vec2(5.0, -3.0), c + vec2(0.0, 4.0)]
                        };
                        ui.painter().add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
                        if resp.on_hover_text(name).clicked() {
                            toggle = Some(i);
                        }
                    } else {
                        ui.add_space(18.0);
                    }
                    let title = if o.title.is_empty() { format!("（第 {} 頁）", o.page + 1) } else { o.title.clone() };
                    let r = ui.selectable_label(current == Some(i), title).on_hover_text(format!("第 {} 頁", o.page + 1));
                    if r.clicked() {
                        goto = Some(o.page);
                    }
                });
                if has_children && collapsed {
                    hidden_below = Some(o.level);
                }
            }
        });
        if let Some(i) = toggle {
            if !self.collapsed.remove(&i) {
                self.collapsed.insert(i);
            }
        }
        if let Some(p) = goto {
            self.go_to_page(p);
        }
    }
}
