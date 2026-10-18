// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Fufeng Project

//! 選取框：以選取工具（檢視模式下任何時候）在頁面上拖曳出矩形，
//! 選取框內的文字（含尚未存檔的 OCR 結果）並可複製文字與圖片到剪貼簿。

use crate::app::App;
use pdfcore::{Image, Pt, Quad, Rect};

/// 複製圖片的解析度（框內的頁面與註記，依框的大小自動降低以免過大）
const COPY_DPI: f64 = 300.0;
const COPY_MAX_SIDE: f64 = 6000.0;

/// 拖曳出的選取範圍
#[derive(Clone, Debug)]
pub struct Region {
    pub page: usize,
    pub rect: Rect,
    /// 框內的文字（依閱讀順序，每行一行）
    pub text: String,
    /// 選到的文字範圍（每段一個四邊形），顯示用
    pub quads: Vec<Quad>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CopyWhat {
    /// 文字與圖片一起放進剪貼簿：貼到記事本、Word 是文字，貼到小畫家、LINE 是圖片
    Both,
    Text,
    Image,
}

impl App {
    /// 依拖曳的兩點計算選取範圍（框內中心落在框裡的字）
    pub fn region_from(&mut self, a: Pt, b: Pt) -> Option<Region> {
        let rect = Rect::from_points(a, b);
        let text = self.page_selectable_text()?;
        let glyphs = text.glyphs_in([rect.x0 as f32, rect.y0 as f32, rect.x1 as f32, rect.y1 as f32]);
        let quads = text.glyph_rects(&glyphs).into_iter().map(|r| Rect::new(r[0] as f64, r[1] as f64, r[2] as f64, r[3] as f64).corners()).collect();
        Some(Region { page: self.page, rect, text: text.column_text(&glyphs), quads })
    }

    /// 選取框內的頁面影像（含註記，不含看不見的 OCR 文字層）
    pub fn region_image(&self) -> Option<Image> {
        let r = self.region.as_ref()?;
        let doc = self.doc.as_ref()?;
        let (pw, ph) = doc.page_size(r.page);
        let rect = Rect::new(r.rect.x0.max(0.0), r.rect.y0.max(0.0), r.rect.x1.min(pw), r.rect.y1.min(ph));
        if rect.width() < 1.0 || rect.height() < 1.0 {
            return None;
        }
        let scale = (COPY_DPI / 72.0).min(COPY_MAX_SIDE / rect.width().max(rect.height()));
        let page = doc.render(r.page, scale)?;
        // 實際比例（頁面過大時渲染器會自動縮小）
        let real = page.width as f64 / pw.max(1.0);
        let full = pdfcore::raster::render_page_with_marks(page, &self.model.marks, r.page, real);
        let x0 = ((rect.x0 * real).floor() as u32).min(full.width);
        let y0 = ((rect.y0 * real).floor() as u32).min(full.height);
        let x1 = ((rect.x1 * real).ceil() as u32).clamp(x0, full.width);
        let y1 = ((rect.y1 * real).ceil() as u32).clamp(y0, full.height);
        let (w, h) = (x1 - x0, y1 - y0);
        if w == 0 || h == 0 {
            return None;
        }
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        for y in y0..y1 {
            let row = ((y * full.width + x0) * 4) as usize;
            rgba.extend_from_slice(&full.rgba[row..row + (w * 4) as usize]);
        }
        Some(Image { width: w, height: h, rgba })
    }

    pub fn clear_region(&mut self) {
        self.region = None;
    }

    /// Ctrl+C：有選取框時複製框內的文字與圖片；否則複製最後一次高亮／底線選取的文字
    pub fn copy(&mut self, ctx: &egui::Context) {
        if self.region.is_some() {
            self.copy_region(CopyWhat::Both);
        } else {
            self.copy_text(ctx);
        }
    }

    pub fn copy_region(&mut self, what: CopyWhat) {
        let Some(r) = self.region.clone() else { return };
        let text = r.text.trim();
        let text = (what != CopyWhat::Image && !text.is_empty()).then_some(text);
        let image = if what == CopyWhat::Text { None } else { self.region_image() };
        if text.is_none() && image.is_none() {
            self.message(if what == CopyWhat::Text { "選取框內沒有文字" } else { "選取框內沒有內容" }, 3.0);
            return;
        }
        match crate::clipboard::set(text, image.as_ref()) {
            Ok(()) => {
                let msg = match (text, &image) {
                    (Some(t), Some(_)) => format!("已複製 {} 個字與圖片", t.chars().filter(|c| !c.is_whitespace()).count()),
                    (Some(t), None) => format!("已複製 {} 個字", t.chars().filter(|c| !c.is_whitespace()).count()),
                    _ => "已複製圖片".to_string(),
                };
                self.message(msg, 3.0);
                if let Some(t) = text {
                    self.last_text = t.to_string();
                }
            }
            Err(e) => self.error_box(format!("無法複製到剪貼簿：{e}")),
        }
    }
}
