// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Fufeng Project

//! 辨識流程（PaddleOCR）：文字偵測（DB）→ 旋轉裁切 → 文字方向判斷 → 文字辨識（CTC）。
//! 輸入為頁面影像（RGB），輸出每一行的文字、信心分數與每個字的位置（影像座標）。

use crate::packs::InstalledPack;
use ort::session::Session;
use ort::value::Tensor;
use std::path::Path;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

/// RGB 影像（由上而下逐列）
pub struct RgbImage {
    pub width: usize,
    pub height: usize,
    pub rgb: Vec<u8>,
}

impl RgbImage {
    /// 由 RGBA 轉換
    pub fn from_rgba(width: usize, height: usize, rgba: &[u8]) -> RgbImage {
        let mut rgb = Vec::with_capacity(width * height * 3);
        for p in rgba.chunks_exact(4) {
            rgb.extend_from_slice(&p[..3]);
        }
        RgbImage { width, height, rgb }
    }

    /// 雙線性取樣（座標超出範圍時取邊緣）
    fn sample(&self, x: f32, y: f32, c: usize) -> f32 {
        let (w, h) = (self.width, self.height);
        let x = x.clamp(0.0, (w - 1) as f32);
        let y = y.clamp(0.0, (h - 1) as f32);
        let (x1, y1) = (x as usize, y as usize);
        let (x2, y2) = ((x1 + 1).min(w - 1), (y1 + 1).min(h - 1));
        let (tx, ty) = (x - x1 as f32, y - y1 as f32);
        let at = |xx: usize, yy: usize| self.rgb[(yy * w + xx) * 3 + c] as f32;
        (at(x1, y1) * (1.0 - tx) + at(x2, y1) * tx) * (1.0 - ty) + (at(x1, y2) * (1.0 - tx) + at(x2, y2) * tx) * ty
    }
}

/// 一個字：文字與四邊形（影像座標，依閱讀方向的左上、右上、右下、左下）
#[derive(Clone, Debug, PartialEq)]
pub struct OcrChar {
    pub text: String,
    pub quad: [[f32; 2]; 4],
}

#[derive(Clone, Debug, PartialEq)]
pub struct OcrLine {
    pub text: String,
    /// 平均信心分數（0–1）
    pub confidence: f32,
    pub chars: Vec<OcrChar>,
}

/// 旋轉的文字框：中心、閱讀方向（單位向量）、沿閱讀方向的半長、垂直方向的半高
#[derive(Clone, Copy, Debug)]
struct RBox {
    cx: f32,
    cy: f32,
    ux: f32,
    uy: f32,
    hl: f32,
    hh: f32,
}

impl RBox {
    /// 文字框座標 (a 沿閱讀方向, b 垂直方向) → 影像座標
    fn point(&self, a: f32, b: f32) -> [f32; 2] {
        [self.cx + a * self.ux - b * self.uy, self.cy + a * self.uy + b * self.ux]
    }
}

struct Recognizer {
    session: Session,
    dict: Vec<String>,
    height: usize,
}

pub struct OcrEngine {
    det: Session,
    cls: Option<Session>,
    /// 方向模型的輸入大小（高, 寬）
    cls_size: (usize, usize),
    recs: Vec<Recognizer>,
    /// 偵測時影像最長邊（像素）
    pub det_limit: usize,
    /// 偵測框外擴比例
    pub unclip: f32,
}

fn environment(dll: &Path) -> Result<&'static ort::environment::Environment, String> {
    static ENV: OnceLock<Result<ort::environment::Environment, String>> = OnceLock::new();
    ENV.get_or_init(|| {
        ort::init_from(dll)
            .map_err(|e| format!("無法載入 OCR 引擎（{}）：{e}", dll.display()))?
            .build()
            .map_err(|e| format!("無法初始化 OCR 引擎：{e}"))
    })
    .as_ref()
    .map_err(|e| e.clone())
}

fn threads() -> usize {
    std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(1, 8)
}

fn load_session(env: &ort::environment::Environment, path: &Path) -> Result<Session, String> {
    let builder = Session::builder(env).map_err(|e| e.to_string())?;
    let mut builder = builder.with_intra_threads(threads()).map_err(|e| e.to_string())?;
    builder.commit_from_file(path).map_err(|e| format!("無法載入模型 {}：{e}", path.display()))
}

/// 模型輸入的固定維度（第 `dim` 維）；動態時用預設值
fn input_dim(s: &Session, dim: usize, default: usize) -> usize {
    s.inputs()
        .first()
        .and_then(|i| i.dtype().tensor_shape().map(|d| d.iter().copied().collect::<Vec<i64>>()))
        .and_then(|d| d.get(dim).copied())
        .filter(|&h| h > 0)
        .map(|h| h as usize)
        .unwrap_or(default)
}

impl OcrEngine {
    /// 載入：引擎語言包（onnxruntime.dll、方向模型）、精度語言包（偵測＋中英日辨識）與額外語言
    pub fn load(engine: &InstalledPack, tier: &InstalledPack, extra: &[InstalledPack]) -> Result<OcrEngine, String> {
        let dll = engine.dir.join("onnxruntime.dll");
        let env = environment(&dll)?;
        let det = load_session(env, &tier.path("det").ok_or("語言包缺少偵測模型")?)?;
        let cls = engine.path("cls").map(|p| load_session(env, &p)).transpose()?;
        let mut recs = Vec::new();
        for pack in std::iter::once(tier).chain(extra.iter()) {
            let path = pack.path("rec").ok_or_else(|| format!("{} 缺少辨識模型", pack.info.name))?;
            let session = load_session(env, &path)?;
            let mut dict: Vec<String> = session
                .metadata()
                .ok()
                .and_then(|m| m.custom("character"))
                .unwrap_or_default()
                .lines()
                .map(|s| s.to_string())
                .collect();
            if dict.is_empty() {
                return Err(format!("{} 的辨識模型缺少字典", pack.info.name));
            }
            // 模型輸出：0 為空白（CTC blank），1..=N 為字典，最後一個為空格
            dict.push(" ".into());
            let height = input_dim(&session, 2, 48);
            recs.push(Recognizer { session, dict, height });
        }
        let cls_size = cls.as_ref().map(|c| (input_dim(c, 2, 48), input_dim(c, 3, 192))).unwrap_or((48, 192));
        Ok(OcrEngine { det, cls, cls_size, recs, det_limit: 2560, unclip: 1.4 })
    }

    /// 辨識一張影像；`cancel` 設為 true 時盡快結束
    pub fn recognize(&mut self, img: &RgbImage, cancel: &AtomicBool) -> Result<Vec<OcrLine>, String> {
        if img.width < 8 || img.height < 8 {
            return Ok(Vec::new());
        }
        let boxes = self.detect(img)?;
        let mut lines = Vec::new();
        for b in boxes {
            if cancel.load(Ordering::Relaxed) {
                return Err("已取消".into());
            }
            let flip = self.is_upside_down(img, &b).unwrap_or(false);
            let b = if flip { RBox { ux: -b.ux, uy: -b.uy, ..b } } else { b };
            // 多個辨識模型：逐行取信心分數最高的結果
            let mut best: Option<OcrLine> = None;
            for i in 0..self.recs.len() {
                let line = self.recognize_line(i, img, &b)?;
                if best.as_ref().is_none_or(|bl| line.confidence > bl.confidence) {
                    best = Some(line);
                }
            }
            if let Some(l) = best.filter(|l| !l.text.trim().is_empty() && l.confidence >= 0.5) {
                lines.push(l);
            }
        }
        Ok(lines)
    }

    /// 文字偵測（DB）：回傳依閱讀順序排列的旋轉文字框（影像座標）
    fn detect(&mut self, img: &RgbImage) -> Result<Vec<RBox>, String> {
        let k = (self.det_limit as f32 / img.width.max(img.height) as f32).min(1.0);
        let rw = (((img.width as f32 * k) / 32.0).round() as usize).max(1) * 32;
        let rh = (((img.height as f32 * k) / 32.0).round() as usize).max(1) * 32;
        let (mean, std) = ([0.485f32, 0.456, 0.406], [0.229f32, 0.224, 0.225]);
        let (sx, sy) = (img.width as f32 / rw as f32, img.height as f32 / rh as f32);
        let mut data = vec![0f32; 3 * rw * rh];
        for y in 0..rh {
            for x in 0..rw {
                let (fx, fy) = ((x as f32 + 0.5) * sx - 0.5, (y as f32 + 0.5) * sy - 0.5);
                for c in 0..3 {
                    // PaddleOCR 以 BGR 順序輸入
                    let v = img.sample(fx, fy, 2 - c) / 255.0;
                    data[c * rw * rh + y * rw + x] = (v - mean[c]) / std[c];
                }
            }
        }
        let input = Tensor::from_array(([1usize, 3, rh, rw], data)).map_err(|e| e.to_string())?;
        let out = self.det.run(ort::inputs![input]).map_err(|e| format!("文字偵測失敗：{e}"))?;
        let (_, prob) = out[0].try_extract_tensor::<f32>().map_err(|e| e.to_string())?;
        if prob.len() < rw * rh {
            return Err("文字偵測輸出大小不符".into());
        }
        // 複製輸出，釋放模型的借用
        let prob: Vec<f32> = prob[..rw * rh].to_vec();
        drop(out);
        let bin: Vec<bool> = prob.iter().map(|&p| p > 0.3).collect();
        let mut seen = vec![false; rw * rh];
        let mut boxes = Vec::new();
        let mut stack = Vec::new();
        for start in 0..rw * rh {
            if !bin[start] || seen[start] {
                continue;
            }
            stack.clear();
            stack.push(start);
            seen[start] = true;
            let mut pts: Vec<(f32, f32)> = Vec::new();
            let mut score = 0f32;
            while let Some(i) = stack.pop() {
                let (x, y) = (i % rw, i / rw);
                pts.push((x as f32 + 0.5, y as f32 + 0.5));
                score += prob[i];
                for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    if nx >= 0 && ny >= 0 && (nx as usize) < rw && (ny as usize) < rh {
                        let j = ny as usize * rw + nx as usize;
                        if bin[j] && !seen[j] {
                            seen[j] = true;
                            stack.push(j);
                        }
                    }
                }
            }
            let n = pts.len();
            if n < 10 || score / (n as f32) < 0.6 {
                continue;
            }
            if let Some(b) = component_box(&pts, sx, sy, self.unclip) {
                boxes.push(b);
            }
        }
        Ok(reading_order(boxes))
    }

}

/// 連通區域 → 旋轉矩形（以主軸為閱讀方向），再外擴
fn component_box(pts: &[(f32, f32)], sx: f32, sy: f32, unclip: f32) -> Option<RBox> {
    {
        let n = pts.len() as f32;
        let (mx, my) = pts.iter().fold((0.0, 0.0), |a, p| (a.0 + p.0, a.1 + p.1));
        let (mx, my) = (mx / n, my / n);
        let (mut sxx, mut syy, mut sxy) = (0f32, 0f32, 0f32);
        for p in pts {
            let (dx, dy) = (p.0 - mx, p.1 - my);
            sxx += dx * dx;
            syy += dy * dy;
            sxy += dx * dy;
        }
        let ang = 0.5 * (2.0 * sxy).atan2(sxx - syy);
        let (mut ux, mut uy) = (ang.cos(), ang.sin());
        // 直排文字：閱讀方向為由上而下
        if uy.abs() > ux.abs() && uy < 0.0 {
            ux = -ux;
            uy = -uy;
        }
        let (mut a0, mut a1, mut b0, mut b1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        for p in pts {
            let (dx, dy) = (p.0 - mx, p.1 - my);
            let a = dx * ux + dy * uy;
            let b = -dx * uy + dy * ux;
            a0 = a0.min(a - 0.5);
            a1 = a1.max(a + 0.5);
            b0 = b0.min(b - 0.5);
            b1 = b1.max(b + 0.5);
        }
        let (len, thick) = (a1 - a0, b1 - b0);
        if !(len.is_finite() && thick.is_finite()) || len <= 0.0 || thick <= 0.0 {
            return None;
        }
        let d = len * thick * unclip / (2.0 * (len + thick));
        let (ca, cb) = ((a0 + a1) / 2.0, (b0 + b1) / 2.0);
        // 偵測影像 → 原影像（兩軸縮放比例幾乎相同）
        let s = (sx + sy) / 2.0;
        let b = RBox {
            cx: (mx + ca * ux - cb * uy) * sx,
            cy: (my + ca * uy + cb * ux) * sy,
            ux,
            uy,
            hl: (len / 2.0 + d) * s,
            hh: (thick / 2.0 + d) * s,
        };
        (b.hl > 2.0 && b.hh > 2.0).then_some(b)
    }
}

impl OcrEngine {

    /// 依文字框取樣成模型輸入（高度 h，寬度依比例），PaddleOCR 的 BGR、歸一化到 −1…1
    fn crop(img: &RgbImage, b: &RBox, h: usize, w: usize) -> Vec<f32> {
        let mut data = vec![0f32; 3 * w * h];
        for v in 0..h {
            for u in 0..w {
                let a = ((u as f32 + 0.5) / w as f32 * 2.0 - 1.0) * b.hl;
                let bb = ((v as f32 + 0.5) / h as f32 * 2.0 - 1.0) * b.hh;
                let [x, y] = b.point(a, bb);
                for c in 0..3 {
                    data[c * w * h + v * w + u] = (img.sample(x, y, 2 - c) / 255.0 - 0.5) / 0.5;
                }
            }
        }
        data
    }

    /// 文字方向：上下顛倒（180°）時回傳 true
    fn is_upside_down(&mut self, img: &RgbImage, b: &RBox) -> Result<bool, String> {
        let (h, w) = self.cls_size;
        let Some(cls) = self.cls.as_mut() else { return Ok(false) };
        let data = Self::crop(img, b, h, w);
        let input = Tensor::from_array(([1usize, 3, h, w], data)).map_err(|e| e.to_string())?;
        let out = cls.run(ort::inputs![input]).map_err(|e| e.to_string())?;
        let (_, p) = out[0].try_extract_tensor::<f32>().map_err(|e| e.to_string())?;
        Ok(p.len() >= 2 && p[1] > p[0] && p[1] > 0.9)
    }

    /// 文字辨識（CTC 貪婪解碼），並依每個字出現的時間步推算字的位置
    fn recognize_line(&mut self, i: usize, img: &RgbImage, b: &RBox) -> Result<OcrLine, String> {
        let rec = &mut self.recs[i];
        let h = rec.height;
        let w = ((b.hl / b.hh * h as f32).ceil() as usize).clamp(16, 4800);
        let data = Self::crop(img, b, h, w);
        let input = Tensor::from_array(([1usize, 3, h, w], data)).map_err(|e| e.to_string())?;
        let out = rec.session.run(ort::inputs![input]).map_err(|e| format!("文字辨識失敗：{e}"))?;
        let (shape, probs) = out[0].try_extract_tensor::<f32>().map_err(|e| e.to_string())?;
        if shape.len() != 3 {
            return Err("文字辨識輸出格式不符".into());
        }
        let (t, c) = (shape[1] as usize, shape[2] as usize);
        // (字典索引, 起始時間步, 結束時間步, 機率)
        let mut tokens: Vec<(usize, usize, usize, f32)> = Vec::new();
        let mut last = 0usize;
        for step in 0..t {
            let Some(row) = probs.get(step * c..(step + 1) * c) else { break };
            let (idx, p) = row.iter().enumerate().fold((0, f32::MIN), |m, (k, &v)| if v > m.1 { (k, v) } else { m });
            if idx != 0 {
                if idx == last {
                    if let Some(tk) = tokens.last_mut() {
                        tk.2 = step;
                        tk.3 = tk.3.max(p);
                    }
                } else {
                    tokens.push((idx, step, step, p));
                }
            }
            last = idx;
        }
        let mut text = String::new();
        let mut chars = Vec::new();
        let mut conf = 0f32;
        let step_len = 2.0 * b.hl / t.max(1) as f32;
        for (k, &(idx, s0, s1, p)) in tokens.iter().enumerate() {
            let Some(ch) = rec.dict.get(idx - 1) else { continue };
            text.push_str(ch);
            conf += p;
            if ch.trim().is_empty() {
                continue;
            }
            // 字的範圍：從前一個字與本字的中點，到本字與下一個字的中点（限制在一個字高左右）
            let center = |s: f32| -b.hl + (s + 0.5) * step_len;
            let me = center((s0 + s1) as f32 / 2.0);
            let half = (b.hh * 0.9).max(step_len);
            let prev = if k > 0 { center((tokens[k - 1].1 + tokens[k - 1].2) as f32 / 2.0) } else { me - 2.0 * half };
            let next = tokens.get(k + 1).map(|n| center((n.1 + n.2) as f32 / 2.0)).unwrap_or(me + 2.0 * half);
            let a0 = ((prev + me) / 2.0).max(me - half).max(-b.hl);
            let a1 = ((me + next) / 2.0).min(me + half).min(b.hl);
            // 字高：外擴前的文字高度
            let th = b.hh / (1.0 + self.unclip * 0.5).max(1.0) * 1.15;
            chars.push(OcrChar { text: ch.clone(), quad: [b.point(a0, -th), b.point(a1, -th), b.point(a1, th), b.point(a0, th)] });
        }
        let confidence = if tokens.is_empty() { 0.0 } else { conf / tokens.len() as f32 };
        Ok(OcrLine { text, confidence, chars })
    }
}

/// 閱讀順序：先依垂直位置排序，再分行（與該行第一個框中心差距小於半個字高視為同一行），行內由左而右
fn reading_order(mut boxes: Vec<RBox>) -> Vec<RBox> {
    boxes.sort_by(|a, b| a.cy.total_cmp(&b.cy));
    let mut lines: Vec<Vec<RBox>> = Vec::new();
    for b in boxes {
        match lines.last_mut() {
            Some(line) if (b.cy - line[0].cy).abs() < line[0].hh.min(b.hh) => line.push(b),
            _ => lines.push(vec![b]),
        }
    }
    lines
        .into_iter()
        .flat_map(|mut l| {
            l.sort_by(|a, b| a.cx.total_cmp(&b.cx));
            l
        })
        .collect()
}
