// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Fufeng Project

//! 自動測試（不開視窗）：對範例 PDF 加上各種註記，輸出完整檔與加密的部分頁面，
//! 並以編輯畫面相同的繪圖程式輸出第 1 頁預覽（preview.png），供與 PDF 結果比對。

use crate::document::Document;
use crate::geom::{Pt, Rgba};
use crate::marks::{Mark, MarkType};
use crate::raster::{render_page_with_marks, save_png};
use crate::save::{SaveOptions, save};

pub fn run(input: &str, out_dir: &str) -> Result<String, String> {
    let doc = Document::open_file(input, "").map_err(|e| format!("open: {e}"))?;
    std::fs::create_dir_all(out_dir).map_err(|e| e.to_string())?;
    let mut log = String::new();

    let mut marks = Vec::new();
    let (quads, text) = doc.select_text(0, Pt::new(60.0, 60.0), Pt::new(400.0, 75.0));
    log.push_str(&format!("selected {} quads: {}\n", quads.len(), text.split_whitespace().collect::<Vec<_>>().join(" ")));
    marks.push(Mark { kind: MarkType::Highlight, color: Rgba::rgb(0xFF, 0xEB, 0x3B), quads, ..Default::default() });

    let (quads, _) = doc.select_text(0, Pt::new(60.0, 110.0), Pt::new(400.0, 125.0));
    marks.push(Mark { kind: MarkType::Underline, color: Rgba::rgb(0xE5, 0x39, 0x35), quads, ..Default::default() });

    let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
    let font = format!("{windir}\\Fonts\\msjh.ttc");
    let t = Mark {
        kind: MarkType::Text,
        color: Rgba::rgb(0x1E, 0x63, 0xD6),
        text: "新增文字測試 ABC 123\n第二行：微軟正黑體".into(),
        font_path: font.clone(),
        font_size: 18.0,
        pos: Pt::new(72.0, 300.0),
        offset: Pt::new(10.0, 5.0),
        ..Default::default()
    };
    marks.push(t.clone());

    let ln = Mark { kind: MarkType::Line, color: Rgba::rgb(0x2E, 0x7D, 0x32), p1: Pt::new(72.0, 420.0), p2: Pt::new(300.0, 420.0), width: 2.0, ..Default::default() };
    marks.push(ln.clone());
    // 虛線線條
    marks.push(Mark { p1: Pt::new(320.0, 420.0), p2: Pt::new(540.0, 420.0), dashed: true, dash_len: 8.0, gap_len: 4.0, ..ln.clone() });
    // 實線、大箭頭
    let ar = Mark { kind: MarkType::Arrow, color: Rgba::rgb(0xE5, 0x39, 0x35), p1: Pt::new(72.0, 470.0), p2: Pt::new(300.0, 520.0), width: 3.0, arrow_size: 30.0, ..ln.clone() };
    marks.push(ar.clone());
    // 虛線、小箭頭
    let da = Mark {
        color: Rgba::rgb(0x1E, 0x63, 0xD6),
        p1: Pt::new(320.0, 470.0),
        p2: Pt::new(540.0, 540.0),
        width: 1.5,
        arrow_size: 8.0,
        dashed: true,
        dash_len: 3.0,
        gap_len: 3.0,
        ..ar.clone()
    };
    marks.push(da.clone());
    // 粗虛線、中箭頭、雙向
    marks.push(Mark { p1: Pt::new(72.0, 600.0), p2: Pt::new(400.0, 600.0), width: 5.0, arrow_size: 20.0, dash_len: 12.0, gap_len: 6.0, both_ends: true, ..da });
    // 矩形、圓角矩形（虛線）、圓形框
    let rect = Mark { kind: MarkType::Rect, color: Rgba::rgb(0xE5, 0x39, 0x35), p1: Pt::new(72.0, 650.0), p2: Pt::new(200.0, 730.0), width: 2.0, ..Default::default() };
    marks.push(rect.clone());
    marks.push(Mark { p1: Pt::new(230.0, 650.0), p2: Pt::new(370.0, 730.0), radius: 16.0, dashed: true, dash_len: 6.0, gap_len: 3.0, color: Rgba::rgb(0x1E, 0x63, 0xD6), ..rect.clone() });
    marks.push(Mark { kind: MarkType::Ellipse, p1: Pt::new(400.0, 650.0), p2: Pt::new(480.0, 730.0), width: 3.0, color: Rgba::rgb(0x2E, 0x7D, 0x32), ..rect });
    // 外框 + 半透明背景
    marks.push(Mark {
        text: "外框與背景 Box".into(),
        pos: Pt::new(330.0, 300.0),
        offset: Pt::default(),
        font_size: 14.0,
        color: Rgba::rgb(0x1A, 0x1A, 0x1A),
        text_border: true,
        border_color: Rgba::rgb(0xE5, 0x39, 0x35),
        border_width: 1.5,
        text_fill: true,
        fill_color: Rgba::rgba(0xFF, 0xEB, 0x3B, 160),
        ..t.clone()
    });
    // 第 2 頁也放一個註記，確認匯出單頁時只會帶出所選頁
    if doc.page_count() > 1 {
        marks.push(Mark { page: 1, text: "第二頁的文字".into(), ..t });
    }

    let full = format!("{out_dir}/full.pdf");
    save(&doc, &marks, &SaveOptions::default(), &full).map_err(|e| format!("save: {e}"))?;

    let pages: Vec<usize> = [0usize, 2].into_iter().filter(|&p| p < doc.page_count()).collect();
    let enc = SaveOptions {
        pages,
        encrypt: true,
        user_password: "1234".into(),
        owner_password: "owner".into(),
        allow_copy: false,
        ..Default::default()
    };
    save(&doc, &marks, &enc, &format!("{out_dir}/pages_enc.pdf")).map_err(|e| format!("export: {e}"))?;

    // 以編輯畫面相同的繪圖程式畫出第 1 頁，與 PDF 輸出比對
    let scale = 110.0 / 72.0;
    if let Some(img) = doc.render(0, scale) {
        let img = render_page_with_marks(img, &marks, 0, scale);
        save_png(&img, &format!("{out_dir}/preview.png"))?;
    }
    // 存檔結果也渲染一張，供比對
    if let Ok(saved) = Document::open_file(&full, "") {
        if let Some(img) = saved.render(0, scale) {
            save_png(&img, &format!("{out_dir}/full_p1.png"))?;
        }
    }
    log.push_str("ok\n");
    Ok(log)
}
