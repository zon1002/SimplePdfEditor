// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Fufeng Project

//! 以實際的語言包辨識測試頁。語言包由 `ocr-packs build` 產生於 target/ocr-packs；
//! 找不到時略過（CI 或未下載模型的電腦）。

use ocr::{OcrEngine, RgbImage, packs};
use pdfcore::{Mark, MarkType, Pt, Rgba};
use std::sync::atomic::AtomicBool;

fn packs_dir() -> Option<std::path::PathBuf> {
    let d = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/ocr-packs");
    d.join("std/pack.json").exists().then_some(d)
}

/// 每行一種語言的測試頁（200 dpi）
fn page(lines: &[(&str, &str)], upside_down: bool) -> RgbImage {
    let windir = std::env::var("WINDIR").unwrap_or("C:\\Windows".into());
    let scale = 200.0 / 72.0;
    let (w, h) = ((595.0 * scale) as u32, (300.0 * scale) as u32);
    let mut pix = tiny_skia::Pixmap::new(w, h).unwrap();
    pix.fill(tiny_skia::Color::WHITE);
    for (i, (font, text)) in lines.iter().enumerate() {
        let m = Mark {
            kind: MarkType::Text,
            color: Rgba::rgb(0x10, 0x10, 0x10),
            text: text.to_string(),
            font_path: format!("{windir}\\Fonts\\{font}"),
            font_size: 14.0,
            pos: Pt::new(40.0, 30.0 + i as f64 * 40.0),
            ..Default::default()
        };
        pdfcore::raster::draw_mark(&mut pix, &m, Pt::default(), scale);
    }
    let img = pdfcore::raster::pixmap_to_image(pix);
    let mut out = RgbImage::from_rgba(img.width as usize, img.height as usize, &img.rgba);
    if upside_down {
        let (w, h) = (out.width, out.height);
        let mut r = vec![0u8; out.rgb.len()];
        for y in 0..h {
            for x in 0..w {
                let (s, d) = ((y * w + x) * 3, ((h - 1 - y) * w + (w - 1 - x)) * 3);
                r[d..d + 3].copy_from_slice(&out.rgb[s..s + 3]);
            }
        }
        out.rgb = r;
    }
    out
}

fn engine(extra: &[&str]) -> Option<OcrEngine> {
    let dir = packs_dir()?;
    // SAFETY: 測試在設定好環境變數後才讀取語言包
    unsafe { std::env::set_var("SPE_OCR_PACKS", &dir) };
    let find = |id: &str| packs::find_installed(id).unwrap_or_else(|| panic!("找不到語言包 {id}"));
    let extra: Vec<_> = extra.iter().map(|id| find(id)).collect();
    Some(OcrEngine::load(&find("engine"), &find("std"), &extra).expect("載入 OCR"))
}

const LINES: &[(&str, &str)] = &[
    ("msjh.ttc", "臺灣的氣候屬於亞熱帶季風氣候"),
    ("msyh.ttc", "数据显示今年销售额增长了百分之十二"),
    ("arial.ttf", "The quarterly report was submitted on October 19, 2026."),
    ("YuGothM.ttc", "東京都の天気は晴れのち曇りです"),
];

#[test]
fn recognizes_chinese_english_japanese_with_char_positions() {
    let Some(mut e) = engine(&[]) else {
        eprintln!("略過：找不到 target/ocr-packs");
        return;
    };
    let img = page(LINES, false);
    let lines = e.recognize(&img, &AtomicBool::new(false)).unwrap();
    let texts: Vec<String> = lines.iter().map(|l| l.text.replace(' ', "")).collect();
    for (_, want) in LINES {
        let want = want.replace(' ', "");
        assert!(texts.iter().any(|t| t == &want), "找不到「{want}」，辨識結果：{texts:?}");
    }
    // 每個字都有位置，且依閱讀方向由左而右、落在影像內
    for l in &lines {
        assert_eq!(l.chars.iter().map(|c| c.text.as_str()).collect::<String>(), l.text.replace(' ', ""));
        for w in l.chars.windows(2) {
            assert!(w[1].quad[0][0] >= w[0].quad[0][0] - 1.0, "字的順序應由左而右");
        }
        for c in &l.chars {
            for p in c.quad {
                assert!(p[0] >= -5.0 && p[1] >= -5.0 && p[0] <= img.width as f32 + 5.0 && p[1] <= img.height as f32 + 5.0);
            }
        }
        assert!(l.confidence > 0.8, "{}: {}", l.text, l.confidence);
    }
}

#[test]
fn upside_down_page_is_read_correctly() {
    let Some(mut e) = engine(&[]) else { return };
    let img = page(&LINES[..1], true);
    let lines = e.recognize(&img, &AtomicBool::new(false)).unwrap();
    assert!(lines.iter().any(|l| l.text == "臺灣的氣候屬於亞熱帶季風氣候"), "{:?}", lines.iter().map(|l| &l.text).collect::<Vec<_>>());
}

#[test]
fn korean_pack_is_chosen_per_line_by_confidence() {
    if packs_dir().is_none_or(|d| !d.join("ko/pack.json").exists()) {
        return;
    }
    let Some(mut e) = engine(&["ko"]) else { return };
    let img = page(&[("msjh.ttc", "臺灣的氣候屬於亞熱帶季風氣候"), ("malgun.ttf", "오늘 회의는 오후 세 시에 진행됩니다")], false);
    let lines = e.recognize(&img, &AtomicBool::new(false)).unwrap();
    let texts: Vec<String> = lines.iter().map(|l| l.text.replace(' ', "")).collect();
    assert!(texts.contains(&"臺灣的氣候屬於亞熱帶季風氣候".to_string()), "{texts:?}");
    assert!(texts.contains(&"오늘회의는오후세시에진행됩니다".to_string()), "{texts:?}");
}
