// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Fufeng Project

//! 列印預覽：以預設印表機的紙張，依實際列印相同的規則擺放頁面（含所有註記）。

use crate::print::{Paper, default_paper, place_page, rotate_cw};
use egui::{Color32, Rect, Sense, pos2, vec2};
use pdfcore::Document;

pub struct PrintPreview {
    doc: Document,
    pub page: usize,
    paper: Paper,
    printer_found: bool,
    landscape: bool,
    /// 每吋幾個介面點；None = 符合視窗
    zoom: Option<f32>,
    tex: Option<((usize, bool, u64), egui::TextureHandle)>,
}

pub enum PreviewAction {
    None,
    Close,
    Print,
}

impl PrintPreview {
    pub fn new(doc: Document, page: usize) -> Self {
        let (paper, found) = match default_paper() {
            Some(p) => (p, true),
            None => (Paper::a4(), false),
        };
        let page = page.min(doc.page_count().saturating_sub(1));
        Self { doc, page, paper, printer_found: found, landscape: paper_is_landscape(&paper), zoom: None, tex: None }
    }

    fn paper(&self) -> Paper {
        if self.landscape == paper_is_landscape(&self.paper) { self.paper } else { self.paper.rotated() }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> PreviewAction {
        let mut action = PreviewAction::None;
        let mut open = true;
        let n = self.doc.page_count();
        egui::Window::new("列印預覽").open(&mut open).default_size([900.0, 760.0]).min_size([480.0, 360.0]).collapsible(false).show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("列印…").clicked() {
                    action = PreviewAction::Print;
                }
                ui.separator();
                if ui.add_enabled(self.page > 0, egui::Button::new("◀")).clicked() {
                    self.page -= 1;
                }
                ui.label(format!("{} / {}", self.page + 1, n));
                if ui.add_enabled(self.page + 1 < n, egui::Button::new("▶")).clicked() {
                    self.page += 1;
                }
                ui.separator();
                ui.selectable_value(&mut self.landscape, false, "直向");
                ui.selectable_value(&mut self.landscape, true, "橫向");
                ui.separator();
                if ui.selectable_label(self.zoom.is_none(), "符合視窗").clicked() {
                    self.zoom = None;
                }
                for (label, z) in [("50%", 48.0), ("100%", 96.0), ("150%", 144.0)] {
                    if ui.selectable_label(self.zoom == Some(z), label).clicked() {
                        self.zoom = Some(z);
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("關閉").clicked() {
                        action = PreviewAction::Close;
                    }
                });
            });
            if !self.printer_found {
                ui.label(egui::RichText::new("找不到預設印表機，以 A4 紙張預覽").color(ui.visuals().weak_text_color()));
            }
            ui.separator();
            let paper = self.paper();
            let avail = ui.available_size();
            let fit = ((avail.x - 40.0) / paper.width as f32).min((avail.y - 40.0) / paper.height as f32).max(10.0);
            let per_inch = self.zoom.unwrap_or(fit);
            let size = vec2(paper.width as f32 * per_inch, paper.height as f32 * per_inch);
            egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                let (rect, _) = ui.allocate_exact_size(size + vec2(40.0, 40.0), Sense::hover());
                let paper_rect = Rect::from_min_size(rect.min + vec2(20.0, 20.0), size);
                let painter = ui.painter();
                painter.rect_filled(paper_rect.translate(vec2(4.0, 4.0)), 0.0, Color32::from_black_alpha(80));
                painter.rect_filled(paper_rect, 0.0, Color32::WHITE);
                // 可列印範圍
                let [x0, y0, x1, y1] = paper.printable;
                let area = Rect::from_min_max(
                    paper_rect.min + vec2(x0 as f32 * per_inch, y0 as f32 * per_inch),
                    paper_rect.min + vec2(x1 as f32 * per_inch, y1 as f32 * per_inch),
                );
                let (pw, ph) = self.doc.page_size(self.page);
                let units_per_pt = per_inch as f64 / 72.0;
                let (rotate, t) = place_page(pw, ph, area.width() as f64, area.height() as f64, units_per_pt);
                let target = Rect::from_min_size(area.min + vec2(t[0] as f32, t[1] as f32), vec2(t[2] as f32, t[3] as f32));
                let ppp = ctx.pixels_per_point();
                let scale = (if rotate { t[3] / pw } else { t[2] / pw }) * ppp as f64;
                let key = (self.page, rotate, scale.to_bits());
                if self.tex.as_ref().map(|(k, _)| *k) != Some(key) {
                    if let Some(mut img) = self.doc.render(self.page, scale) {
                        if rotate {
                            img = rotate_cw(&img);
                        }
                        let ci = egui::ColorImage::from_rgba_unmultiplied([img.width as usize, img.height as usize], &img.rgba);
                        self.tex = Some((key, ctx.load_texture("preview", ci, egui::TextureOptions::LINEAR)));
                    }
                }
                if let Some((_, tex)) = &self.tex {
                    painter.image(tex.id(), target, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                }
            });
        });
        if !open {
            action = PreviewAction::Close;
        }
        action
    }
}

fn paper_is_landscape(p: &Paper) -> bool {
    p.width > p.height
}
