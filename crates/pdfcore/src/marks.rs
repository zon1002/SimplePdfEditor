// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Fu Project

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
        }
    }
}

impl Mark {
    pub fn is_line(&self) -> bool {
        matches!(self.kind, MarkType::Line | MarkType::Arrow)
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
