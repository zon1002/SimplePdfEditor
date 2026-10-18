// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Fufeng Project

//! 開啟 PDF、渲染頁面與文字選取（hayro）。

use crate::geom::{Affine, Pt, Quad, Rect};
use crate::text::{PageText, extract_page};

use hayro::hayro_syntax::Pdf;
use hayro::hayro_syntax::LoadPdfError;
use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::{RenderCache, RenderSettings, render_into, render_size};
use std::cell::RefCell;
use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub enum OpenError {
    /// 需要密碼（未提供或密碼錯誤）
    NeedsPassword,
    Invalid(String),
}

impl std::fmt::Display for OpenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OpenError::NeedsPassword => write!(f, "需要密碼"),
            OpenError::Invalid(s) => write!(f, "{s}"),
        }
    }
}

/// 渲染結果：RGBA8，由上而下逐列
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// 書籤（PDF 大綱）的一個項目
#[derive(Clone, Debug, PartialEq)]
pub struct OutlineItem {
    /// 階層：1 為最上層
    pub level: usize,
    pub title: String,
    /// 目的頁（0 起算）
    pub page: usize,
}

/// 單頁渲染的最大邊長（像素）
pub const MAX_RENDER_SIDE: f64 = 8000.0;

pub struct Document {
    bytes: Arc<Vec<u8>>,
    password: String,
    path: String,
    pdf: Pdf,
    text: RefCell<HashMap<usize, Rc<PageText>>>,
    outline: RefCell<Option<Rc<Vec<OutlineItem>>>>,
}

impl Document {
    /// 從記憶體開啟；加密檔需提供正確密碼（空字串可開啟只有權限密碼的檔案）
    pub fn open_bytes(bytes: Vec<u8>, path: &str, password: &str) -> Result<Document, OpenError> {
        let bytes = Arc::new(bytes);
        let pdf = match catch_unwind(AssertUnwindSafe(|| Pdf::new_with_password(bytes.clone(), password))) {
            Ok(Ok(pdf)) => pdf,
            // DecryptionError 未公開，以名稱判斷「需要密碼」
            Ok(Err(LoadPdfError::Decryption(e))) if format!("{e:?}") == "PasswordProtected" => {
                return Err(OpenError::NeedsPassword);
            }
            Ok(Err(LoadPdfError::Decryption(e))) => return Err(OpenError::Invalid(format!("無法解密此 PDF（{e:?}）"))),
            Ok(Err(_)) => return Err(OpenError::Invalid("不是有效的 PDF 檔案".into())),
            Err(_) => return Err(OpenError::Invalid("解析 PDF 時發生錯誤".into())),
        };
        if pdf.pages().is_empty() {
            return Err(OpenError::Invalid("PDF 沒有任何頁面".into()));
        }
        Ok(Document {
            bytes,
            password: password.to_string(),
            path: path.to_string(),
            pdf,
            text: RefCell::new(HashMap::new()),
            outline: RefCell::new(None),
        })
    }

    pub fn open_file(path: &str, password: &str) -> Result<Document, OpenError> {
        let bytes = std::fs::read(path).map_err(|e| OpenError::Invalid(format!("無法讀取檔案：{e}")))?;
        Self::open_bytes(bytes, path, password)
    }

    pub fn path(&self) -> &str {
        &self.path
    }
    pub fn bytes(&self) -> &Arc<Vec<u8>> {
        &self.bytes
    }
    pub fn password(&self) -> &str {
        &self.password
    }
    pub fn page_count(&self) -> usize {
        self.pdf.pages().len()
    }

    /// 頁面大小（pt，已含旋轉）
    pub fn page_size(&self, page: usize) -> (f64, f64) {
        self.pdf.pages().get(page).map(|p| p.render_dimensions()).map(|(w, h)| (w as f64, h as f64)).unwrap_or((612.0, 792.0))
    }

    pub fn page_rect(&self, page: usize) -> Rect {
        let (w, h) = self.page_size(page);
        Rect::new(0.0, 0.0, w, h)
    }

    /// PDF 使用者空間 -> 頁面檢視座標
    pub fn pdf_to_view(&self, page: usize) -> Affine {
        self.pdf.pages().get(page).map(|p| Affine(p.initial_transform(true).as_coeffs())).unwrap_or(Affine::IDENTITY)
    }

    /// 每頁的轉換矩陣（存檔時把檢視座標換回 PDF 座標）
    pub fn view_transforms(&self) -> Vec<Affine> {
        (0..self.page_count()).map(|p| self.pdf_to_view(p).inverse()).collect()
    }

    /// 以指定縮放（像素 / pt）渲染；過大時自動縮小
    pub fn render(&self, page: usize, scale: f64) -> Option<Image> {
        let pages = self.pdf.pages();
        let p = pages.get(page)?;
        let (w, h) = p.render_dimensions();
        if !(w.is_finite() && h.is_finite()) || w < 0.5 || h < 0.5 {
            return None;
        }
        let mut scale = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
        let longest = (w.max(h) as f64) * scale;
        if longest > MAX_RENDER_SIDE {
            scale *= MAX_RENDER_SIDE / longest;
        }
        let side = |pt: f32| ((pt as f64 * scale).ceil().clamp(1.0, 65535.0)) as u16;
        let rs = RenderSettings {
            x_scale: scale as f32,
            y_scale: scale as f32,
            width: Some(side(w)),
            height: Some(side(h)),
            bg_color: WHITE,
            ..Default::default()
        };
        let (pw, ph) = render_size(p, &rs);
        let mut rgba = vec![0u8; pw as usize * ph as usize * 4];
        let ok = catch_unwind(AssertUnwindSafe(|| {
            let cache = RenderCache::new();
            render_into(p, &cache, &crate::cjkfonts::interpreter_settings(), &rs, &mut rgba);
        }));
        if ok.is_err() {
            // 渲染失敗時仍顯示白色頁面，避免整個程式無法使用
            rgba.iter_mut().for_each(|b| *b = 255);
        }
        Some(Image { width: pw as u32, height: ph as u32, rgba })
    }

    /// 書籤（PDF 大綱），依文件順序；沒有書籤或無法解析時為空（快取）
    pub fn outline(&self) -> Rc<Vec<OutlineItem>> {
        if let Some(o) = self.outline.borrow().as_ref() {
            return o.clone();
        }
        let pages = self.page_count();
        let items = catch_unwind(AssertUnwindSafe(|| {
            let doc = lopdf::Document::load_mem_with_options(&self.bytes, lopdf::LoadOptions::with_password(&self.password)).ok()?;
            let toc = doc.get_toc().ok()?;
            Some(
                toc.toc
                    .into_iter()
                    .filter(|t| t.page >= 1 && t.page <= pages)
                    .map(|t| OutlineItem {
                        level: t.level.max(1),
                        title: t.title.trim_matches(|c: char| c == '\0' || c.is_whitespace()).to_string(),
                        page: t.page - 1,
                    })
                    .collect::<Vec<_>>(),
            )
        }))
        .ok()
        .flatten()
        .unwrap_or_default();
        let items = Rc::new(items);
        *self.outline.borrow_mut() = Some(items.clone());
        items
    }

    /// 頁面的文字（快取）
    pub fn page_text(&self, page: usize) -> Rc<PageText> {
        if let Some(t) = self.text.borrow().get(&page) {
            return t.clone();
        }
        let t = catch_unwind(AssertUnwindSafe(|| extract_page(&self.pdf, page, &crate::cjkfonts::interpreter_settings())))
            .ok()
            .flatten()
            .unwrap_or_default();
        let t = Rc::new(t);
        self.text.borrow_mut().insert(page, t.clone());
        t
    }

    /// 依拖曳起訖點選取文字（閱讀順序）；回傳每行一個 quad（ul, ur, lr, ll）與選到的文字
    pub fn select_text(&self, page: usize, a: Pt, b: Pt) -> (Vec<Quad>, String) {
        select_in(&self.page_text(page), a, b)
    }
}

/// OCR 結果（尚未存檔）轉成可選取的頁面文字；`size` 為頁面大小（檢視座標），直書文字依此排列
pub fn ocr_page_text(lines: &[crate::marks::OcrTextLine], offset: Pt, size: (f64, f64)) -> PageText {
    crate::text::layout_on_page(ocr_glyphs(lines, offset), size.0 as f32, size.1 as f32)
}

/// 頁面原有的文字加上 OCR 結果，合併成一份可選取的文字（依閱讀順序重新排列）。
/// 掃描檔、CAD 圖面常帶有頁尾或浮水印文字，兩者都要能選取。
pub fn merge_ocr_text(existing: &PageText, lines: &[crate::marks::OcrTextLine], offset: Pt, size: (f64, f64)) -> PageText {
    if lines.is_empty() {
        return existing.clone();
    }
    let mut glyphs = existing.glyphs.clone();
    glyphs.extend(ocr_glyphs(lines, offset));
    crate::text::layout_on_page(glyphs, size.0 as f32, size.1 as f32)
}

fn ocr_glyphs(lines: &[crate::marks::OcrTextLine], offset: Pt) -> Vec<crate::text::TextGlyph> {
    let mut glyphs = Vec::new();
    for l in lines {
        for g in &l.glyphs {
            let q = g.quad.map(|p| p + offset);
            // PageText 的四邊形：先是朝向下一行的邊（底邊，依閱讀方向），再是對邊
            let quad = [[q[3].x as f32, q[3].y as f32], [q[2].x as f32, q[2].y as f32], [q[1].x as f32, q[1].y as f32], [q[0].x as f32, q[0].y as f32]];
            let mut r = crate::geom::Rect::EMPTY;
            for p in &q {
                r.include(*p);
            }
            let d = q[2] - q[3];
            let len = d.length().max(1e-6);
            glyphs.push(crate::text::TextGlyph {
                text: g.text.clone(),
                rect: [r.x0 as f32, r.y0 as f32, r.x1 as f32, r.y1 as f32],
                quad,
                direction: [(d.x / len) as f32, (d.y / len) as f32],
            });
        }
    }
    glyphs
}

/// 在頁面文字中依拖曳起訖點選取
pub fn select_in(t: &PageText, a: Pt, b: Pt) -> (Vec<Quad>, String) {
    {
        if t.is_empty() {
            return (Vec::new(), String::new());
        }
        let (Some(ia), Some(ib)) = (t.nearest(a.x as f32, a.y as f32), t.nearest(b.x as f32, b.y as f32)) else {
            return (Vec::new(), String::new());
        };
        let (lo, hi) = if ia <= ib { (ia, ib) } else { (ib, ia) };
        let range = lo..hi + 1;
        // 每行一個沿著書寫方向的四邊形：直書、旋轉的文字，底線也沿著文字畫
        let quads = t.line_quads(range.clone()).into_iter().map(|q| q.map(|p| Pt::new(p[0] as f64, p[1] as f64))).collect();
        (quads, t.text_of(range))
    }
}
