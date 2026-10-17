// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Fufeng Project

//! 對話框：文字輸入、密碼、匯出頁面 / 加密、未儲存確認、訊息。

use crate::app::{APP_TITLE, App};
use egui::{Id, Key, Modal, RichText};
use pdfcore::Pt;
use pdfcore::pagerange::parse_page_range;
use pdfcore::save::SaveOptions;

#[derive(Clone, Copy, Debug)]
pub enum TextTarget {
    New(Pt),
    Edit(usize),
}

/// 未儲存確認後要繼續執行的動作
#[derive(Clone, Debug)]
pub enum Pending {
    OpenDialog,
    OpenPath(String),
    Close,
}

pub struct ExportForm {
    pub current: usize,
    pub page_count: usize,
    pub range: String,
    pub include_marks: bool,
    pub encrypt: bool,
    pub pw1: String,
    pub pw2: String,
    pub owner: String,
    pub allow_print: bool,
    pub allow_copy: bool,
    pub allow_modify: bool,
    pub error: Option<String>,
    pub focus: bool,
}

impl ExportForm {
    pub fn new(current: usize, page_count: usize) -> Self {
        Self {
            current,
            page_count,
            range: (current + 1).to_string(),
            include_marks: true,
            encrypt: true,
            pw1: String::new(),
            pw2: String::new(),
            owner: String::new(),
            allow_print: true,
            allow_copy: false,
            allow_modify: false,
            error: None,
            focus: true,
        }
    }

    fn validate(&mut self) -> Option<SaveOptions> {
        let pages = match parse_page_range(&self.range, self.page_count) {
            Ok(p) => p,
            Err(e) => {
                self.error = Some(e);
                return None;
            }
        };
        let mut opt = SaveOptions { pages, include_marks: self.include_marks, encrypt: self.encrypt, ..Default::default() };
        if self.encrypt {
            if self.pw1.is_empty() {
                self.error = Some("請輸入開啟密碼".into());
                return None;
            }
            if self.pw1 != self.pw2 {
                self.error = Some("兩次輸入的密碼不一致".into());
                return None;
            }
            opt.user_password = self.pw1.clone();
            opt.owner_password = self.owner.clone();
            opt.allow_print = self.allow_print;
            opt.allow_copy = self.allow_copy;
            opt.allow_modify = self.allow_modify;
        }
        Some(opt)
    }
}

pub enum Dialog {
    Message { title: String, text: String },
    Password { path: String, bytes: Vec<u8>, input: String, error: bool },
    Text { title: &'static str, text: String, target: TextTarget, focus: bool },
    Export(ExportForm),
    Discard { then: Pending },
}

enum Outcome {
    Keep,
    Close,
    /// 開啟密碼保護的檔案
    Password(String, Vec<u8>, String),
    Text(TextTarget, String),
    Export(SaveOptions, String),
    DiscardSave(Pending),
    DiscardDrop(Pending),
}

fn buttons(ui: &mut egui::Ui, ok: &str, cancel: &str) -> (bool, bool) {
    let mut r = (false, false);
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if ui.button(cancel).clicked() {
            r.1 = true;
        }
        if ui.add(egui::Button::new(ok).min_size(egui::vec2(72.0, 0.0))).clicked() {
            r.0 = true;
        }
    });
    r
}

fn password_field(ui: &mut egui::Ui, text: &mut String, hint: &str) -> egui::Response {
    ui.add(egui::TextEdit::singleline(text).password(true).hint_text(hint).desired_width(240.0))
}

pub fn show(app: &mut App, ctx: &egui::Context) {
    let Some(dialog) = app.dialog.as_mut() else { return };
    let mut outcome = Outcome::Keep;
    let escape = ctx.input(|i| i.key_pressed(Key::Escape));
    match dialog {
        Dialog::Message { title, text } => {
            Modal::new(Id::new("dlg_message")).show(ctx, |ui| {
                ui.set_max_width(460.0);
                ui.label(RichText::new(title.as_str()).strong().size(16.0));
                ui.add_space(6.0);
                ui.label(text.as_str());
                ui.add_space(10.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::Button::new("確定").min_size(egui::vec2(72.0, 0.0))).clicked() || ui.input(|i| i.key_pressed(Key::Enter)) {
                        outcome = Outcome::Close;
                    }
                });
            });
            if escape {
                outcome = Outcome::Close;
            }
        }
        Dialog::Password { path, bytes, input, error } => {
            Modal::new(Id::new("dlg_password")).show(ctx, |ui| {
                let name = std::path::Path::new(path.as_str()).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                ui.label(RichText::new("需要密碼").strong().size(16.0));
                ui.add_space(6.0);
                ui.label(format!("「{name}」已加密，請輸入密碼："));
                let r = password_field(ui, input, "");
                r.request_focus();
                if *error {
                    ui.colored_label(ui.visuals().error_fg_color, "密碼錯誤，請再試一次。");
                }
                ui.add_space(8.0);
                let (ok, cancel) = buttons(ui, "確定", "取消");
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                if ok || enter {
                    outcome = Outcome::Password(path.clone(), std::mem::take(bytes), input.clone());
                } else if cancel {
                    outcome = Outcome::Close;
                }
            });
            if escape {
                outcome = Outcome::Close;
            }
        }
        Dialog::Text { title, text, target, focus } => {
            Modal::new(Id::new("dlg_text")).show(ctx, |ui| {
                ui.label(RichText::new(*title).strong().size(16.0));
                ui.add_space(6.0);
                ui.label("輸入文字（可多行）：");
                // Ctrl+Enter 也可以確定（Enter 用來換行）；需在輸入框取走 Enter 之前判斷
                let ctrl_enter = ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, Key::Enter));
                let r = ui.add(egui::TextEdit::multiline(text).desired_width(380.0).desired_rows(5).font(egui::FontId::proportional(18.0)));
                if *focus {
                    r.request_focus();
                    *focus = false;
                }
                ui.add_space(8.0);
                let (ok, cancel) = buttons(ui, "確定", "取消");
                if ok || ctrl_enter {
                    outcome = Outcome::Text(*target, text.clone());
                } else if cancel {
                    outcome = Outcome::Close;
                }
            });
            if escape {
                outcome = Outcome::Close;
            }
        }
        Dialog::Export(f) => {
            Modal::new(Id::new("dlg_export")).show(ctx, |ui| {
                ui.set_width(420.0);
                ui.label(RichText::new("匯出頁面 / 加密").strong().size(16.0));
                ui.add_space(6.0);
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new("頁面").strong());
                    ui.horizontal(|ui| {
                        ui.label("頁碼範圍：");
                        ui.add(egui::TextEdit::singleline(&mut f.range).hint_text("例如：1,3,5-8").desired_width(160.0));
                        if ui.button("目前頁").clicked() {
                            f.range = (f.current + 1).to_string();
                        }
                        if ui.button("全部").clicked() {
                            f.range = format!("1-{}", f.page_count);
                        }
                    });
                    ui.label(RichText::new(format!("共 {} 頁，可用逗號與連字號，例如 1,3,5-8", f.page_count)).color(ui.visuals().weak_text_color()));
                    ui.checkbox(&mut f.include_marks, "包含高亮、底線、文字、線條等註記");
                });
                ui.add_space(4.0);
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.checkbox(&mut f.encrypt, RichText::new("加密（AES-256）").strong());
                    ui.add_enabled_ui(f.encrypt, |ui| {
                        egui::Grid::new("export_pw").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
                            ui.label("開啟密碼：");
                            let r = password_field(ui, &mut f.pw1, "");
                            if f.focus {
                                r.request_focus();
                                f.focus = false;
                            }
                            ui.end_row();
                            ui.label("確認密碼：");
                            password_field(ui, &mut f.pw2, "");
                            ui.end_row();
                            ui.label("權限密碼：");
                            password_field(ui, &mut f.owner, "選填；留空 = 與開啟密碼相同");
                            ui.end_row();
                        });
                        ui.checkbox(&mut f.allow_print, "允許列印");
                        ui.checkbox(&mut f.allow_copy, "允許複製內容");
                        ui.checkbox(&mut f.allow_modify, "允許修改與註解");
                        ui.label(RichText::new("※ 權限限制需設定與開啟密碼不同的權限密碼才有效").color(ui.visuals().weak_text_color()));
                    });
                });
                if let Some(e) = &f.error {
                    ui.colored_label(ui.visuals().error_fg_color, e);
                }
                ui.add_space(8.0);
                let (ok, cancel) = buttons(ui, "匯出…", "取消");
                if ok {
                    if let Some(opt) = f.validate() {
                        outcome = Outcome::Export(opt, f.range.trim().to_string());
                    }
                } else if cancel {
                    outcome = Outcome::Close;
                }
            });
            if escape {
                outcome = Outcome::Close;
            }
        }
        Dialog::Discard { then } => {
            Modal::new(Id::new("dlg_discard")).show(ctx, |ui| {
                ui.label(RichText::new(APP_TITLE).strong().size(16.0));
                ui.add_space(6.0);
                ui.label("目前的編輯尚未儲存，要先另存新檔嗎？");
                ui.add_space(10.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("取消").clicked() {
                        outcome = Outcome::Close;
                    }
                    if ui.button("不儲存").clicked() {
                        outcome = Outcome::DiscardDrop(then.clone());
                    }
                    if ui.add(egui::Button::new("儲存…").min_size(egui::vec2(72.0, 0.0))).clicked() {
                        outcome = Outcome::DiscardSave(then.clone());
                    }
                });
            });
            if escape {
                outcome = Outcome::Close;
            }
        }
    }

    match outcome {
        Outcome::Keep => {}
        Outcome::Close => app.dialog = None,
        Outcome::Password(path, bytes, pw) => {
            app.dialog = None;
            app.open_bytes(&path, bytes, &pw);
        }
        Outcome::Text(target, text) => {
            app.dialog = None;
            app.finish_text(target, text);
        }
        Outcome::Export(opt, range) => {
            app.dialog = None;
            app.export(opt, &range);
        }
        Outcome::DiscardSave(then) => {
            app.dialog = None;
            if app.save_as() {
                app.run_pending(then);
            }
        }
        Outcome::DiscardDrop(then) => {
            app.dialog = None;
            app.model.dirty = false;
            app.run_pending(then);
        }
    }
}

/// 列印進度（背景列印時顯示，可取消）
pub fn show_print_progress(app: &mut App, ctx: &egui::Context) {
    let Some(job) = app.print_job.as_mut() else { return };
    let done = job.progress.load(std::sync::atomic::Ordering::Relaxed);
    let finished = job.handle.as_ref().is_some_and(|h| h.is_finished());
    if finished {
        let result = job.handle.take().map(|h| h.join().unwrap_or_else(|_| Err("列印時發生錯誤".into())));
        app.print_job = None;
        match result {
            Some(Ok(true)) => app.message("已送出列印", 4.0),
            Some(Ok(false)) => app.message("已取消列印", 4.0),
            Some(Err(e)) => app.error_box(e),
            None => {}
        }
        return;
    }
    Modal::new(Id::new("dlg_print")).show(ctx, |ui| {
        ui.set_width(320.0);
        ui.label("正在列印…");
        ui.add(egui::ProgressBar::new(done as f32 / job.total.max(1) as f32).text(format!("{done} / {}", job.total)));
        ui.add_space(6.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("取消").clicked() {
                job.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        });
    });
    ctx.request_repaint_after(std::time::Duration::from_millis(100));
}
