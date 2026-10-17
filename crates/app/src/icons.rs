// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Fufeng Project

//! 工具列圖示：全部以程式繪製（不需外部圖檔），提供多種風格切換。
//! 以 24×24 的設計座標繪製，再依實際像素大小縮放。

use tiny_skia::{FillRule, LineCap, LineJoin, Paint, Path, PathBuilder, Pixmap, Stroke, Transform};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IconId {
    Open,
    Save,
    Export,
    Select,
    Highlight,
    Underline,
    Text,
    Line,
    Arrow,
    Undo,
    Redo,
    Prev,
    Next,
    ZoomOut,
    ZoomIn,
    FitWidth,
    Print,
    Help,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IconStyle {
    /// 線條：單色細線，簡潔
    Outline,
    /// 實心：單色填滿，粗體醒目
    Filled,
    /// 彩色：每個工具有代表色
    Color,
    /// 雙色：線條＋淡色填底，統一藍色調
    Duotone,
}

impl IconStyle {
    pub const ALL: [IconStyle; 4] = [IconStyle::Color, IconStyle::Outline, IconStyle::Filled, IconStyle::Duotone];

    pub fn name(self) -> &'static str {
        match self {
            IconStyle::Outline => "線條（簡潔單色）",
            IconStyle::Filled => "實心（粗體醒目）",
            IconStyle::Color => "彩色（各工具代表色）",
            IconStyle::Duotone => "雙色（藍色調）",
        }
    }
    pub fn to_index(self) -> u8 {
        match self {
            IconStyle::Outline => 0,
            IconStyle::Filled => 1,
            IconStyle::Color => 2,
            IconStyle::Duotone => 3,
        }
    }
    pub fn from_index(i: u8) -> Self {
        match i {
            0 => IconStyle::Outline,
            1 => IconStyle::Filled,
            3 => IconStyle::Duotone,
            _ => IconStyle::Color,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
struct C(u8, u8, u8, u8);

impl C {
    fn rgb(r: u8, g: u8, b: u8) -> C {
        C(r, g, b, 255)
    }
    fn alpha(self, a: u8) -> C {
        C(self.0, self.1, self.2, a)
    }
    /// 與 Qt 的 QColor::lighter / darker 相同的意義（以 HSV 的 V 調整）
    fn scaled(self, factor: f32) -> C {
        let f = |v: u8| ((v as f32 * factor).round().clamp(0.0, 255.0)) as u8;
        C(f(self.0), f(self.1), f(self.2), self.3)
    }
    fn paint(self) -> Paint<'static> {
        let mut p = Paint::default();
        p.set_color_rgba8(self.0, self.1, self.2, self.3);
        p.anti_alias = true;
        p
    }
}

#[derive(Clone, Copy)]
struct Pen {
    color: C,
    width: f32,
}

impl Pen {
    fn thicker(self, add: f32) -> Pen {
        Pen { width: self.width + add, ..self }
    }
}

/// 單一圖示的畫筆組合：edge = 外框線、body = 主體填色、detail = 主體內的細節線、solid = 實心小部件
struct Ink {
    edge: Pen,
    body: Option<C>,
    detail: Pen,
    solid: C,
    /// 高亮圖示底下的螢光帶
    band: C,
    /// 底線圖示的底線顏色
    mark: C,
    /// 細線圖示（文字、線條）額外加粗
    boost: f32,
    style: IconStyle,
}

fn accent_of(id: IconId) -> C {
    match id {
        IconId::Open => C::rgb(0xE8, 0xA3, 0x17),
        IconId::Save => C::rgb(0x3B, 0x82, 0xF6),
        IconId::Export => C::rgb(0x8B, 0x5C, 0xF6),
        IconId::Select => C::rgb(0x64, 0x74, 0x8B),
        IconId::Highlight => C::rgb(0xEA, 0xB3, 0x08),
        IconId::Underline => C::rgb(0xEF, 0x44, 0x44),
        IconId::Text => C::rgb(0x0E, 0xA5, 0xE9),
        IconId::Line => C::rgb(0x10, 0xB9, 0x81),
        IconId::Arrow => C::rgb(0xF9, 0x73, 0x16),
        IconId::Undo | IconId::Redo => C::rgb(0x63, 0x66, 0xF1),
        IconId::Prev | IconId::Next => C::rgb(0x64, 0x74, 0x8B),
        IconId::ZoomOut | IconId::ZoomIn | IconId::FitWidth => C::rgb(0x14, 0xB8, 0xA6),
        IconId::Print => C::rgb(0x64, 0x74, 0x8B),
        IconId::Help => C::rgb(0x22, 0xC5, 0x5E),
    }
}

fn ink_for(id: IconId, style: IconStyle, dark: bool) -> Ink {
    let (fg, bg) = if dark { (C::rgb(0xE6, 0xE6, 0xE6), C::rgb(0x2B, 0x2B, 0x2B)) } else { (C::rgb(0x1A, 0x1A, 0x1A), C::rgb(0xF5, 0xF5, 0xF5)) };
    let pen = |c: C, w: f32| Pen { color: c, width: w };
    match style {
        IconStyle::Outline => Ink { edge: pen(fg, 1.7), body: None, detail: pen(fg, 1.7), solid: fg, band: fg.alpha(110), mark: fg, boost: 0.4, style },
        // 填滿後細節用背景色「挖空」
        IconStyle::Filled => Ink { edge: pen(fg, 2.0), body: Some(fg), detail: pen(bg, 1.8), solid: fg, band: fg.alpha(150), mark: fg, boost: 0.8, style },
        IconStyle::Color => {
            let a = accent_of(id);
            let line = if dark { a.scaled(1.2) } else { a.scaled(0.8) };
            Ink {
                edge: pen(line, 1.7),
                body: Some(a.alpha(if dark { 95 } else { 70 })),
                detail: pen(line, 1.7),
                solid: line,
                band: C::rgb(0xFA, 0xCC, 0x15),
                mark: accent_of(IconId::Underline),
                boost: 0.4,
                style,
            }
        }
        IconStyle::Duotone => {
            let a = if dark { C::rgb(0x60, 0xA5, 0xFA) } else { C::rgb(0x25, 0x63, 0xEB) };
            Ink { edge: pen(a, 1.7), body: Some(a.alpha(if dark { 70 } else { 45 })), detail: pen(a, 1.7), solid: a, band: a.alpha(120), mark: a, boost: 0.4, style }
        }
    }
}

// ---------------------------------------------------------------- 路徑工具

/// 以 Qt 的 arcTo 慣例加入弧線（角度以度為單位，0° 在 3 點鐘方向，正值為逆時針），
/// 並從目前位置畫線到弧的起點。
fn arc_to(pb: &mut PathBuilder, x: f32, y: f32, w: f32, h: f32, start: f32, sweep: f32) {
    let (cx, cy, rx, ry) = (x + w / 2.0, y + h / 2.0, w / 2.0, h / 2.0);
    let pt = |deg: f32| {
        let r = deg.to_radians();
        (cx + rx * r.cos(), cy - ry * r.sin())
    };
    let (sx, sy) = pt(start);
    if pb.is_empty() {
        pb.move_to(sx, sy);
    } else {
        pb.line_to(sx, sy);
    }
    let segs = (sweep.abs() / 90.0).ceil().max(1.0) as i32;
    let step = sweep / segs as f32;
    for i in 0..segs {
        let a0 = (start + step * i as f32).to_radians();
        let a1 = (start + step * (i + 1) as f32).to_radians();
        let k = 4.0 / 3.0 * ((a1 - a0) / 4.0).tan();
        let (c0, s0, c1, s1) = (a0.cos(), a0.sin(), a1.cos(), a1.sin());
        pb.cubic_to(
            cx + rx * (c0 - k * s0),
            cy - ry * (s0 + k * c0),
            cx + rx * (c1 + k * s1),
            cy - ry * (s1 - k * c1),
            cx + rx * c1,
            cy - ry * s1,
        );
    }
}

fn ellipse(cx: f32, cy: f32, rx: f32, ry: f32) -> Option<Path> {
    let mut pb = PathBuilder::new();
    pb.push_oval(tiny_skia::Rect::from_xywh(cx - rx, cy - ry, rx * 2.0, ry * 2.0)?);
    pb.finish()
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> Option<Path> {
    let mut pb = PathBuilder::new();
    pb.push_rect(tiny_skia::Rect::from_xywh(x, y, w, h)?);
    pb.finish()
}

fn rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<Path> {
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.quad_to(x + w, y, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.quad_to(x + w, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.quad_to(x, y + h, x, y + h - r);
    pb.line_to(x, y + r);
    pb.quad_to(x, y, x + r, y);
    pb.close();
    pb.finish()
}

fn poly(pts: &[(f32, f32)], close: bool) -> Option<Path> {
    let mut pb = PathBuilder::new();
    for (i, &(x, y)) in pts.iter().enumerate() {
        if i == 0 { pb.move_to(x, y) } else { pb.line_to(x, y) }
    }
    if close {
        pb.close();
    }
    pb.finish()
}

struct Canvas<'a> {
    pix: &'a mut Pixmap,
    t: Transform,
}

impl Canvas<'_> {
    fn fill(&mut self, path: &Option<Path>, c: C) {
        if let Some(p) = path {
            self.pix.fill_path(p, &c.paint(), FillRule::Winding, self.t, None);
        }
    }
    fn stroke(&mut self, path: &Option<Path>, pen: Pen) {
        self.stroke_cap(path, pen, LineCap::Round);
    }
    fn stroke_cap(&mut self, path: &Option<Path>, pen: Pen, cap: LineCap) {
        if let Some(p) = path {
            let s = Stroke { width: pen.width, line_cap: cap, line_join: LineJoin::Round, ..Default::default() };
            self.pix.stroke_path(p, &pen.color.paint(), &s, self.t, None);
        }
    }
    /// 外框線 + 主體填色
    fn shape(&mut self, path: &Option<Path>, k: &Ink) {
        if let Some(b) = k.body {
            self.fill(path, b);
        }
        self.stroke(path, k.edge);
    }
}

fn draw_undo(cv: &mut Canvas<'_>, k: &Ink) {
    let mut pb = PathBuilder::new();
    pb.move_to(4.5, 10.0);
    pb.line_to(14.5, 10.0);
    pb.cubic_to(21.5, 10.0, 21.5, 20.0, 14.5, 20.0);
    pb.line_to(9.0, 20.0);
    cv.stroke(&pb.finish(), k.edge.thicker(k.boost));
    cv.stroke(&poly(&[(8.5, 5.5), (4.0, 10.0), (8.5, 14.5)], false), k.edge.thicker(k.boost));
}

fn draw_zoom(cv: &mut Canvas<'_>, k: &Ink, plus: bool) {
    cv.shape(&ellipse(10.0, 10.0, 6.5, 6.5), k);
    cv.stroke(&poly(&[(15.0, 15.0), (20.5, 20.5)], false), k.edge.thicker(1.2));
    cv.stroke(&poly(&[(7.0, 10.0), (13.0, 10.0)], false), k.detail);
    if plus {
        cv.stroke(&poly(&[(10.0, 7.0), (10.0, 13.0)], false), k.detail);
    }
}

fn draw_icon(cv: &mut Canvas<'_>, id: IconId, k: &Ink) {
    match id {
        IconId::Open => {
            let mut pb = PathBuilder::new();
            pb.move_to(3.0, 6.5);
            pb.line_to(3.0, 19.0);
            pb.quad_to(3.0, 20.0, 4.0, 20.0);
            pb.line_to(20.0, 20.0);
            pb.quad_to(21.0, 20.0, 21.0, 19.0);
            pb.line_to(21.0, 9.0);
            pb.quad_to(21.0, 8.0, 20.0, 8.0);
            pb.line_to(11.5, 8.0);
            pb.line_to(9.5, 5.5);
            pb.line_to(4.0, 5.5);
            pb.quad_to(3.0, 5.5, 3.0, 6.5);
            pb.close();
            cv.shape(&pb.finish(), k);
            cv.stroke(&poly(&[(3.0, 11.0), (21.0, 11.0)], false), k.detail);
        }
        IconId::Save => {
            cv.shape(&rounded_rect(4.0, 4.0, 16.0, 16.0, 2.2), k);
            cv.stroke(&rect(8.0, 4.0, 8.0, 5.0), k.detail);
            cv.stroke(&rounded_rect(7.0, 13.0, 10.0, 7.0, 1.0), k.detail);
        }
        IconId::Export => {
            cv.shape(&poly(&[(4.0, 3.0), (12.5, 3.0), (16.0, 6.5), (16.0, 11.0), (11.0, 11.0), (11.0, 20.0), (4.0, 20.0)], true), k);
            cv.stroke(&poly(&[(7.0, 8.0), (12.0, 8.0)], false), k.detail);
            let mut pb = PathBuilder::new();
            pb.move_to(14.0, 14.0);
            pb.line_to(14.0, 12.5);
            arc_to(&mut pb, 14.0, 9.5, 6.0, 6.0, 180.0, -180.0);
            pb.line_to(20.0, 14.0);
            cv.stroke(&pb.finish(), k.edge);
            cv.fill(&rounded_rect(12.5, 14.0, 9.0, 7.0, 1.5), k.solid);
        }
        IconId::Select => {
            cv.shape(&poly(&[(6.0, 3.0), (6.0, 19.0), (10.3, 15.2), (13.3, 21.5), (16.2, 20.2), (13.2, 14.0), (19.0, 14.0)], true), k);
        }
        IconId::Highlight => {
            cv.fill(&rounded_rect(3.0, 19.5, 18.0, 3.0, 1.0), k.band);
            cv.shape(&poly(&[(13.5, 3.0), (20.5, 10.0), (12.0, 18.5), (5.0, 11.5)], true), k);
            cv.fill(&poly(&[(5.0, 11.5), (12.0, 18.5), (8.5, 19.0), (4.5, 15.0)], true), k.solid);
        }
        IconId::Underline => {
            let mut pb = PathBuilder::new();
            pb.move_to(7.0, 4.0);
            pb.line_to(7.0, 11.0);
            arc_to(&mut pb, 7.0, 6.0, 10.0, 10.0, 180.0, 180.0);
            pb.line_to(17.0, 4.0);
            cv.stroke(&pb.finish(), k.edge.thicker(k.boost));
            let bar = Pen { color: k.mark, ..k.edge.thicker(0.8) };
            cv.stroke(&poly(&[(5.0, 20.0), (19.0, 20.0)], false), bar);
        }
        IconId::Text => {
            let pen = k.edge.thicker(k.boost + 0.4);
            cv.stroke(&poly(&[(5.0, 6.5), (5.0, 4.0), (19.0, 4.0), (19.0, 6.5)], false), pen);
            cv.stroke(&poly(&[(12.0, 4.0), (12.0, 20.0)], false), pen);
            cv.stroke(&poly(&[(9.0, 20.0), (15.0, 20.0)], false), pen);
        }
        IconId::Line => {
            cv.stroke(&poly(&[(5.0, 19.0), (19.0, 5.0)], false), k.edge.thicker(k.boost + 0.4));
            cv.fill(&ellipse(5.0, 19.0, 1.8, 1.8), k.solid);
            cv.fill(&ellipse(19.0, 5.0, 1.8, 1.8), k.solid);
        }
        IconId::Arrow => {
            cv.stroke(&poly(&[(5.0, 19.0), (15.5, 8.5)], false), k.edge.thicker(k.boost + 0.4));
            let head = poly(&[(19.5, 4.5), (9.5, 7.5), (16.5, 14.5)], true);
            cv.fill(&head, k.solid);
            cv.stroke(&head, Pen { color: k.solid, width: 1.2 });
        }
        IconId::Undo => draw_undo(cv, k),
        IconId::Redo => {
            let saved = cv.t;
            cv.t = saved.pre_translate(24.0, 0.0).pre_scale(-1.0, 1.0);
            draw_undo(cv, k);
            cv.t = saved;
        }
        IconId::Prev => cv.stroke(&poly(&[(15.0, 5.0), (8.0, 12.0), (15.0, 19.0)], false), k.edge.thicker(0.8)),
        IconId::Next => cv.stroke(&poly(&[(9.0, 5.0), (16.0, 12.0), (9.0, 19.0)], false), k.edge.thicker(0.8)),
        IconId::ZoomOut => draw_zoom(cv, k, false),
        IconId::ZoomIn => draw_zoom(cv, k, true),
        IconId::FitWidth => {
            cv.shape(&rounded_rect(3.0, 5.0, 18.0, 14.0, 1.5), k);
            cv.stroke(&poly(&[(6.5, 12.0), (17.5, 12.0)], false), k.detail);
            cv.stroke(&poly(&[(9.0, 9.5), (6.5, 12.0), (9.0, 14.5)], false), k.detail);
            cv.stroke(&poly(&[(15.0, 9.5), (17.5, 12.0), (15.0, 14.5)], false), k.detail);
        }
        IconId::Print => {
            cv.stroke(&rect(7.0, 3.0, 10.0, 5.0), k.edge);
            cv.shape(&rounded_rect(3.0, 8.0, 18.0, 9.0, 2.0), k);
            cv.fill(&ellipse(17.5, 11.0, 1.0, 1.0), k.detail.color);
            let paper = rect(7.0, 13.5, 10.0, 7.5);
            if k.style == IconStyle::Filled {
                cv.fill(&paper, k.detail.color);
            }
            cv.stroke(&paper, k.edge);
            let mut pb = PathBuilder::new();
            pb.move_to(9.0, 16.5);
            pb.line_to(15.0, 16.5);
            pb.move_to(9.0, 19.0);
            pb.line_to(13.0, 19.0);
            cv.stroke(&pb.finish(), Pen { color: k.edge.color, width: 1.2 });
        }
        IconId::Help => {
            cv.shape(&ellipse(12.0, 12.0, 9.0, 9.0), k);
            let mut pb = PathBuilder::new();
            pb.move_to(9.2, 9.5);
            pb.quad_to(9.2, 6.6, 12.0, 6.6);
            pb.quad_to(14.8, 6.6, 14.8, 9.2);
            pb.quad_to(14.8, 11.0, 12.8, 12.0);
            pb.quad_to(12.0, 12.5, 12.0, 14.0);
            cv.stroke(&pb.finish(), k.detail.thicker(0.3));
            cv.fill(&ellipse(12.0, 17.2, 1.25, 1.25), k.detail.color);
        }
    }
}

/// 畫出圖示，回傳 RGBA（非預乘）
pub fn render_icon(id: IconId, style: IconStyle, dark: bool, size: u32) -> Option<(u32, Vec<u8>)> {
    let mut pix = Pixmap::new(size, size)?;
    let s = size as f32 / 24.0;
    let ink = ink_for(id, style, dark);
    let mut cv = Canvas { pix: &mut pix, t: Transform::from_scale(s, s) };
    draw_icon(&mut cv, id, &ink);
    let img = pdfcore::raster::pixmap_to_image(pix);
    Some((size, img.rgba))
}
