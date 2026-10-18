// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Fufeng Project

//! 文字辨識（OCR）的介面：辨識對話框、背景辨識與進度、OCR 語言管理（下載／從檔案加入／移除）。

use crate::app::App;
use egui::{Id, Modal, RichText};
use ocr::packs::{self, InstalledPack, PackInfo, PackKind};
use pdfcore::{Mark, MarkType, OcrGlyph, OcrTextLine, Pt};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// 頁面辨識的解析度與最長邊上限
const OCR_DPI: f64 = 200.0;
const OCR_MAX_SIDE: f64 = 3600.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OcrRange {
    Current,
    All,
    Pages,
}

/// 辨識對話框的狀態
pub struct OcrDialog {
    pub range: OcrRange,
    pub pages: String,
    pub tier: String,
    pub extra: Vec<String>,
    pub skip_text: bool,
    pub error: Option<String>,
    installed: Vec<InstalledPack>,
}

/// 背景辨識工作
pub struct OcrJob {
    pub progress: Arc<AtomicUsize>,
    pub total: usize,
    pub cancel: Arc<AtomicBool>,
    pub status: Arc<Mutex<String>>,
    /// (頁碼, 辨識結果)
    pub handle: Option<std::thread::JoinHandle<Result<Vec<(usize, Vec<OcrTextLine>)>, String>>>,
}

/// 語言包下載中
pub struct PackDownload {
    pub id: String,
    pub done: Arc<AtomicU64>,
    pub total: u64,
    pub cancel: Arc<AtomicBool>,
    pub handle: Option<std::thread::JoinHandle<Result<PackInfo, String>>>,
}

/// OCR 語言管理的狀態
#[derive(Default)]
pub struct PackManager {
    pub open: bool,
    installed: Vec<InstalledPack>,
    /// 最新清單（從網路取得）；None 時使用隨程式附帶的清單
    remote: Option<packs::Manifest>,
    fetching: Option<std::thread::JoinHandle<Result<packs::Manifest, String>>>,
    importing: Option<std::thread::JoinHandle<Result<PackInfo, String>>>,
    download: Option<PackDownload>,
    message: Option<String>,
    /// 最近一次失敗：(語言包 id，空字串表示整體), 說明
    error: Option<(String, String)>,
    loaded: bool,
    /// 開啟時自動取得最新清單（每次執行一次）；自動取得失敗時不打擾使用者，改用隨程式附帶的清單
    auto_fetched: bool,
    quiet_fetch: bool,
}

fn mb(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / 1048576.0)
}

impl PackManager {
    pub fn downloading(&self) -> bool {
        self.download.is_some()
    }

    fn refresh(&mut self) {
        self.installed = packs::installed();
        self.loaded = true;
    }

    fn catalog(&self) -> Vec<PackInfo> {
        let mut list = packs::bundled_manifest().packs;
        if let Some(remote) = &self.remote {
            for p in &remote.packs {
                match list.iter_mut().find(|q| q.id == p.id) {
                    Some(q) if p.version > q.version => *q = p.clone(),
                    Some(_) => {}
                    None => list.push(p.clone()),
                }
            }
        }
        list.sort_by_key(|p| (p.kind as u8, p.order));
        list
    }
}

/// 已安裝的語言包中，可用來辨識的精度（偵測＋中英日）與額外語言
fn usable(installed: &[InstalledPack]) -> (Option<InstalledPack>, Vec<InstalledPack>, Vec<InstalledPack>) {
    let engine = installed.iter().find(|p| p.info.kind == PackKind::Engine).cloned();
    let tiers = installed.iter().filter(|p| p.info.kind == PackKind::Tier).cloned().collect();
    let langs = installed.iter().filter(|p| p.info.kind == PackKind::Language).cloned().collect();
    (engine, tiers, langs)
}

impl App {
    /// 「文字辨識 (OCR)…」：未安裝語言包時改開語言管理
    pub fn open_ocr_dialog(&mut self) {
        if self.doc.is_none() || !self.editing {
            return;
        }
        let installed = packs::installed();
        let (engine, tiers, _) = usable(&installed);
        if engine.is_none() || tiers.is_empty() {
            self.packs_ui.open = true;
            self.packs_ui.refresh();
            self.packs_ui.message = Some("尚未安裝 OCR 語言包：請先下載「OCR 辨識引擎」與至少一個中英日語言包。".into());
            return;
        }
        let tier = if tiers.iter().any(|t| t.info.id == self.settings.ocr_tier) { self.settings.ocr_tier.clone() } else { tiers[0].info.id.clone() };
        let extra = self.settings.ocr_langs.iter().filter(|l| installed.iter().any(|p| &p.info.id == *l)).cloned().collect();
        self.ocr_dialog = Some(OcrDialog {
            range: OcrRange::Current,
            pages: (self.page + 1).to_string(),
            tier,
            extra,
            skip_text: true,
            error: None,
            installed,
        });
    }

    pub fn show_ocr_dialog(&mut self, ctx: &egui::Context) {
        let Some(d) = self.ocr_dialog.as_mut() else { return };
        let (_, tiers, langs) = usable(&d.installed);
        let mut start = false;
        let mut close = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let mut manage = false;
        let page_count = self.doc.as_ref().map(|d| d.page_count()).unwrap_or(0);
        Modal::new(Id::new("dlg_ocr")).show(ctx, |ui| {
            ui.set_width(520.0);
            ui.label(RichText::new("文字辨識 (OCR)").strong().size(16.0));
            ui.add_space(6.0);
            ui.label(RichText::new("辨識掃描頁面上的文字，加上看不見的文字層：頁面外觀不變，但可以搜尋、選取、複製，也能沿著文字高亮。").color(ui.visuals().weak_text_color()));
            ui.add_space(6.0);
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new("頁面").strong());
                ui.horizontal(|ui| {
                    ui.radio_value(&mut d.range, OcrRange::Current, "目前頁");
                    ui.radio_value(&mut d.range, OcrRange::All, format!("全部（{page_count} 頁）"));
                    ui.radio_value(&mut d.range, OcrRange::Pages, "指定頁碼：");
                    ui.add_enabled(d.range == OcrRange::Pages, egui::TextEdit::singleline(&mut d.pages).hint_text("例如：1,3,5-8").desired_width(120.0));
                });
                ui.checkbox(&mut d.skip_text, "略過已有文字的頁面（非掃描頁）");
            });
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new("精度（中文繁／簡、英文、日文）").strong());
                for t in &tiers {
                    ui.radio_value(&mut d.tier, t.info.id.clone(), &t.info.name);
                    ui.indent(&t.info.id, |ui| ui.label(RichText::new(&t.info.description).color(ui.visuals().weak_text_color())));
                }
                if !langs.is_empty() {
                    ui.add_space(4.0);
                    ui.label(RichText::new("其他語言（與上面的中英日一起辨識，逐行取較佳的結果）").strong());
                    for l in &langs {
                        let mut on = d.extra.contains(&l.info.id);
                        if ui.checkbox(&mut on, &l.info.name).changed() {
                            if on {
                                d.extra.push(l.info.id.clone());
                            } else {
                                d.extra.retain(|x| x != &l.info.id);
                            }
                        }
                    }
                }
            });
            ui.label(RichText::new("※ 辨識結果與其他編輯相同：另存新檔才會寫入，取消編輯即捨棄。").color(ui.visuals().weak_text_color()));
            if let Some(e) = &d.error {
                ui.colored_label(ui.visuals().error_fg_color, e);
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("OCR 語言管理…").clicked() {
                    manage = true;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("取消").clicked() {
                        close = true;
                    }
                    if ui.add(egui::Button::new("開始辨識").min_size(egui::vec2(80.0, 0.0))).clicked() {
                        start = true;
                    }
                });
            });
        });
        if manage {
            self.ocr_dialog = None;
            self.packs_ui.open = true;
            self.packs_ui.refresh();
            return;
        }
        if close {
            self.ocr_dialog = None;
            return;
        }
        if start {
            if let Err(e) = self.start_ocr() {
                if let Some(d) = self.ocr_dialog.as_mut() {
                    d.error = Some(e);
                }
            }
        }
    }

    fn start_ocr(&mut self) -> Result<(), String> {
        let d = self.ocr_dialog.as_ref().ok_or("")?;
        let doc = self.doc.as_ref().ok_or("尚未開啟文件")?;
        let pages: Vec<usize> = match d.range {
            OcrRange::Current => vec![self.page],
            OcrRange::All => (0..doc.page_count()).collect(),
            OcrRange::Pages => pdfcore::pagerange::parse_page_range(&d.pages, doc.page_count())?,
        };
        let find = |id: &str| d.installed.iter().find(|p| p.info.id == id).cloned();
        let engine = d.installed.iter().find(|p| p.info.kind == PackKind::Engine).cloned().ok_or("缺少 OCR 辨識引擎")?;
        let tier = find(&d.tier).ok_or("請選擇精度")?;
        let extra: Vec<InstalledPack> = d.extra.iter().filter_map(|id| find(id)).collect();
        self.settings.ocr_tier = d.tier.clone();
        self.settings.ocr_langs = d.extra.clone();
        self.settings.save();

        let skip_text = d.skip_text;
        let bytes = doc.bytes().clone();
        let password = doc.password().to_string();
        let progress = Arc::new(AtomicUsize::new(0));
        let cancel = Arc::new(AtomicBool::new(false));
        let status = Arc::new(Mutex::new("正在載入辨識模型…".to_string()));
        let total = pages.len();
        let (p2, c2, s2) = (progress.clone(), cancel.clone(), status.clone());
        let handle = std::thread::spawn(move || {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_ocr(&bytes, &password, &pages, skip_text, &engine, &tier, &extra, &p2, &c2, &s2)))
                .unwrap_or_else(|_| Err("辨識時發生錯誤".into()))
        });
        self.ocr_job = Some(OcrJob { progress, total, cancel, status, handle: Some(handle) });
        self.ocr_dialog = None;
        Ok(())
    }

    pub fn show_ocr_progress(&mut self, ctx: &egui::Context) {
        let Some(job) = self.ocr_job.as_mut() else { return };
        if job.handle.as_ref().is_some_and(|h| h.is_finished()) {
            let result = job.handle.take().map(|h| h.join().unwrap_or_else(|_| Err("辨識時發生錯誤".into())));
            self.ocr_job = None;
            match result {
                Some(Ok(pages)) => self.apply_ocr(pages),
                Some(Err(e)) if e == "已取消" => self.message("已取消文字辨識", 4.0),
                Some(Err(e)) => self.error_box(format!("文字辨識失敗：{e}")),
                None => {}
            }
            return;
        }
        let done = job.progress.load(Ordering::Relaxed);
        let status = job.status.lock().map(|s| s.clone()).unwrap_or_default();
        Modal::new(Id::new("dlg_ocr_progress")).show(ctx, |ui| {
            ui.set_width(360.0);
            ui.label(RichText::new("文字辨識 (OCR)").strong());
            ui.label(status);
            ui.add(egui::ProgressBar::new(done as f32 / job.total.max(1) as f32).text(format!("{done} / {} 頁", job.total)));
            ui.add_space(6.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("取消").clicked() {
                    job.cancel.store(true, Ordering::Relaxed);
                }
            });
        });
        ctx.request_repaint_after(std::time::Duration::from_millis(150));
    }

    /// 把辨識結果加入編輯（同一頁原有的 OCR 結果會被取代）；整次辨識為一個復原步驟
    fn apply_ocr(&mut self, pages: Vec<(usize, Vec<OcrTextLine>)>) {
        let chars: usize = pages.iter().map(|(_, l)| l.iter().map(|x| x.text.chars().filter(|c| !c.is_whitespace()).count()).sum::<usize>()).sum();
        let found: Vec<_> = pages.into_iter().filter(|(_, l)| !l.is_empty()).collect();
        if found.is_empty() {
            self.message("沒有辨識到文字（可能已有文字層，或頁面沒有文字）", 5.0);
            return;
        }
        self.model.snapshot();
        let n = found.len();
        for (page, lines) in found {
            self.model.marks.retain(|m| !(m.kind == MarkType::Ocr && m.page == page));
            self.model.marks.push(Mark { kind: MarkType::Ocr, page, ocr: lines, ..Default::default() });
        }
        self.selection.clear();
        self.coalesce = None;
        self.touch();
        self.message(format!("已辨識 {n} 頁、共 {chars} 字；另存新檔即可保存文字層"), 6.0);
    }

    // ------------------------------------------------------------ OCR 語言管理

    pub fn open_pack_manager(&mut self) {
        self.packs_ui.open = true;
        self.packs_ui.refresh();
    }

    pub fn show_pack_manager(&mut self, ctx: &egui::Context) {
        let pm = &mut self.packs_ui;
        if !pm.open {
            return;
        }
        if !pm.loaded {
            pm.refresh();
        }
        if !pm.auto_fetched && pm.fetching.is_none() {
            pm.auto_fetched = true;
            pm.quiet_fetch = true;
            pm.fetching = Some(std::thread::spawn(packs::fetch_manifest));
        }
        let mut popup: Option<String> = None;
        // 背景工作完成
        if pm.fetching.as_ref().is_some_and(|h| h.is_finished()) {
            match pm.fetching.take().map(|h| h.join().unwrap_or_else(|_| Err("取得清單失敗".into()))) {
                Some(Ok(m)) => {
                    pm.remote = Some(m);
                    if !pm.quiet_fetch {
                        pm.message = Some("已取得最新的語言包清單".into());
                    }
                }
                Some(Err(_)) if pm.quiet_fetch => {}
                Some(Err(e)) => {
                    pm.message = None;
                    pm.error = Some((String::new(), e));
                }
                None => {}
            }
        }
        if pm.importing.as_ref().is_some_and(|h| h.is_finished()) {
            match pm.importing.take().map(|h| h.join().unwrap_or_else(|_| Err("加入失敗".into()))) {
                Some(Ok(info)) => pm.message = Some(format!("已加入：{}", info.name)),
                Some(Err(e)) => {
                    pm.message = None;
                    pm.error = Some((String::new(), format!("加入失敗：{e}")));
                    popup = Some(format!("加入 OCR 語言包失敗：
{e}"));
                }
                None => {}
            }
            pm.refresh();
        }
        if let Some(dl) = pm.download.as_mut() {
            if dl.handle.as_ref().is_some_and(|h| h.is_finished()) {
                let r = dl.handle.take().map(|h| h.join().unwrap_or_else(|_| Err("下載時發生錯誤".into())));
                let id = dl.id.clone();
                pm.download = None;
                match r {
                    Some(Ok(info)) => pm.message = Some(format!("已安裝：{}", info.name)),
                    Some(Err(e)) if e == "已取消下載" => pm.message = Some(e),
                    Some(Err(e)) => {
                        pm.message = None;
                        popup = Some(format!("OCR 語言包下載失敗：
{e}"));
                        pm.error = Some((id, e));
                    }
                    None => {}
                }
                pm.refresh();
            }
        }
        let busy = pm.download.is_some() || pm.importing.is_some();
        let catalog = pm.catalog();
        let mut open = pm.open;
        let mut action: Option<(&'static str, PackInfo)> = None;
        let mut remove: Option<InstalledPack> = None;
        let mut fetch = false;
        let mut import = false;
        let center = ctx.content_rect().center();
        egui::Window::new("OCR 語言管理")
            .open(&mut open)
            .default_size([640.0, 560.0])
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(center)
            .collapsible(false)
            .show(ctx, |ui| {
            ui.label(RichText::new("OCR 需要「辨識引擎」與至少一個中英日語言包。下載的語言包存放在使用者資料夾，不需系統管理員權限。").color(ui.visuals().weak_text_color()));
            ui.horizontal(|ui| {
                ui.label("下載網站：");
                ui.hyperlink_to(packs::DOWNLOAD_PAGE, packs::DOWNLOAD_PAGE)
                    .on_hover_text("所有版本的 OCR 語言包都放在這裡；沒有網路的電腦可在別台下載 zip 後以「從檔案加入…」安裝");
            });
            ui.add_space(4.0);
            egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
                for info in &catalog {
                    let inst = pm.installed.iter().find(|p| p.info.id == info.id);
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&info.name).strong());
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                match inst {
                                    Some(p) if p.info.version < info.version => {
                                        if ui.add_enabled(!busy, egui::Button::new("更新")).clicked() {
                                            action = Some(("download", info.clone()));
                                        }
                                        ui.label("有新版本");
                                    }
                                    Some(p) => {
                                        if p.user {
                                            if ui.add_enabled(!busy, egui::Button::new("移除")).clicked() {
                                                remove = Some(p.clone());
                                            }
                                        }
                                        ui.label(RichText::new(if p.user { "✔ 已安裝" } else { "✔ 已安裝（安裝程式）" }).color(egui::Color32::from_rgb(0x2E, 0xA0, 0x43)));
                                    }
                                    None => {
                                        let downloading = pm.download.as_ref().is_some_and(|d| d.id == info.id);
                                        if !downloading && ui.add_enabled(!busy, egui::Button::new(format!("下載（{}）", mb(info.zip_size)))).clicked() {
                                            action = Some(("download", info.clone()));
                                        }
                                    }
                                }
                            });
                        });
                        ui.label(RichText::new(format!("{}（安裝後 {}）", info.description, mb(info.total_size()))).color(ui.visuals().weak_text_color()));
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("檔案：{}", info.url.rsplit('/').next().unwrap_or_default())).small().color(ui.visuals().weak_text_color()));
                            ui.hyperlink_to(RichText::new("手動下載").small(), &info.url)
                                .on_hover_text(format!("{}
下載後以「從檔案加入…」安裝", info.url));
                        });
                        if let Some((_, e)) = pm.error.as_ref().filter(|(id, _)| *id == info.id) {
                            ui.colored_label(ui.visuals().error_fg_color, format!("⚠ {e}"));
                        }
                        if let Some(dl) = pm.download.as_ref().filter(|d| d.id == info.id) {
                            ui.horizontal(|ui| {
                                let done = dl.done.load(Ordering::Relaxed);
                                ui.add(egui::ProgressBar::new(done as f32 / dl.total.max(1) as f32).text(format!("{} / {}", mb(done), mb(dl.total))).desired_width(360.0));
                                if ui.button("取消").clicked() {
                                    dl.cancel.store(true, Ordering::Relaxed);
                                }
                            });
                        }
                    });
                }
            });
            if let Some((_, e)) = pm.error.as_ref().filter(|(id, _)| id.is_empty()) {
                ui.colored_label(ui.visuals().error_fg_color, e);
            }
            if let Some(m) = &pm.message {
                ui.label(m.as_str());
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.add_enabled(pm.fetching.is_none(), egui::Button::new("重新整理清單")).on_hover_text("從網路取得最新的語言包清單").clicked() {
                    fetch = true;
                }
                if ui.add_enabled(!busy, egui::Button::new("從檔案加入…")).on_hover_text("加入已下載的語言包 zip（給沒有網路的電腦）").clicked() {
                    import = true;
                }
                if pm.fetching.is_some() || pm.importing.is_some() {
                    ui.spinner();
                }
            });
        });
        pm.open = open;
        if fetch {
            pm.message = Some("正在取得語言包清單…".into());
            pm.error = None;
            pm.quiet_fetch = false;
            pm.fetching = Some(std::thread::spawn(packs::fetch_manifest));
        }
        if import {
            if let Some(path) = rfd::FileDialog::new().set_title("加入 OCR 語言包").add_filter("OCR 語言包", &["zip"]).pick_file() {
                pm.message = Some("正在驗證並加入語言包…".into());
                pm.error = None;
                pm.importing = Some(std::thread::spawn(move || packs::install_zip(&path, None)));
            }
        }
        if let Some(p) = remove {
            pm.message = Some(match packs::remove(&p) {
                Ok(()) => format!("已移除：{}", p.info.name),
                Err(e) => e,
            });
            pm.refresh();
        }
        if let Some(("download", info)) = action {
            let done = Arc::new(AtomicU64::new(0));
            let cancel = Arc::new(AtomicBool::new(false));
            let (d2, c2) = (done.clone(), cancel.clone());
            // 還沒有辨識引擎時一起下載（語言包都需要它）
            let engine = (info.kind != PackKind::Engine && !pm.installed.iter().any(|p| p.info.kind == PackKind::Engine))
                .then(|| catalog.iter().find(|p| p.kind == PackKind::Engine).cloned())
                .flatten();
            let total = info.zip_size + engine.as_ref().map_or(0, |e| e.zip_size);
            let id = info.id.clone();
            let handle = std::thread::spawn(move || {
                let mut base = 0;
                if let Some(e) = &engine {
                    let mut progress = |n: u64, _: u64| {
                        d2.store(n, Ordering::Relaxed);
                        !c2.load(Ordering::Relaxed)
                    };
                    packs::download_and_install(e, &mut progress).map_err(|err| format!("{}：{err}", e.name))?;
                    base = e.zip_size;
                }
                let mut progress = |n: u64, _: u64| {
                    d2.store(base + n, Ordering::Relaxed);
                    !c2.load(Ordering::Relaxed)
                };
                packs::download_and_install(&info, &mut progress)
            });
            pm.message = None;
            pm.error = None;
            pm.download = Some(PackDownload { id, done, total, cancel, handle: Some(handle) });
        }
        if pm.fetching.is_some() || pm.download.is_some() || pm.importing.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
        if let Some(e) = popup {
            self.error_box(e);
        }
    }
}

/// 頁面文字選取：PDF 沒有文字時改用 OCR 結果（尚未存檔）
pub fn select_on_page(doc: &pdfcore::Document, marks: &[Mark], page: usize, a: Pt, b: Pt) -> (Vec<pdfcore::Quad>, String) {
    let pt = doc.page_text(page);
    if !pt.is_empty() {
        return pdfcore::document::select_in(&pt, a, b);
    }
    let lines: Vec<OcrTextLine> = marks.iter().filter(|m| m.kind == MarkType::Ocr && m.page == page).flat_map(|m| m.ocr.iter().cloned()).collect();
    if lines.is_empty() {
        return (Vec::new(), String::new());
    }
    pdfcore::document::select_in(&pdfcore::document::ocr_page_text(&lines, Pt::default()), a, b)
}

/// 背景執行：逐頁渲染並辨識；回傳每頁的結果（頁面座標）
#[allow(clippy::too_many_arguments)]
fn run_ocr(
    bytes: &[u8],
    password: &str,
    pages: &[usize],
    skip_text: bool,
    engine: &InstalledPack,
    tier: &InstalledPack,
    extra: &[InstalledPack],
    progress: &AtomicUsize,
    cancel: &AtomicBool,
    status: &Mutex<String>,
) -> Result<Vec<(usize, Vec<OcrTextLine>)>, String> {
    let doc = pdfcore::Document::open_bytes(bytes.to_vec(), "ocr", password).map_err(|e| e.to_string())?;
    let mut eng = ocr::OcrEngine::load(engine, tier, extra)?;
    let set = |s: String| {
        if let Ok(mut g) = status.lock() {
            *g = s;
        }
    };
    let mut out = Vec::new();
    for (i, &page) in pages.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return Err("已取消".into());
        }
        if skip_text && doc.page_text(page).plain_text().chars().filter(|c| !c.is_whitespace()).count() >= 10 {
            set(format!("第 {} 頁已有文字，略過", page + 1));
            progress.store(i + 1, Ordering::Relaxed);
            continue;
        }
        set(format!("正在辨識第 {} 頁…", page + 1));
        let (pw, ph) = doc.page_size(page);
        let scale = (OCR_DPI / 72.0).min(OCR_MAX_SIDE / pw.max(ph).max(1.0));
        let Some(img) = doc.render(page, scale) else { continue };
        // 實際比例（頁面過大時渲染器會自動縮小）
        let real = img.width as f64 / pw.max(1.0);
        let rgb = ocr::RgbImage::from_rgba(img.width as usize, img.height as usize, &img.rgba);
        let lines = eng.recognize(&rgb, cancel)?;
        let to_pt = |p: [f32; 2]| Pt::new(p[0] as f64 / real, p[1] as f64 / real);
        let lines: Vec<OcrTextLine> = lines
            .into_iter()
            .map(|l| OcrTextLine {
                text: l.text,
                confidence: l.confidence,
                glyphs: l.chars.into_iter().map(|c| OcrGlyph { text: c.text, quad: c.quad.map(to_pt) }).collect(),
            })
            .collect();
        out.push((page, lines));
        progress.store(i + 1, Ordering::Relaxed);
    }
    Ok(out)
}
