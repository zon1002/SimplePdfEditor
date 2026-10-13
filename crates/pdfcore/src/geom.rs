// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Fu Project

//! 基本幾何型別。座標一律為「頁面檢視座標」：單位 pt，原點在頁面左上角、y 向下，已套用頁面旋轉。

use std::ops::{Add, Mul, Neg, Sub};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pt {
    pub x: f64,
    pub y: f64,
}

impl Pt {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
    pub fn length(self) -> f64 {
        self.x.hypot(self.y)
    }
    pub fn distance(self, o: Pt) -> f64 {
        (self - o).length()
    }
}

impl Add for Pt {
    type Output = Pt;
    fn add(self, o: Pt) -> Pt {
        Pt::new(self.x + o.x, self.y + o.y)
    }
}

impl Sub for Pt {
    type Output = Pt;
    fn sub(self, o: Pt) -> Pt {
        Pt::new(self.x - o.x, self.y - o.y)
    }
}

impl Mul<f64> for Pt {
    type Output = Pt;
    fn mul(self, k: f64) -> Pt {
        Pt::new(self.x * k, self.y * k)
    }
}

impl Neg for Pt {
    type Output = Pt;
    fn neg(self) -> Pt {
        Pt::new(-self.x, -self.y)
    }
}

/// 軸對齊矩形（x0 ≤ x1、y0 ≤ y1 時為有效）。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

impl Rect {
    pub const EMPTY: Rect = Rect { x0: f64::INFINITY, y0: f64::INFINITY, x1: f64::NEG_INFINITY, y1: f64::NEG_INFINITY };

    pub const fn new(x0: f64, y0: f64, x1: f64, y1: f64) -> Self {
        Self { x0, y0, x1, y1 }
    }
    pub fn from_points(a: Pt, b: Pt) -> Self {
        Self::new(a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y))
    }
    pub fn from_pos_size(p: Pt, w: f64, h: f64) -> Self {
        Self::new(p.x, p.y, p.x + w, p.y + h)
    }
    pub fn width(&self) -> f64 {
        self.x1 - self.x0
    }
    pub fn height(&self) -> f64 {
        self.y1 - self.y0
    }
    pub fn is_empty(&self) -> bool {
        !(self.x1 > self.x0 && self.y1 > self.y0)
    }
    pub fn include(&mut self, p: Pt) {
        self.x0 = self.x0.min(p.x);
        self.y0 = self.y0.min(p.y);
        self.x1 = self.x1.max(p.x);
        self.y1 = self.y1.max(p.y);
    }
    pub fn union(mut self, o: &Rect) -> Rect {
        self.x0 = self.x0.min(o.x0);
        self.y0 = self.y0.min(o.y0);
        self.x1 = self.x1.max(o.x1);
        self.y1 = self.y1.max(o.y1);
        self
    }
    pub fn expand(&self, d: f64) -> Rect {
        Rect::new(self.x0 - d, self.y0 - d, self.x1 + d, self.y1 + d)
    }
    pub fn translate(&self, d: Pt) -> Rect {
        Rect::new(self.x0 + d.x, self.y0 + d.y, self.x1 + d.x, self.y1 + d.y)
    }
    pub fn contains(&self, p: Pt) -> bool {
        p.x >= self.x0 && p.x <= self.x1 && p.y >= self.y0 && p.y <= self.y1
    }
    pub fn top_left(&self) -> Pt {
        Pt::new(self.x0, self.y0)
    }
    pub fn center(&self) -> Pt {
        Pt::new((self.x0 + self.x1) / 2.0, (self.y0 + self.y1) / 2.0)
    }
    /// 四個角：左上、右上、右下、左下
    pub fn corners(&self) -> Quad {
        [Pt::new(self.x0, self.y0), Pt::new(self.x1, self.y0), Pt::new(self.x1, self.y1), Pt::new(self.x0, self.y1)]
    }
}

/// 文字選取的四邊形，依序為 ul、ur、lr、ll。
pub type Quad = [Pt; 4];

pub fn quad_bounds(q: &Quad) -> Rect {
    let mut r = Rect::EMPTY;
    for p in q {
        r.include(*p);
    }
    r
}

/// 點是否在凸多邊形內（多邊形頂點順序不拘）。
pub fn point_in_polygon(p: Pt, poly: &[Pt]) -> bool {
    let n = poly.len();
    if n < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a.y > p.y) != (b.y > p.y) {
            let t = (p.y - a.y) / (b.y - a.y);
            if p.x < a.x + t * (b.x - a.x) {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

/// 點到線段的距離
pub fn distance_to_segment(p: Pt, a: Pt, b: Pt) -> f64 {
    let d = b - a;
    let len2 = d.x * d.x + d.y * d.y;
    if len2 <= 1e-12 {
        return p.distance(a);
    }
    let t = (((p.x - a.x) * d.x + (p.y - a.y) * d.y) / len2).clamp(0.0, 1.0);
    p.distance(a + d * t)
}

/// sRGB 顏色（含透明度）
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
    pub fn with_alpha(self, a: u8) -> Self {
        Self { a, ..self }
    }
    pub fn rf(&self) -> f64 {
        self.r as f64 / 255.0
    }
    pub fn gf(&self) -> f64 {
        self.g as f64 / 255.0
    }
    pub fn bf(&self) -> f64 {
        self.b as f64 / 255.0
    }
    pub fn af(&self) -> f64 {
        self.a as f64 / 255.0
    }
    /// "#RRGGBB"（不含透明度）
    pub fn hex(&self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

/// 2D 仿射轉換 [a b c d e f]：x' = a·x + c·y + e，y' = b·x + d·y + f（與 PDF 的 cm 相同）
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine(pub [f64; 6]);

impl Affine {
    pub const IDENTITY: Affine = Affine([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);

    pub fn apply(&self, p: Pt) -> Pt {
        let [a, b, c, d, e, f] = self.0;
        Pt::new(a * p.x + c * p.y + e, b * p.x + d * p.y + f)
    }
    pub fn inverse(&self) -> Affine {
        let [a, b, c, d, e, f] = self.0;
        let det = a * d - b * c;
        if det.abs() < 1e-12 {
            return Affine::IDENTITY;
        }
        let inv = 1.0 / det;
        Affine([d * inv, -b * inv, -c * inv, a * inv, (c * f - d * e) * inv, (b * e - a * f) * inv])
    }
    /// 線寬等長度的縮放係數（旋轉／翻轉不影響）
    pub fn scale_factor(&self) -> f64 {
        let [a, b, c, d, _, _] = self.0;
        (a * d - b * c).abs().sqrt()
    }
    pub fn transform_rect(&self, r: &Rect) -> Rect {
        let mut out = Rect::EMPTY;
        for p in r.corners() {
            out.include(self.apply(p));
        }
        out
    }
}
