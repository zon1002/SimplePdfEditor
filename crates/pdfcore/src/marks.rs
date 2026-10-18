// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Fufeng Project

//! 使用者在頁面上新增的「註記」資料模型。
//! 所有座標都以頁面檢視座標（pt，原點左上、y 向下）表示。
//! 編輯期間只存在於記憶體，儲存時才寫入 PDF（見 save.rs）。

use crate::geom::{Pt, Quad, Rgba};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MarkType {
    Highlight,
    Underline,
    Text,
    Line,
    Arrow,
    /// 矩形框（中間鏤空，不遮蓋內容）；radius > 0 時四角為圓弧
    Rect,
    /// 橢圓／圓形框（中間鏤空）
    Ellipse,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    pub kind: MarkType,
    pub page: usize,
    pub color: Rgba,
    /// 使用者拖曳後的位移
    pub offset: Pt,

    // Highlight / Underline：每個 quad 依序為 ul, ur, lr, ll
    pub quads: Vec<Quad>,

    // Text
    pub text: String,
    /// 字型檔（嵌入 PDF 用）與字型集合（.ttc）中的索引
    pub font_path: String,
    pub font_index: u32,
    pub font_size: f64,
    /// 文字方塊左上角
    pub pos: Pt,
    pub text_border: bool,
    pub border_color: Rgba,
    pub border_width: f64,
    /// 背景色（可含透明度）
    pub text_fill: bool,
    pub fill_color: Rgba,

    // Line / Arrow
    pub p1: Pt,
    pub p2: Pt,
    pub width: f64,
    /// 箭頭長度（pt），寬度為長度的一半
    pub arrow_size: f64,
    /// 雙向箭頭（起點也畫箭頭）
    pub both_ends: bool,
    pub dashed: bool,
    /// 虛線線段長與間隔（pt）
    pub dash_len: f64,
    pub gap_len: f64,

    // Rect / Ellipse：p1、p2 為拖曳的兩個對角；width、dashed 等線型屬性與線條共用
    /// 矩形四角的圓弧半徑（pt），0 = 直角
    pub radius: f64,
}

impl Default for Mark {
    fn default() -> Self {
        Self {
            kind: MarkType::Highlight,
            page: 0,
            color: Rgba::rgb(0xFF, 0xFF, 0x00),
            offset: Pt::default(),
            quads: Vec::new(),
            text: String::new(),
            font_path: String::new(),
            font_index: 0,
            font_size: 14.0,
            pos: Pt::default(),
            text_border: false,
            border_color: Rgba::rgb(0x1A, 0x1A, 0x1A),
            border_width: 1.0,
            text_fill: false,
            fill_color: Rgba::rgb(0xFF, 0xF5, 0x9D),
            p1: Pt::default(),
            p2: Pt::default(),
            width: 2.0,
            arrow_size: 14.0,
            both_ends: false,
            dashed: false,
            dash_len: 6.0,
            gap_len: 4.0,
            radius: 0.0,
        }
    }
}

impl Mark {
    pub fn is_line(&self) -> bool {
        matches!(self.kind, MarkType::Line | MarkType::Arrow)
    }
    /// 矩形或橢圓框
    pub fn is_shape(&self) -> bool {
        matches!(self.kind, MarkType::Rect | MarkType::Ellipse)
    }
    /// 有線寬、線型（實線／虛線）的註記
    pub fn is_stroked(&self) -> bool {
        self.is_line() || self.is_shape()
    }
}

/// 線條／箭頭的幾何（不含位移）：預覽與寫入 PDF 共用，確保兩者一致
#[derive(Clone, Debug, Default)]
pub struct LineGeometry {
    pub shaft_start: Pt,
    pub shaft_end: Pt,
    /// 箭頭三角形（尖端、兩翼）；線條為空，雙向箭頭有兩個
    pub heads: Vec<[Pt; 3]>,
}

pub fn line_geometry(m: &Mark) -> LineGeometry {
    let mut g = LineGeometry { shaft_start: m.p1, shaft_end: m.p2, heads: Vec::new() };
    let d = m.p2 - m.p1;
    let len = d.length();
    if m.kind == MarkType::Arrow && len > 0.01 {
        let dir = d * (1.0 / len);
        let nrm = Pt::new(-dir.y, dir.x);
        // 雙向時兩個箭頭各最多佔一半長度，避免重疊
        let l = m.arrow_size.min(if m.both_ends { len / 2.0 } else { len });
        let head_at = |tip: Pt, toward: Pt| {
            let base = tip + toward * l;
            [tip, base + nrm * (l * 0.5), base - nrm * (l * 0.5)]
        };
        g.heads.push(head_at(m.p2, -dir));
        // 線身伸入箭頭內一點，避免與箭頭間出現縫隙，但不超出尖端
        g.shaft_end = m.p2 - dir * (l * 0.8);
        if m.both_ends {
            g.heads.push(head_at(m.p1, dir));
            g.shaft_start = m.p1 + dir * (l * 0.8);
        }
    }
    g
}

/// 路徑片段（頁面座標）：預覽、存檔與點選判斷共用同一份外框幾何
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Seg {
    Move(Pt),
    Line(Pt),
    /// 三次貝茲曲線：兩個控制點與終點
    Cubic(Pt, Pt, Pt),
    Close,
}

/// 以四段貝茲曲線近似 90° 圓弧的控制點係數
const KAPPA: f64 = 0.552_284_749_830_793_4;

/// 框的範圍（不含位移）
pub fn shape_rect(m: &Mark) -> crate::geom::Rect {
    crate::geom::Rect::from_points(m.p1, m.p2)
}

/// 實際使用的圓角半徑：不超過短邊的一半
pub fn corner_radius(m: &Mark) -> f64 {
    let r = shape_rect(m);
    if m.kind != MarkType::Rect || !m.radius.is_finite() {
        return 0.0;
    }
    m.radius.clamp(0.0, r.width().min(r.height()) / 2.0)
}

/// 矩形（可圓角）或橢圓的外框路徑（不含位移），順時針
pub fn shape_path(m: &Mark) -> Vec<Seg> {
    let r = shape_rect(m);
    let (x0, y0, x1, y1) = (r.x0, r.y0, r.x1, r.y1);
    let p = Pt::new;
    match m.kind {
        MarkType::Ellipse => {
            let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
            let (rx, ry) = (r.width() / 2.0, r.height() / 2.0);
            let (kx, ky) = (rx * KAPPA, ry * KAPPA);
            vec![
                Seg::Move(p(cx, y0)),
                Seg::Cubic(p(cx + kx, y0), p(x1, cy - ky), p(x1, cy)),
                Seg::Cubic(p(x1, cy + ky), p(cx + kx, y1), p(cx, y1)),
                Seg::Cubic(p(cx - kx, y1), p(x0, cy + ky), p(x0, cy)),
                Seg::Cubic(p(x0, cy - ky), p(cx - kx, y0), p(cx, y0)),
                Seg::Close,
            ]
        }
        _ => {
            let rad = corner_radius(m);
            if rad <= 0.0 {
                return vec![Seg::Move(p(x0, y0)), Seg::Line(p(x1, y0)), Seg::Line(p(x1, y1)), Seg::Line(p(x0, y1)), Seg::Close];
            }
            let k = rad * (1.0 - KAPPA);
            vec![
                Seg::Move(p(x0 + rad, y0)),
                Seg::Line(p(x1 - rad, y0)),
                Seg::Cubic(p(x1 - k, y0), p(x1, y0 + k), p(x1, y0 + rad)),
                Seg::Line(p(x1, y1 - rad)),
                Seg::Cubic(p(x1, y1 - k), p(x1 - k, y1), p(x1 - rad, y1)),
                Seg::Line(p(x0 + rad, y1)),
                Seg::Cubic(p(x0 + k, y1), p(x0, y1 - k), p(x0, y1 - rad)),
                Seg::Line(p(x0, y0 + rad)),
                Seg::Cubic(p(x0, y0 + k), p(x0 + k, y0), p(x0 + rad, y0)),
                Seg::Close,
            ]
        }
    }
}

/// 把路徑攤平成折線（封閉路徑的最後一點會回到起點）；曲線每段切成 `steps` 段
pub fn flatten(path: &[Seg], steps: usize) -> Vec<Pt> {
    let steps = steps.max(1);
    let mut out: Vec<Pt> = Vec::new();
    let mut start = Pt::default();
    for s in path {
        match *s {
            Seg::Move(a) => {
                start = a;
                out.push(a);
            }
            Seg::Line(a) => out.push(a),
            Seg::Cubic(c1, c2, e) => {
                let s0 = out.last().copied().unwrap_or(start);
                for i in 1..=steps {
                    let t = i as f64 / steps as f64;
                    let u = 1.0 - t;
                    let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
                    out.push(Pt::new(
                        w[0] * s0.x + w[1] * c1.x + w[2] * c2.x + w[3] * e.x,
                        w[0] * s0.y + w[1] * c1.y + w[2] * c2.y + w[3] * e.y,
                    ));
                }
            }
            Seg::Close => out.push(start),
        }
    }
    out
}

pub type MarkList = Vec<Mark>;

/// 文件的編輯狀態：註記清單 + 復原/重做堆疊
#[derive(Default)]
pub struct EditModel {
    pub marks: MarkList,
    pub undo_stack: Vec<MarkList>,
    pub redo_stack: Vec<MarkList>,
    pub dirty: bool,
}

impl EditModel {
    /// 修改前呼叫：把目前狀態推入復原堆疊
    pub fn snapshot(&mut self) {
        self.undo_stack.push(self.marks.clone());
        self.redo_stack.clear();
        self.dirty = true;
    }
    /// 以修改前的狀態 `before` 記錄一次已完成的修改
    pub fn record(&mut self, before: MarkList) {
        self.undo_stack.push(before);
        self.redo_stack.clear();
        self.dirty = true;
    }
    pub fn undo(&mut self) -> bool {
        let Some(prev) = self.undo_stack.pop() else { return false };
        self.redo_stack.push(std::mem::replace(&mut self.marks, prev));
        self.dirty = true;
        true
    }
    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo_stack.pop() else { return false };
        self.undo_stack.push(std::mem::replace(&mut self.marks, next));
        self.dirty = true;
        true
    }
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}
