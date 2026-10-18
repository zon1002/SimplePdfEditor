// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Fufeng Project

//! 頁面檢視四周的尺規（只供編輯時對位，不會寫入 PDF）。
//! 原點為頁面左上角；刻度隨縮放自動調整密度。

use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Stroke, pos2};

pub const THICKNESS: f32 = 24.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RulerUnit {
    Millimeter,
    Centimeter,
    Inch,
    Point,
}

impl RulerUnit {
    pub const ALL: [RulerUnit; 4] = [RulerUnit::Millimeter, RulerUnit::Centimeter, RulerUnit::Inch, RulerUnit::Point];

    /// 「公釐 (mm)」等，供選單顯示
    pub fn name(self) -> &'static str {
        match self {
            RulerUnit::Millimeter => "公釐 (mm)",
            RulerUnit::Centimeter => "公分 (cm)",
            RulerUnit::Inch => "英吋 (in)",
            RulerUnit::Point => "點 (pt)",
        }
    }
    /// 「mm」等，供刻度角落與座標顯示
    pub fn suffix(self) -> &'static str {
        match self {
            RulerUnit::Millimeter => "mm",
            RulerUnit::Centimeter => "cm",
            RulerUnit::Inch => "in",
            RulerUnit::Point => "pt",
        }
    }
    /// 1 單位 = 幾 pt
    pub fn points(self) -> f64 {
        match self {
            RulerUnit::Millimeter => 72.0 / 25.4,
            RulerUnit::Centimeter => 72.0 / 2.54,
            RulerUnit::Inch => 72.0,
            RulerUnit::Point => 1.0,
        }
    }
}

/// 依縮放挑選主刻度間距（單位數）：1、2、5 × 10^k，使標籤間距至少 min_px 像素
fn nice_step(px_per_unit: f64, min_px: f64) -> f64 {
    let mut step = 10f64.powf((min_px / px_per_unit).log10().floor());
    for _ in 0..64 {
        for m in [1.0, 2.0, 5.0] {
            if step * m * px_per_unit >= min_px {
                return step * m;
            }
        }
        step *= 10.0;
    }
    step
}

fn label(v: f64) -> String {
    // 去掉浮點誤差；小於 1 的步距保留一位小數
    let r = (v * 10.0).round() / 10.0;
    if (r - r.round()).abs() < 1e-6 { format!("{}", r.round() as i64) } else { format!("{r:.1}") }
}

pub struct RulerColors {
    pub outside: Color32,
    pub inside: Color32,
    pub tick: Color32,
    pub edge: Color32,
}

/// 畫一條尺規。`rect` 為尺規區域；`origin` 為頁面原點在螢幕上的座標（沿尺規方向）；
/// `px_per_pt` 為每 pt 的點數；`page_len` 為頁面長度（pt）；`cursor` 為滑鼠位置（沿尺規方向）
#[allow(clippy::too_many_arguments)]
pub fn paint_ruler(
    painter: &Painter,
    rect: Rect,
    horizontal: bool,
    origin: f32,
    px_per_pt: f64,
    page_len: f64,
    unit: RulerUnit,
    cursor: Option<f32>,
    colors: &RulerColors,
) {
    let painter = painter.with_clip_rect(rect);
    let (start, len, thick) = if horizontal { (rect.left(), rect.width(), rect.height()) } else { (rect.top(), rect.height(), rect.width()) };
    // 以「沿尺規方向的座標 a、垂直方向的座標 b」繪製，水平與垂直共用同一段程式
    let pt = |a: f32, b: f32| -> Pos2 { if horizontal { pos2(a, rect.top() + b) } else { pos2(rect.left() + b, a) } };
    painter.rect_filled(rect, 0.0, colors.outside);
    let page_px = (page_len * px_per_pt) as f32;
    let inside = if horizontal {
        Rect::from_min_max(pos2(origin, rect.top()), pos2(origin + page_px, rect.bottom()))
    } else {
        Rect::from_min_max(pos2(rect.left(), origin), pos2(rect.right(), origin + page_px))
    };
    painter.rect_filled(inside.intersect(rect), 0.0, colors.inside);

    let px_per_unit = px_per_pt * unit.points();
    if px_per_unit > 0.0 && px_per_unit.is_finite() {
        let major = nice_step(px_per_unit, 56.0);
        let mut minor = major / 10.0;
        if minor * px_per_unit < 5.0 {
            minor = major / 5.0;
        }
        if minor * px_per_unit < 5.0 {
            minor = major / 2.0;
        }
        let stroke = Stroke::new(1.0, colors.tick);
        let font = FontId::proportional(10.0);
        // 以整數索引走刻度，避免浮點累加誤差讓主刻度判斷失準
        let per_major = (major / minor).round().max(1.0) as i64;
        let rel0 = (start - origin) as f64;
        let i_first = (rel0 / px_per_unit / minor).floor() as i64;
        let i_last = ((rel0 + len as f64) / px_per_unit / minor).ceil() as i64;
        if i_last - i_first < 5000 {
            for i in i_first..=i_last {
                let u = i as f64 * minor;
                let a = (origin as f64 + u * px_per_unit).round() as f32 + 0.5;
                let m = i.rem_euclid(per_major);
                let is_major = m == 0;
                let is_half = !is_major && per_major % 2 == 0 && per_major > 2 && m == per_major / 2;
                let h = thick * if is_major { 0.55 } else if is_half { 0.38 } else { 0.22 };
                painter.line_segment([pt(a, thick - h), pt(a, thick)], stroke);
                if is_major {
                    let s = label(u);
                    if horizontal {
                        painter.text(pt(a + 3.0, 1.0), Align2::LEFT_TOP, s, font.clone(), colors.tick);
                    } else {
                        // 垂直尺規的數字逐字直排
                        let mut y = a + 3.0;
                        for ch in s.chars() {
                            painter.text(pt(y, 2.0), Align2::LEFT_TOP, ch, font.clone(), colors.tick);
                            y += 9.0;
                        }
                    }
                }
            }
        }
    }
    // 與頁面相接的邊框線
    painter.line_segment([pt(start, thick - 0.5), pt(start + len, thick - 0.5)], Stroke::new(1.0, colors.edge));
    // 滑鼠位置
    if let Some(c) = cursor {
        let a = c.round() + 0.5;
        painter.line_segment([pt(a, 0.0), pt(a, thick)], Stroke::new(1.0, Color32::from_rgb(0xE5, 0x39, 0x35)));
    }
}
