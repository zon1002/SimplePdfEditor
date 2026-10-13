// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Fu Project

//! 單頁檢視：顯示 PDF 頁面並處理各工具的滑鼠操作。
//! 頁面座標（pt）× 縮放 = 介面座標；頁面四周留 12 pt 邊界。

use crate::app::{App, Drag, Tool};
use crate::ruler::{self, RulerColors, THICKNESS};
use egui::{Color32, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};
use pdfcore::fonts::{TEXT_PADDING, text_box_rect};
use pdfcore::geom::{distance_to_segment, point_in_polygon, quad_bounds};
use pdfcore::{Mark, MarkType, Pt, Rgba, line_geometry};
use std::hash::{Hash, Hasher};

const MARGIN_PT: f64 = 12.0;
const VIEW_BG: Color32 = Color32::from_rgb(0x52, 0x56, 0x5c);
const SELECT_BLUE: Color32 = Color32::from_rgb(0, 120, 215);

fn c32(c: Rgba) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
}

/// 註記的範圍（不含位移），用於選取框
pub fn mark_bounds(m: &Mark) -> pdfcore::Rect {
    match m.kind {
        MarkType::Highlight | MarkType::Underline => {
            let mut r = pdfcore::Rect::EMPTY;
            for q in &m.quads {
                r = r.union(&quad_bounds(q));
            }
            r.expand(2.0)
        }
        MarkType::Text => {
            let pad = if m.text_border { m.border_width } else { 0.0 } + 1.0;
            text_box_rect(m).expand(pad)
        }
        MarkType::Line | MarkType::Arrow => {
            let g = line_geometry(m);
            let pad = m.width.max(6.0) + 2.0;
            let mut r = pdfcore::Rect::from_points(m.p1, m.p2);
            for h in &g.heads {
                for p in h {
                    r.include(*p);
                }
            }
            r.expand(pad)
        }
    }
}

/// 點是否落在註記上（頁面座標，已含位移）
fn hit(m: &Mark, p: Pt) -> bool {
    let p = p - m.offset;
    match m.kind {
        MarkType::Line | MarkType::Arrow => {
            let tol = (8.0f64).max(m.width + 6.0) / 2.0;
            distance_to_segment(p, m.p1, m.p2) <= tol || line_geometry(m).heads.iter().any(|h| point_in_polygon(p, h))
        }
        MarkType::Text => text_box_rect(m).contains(p),
        _ => m.quads.iter().any(|q| point_in_polygon(p, q)),
    }
}

fn z_order(k: MarkType) -> u8 {
    match k {
        MarkType::Highlight => 1,
        MarkType::Underline => 2,
        MarkType::Line | MarkType::Arrow => 3,
        MarkType::Text => 4,
    }
}

/// 按住 Shift 時把線條限制在 45° 倍數
fn constrain_angle(a: Pt, b: Pt) -> Pt {
    let d = b - a;
    let len = d.length();
    let ang = (-d.y).atan2(d.x);
    let snapped = (ang / std::f64::consts::FRAC_PI_4).round() * std::f64::consts::FRAC_PI_4;
    Pt::new(a.x + len * snapped.cos(), a.y - len * snapped.sin())
}

struct Mapper {
    origin: Pos2,
    zoom: f64,
}

impl Mapper {
    fn to_screen(&self, p: Pt) -> Pos2 {
        pos2(self.origin.x + (p.x * self.zoom) as f32, self.origin.y + (p.y * self.zoom) as f32)
    }
    fn to_page(&self, p: Pos2) -> Pt {
        Pt::new((p.x - self.origin.x) as f64 / self.zoom, (p.y - self.origin.y) as f64 / self.zoom)
    }
    fn rect(&self, r: &pdfcore::Rect) -> Rect {
        Rect::from_min_max(self.to_screen(r.top_left()), self.to_screen(Pt::new(r.x1, r.y1)))
    }
}

/// 虛線：沿線段切成多段
fn dashed_segments(a: Pos2, b: Pos2, dash: f32, gap: f32, out: &mut Vec<[Pos2; 2]>) {
    let d = b - a;
    let len = d.length();
    if len <= 0.0 || dash <= 0.0 {
        return;
    }
    let dir = d / len;
    let mut t = 0.0;
    let mut n = 0;
    while t < len && n < 20000 {
        let e = (t + dash).min(len);
        out.push([a + dir * t, a + dir * e]);
        t = e + gap.max(0.1);
        n += 1;
    }
}

fn paint_line_mark(painter: &egui::Painter, mp: &Mapper, m: &Mark) {
    let g = line_geometry(m);
    let color = c32(m.color.with_alpha(255));
    let off = m.offset;
    let a = mp.to_screen(g.shaft_start + off);
    let b = mp.to_screen(g.shaft_end + off);
    let w = (m.width * mp.zoom) as f32;
    if m.dashed {
        let mut segs = Vec::new();
        dashed_segments(a, b, (m.dash_len.max(0.1) * mp.zoom) as f32, (m.gap_len.max(0.1) * mp.zoom) as f32, &mut segs);
        for s in segs {
            painter.line_segment(s, Stroke::new(w, color));
        }
    } else {
        painter.line_segment([a, b], Stroke::new(w, color));
        // 實線用圓頭
        painter.circle_filled(a, w / 2.0, color);
        painter.circle_filled(b, w / 2.0, color);
    }
    for h in &g.heads {
        let pts: Vec<Pos2> = h.iter().map(|p| mp.to_screen(*p + off)).collect();
        painter.add(Shape::convex_polygon(pts, color, Stroke::NONE));
    }
}

fn paint_underline(painter: &egui::Painter, mp: &Mapper, m: &Mark) {
    for q in &m.quads {
        let (ul, ll, lr) = (q[0] + m.offset, q[3] + m.offset, q[2] + m.offset);
        let h = ul.distance(ll);
        let t = (h / 14.0).max(0.8);
        let up = (ul - ll) * (t / h.max(0.01)) * 0.5;
        painter.line_segment([mp.to_screen(ll + up), mp.to_screen(lr + up)], Stroke::new((t * mp.zoom) as f32, c32(m.color.with_alpha(255))));
    }
}

fn paint_quads(painter: &egui::Painter, mp: &Mapper, quads: &[pdfcore::Quad], offset: Pt, color: Color32) {
    for q in quads {
        let pts: Vec<Pos2> = q.iter().map(|p| mp.to_screen(*p + offset)).collect();
        painter.add(Shape::convex_polygon(pts, color, Stroke::NONE));
    }
}

fn dashed_rect(painter: &egui::Painter, r: Rect, color: Color32) {
    let corners = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
    let mut segs = Vec::new();
    for w in corners.windows(2) {
        dashed_segments(w[0], w[1], 4.0, 3.0, &mut segs);
    }
    for s in segs {
        painter.line_segment(s, Stroke::new(1.0, color));
    }
}

fn texture_from(ctx: &egui::Context, name: &str, img: &pdfcore::Image) -> egui::TextureHandle {
    let ci = egui::ColorImage::from_rgba_unmultiplied([img.width as usize, img.height as usize], &img.rgba);
    ctx.load_texture(name, ci, egui::TextureOptions::LINEAR)
}

impl App {
    /// 頁面貼圖（含相乘混色的高亮）；縮放或註記改變時重新產生
    fn ensure_page_texture(&mut self, ctx: &egui::Context, scale: f64, exclude_moving: bool) -> Option<egui::TextureHandle> {
        let doc = self.doc.as_ref()?;
        let scale_key = scale.to_bits();
        let key = (self.page, scale_key, self.marks_gen, exclude_moving);
        if let Some((k, tex)) = &self.page_tex {
            if *k == key {
                return Some(tex.clone());
            }
        }
        if self.base_img.as_ref().map(|(k, _)| *k) != Some((self.page, scale_key)) {
            let img = doc.render(self.page, scale)?;
            self.base_img = Some(((self.page, scale_key), img));
        }
        let (_, base) = self.base_img.as_ref()?;
        let mut img = pdfcore::Image { width: base.width, height: base.height, rgba: base.rgba.clone() };
        // 實際渲染比例（頁面過大時會自動縮小）
        let (pw, _) = doc.page_size(self.page);
        let real_scale = img.width as f64 / pw.max(1.0);
        let page = self.page;
        let sel = &self.selection;
        let marks = self.model.marks.iter().enumerate().filter(|(i, m)| m.page == page && !(exclude_moving && sel.contains(i))).map(|(_, m)| m);
        pdfcore::raster::composite_highlights(&mut img, marks, real_scale);
        let tex = texture_from(ctx, "page", &img);
        self.page_tex = Some((key, tex.clone()));
        Some(tex)
    }

    /// 文字註記的貼圖（不含位移）；內容或縮放改變時重新產生
    fn text_texture(&mut self, ctx: &egui::Context, m: &Mark, scale: f64) -> Option<(egui::TextureHandle, pdfcore::Rect)> {
        let mut base = m.clone();
        base.offset = Pt::default();
        base.page = 0;
        let mut h = std::collections::hash_map::DefaultHasher::new();
        format!("{base:?}").hash(&mut h);
        scale.to_bits().hash(&mut h);
        let key = h.finish();
        if let Some(t) = self.text_tex.get(&key) {
            return Some(t.clone());
        }
        let (img, area) = pdfcore::raster::render_text_mark(&base, scale)?;
        let tex = texture_from(ctx, "text", &img);
        if self.text_tex.len() > 256 {
            self.text_tex.clear();
        }
        self.text_tex.insert(key, (tex.clone(), area));
        Some((tex, area))
    }

    /// 最上層的註記（z 順序：文字 > 線條 > 底線 > 高亮；同類後加的在上）
    fn mark_at(&self, p: Pt) -> Option<usize> {
        let mut best: Option<(u8, usize)> = None;
        for (i, m) in self.model.marks.iter().enumerate() {
            if m.page != self.page || !hit(m, p) {
                continue;
            }
            let z = z_order(m.kind);
            if best.is_none_or(|(bz, bi)| (z, i) > (bz, bi)) {
                best = Some((z, i));
            }
        }
        best.map(|(_, i)| i)
    }

    fn text_mark_at(&self, p: Pt) -> Option<usize> {
        self.model.marks.iter().enumerate().rev().find(|(_, m)| m.page == self.page && m.kind == MarkType::Text && hit(m, p)).map(|(i, _)| i)
    }

    pub fn show_view(&mut self, ui: &mut Ui) {
        let full = ui.available_rect_before_wrap();
        ui.painter().rect_filled(full, 0.0, VIEW_BG);
        let rulers = self.settings.rulers;
        let t = if rulers { THICKNESS } else { 0.0 };
        let view_rect = Rect::from_min_max(full.min + vec2(t, t), full.max);
        self.view_size = view_rect.size();

        if self.doc.is_none() {
            ui.allocate_rect(full, Sense::hover());
            ui.painter().text(
                view_rect.center(),
                egui::Align2::CENTER_CENTER,
                "按「開啟」選擇 PDF，或把 PDF 檔拖曳到這裡",
                egui::FontId::proportional(16.0),
                Color32::from_gray(220),
            );
            if rulers {
                self.paint_rulers(ui, full, view_rect, None);
            }
            return;
        }
        if self.fit_pending && view_rect.width() > 50.0 {
            self.fit_pending = false;
            self.fit_width();
        }

        let ctx = ui.ctx().clone();
        let ppp = ctx.pixels_per_point() as f64;
        let (pw, ph) = self.doc.as_ref().map(|d| d.page_size(self.page)).unwrap_or((612.0, 792.0));

        // Ctrl + 滾輪縮放（以滑鼠位置為中心）
        let pointer = ctx.input(|i| i.pointer.hover_pos());
        let mut zoom_steps = 0.0f32;
        ctx.input(|i| {
            for e in &i.events {
                if let egui::Event::MouseWheel { delta, modifiers, unit, .. } = e {
                    if modifiers.command {
                        zoom_steps += match unit {
                            egui::MouseWheelUnit::Line | egui::MouseWheelUnit::Page => delta.y,
                            egui::MouseWheelUnit::Point => delta.y / 40.0,
                        };
                    }
                }
            }
        });
        let hovering_view = pointer.is_some_and(|p| view_rect.contains(p));

        let zoom = self.zoom;
        let margin = (MARGIN_PT * zoom) as f32;
        let content = vec2((pw * zoom) as f32 + 2.0 * margin, (ph * zoom) as f32 + 2.0 * margin);

        let mut sa = egui::ScrollArea::both().id_salt("page_view").auto_shrink([false, false]);
        if self.scroll_to_top {
            self.scroll_to_top = false;
            let x = ((content.x - view_rect.width()) / 2.0).max(0.0);
            self.pending_scroll = Some(vec2(x, 0.0));
        }
        if let Some(off) = self.pending_scroll.take() {
            sa = sa.scroll_offset(off);
        }
        if zoom_steps != 0.0 && hovering_view {
            // 縮放時停用捲動，避免 Ctrl + 滾輪同時捲動頁面
            sa = sa.wheel_scroll_multiplier(egui::Vec2::ZERO);
        }

        let mut view_ui = ui.new_child(egui::UiBuilder::new().max_rect(view_rect));
        let mut origin_out = Pos2::ZERO;
        let out = sa.show_viewport(&mut view_ui, |ui, viewport| {
            let (rect, resp) = ui.allocate_exact_size(content.max(viewport.size()), Sense::click_and_drag());
            // 內容比視窗窄時水平置中
            let x0 = rect.left() + ((rect.width() - content.x) / 2.0).max(0.0);
            let origin = pos2(x0 + margin, rect.top() + margin);
            origin_out = origin;
            self.paint_and_interact(ui, &ctx, resp, origin, ppp, pw, ph);
        });
        let offset = out.state.offset;

        if zoom_steps != 0.0 && hovering_view {
            let new_zoom = (self.zoom * 1.15f64.powf(zoom_steps as f64)).clamp(0.25, 5.0);
            if let Some(p) = pointer {
                // 滑鼠下的頁面位置維持不動
                let page_pt = Pt::new((p.x - origin_out.x) as f64 / self.zoom, (p.y - origin_out.y) as f64 / self.zoom);
                let new_margin = MARGIN_PT * new_zoom;
                let new_content_x = pw * new_zoom + 2.0 * new_margin;
                let center_pad = ((view_rect.width() as f64 - new_content_x) / 2.0).max(0.0);
                let sx = center_pad + new_margin + page_pt.x * new_zoom - (p.x - view_rect.left()) as f64;
                let sy = new_margin + page_pt.y * new_zoom - (p.y - view_rect.top()) as f64;
                self.pending_scroll = Some(vec2(sx.max(0.0) as f32, sy.max(0.0) as f32));
            }
            self.set_zoom(new_zoom);
            ctx.request_repaint();
        }
        let _ = offset;

        if rulers {
            self.paint_rulers(ui, full, view_rect, Some(origin_out));
        }
        ui.allocate_rect(full, Sense::hover());
    }

    fn paint_rulers(&mut self, ui: &mut Ui, full: Rect, view_rect: Rect, origin: Option<Pos2>) {
        let painter = ui.painter();
        let dark = ui.visuals().dark_mode;
        let win = ui.visuals().panel_fill;
        let colors = RulerColors {
            outside: if dark { win.gamma_multiply(0.75) } else { Color32::from_gray(222) },
            inside: if dark { Color32::from_gray(70) } else { Color32::WHITE },
            tick: ui.visuals().text_color(),
            edge: if dark { Color32::from_gray(140) } else { Color32::from_gray(110) },
        };
        let t = THICKNESS;
        let h_rect = Rect::from_min_max(pos2(view_rect.left(), full.top()), pos2(full.right(), full.top() + t));
        let v_rect = Rect::from_min_max(pos2(full.left(), view_rect.top()), pos2(full.left() + t, full.bottom()));
        let (pw, ph) = self.doc.as_ref().map(|d| d.page_size(self.page)).unwrap_or((0.0, 0.0));
        let o = origin.unwrap_or(view_rect.min);
        let cursor = self.cursor_screen.filter(|p| view_rect.contains(*p));
        ruler::paint_ruler(painter, h_rect, true, o.x, self.zoom, if origin.is_some() { pw } else { 0.0 }, self.settings.ruler_unit, cursor.map(|p| p.x), &colors);
        ruler::paint_ruler(painter, v_rect, false, o.y, self.zoom, if origin.is_some() { ph } else { 0.0 }, self.settings.ruler_unit, cursor.map(|p| p.y), &colors);

        // 左上角：顯示單位，點選可切換
        let corner = Rect::from_min_size(full.min, vec2(t, t));
        painter.rect_filled(corner, 0.0, colors.outside);
        let id = ui.id().with("ruler_corner");
        let resp = ui.interact(corner, id, Sense::click()).on_hover_text("尺規單位");
        ui.painter().text(corner.center(), egui::Align2::CENTER_CENTER, self.settings.ruler_unit.suffix(), egui::FontId::proportional(9.0), colors.tick);
        egui::Popup::menu(&resp).show(|ui| {
            for u in ruler::RulerUnit::ALL {
                if ui.radio(self.settings.ruler_unit == u, u.name()).clicked() {
                    self.settings.ruler_unit = u;
                    self.settings.save();
                    ui.close();
                }
            }
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_and_interact(&mut self, ui: &mut Ui, ctx: &egui::Context, resp: egui::Response, origin: Pos2, ppp: f64, pw: f64, ph: f64) {
        let mp = Mapper { origin, zoom: self.zoom };
        let page_rect = mp.rect(&pdfcore::Rect::new(0.0, 0.0, pw, ph));
        let painter = ui.painter_at(ui.clip_rect());

        // ---- 滑鼠 ----
        let (pressed, released, down, double, mods, pos) = ui.input(|i| {
            (i.pointer.primary_pressed(), i.pointer.primary_released(), i.pointer.primary_down(), false, i.modifiers, i.pointer.interact_pos())
        });
        let _ = double;
        let hover_pos = resp.hover_pos();
        self.cursor_screen = hover_pos;
        self.cursor_pt = hover_pos.map(|p| {
            let pt = mp.to_page(p);
            (pt, pt.x >= 0.0 && pt.y >= 0.0 && pt.x <= pw && pt.y <= ph)
        });
        let page_bounds = pdfcore::Rect::new(0.0, 0.0, pw, ph);
        let clamp = |p: Pt| Pt::new(p.x.clamp(0.0, pw), p.y.clamp(0.0, ph));

        if pressed && resp.hovered() && self.dialog.is_none() {
            if let Some(p) = pos {
                let pt = mp.to_page(p);
                self.on_press(pt, page_bounds.contains(pt), mods.command);
            }
        }
        if down {
            if let Some(p) = pos {
                let pt = mp.to_page(p);
                self.on_move(clamp(pt), pt, mods.shift);
            }
        }
        if resp.double_clicked() && self.tool == Tool::Select {
            if let Some(p) = pos {
                if let Some(i) = self.text_mark_at(mp.to_page(p)) {
                    self.drag = Drag::None;
                    self.begin_edit_text(i);
                }
            }
        }
        if released {
            let pt = pos.map(|p| mp.to_page(p));
            self.on_release(pt.map(clamp), mods.shift);
        }

        // ---- 游標 ----
        if resp.hovered() {
            let icon = match self.tool {
                Tool::Select => egui::CursorIcon::Default,
                Tool::Highlight | Tool::Underline | Tool::Text => egui::CursorIcon::Text,
                Tool::Line | Tool::Arrow => egui::CursorIcon::Crosshair,
            };
            ctx.set_cursor_icon(icon);
        }

        // ---- 繪製 ----
        painter.rect_filled(page_rect.translate(vec2(3.0, 3.0)), 0.0, Color32::from_black_alpha(90));
        let moving = matches!(self.drag, Drag::Moving { moved: true, .. });
        let scale = self.zoom * ppp;
        if let Some(tex) = self.ensure_page_texture(ctx, scale, moving) {
            painter.image(tex.id(), page_rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        } else {
            painter.rect_filled(page_rect, 0.0, Color32::WHITE);
        }

        let page = self.page;
        let marks: Vec<(usize, Mark)> = self.model.marks.iter().enumerate().filter(|(_, m)| m.page == page).map(|(i, m)| (i, m.clone())).collect();
        // 移動中的高亮另外以半透明顯示
        if moving {
            for (i, m) in &marks {
                if m.kind == MarkType::Highlight && self.selection.contains(i) {
                    paint_quads(&painter, &mp, &m.quads, m.offset, c32(m.color.with_alpha(120)));
                }
            }
        }
        for kind in [MarkType::Underline, MarkType::Line, MarkType::Text] {
            for (_, m) in marks.iter().filter(|(_, m)| z_order(m.kind) == z_order(kind)) {
                match m.kind {
                    MarkType::Underline => paint_underline(&painter, &mp, m),
                    MarkType::Line | MarkType::Arrow => paint_line_mark(&painter, &mp, m),
                    MarkType::Text => {
                        if let Some((tex, area)) = self.text_texture(ctx, m, scale) {
                            let r = mp.rect(&area.translate(m.offset));
                            painter.image(tex.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                        }
                    }
                    _ => {}
                }
            }
        }
        // 選取框
        for &i in &self.selection {
            if let Some(m) = self.model.marks.get(i).filter(|m| m.page == page) {
                let r = mp.rect(&mark_bounds(m).translate(m.offset)).shrink(1.0);
                dashed_rect(&painter, r, SELECT_BLUE);
            }
        }
        // 拖曳中的預覽
        match &self.drag {
            Drag::Selecting { quads, .. } => {
                paint_quads(&painter, &mp, quads, Pt::default(), c32(self.tool_color().with_alpha(110)));
            }
            Drag::Drawing { start, cur } => {
                let m = self.new_line_mark(*start, *cur);
                paint_line_mark(&painter, &mp, &m);
            }
            _ => {}
        }
        let _ = (TEXT_PADDING, StrokeKind::Inside, Vec2::ZERO);
    }

    fn on_press(&mut self, pt: Pt, on_page: bool, ctrl: bool) {
        match self.tool {
            Tool::Select => {
                let hit = self.mark_at(pt);
                let mut sel = self.selection.clone();
                match hit {
                    Some(i) => {
                        if ctrl {
                            if let Some(k) = sel.iter().position(|&s| s == i) {
                                sel.remove(k);
                            } else {
                                sel.push(i);
                            }
                        } else if !sel.contains(&i) {
                            sel = vec![i];
                        }
                        sel.sort_unstable();
                        let start_move = sel.contains(&i);
                        self.set_selection(sel);
                        if start_move {
                            self.drag = Drag::Moving { start: pt, before: self.model.marks.clone(), moved: false };
                        }
                    }
                    None => {
                        if !ctrl {
                            self.set_selection(Vec::new());
                        }
                    }
                }
            }
            Tool::Text => {
                // 對話框延到放開滑鼠後才開
                if on_page {
                    self.drag = Drag::TextClick { at: pt };
                }
            }
            Tool::Highlight | Tool::Underline => {
                if on_page {
                    self.drag = Drag::Selecting { start: pt, quads: Vec::new(), text: String::new() };
                }
            }
            Tool::Line | Tool::Arrow => {
                if on_page {
                    self.drag = Drag::Drawing { start: pt, cur: pt };
                }
            }
        }
    }

    fn on_move(&mut self, cur: Pt, raw: Pt, shift: bool) {
        match &mut self.drag {
            Drag::Moving { start, before, moved } => {
                let d = raw - *start;
                if d.length() > 0.0 {
                    *moved = true;
                }
                for &i in &self.selection {
                    if let (Some(m), Some(b)) = (self.model.marks.get_mut(i), before.get(i)) {
                        m.offset = b.offset + d;
                    }
                }
            }
            Drag::Selecting { start, quads, text } => {
                let Some(doc) = &self.doc else { return };
                let (q, t) = doc.select_text(self.page, *start, cur);
                *quads = q;
                *text = t;
                if quads.is_empty() {
                    // 沒有文字層（例如掃描檔）：改用拖曳出的矩形
                    let r = pdfcore::Rect::from_points(*start, cur);
                    if r.width() > 3.0 && r.height() > 3.0 {
                        quads.push(r.corners());
                    }
                }
            }
            Drag::Drawing { start, cur: c } => {
                *c = if shift { constrain_angle(*start, cur) } else { cur };
            }
            _ => {}
        }
    }

    fn on_release(&mut self, pt: Option<Pt>, shift: bool) {
        let drag = std::mem::replace(&mut self.drag, Drag::None);
        match drag {
            Drag::Moving { before, moved, .. } => {
                let changed = moved && before.len() == self.model.marks.len() && before.iter().zip(&self.model.marks).any(|(a, b)| a.offset != b.offset);
                if changed {
                    self.model.record(before);
                    self.coalesce = None;
                }
                // 移動結束才重新產生頁面貼圖（含高亮）
                self.touch();
            }
            Drag::TextClick { at } => match self.text_mark_at(at) {
                Some(i) => self.begin_edit_text(i),
                None => self.begin_add_text(at),
            },
            Drag::Selecting { quads, text, .. } => {
                if !quads.is_empty() {
                    let kind = if self.tool == Tool::Highlight { MarkType::Highlight } else { MarkType::Underline };
                    let m = Mark { kind, page: self.page, color: self.tool_color(), quads, ..Default::default() };
                    if !text.is_empty() {
                        let s: String = text.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(40).collect();
                        self.message(format!("已標記：{s}"), 4.0);
                    }
                    self.last_text = text;
                    self.add_mark(m);
                }
            }
            Drag::Drawing { start, cur } => {
                let end = match pt {
                    Some(p) if shift => constrain_angle(start, p),
                    Some(p) => p,
                    None => cur,
                };
                let m = self.new_line_mark(start, end);
                if m.p1.distance(m.p2) > 3.0 {
                    self.add_mark(m);
                }
            }
            Drag::None => {}
        }
    }
}

