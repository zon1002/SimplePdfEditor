// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Fu Project

//! 以 tiny-skia 把註記畫成點陣圖：編輯畫面的文字預覽、高亮的相乘混色，以及自動測試的預覽圖。
//! 幾何與存檔共用（line_geometry、layout_text），確保畫面與 PDF 結果一致。

use crate::document::Image;
use crate::fonts::{TEXT_PADDING, layout_text, text_outline};
use crate::geom::{Pt, Rect, Rgba};
use crate::marks::{Mark, MarkType, line_geometry};
use tiny_skia::{BlendMode, FillRule, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Stroke, StrokeDash, Transform};

fn paint(c: Rgba) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color_rgba8(c.r, c.g, c.b, c.a);
    p.anti_alias = true;
    p
}

fn poly_path(pts: &[Pt], origin: Pt, scale: f64) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    for (i, p) in pts.iter().enumerate() {
        let (x, y) = (((p.x - origin.x) * scale) as f32, ((p.y - origin.y) * scale) as f32);
        if i == 0 { pb.move_to(x, y) } else { pb.line_to(x, y) }
    }
    pb.close();
    pb.finish()
}

fn rect_path(r: &Rect, origin: Pt, scale: f64) -> Option<tiny_skia::Path> {
    poly_path(&r.corners(), origin, scale)
}

/// 把一個註記（含位移）畫到 `pix`；`origin` 為畫布左上角對應的頁面座標，`scale` 為像素 / pt
pub fn draw_mark(pix: &mut Pixmap, m: &Mark, origin: Pt, scale: f64) {
    let origin = origin - m.offset;
    let id = Transform::identity();
    match m.kind {
        MarkType::Highlight => {
            let mut p = paint(m.color.with_alpha(255));
            p.blend_mode = BlendMode::Multiply;
            for q in &m.quads {
                if let Some(path) = poly_path(q, origin, scale) {
                    pix.fill_path(&path, &p, FillRule::Winding, id, None);
                }
            }
        }
        MarkType::Underline => {
            for q in &m.quads {
                let (ul, ll, lr) = (q[0], q[3], q[2]);
                let h = ul.distance(ll);
                let t = (h / 14.0).max(0.8);
                let up = (ul - ll) * (t / h.max(0.01)) * 0.5;
                let mut pb = PathBuilder::new();
                let (a, b) = (ll + up - origin, lr + up - origin);
                pb.move_to((a.x * scale) as f32, (a.y * scale) as f32);
                pb.line_to((b.x * scale) as f32, (b.y * scale) as f32);
                if let Some(path) = pb.finish() {
                    let s = Stroke { width: (t * scale) as f32, line_cap: LineCap::Butt, ..Default::default() };
                    pix.stroke_path(&path, &paint(m.color), &s, id, None);
                }
            }
        }
        MarkType::Text => {
            let layout = layout_text(m);
            let bx = layout.content.expand(TEXT_PADDING);
            if let Some(path) = rect_path(&bx, origin, scale) {
                if m.text_fill {
                    pix.fill_path(&path, &paint(m.fill_color), FillRule::Winding, id, None);
                }
                if m.text_border {
                    let s = Stroke { width: (m.border_width * scale) as f32, line_join: LineJoin::Miter, line_cap: LineCap::Square, ..Default::default() };
                    pix.stroke_path(&path, &paint(m.border_color), &s, id, None);
                }
            }
            if let Some(path) = text_outline(m, &layout, origin, scale) {
                pix.fill_path(&path, &paint(m.color.with_alpha(255)), FillRule::Winding, id, None);
            }
        }
        MarkType::Line | MarkType::Arrow => {
            let g = line_geometry(m);
            let mut pb = PathBuilder::new();
            let (a, b) = (g.shaft_start - origin, g.shaft_end - origin);
            pb.move_to((a.x * scale) as f32, (a.y * scale) as f32);
            pb.line_to((b.x * scale) as f32, (b.y * scale) as f32);
            if let Some(path) = pb.finish() {
                let mut s = Stroke { width: (m.width * scale) as f32, line_join: LineJoin::Miter, ..Default::default() };
                if m.dashed {
                    s.line_cap = LineCap::Butt;
                    s.dash = StrokeDash::new(vec![(m.dash_len.max(0.1) * scale) as f32, (m.gap_len.max(0.1) * scale) as f32], 0.0);
                } else {
                    s.line_cap = LineCap::Round;
                }
                pix.stroke_path(&path, &paint(m.color.with_alpha(255)), &s, id, None);
            }
            for h in &g.heads {
                if let Some(path) = poly_path(h, origin, scale) {
                    pix.fill_path(&path, &paint(m.color.with_alpha(255)), FillRule::Winding, id, None);
                }
            }
        }
    }
}

/// 文字註記（含外框、背景）的點陣圖與其涵蓋的頁面範圍（含位移）
pub fn render_text_mark(m: &Mark, scale: f64) -> Option<(Image, Rect)> {
    let pad = if m.text_border { m.border_width } else { 0.0 } + 1.0;
    let area = layout_text(m).content.expand(TEXT_PADDING + pad).translate(m.offset);
    let w = (area.width() * scale).ceil().clamp(1.0, 8192.0) as u32;
    let h = (area.height() * scale).ceil().clamp(1.0, 8192.0) as u32;
    let mut pix = Pixmap::new(w, h)?;
    draw_mark(&mut pix, m, area.top_left(), scale);
    Some((pixmap_to_image(pix), area))
}

/// 預乘 RGBA -> 一般 RGBA
pub fn pixmap_to_image(pix: Pixmap) -> Image {
    let (width, height) = (pix.width(), pix.height());
    let mut rgba = pix.take();
    for px in rgba.chunks_exact_mut(4) {
        let a = px[3] as u32;
        if a > 0 && a < 255 {
            for c in &mut px[..3] {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    Image { width, height, rgba }
}

/// 把高亮以相乘混色疊到頁面影像（不透明 RGBA）上；`scale` 為像素 / pt
pub fn composite_highlights<'a>(img: &mut Image, marks: impl Iterator<Item = &'a Mark>, scale: f64) {
    let Some(size) = tiny_skia::IntSize::from_wh(img.width, img.height) else { return };
    let Some(mut pix) = Pixmap::from_vec(std::mem::take(&mut img.rgba), size) else { return };
    for m in marks.filter(|m| m.kind == MarkType::Highlight) {
        draw_mark(&mut pix, m, Pt::default(), scale);
    }
    img.rgba = pix.take();
}

/// 頁面影像 + 所有註記（自動測試用的預覽圖）
pub fn render_page_with_marks(page: Image, marks: &[Mark], page_no: usize, scale: f64) -> Image {
    let Some(size) = tiny_skia::IntSize::from_wh(page.width, page.height) else { return page };
    let Some(mut pix) = Pixmap::from_vec(page.rgba, size) else { return Image { width: 0, height: 0, rgba: Vec::new() } };
    let order = [MarkType::Highlight, MarkType::Underline, MarkType::Line, MarkType::Arrow, MarkType::Text];
    for kind in order {
        for m in marks.iter().filter(|m| m.page == page_no && m.kind == kind) {
            draw_mark(&mut pix, m, Pt::default(), scale);
        }
    }
    pixmap_to_image(pix)
}

/// 存成 PNG（測試用）
pub fn save_png(img: &Image, path: &str) -> Result<(), String> {
    let size = tiny_skia::IntSize::from_wh(img.width, img.height).ok_or("影像大小錯誤")?;
    let mut data = img.rgba.clone();
    // tiny-skia 需要預乘資料；頁面影像為不透明，直接使用
    for px in data.chunks_exact_mut(4) {
        let a = px[3] as u32;
        for c in &mut px[..3] {
            *c = (*c as u32 * a / 255) as u8;
        }
    }
    let pix = Pixmap::from_vec(data, size).ok_or("影像資料錯誤")?;
    pix.save_png(path).map_err(|e| e.to_string())
}
