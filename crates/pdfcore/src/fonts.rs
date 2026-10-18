// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Fufeng Project

//! 字型：載入字型檔、字型度量與文字排版。
//! 編輯畫面的預覽與寫入 PDF 都使用這裡的排版結果，確保兩者位置一致。

use crate::geom::{Pt, Rect};
use crate::marks::Mark;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

/// 文字排版共用常數：行距（字級的倍數）
pub const LINE_SPACING: f64 = 1.25;
/// 外框／背景與文字之間的留白（pt）
pub const TEXT_PADDING: f64 = 4.0;

/// 一個字型（字型檔 + 集合中的索引）
pub struct FontFace {
    pub path: String,
    pub index: u32,
    pub data: Arc<Vec<u8>>,
    pub units_per_em: f64,
    /// 上升／下降高度（字型單位，descent 為正值）
    pub ascent: f64,
    pub descent: f64,
    /// TrueType 外框（glyf）；否則為 CFF
    pub truetype: bool,
    pub family: String,
    pub glyph_count: u16,
}

impl FontFace {
    fn load(path: &str, index: u32) -> Option<FontFace> {
        let data = std::fs::read(path).ok()?;
        let face = ttf_parser::Face::parse(&data, index).ok()?;
        let upem = face.units_per_em() as f64;
        if upem <= 0.0 {
            return None;
        }
        let family = face
            .names()
            .into_iter()
            .filter(|n| n.name_id == ttf_parser::name_id::FAMILY && n.is_unicode())
            .filter_map(|n| n.to_string())
            .next()
            .unwrap_or_default();
        let truetype = face.tables().glyf.is_some();
        let (ascent, descent) = (face.ascender() as f64, -(face.descender() as f64));
        let glyph_count = face.number_of_glyphs();
        Some(FontFace {
            path: path.to_string(),
            index,
            units_per_em: upem,
            ascent: if ascent > 0.0 { ascent } else { upem * 0.88 },
            descent: if descent > 0.0 { descent } else { upem * 0.12 },
            truetype,
            family,
            glyph_count,
            data: Arc::new(data),
        })
    }

    /// 解析字型（每次呼叫重新解析表格目錄，成本很低）
    pub fn face(&self) -> Option<ttf_parser::Face<'_>> {
        ttf_parser::Face::parse(&self.data, self.index).ok()
    }

    /// 字元對應的字形編號（找不到為 0 = .notdef）
    pub fn glyph(face: &ttf_parser::Face<'_>, c: char) -> u16 {
        face.glyph_index(c).map(|g| g.0).unwrap_or(0)
    }

    /// 字形前進寬度（字型單位）
    pub fn advance(face: &ttf_parser::Face<'_>, gid: u16) -> f64 {
        face.glyph_hor_advance(ttf_parser::GlyphId(gid)).unwrap_or(0) as f64
    }
}

fn cache() -> &'static Mutex<HashMap<(String, u32), Option<Arc<FontFace>>>> {
    static CACHE: OnceLock<Mutex<HashMap<(String, u32), Option<Arc<FontFace>>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 取得字型（快取）；無法載入時回傳 None
pub fn font(path: &str, index: u32) -> Option<Arc<FontFace>> {
    if path.is_empty() {
        return None;
    }
    let key = (path.to_string(), index);
    let mut map = cache().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(f) = map.get(&key) {
        return f.clone();
    }
    let f = FontFace::load(path, index).map(Arc::new);
    map.insert(key, f.clone());
    f
}

/// 字型選單的一個項目
#[derive(Clone, Debug, PartialEq)]
pub struct FontEntry {
    pub label: String,
    pub path: String,
    pub index: u32,
}

/// Windows 常用字型（存在才列出）
pub fn system_fonts() -> Vec<FontEntry> {
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
    let dir = Path::new(&windir).join("Fonts");
    let candidates = [
        ("微軟正黑體", "msjh.ttc"),
        ("微軟正黑體 粗體", "msjhbd.ttc"),
        ("細明體", "mingliu.ttc"),
        ("標楷體", "kaiu.ttf"),
        ("微軟雅黑", "msyh.ttc"),
        ("Arial", "arial.ttf"),
        ("Times New Roman", "times.ttf"),
        ("Courier New", "cour.ttf"),
        ("Segoe UI", "segoeui.ttf"),
    ];
    candidates
        .iter()
        .filter_map(|(label, file)| {
            // 只檢查檔案是否存在；實際用到時才載入（中文字型檔動輒數十 MB）
            let p = dir.join(file);
            p.is_file().then(|| FontEntry { label: label.to_string(), path: p.to_string_lossy().into_owned(), index: 0 })
        })
        .collect()
}

/// 使用者選擇的字型檔；可載入時回傳選單項目（名稱為檔名）
pub fn font_entry_from_file(path: &str) -> Option<FontEntry> {
    font(path, 0)?;
    let label = Path::new(path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| path.to_string());
    Some(FontEntry { label, path: path.to_string(), index: 0 })
}

// ---------------------------------------------------------------- 排版

/// 文字排版結果（不含位移）
pub struct TextLayout {
    /// 第一行基線距文字頂端的距離（pt）
    pub ascent: f64,
    pub descent: f64,
    /// 每一行：基線位置與字形（字形編號、相對文字方塊左緣的 x）
    pub lines: Vec<LaidLine>,
    /// 文字本身範圍
    pub content: Rect,
}

pub struct LaidLine {
    pub baseline: f64,
    pub glyphs: Vec<(u16, char, f64)>,
    pub width: f64,
}

/// 依標記的字型與字級排版；字型無法載入時以估計值代替（仍可顯示外框位置）
pub fn layout_text(m: &Mark) -> TextLayout {
    let size = m.font_size;
    let f = font(&m.font_path, m.font_index);
    let face = f.as_ref().and_then(|f| f.face());
    let (ascent, descent, scale) = match &f {
        Some(f) => (f.ascent / f.units_per_em * size, f.descent / f.units_per_em * size, size / f.units_per_em),
        None => (size * 0.88, size * 0.12, 0.0),
    };
    let mut lines = Vec::new();
    let mut max_w: f64 = 0.0;
    for (i, line) in m.text.split('\n').enumerate() {
        let line = line.strip_suffix('\r').unwrap_or(line);
        let mut x = 0.0;
        let mut glyphs = Vec::new();
        for c in line.chars() {
            match &face {
                Some(face) => {
                    let gid = FontFace::glyph(face, c);
                    glyphs.push((gid, c, x));
                    x += FontFace::advance(face, gid) * scale;
                }
                None => {
                    glyphs.push((0, c, x));
                    x += size * if c.is_ascii() { 0.5 } else { 1.0 };
                }
            }
        }
        max_w = max_w.max(x);
        lines.push(LaidLine { baseline: m.pos.y + ascent + i as f64 * size * LINE_SPACING, glyphs, width: x });
    }
    let n = lines.len().max(1);
    let h = (n - 1) as f64 * size * LINE_SPACING + ascent + descent;
    let content = Rect::from_pos_size(m.pos, max_w.max(6.0), h);
    TextLayout { ascent, descent, lines, content }
}

/// 文字本身範圍（不含位移）
pub fn text_content_rect(m: &Mark) -> Rect {
    layout_text(m).content
}

/// 外框／背景範圍 = 文字範圍 + 留白（不含位移）
pub fn text_box_rect(m: &Mark) -> Rect {
    text_content_rect(m).expand(TEXT_PADDING)
}

/// 字形外框轉成 tiny-skia 路徑（字型單位，y 向上）
pub struct OutlineToPath<'a> {
    pub pb: &'a mut tiny_skia::PathBuilder,
    /// 字型單位 -> 輸出座標：x' = ox + x·k，y' = oy − y·k
    pub ox: f32,
    pub oy: f32,
    pub k: f32,
}

impl ttf_parser::OutlineBuilder for OutlineToPath<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.pb.move_to(self.ox + x * self.k, self.oy - y * self.k);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.pb.line_to(self.ox + x * self.k, self.oy - y * self.k);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.pb.quad_to(self.ox + x1 * self.k, self.oy - y1 * self.k, self.ox + x * self.k, self.oy - y * self.k);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.pb.cubic_to(
            self.ox + x1 * self.k,
            self.oy - y1 * self.k,
            self.ox + x2 * self.k,
            self.oy - y2 * self.k,
            self.ox + x * self.k,
            self.oy - y * self.k,
        );
    }
    fn close(&mut self) {
        self.pb.close();
    }
}

/// 把整段文字的字形外框加入路徑；`to_px` 把頁面座標（pt）轉成輸出座標的縮放與原點
pub fn text_outline(m: &Mark, layout: &TextLayout, origin: Pt, scale: f64) -> Option<tiny_skia::Path> {
    let f = font(&m.font_path, m.font_index)?;
    let face = f.face()?;
    let k = (m.font_size / f.units_per_em * scale) as f32;
    let mut pb = tiny_skia::PathBuilder::new();
    for line in &layout.lines {
        for &(gid, _, x) in &line.glyphs {
            let mut b = OutlineToPath {
                pb: &mut pb,
                ox: ((m.pos.x + x - origin.x) * scale) as f32,
                oy: ((line.baseline - origin.y) * scale) as f32,
                k,
            };
            face.outline_glyph(ttf_parser::GlyphId(gid), &mut b);
        }
    }
    pb.finish()
}
