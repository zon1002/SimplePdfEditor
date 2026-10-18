// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Fufeng Project

//! 主視窗的狀態與動作：開檔、存檔、匯出、列印、復原、套用屬性等。

use crate::dialogs::{Dialog, ExportForm, Pending, TextTarget};
use crate::help::HelpState;
use crate::icons::IconId;
use crate::preview::PrintPreview;
use crate::settings::Settings;
use egui::TextureHandle;
use pdfcore::fonts::FontEntry;
use pdfcore::save::{SaveOptions, save, save_to_bytes};
use pdfcore::{Document, EditModel, Image, Mark, MarkList, MarkType, OpenError, Pt, Rgba};
use std::collections::HashMap;
use std::path::Path;

pub const APP_TITLE: &str = "簡易 PDF 編輯器";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tool {
    Select,
    Highlight,
    Underline,
    Text,
    Line,
    Arrow,
    Rect,
    RoundRect,
    Ellipse,
}

impl Tool {
    pub const ALL: [Tool; 9] = [
        Tool::Select,
        Tool::Highlight,
        Tool::Underline,
        Tool::Text,
        Tool::Line,
        Tool::Arrow,
        Tool::Rect,
        Tool::RoundRect,
        Tool::Ellipse,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Tool::Select => "選取",
            Tool::Highlight => "高亮",
            Tool::Underline => "底線",
            Tool::Text => "文字",
            Tool::Line => "線條",
            Tool::Arrow => "箭頭",
            Tool::Rect => "矩形",
            Tool::RoundRect => "圓角矩形",
            Tool::Ellipse => "圓形",
        }
    }
    pub fn key(self) -> egui::Key {
        match self {
            Tool::Select => egui::Key::V,
            Tool::Highlight => egui::Key::H,
            Tool::Underline => egui::Key::U,
            Tool::Text => egui::Key::T,
            Tool::Line => egui::Key::L,
            Tool::Arrow => egui::Key::A,
            Tool::Rect => egui::Key::R,
            Tool::RoundRect => egui::Key::Q,
            Tool::Ellipse => egui::Key::O,
        }
    }
    pub fn key_name(self) -> &'static str {
        match self {
            Tool::Select => "V",
            Tool::Highlight => "H",
            Tool::Underline => "U",
            Tool::Text => "T",
            Tool::Line => "L",
            Tool::Arrow => "A",
            Tool::Rect => "R",
            Tool::RoundRect => "Q",
            Tool::Ellipse => "O",
        }
    }
    pub fn icon(self) -> IconId {
        match self {
            Tool::Select => IconId::Select,
            Tool::Highlight => IconId::Highlight,
            Tool::Underline => IconId::Underline,
            Tool::Text => IconId::Text,
            Tool::Line => IconId::Line,
            Tool::Arrow => IconId::Arrow,
            Tool::Rect => IconId::Rect,
            Tool::RoundRect => IconId::RoundRect,
            Tool::Ellipse => IconId::Ellipse,
        }
    }
    /// 以拖曳畫出的框（矩形、圓角矩形、圓形）
    pub fn is_shape(self) -> bool {
        matches!(self, Tool::Rect | Tool::RoundRect | Tool::Ellipse)
    }
    pub fn hint(self) -> &'static str {
        match self {
            Tool::Select => "選取：在空白處拖曳出選取框，Ctrl+C 複製框內的文字與圖片；點選註記可拖曳移動、Delete 刪除；雙擊文字可編輯",
            Tool::Highlight => "高亮：拖曳選取文字即加上螢光標記（無文字層時以矩形標記）",
            Tool::Underline => "底線：拖曳選取文字即加上底線",
            Tool::Text => "文字：在頁面上點一下輸入文字；點既有文字可修改",
            Tool::Line => "線條：拖曳畫線，按住 Shift 鎖定 45° 角",
            Tool::Arrow => "箭頭：拖曳畫箭頭（終點為箭頭端），按住 Shift 鎖定 45° 角",
            Tool::Rect => "矩形：拖曳畫出矩形框（中間鏤空，不遮蓋內容），按住 Shift 畫正方形",
            Tool::RoundRect => "圓角矩形：拖曳畫出四角圓弧的矩形框，圓弧半徑可調，按住 Shift 畫正方形",
            Tool::Ellipse => "圓形：拖曳畫出圓形／橢圓框（中間鏤空），按住 Shift 畫正圓",
        }
    }
    fn index(self) -> usize {
        Tool::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }
}

/// 新註記使用的屬性（由屬性工具列設定）
pub struct ToolProps {
    pub colors: [Rgba; 9],
    pub font: usize,
    pub font_size: f64,
    pub line_width: f64,
    pub text_border: bool,
    pub text_fill: bool,
    pub border_color: Rgba,
    pub fill_color: Rgba,
    pub border_width: f64,
    pub arrow_size: f64,
    pub both_ends: bool,
    pub dashed: bool,
    pub dash_len: f64,
    pub gap_len: f64,
    /// 圓角矩形的圓弧半徑（pt）
    pub corner_radius: f64,
}

impl Default for ToolProps {
    fn default() -> Self {
        let red = Rgba::rgb(0xE5, 0x39, 0x35);
        Self {
            // 每個工具各自記住顏色：選取、高亮、底線、文字、線條、箭頭
            colors: [red, Rgba::rgb(0xFF, 0xEB, 0x3B), red, Rgba::rgb(0x1A, 0x1A, 0x1A), red, red, red, red, red],
            font: 0,
            font_size: 16.0,
            line_width: 2.0,
            text_border: false,
            text_fill: false,
            border_color: Rgba::rgb(0x1A, 0x1A, 0x1A),
            fill_color: Rgba::rgb(0xFF, 0xF5, 0x9D),
            border_width: 1.0,
            arrow_size: 14.0,
            both_ends: false,
            dashed: false,
            dash_len: 6.0,
            gap_len: 4.0,
            corner_radius: 12.0,
        }
    }
}

/// 屬性工具列要顯示的部分
#[derive(Clone, Copy, Debug, Default)]
pub struct Bars {
    /// 文字樣式
    pub text: bool,
    /// 線寬與線型（線條、箭頭、框）
    pub stroke: bool,
    /// 線條／箭頭專用（箭頭大小、單向／雙向）
    pub line: bool,
    pub arrow: bool,
    /// 圓角半徑
    pub radius: bool,
}

/// 頁面上的滑鼠操作
pub enum Drag {
    None,
    /// 選取工具：拖曳移動選取的註記
    Moving { start: Pt, before: MarkList, moved: bool },
    /// 高亮／底線：拖曳選取文字
    Selecting { start: Pt, quads: Vec<pdfcore::Quad>, text: String },
    /// 線條／箭頭
    Drawing { start: Pt, cur: Pt },
    /// 文字工具：按下後等放開再開啟對話框
    TextClick { at: Pt },
    /// 選取工具（或檢視模式）在空白處拖曳：選取框，選取框內的文字與圖片
    Marquee { start: Pt, cur: Pt },
    /// 選取工具：拖曳框的控制點調整大小（handle：0 左上、1 上、2 右上、3 右、4 右下、5 下、6 左下、7 左）
    Resizing { index: usize, handle: u8, before: MarkList, moved: bool },
}

/// 列印工作（背景執行緒）
pub struct PrintJob {
    pub progress: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    pub cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub total: usize,
    pub handle: Option<std::thread::JoinHandle<Result<bool, String>>>,
}

pub struct App {
    pub doc: Option<Document>,
    pub model: EditModel,
    /// 註記清單的版本：每次修改遞增，用來判斷快取是否過期
    pub marks_gen: u64,
    pub page: usize,
    /// 縮放：每 pt 幾個介面點
    pub zoom: f64,
    pub tool: Tool,
    /// 編輯模式：按「編輯」後才可加入或修改註記；檢視模式不會改動任何內容
    pub editing: bool,
    pub props: ToolProps,
    pub fonts: Vec<FontEntry>,
    pub selection: Vec<usize>,
    pub drag: Drag,
    pub last_text: String,
    pub settings: Settings,

    // 檢視狀態
    pub fit_pending: bool,
    pub pending_scroll: Option<egui::Vec2>,
    pub scroll_to_top: bool,
    pub view_size: egui::Vec2,
    pub cursor_pt: Option<(Pt, bool)>,
    pub cursor_screen: Option<egui::Pos2>,
    /// 目前頁面左上角在畫面上的位置（頁面座標 × 縮放 + 此值 = 畫面座標）
    pub page_origin: egui::Pos2,
    /// 頁面點陣（未疊加高亮）：(頁碼, 縮放) -> 影像
    pub base_img: Option<((usize, u64), Image)>,
    /// 頁面貼圖：(頁碼, 縮放, 註記版本, 是否排除移動中的高亮)
    pub page_tex: Option<((usize, u64, u64, bool), TextureHandle)>,
    pub text_tex: HashMap<u64, (TextureHandle, pdfcore::Rect)>,
    pub icon_tex: HashMap<(IconId, u8, bool, u32), TextureHandle>,
    /// 左側欄的頁面縮圖（頁碼 -> 貼圖）
    pub thumbs: HashMap<usize, TextureHandle>,
    /// 側欄最後捲動到的頁面（目前頁改變時自動捲動）
    pub thumb_follow: Option<usize>,
    /// 收合的書籤（書籤索引）
    pub collapsed: std::collections::HashSet<usize>,

    pub status: Option<(String, f64)>,
    pub dialog: Option<Dialog>,
    pub help: HelpState,
    pub about_open: bool,
    pub preview: Option<PrintPreview>,
    pub print_job: Option<PrintJob>,
    /// 文字辨識：對話框、背景工作、語言管理
    pub ocr_dialog: Option<crate::ocr_ui::OcrDialog>,
    pub ocr_job: Option<crate::ocr_ui::OcrJob>,
    pub packs_ui: crate::ocr_ui::PackManager,
    /// 目前頁可選取的文字（原有文字＋OCR 結果），鍵為 (頁, marks_gen)
    pub select_cache: Option<((usize, u64), std::rc::Rc<pdfcore::text::PageText>)>,
    /// 選取框（可複製文字與圖片）
    pub region: Option<crate::copy::Region>,
    pub hwnd: isize,
    /// 連續調整同一屬性時合併成一次復原
    pub coalesce: Option<(&'static str, f64)>,
    pub allow_close: bool,
    pub title: String,
    pub now: f64,
}

impl App {
    pub fn new(hwnd: isize) -> Self {
        let fonts = pdfcore::fonts::system_fonts();
        Self {
            doc: None,
            model: EditModel::default(),
            marks_gen: 0,
            page: 0,
            zoom: 1.0,
            tool: Tool::Highlight,
            editing: false,
            props: ToolProps::default(),
            fonts,
            selection: Vec::new(),
            drag: Drag::None,
            last_text: String::new(),
            settings: Settings::load(),
            fit_pending: false,
            pending_scroll: None,
            scroll_to_top: false,
            view_size: egui::vec2(800.0, 600.0),
            cursor_pt: None,
            cursor_screen: None,
            page_origin: egui::Pos2::ZERO,
            base_img: None,
            page_tex: None,
            text_tex: HashMap::new(),
            icon_tex: HashMap::new(),
            thumbs: HashMap::new(),
            thumb_follow: None,
            collapsed: std::collections::HashSet::new(),
            status: None,
            dialog: None,
            help: HelpState::default(),
            about_open: false,
            preview: None,
            print_job: None,
            ocr_dialog: None,
            ocr_job: None,
            packs_ui: Default::default(),
            select_cache: None,
            region: None,
            hwnd,
            coalesce: None,
            allow_close: false,
            title: String::new(),
            now: 0.0,
        }
    }

    pub fn is_open(&self) -> bool {
        self.doc.is_some()
    }

    pub fn page_count(&self) -> usize {
        self.doc.as_ref().map(|d| d.page_count()).unwrap_or(0)
    }

    pub fn message(&mut self, text: impl Into<String>, secs: f64) {
        self.status = Some((text.into(), self.now + secs));
    }

    pub fn error_box(&mut self, text: impl Into<String>) {
        self.dialog = Some(Dialog::Message { title: APP_TITLE.into(), text: text.into() });
    }

    /// 註記有變動：遞增版本並清除選取以外的快取
    pub fn touch(&mut self) {
        self.marks_gen = self.marks_gen.wrapping_add(1);
    }

    pub fn tool_color(&self) -> Rgba {
        self.props.colors[self.tool.index()]
    }

    pub fn current_font(&self) -> Option<&FontEntry> {
        self.fonts.get(self.props.font)
    }

    // ------------------------------------------------------------ 開檔

    /// 開啟前確認未儲存的編輯
    pub fn request(&mut self, action: Pending) {
        if self.model.dirty {
            self.dialog = Some(Dialog::Discard { then: action });
        } else {
            self.run_pending(action);
        }
    }

    pub fn run_pending(&mut self, action: Pending) {
        match action {
            Pending::OpenDialog => {
                if let Some(p) = rfd::FileDialog::new().set_title("開啟 PDF").add_filter("PDF 檔案", &["pdf"]).pick_file() {
                    self.open_path(&p.to_string_lossy());
                }
            }
            Pending::OpenPath(p) => self.open_path(&p),
            Pending::Close => self.allow_close = true,
        }
    }

    pub fn open_path(&mut self, path: &str) {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                self.error_box(format!("無法開啟：{path}\n無法讀取檔案：{e}"));
                return;
            }
        };
        self.open_bytes(path, bytes, "");
    }

    pub fn open_bytes(&mut self, path: &str, bytes: Vec<u8>, password: &str) {
        match Document::open_bytes(bytes.clone(), path, password) {
            Ok(doc) => {
                self.doc = Some(doc);
                self.editing = false;
                self.model.reset();
                self.touch();
                self.selection.clear();
                self.region = None;
                self.drag = Drag::None;
                self.page = 0;
                self.base_img = None;
                self.page_tex = None;
                self.text_tex.clear();
                self.thumbs.clear();
                self.thumb_follow = None;
                self.collapsed.clear();
                self.fit_pending = true;
                self.scroll_to_top = true;
                self.dialog = None;
            }
            Err(OpenError::NeedsPassword) => {
                let retry = !password.is_empty();
                self.dialog = Some(Dialog::Password { path: path.to_string(), bytes, input: String::new(), error: retry });
            }
            Err(OpenError::Invalid(e)) => self.error_box(format!("無法開啟：{path}\n{e}")),
        }
    }

    /// 「編輯／取消編輯」按鈕
    pub fn toggle_edit(&mut self) {
        if self.doc.is_none() {
            return;
        }
        if !self.editing {
            self.editing = true;
            self.message("編輯模式：可加入註記；按「取消編輯」放棄所有修改，或另存新檔保留", 5.0);
        } else if self.model.dirty {
            self.dialog = Some(Dialog::CancelEdit);
        } else {
            self.cancel_edit();
        }
    }

    /// 放棄所有編輯，回到原始檔案的狀態
    pub fn cancel_edit(&mut self) {
        self.model.reset();
        self.selection.clear();
        self.drag = Drag::None;
        self.coalesce = None;
        self.editing = false;
        self.touch();
    }

    pub fn go_to_page(&mut self, page: usize) {
        let n = self.page_count();
        if n == 0 {
            return;
        }
        let page = page.min(n - 1);
        if page != self.page {
            self.page = page;
            self.selection.clear();
            self.region = None;
            self.drag = Drag::None;
            self.scroll_to_top = true;
        }
    }

    pub fn set_zoom(&mut self, zoom: f64) {
        self.zoom = zoom.clamp(0.25, 5.0);
    }

    pub fn fit_width(&mut self) {
        let Some(doc) = &self.doc else { return };
        let (w, _) = doc.page_size(self.page);
        // 左右各留 12pt 邊界，需一併算入，否則會出現水平捲軸
        let avail = (self.view_size.x as f64 - 18.0).max(100.0);
        self.set_zoom(avail / (w + 24.0));
    }

    pub fn set_tool(&mut self, t: Tool) {
        if !self.editing {
            return;
        }
        self.tool = t;
        self.drag = Drag::None;
        if t != Tool::Select {
            self.selection.clear();
        }
    }

    // ------------------------------------------------------------ 編輯

    pub fn undo(&mut self) {
        if self.editing && self.model.undo() {
            self.after_undo_redo();
        }
    }

    pub fn redo(&mut self) {
        if self.editing && self.model.redo() {
            self.after_undo_redo();
        }
    }

    fn after_undo_redo(&mut self) {
        self.selection.retain(|&i| i < self.model.marks.len());
        self.coalesce = None;
        self.touch();
    }

    pub fn delete_selected(&mut self) {
        if !self.editing || self.selection.is_empty() {
            return;
        }
        self.model.snapshot();
        let mut idx = std::mem::take(&mut self.selection);
        idx.sort_unstable();
        for &i in idx.iter().rev() {
            if i < self.model.marks.len() {
                self.model.marks.remove(i);
            }
        }
        self.touch();
    }

    pub fn copy_text(&mut self, ctx: &egui::Context) {
        if !self.last_text.is_empty() {
            ctx.copy_text(self.last_text.clone());
            self.message("已複製文字", 2.0);
        }
    }

    pub fn add_mark(&mut self, m: Mark) {
        self.model.snapshot();
        self.model.marks.push(m);
        self.coalesce = None;
        self.touch();
    }

    /// 把屬性套用到選取的註記；`key` 相同且持續調整時合併成一次復原
    pub fn apply_to_selection(&mut self, key: &'static str, f: impl Fn(&mut Mark) -> bool) {
        if !self.editing || self.selection.is_empty() {
            return;
        }
        let before = self.model.marks.clone();
        let mut changed = false;
        for &i in &self.selection {
            if let Some(m) = self.model.marks.get_mut(i) {
                changed |= f(m);
            }
        }
        if !changed {
            return;
        }
        let merge = matches!(self.coalesce, Some((k, t)) if k == key && self.now - t < 1.5);
        if merge {
            self.model.dirty = true;
        } else {
            self.model.record(before);
        }
        self.coalesce = Some((key, self.now));
        self.touch();
    }

    pub fn set_current_color(&mut self, c: Rgba) {
        if self.tool == Tool::Select && !self.selection.is_empty() {
            self.apply_to_selection("color", |m| {
                let changed = m.color != c;
                m.color = c;
                changed
            });
        } else {
            let i = self.tool.index();
            self.props.colors[i] = c;
        }
    }

    /// 選取工具時，屬性工具列依選到的註記類型顯示
    pub fn visible_bars(&self) -> Bars {
        let mut b = Bars {
            text: self.tool == Tool::Text,
            stroke: matches!(self.tool, Tool::Line | Tool::Arrow) || self.tool.is_shape(),
            line: matches!(self.tool, Tool::Line | Tool::Arrow),
            arrow: self.tool == Tool::Arrow,
            radius: self.tool == Tool::RoundRect,
        };
        if self.tool == Tool::Select {
            for &i in &self.selection {
                if let Some(m) = self.model.marks.get(i) {
                    b.text |= m.kind == MarkType::Text;
                    b.stroke |= m.is_stroked();
                    b.line |= m.is_line();
                    b.arrow |= m.kind == MarkType::Arrow;
                    b.radius |= m.kind == MarkType::Rect;
                }
            }
        }
        b
    }

    /// 選取改變時，屬性工具列顯示第一個選取註記的設定
    pub fn sync_from_selection(&mut self) {
        let Some(m) = self.selection.first().and_then(|&i| self.model.marks.get(i)).cloned() else { return };
        match m.kind {
            MarkType::Text => {
                if let Some(i) = self.fonts.iter().position(|f| f.path == m.font_path && f.index == m.font_index) {
                    self.props.font = i;
                }
                self.props.font_size = m.font_size;
                self.props.text_border = m.text_border;
                self.props.text_fill = m.text_fill;
                self.props.border_width = m.border_width;
                self.props.border_color = m.border_color;
                self.props.fill_color = m.fill_color;
            }
            MarkType::Line | MarkType::Arrow | MarkType::Rect | MarkType::Ellipse => {
                self.props.line_width = m.width;
                if m.kind == MarkType::Rect {
                    self.props.corner_radius = m.radius;
                }
                if m.kind == MarkType::Arrow {
                    self.props.arrow_size = m.arrow_size;
                    self.props.both_ends = m.both_ends;
                }
                self.props.dashed = m.dashed;
                self.props.dash_len = m.dash_len;
                self.props.gap_len = m.gap_len;
            }
            _ => {}
        }
    }

    pub fn set_selection(&mut self, sel: Vec<usize>) {
        let changed = sel != self.selection;
        self.selection = sel;
        if changed {
            self.coalesce = None;
            self.sync_from_selection();
        }
    }

    /// 以目前工具列設定建立矩形／圓角矩形／圓形框
    pub fn new_shape_mark(&self, a: Pt, b: Pt) -> Mark {
        Mark {
            kind: if self.tool == Tool::Ellipse { MarkType::Ellipse } else { MarkType::Rect },
            page: self.page,
            color: self.tool_color(),
            p1: a,
            p2: b,
            width: self.props.line_width,
            radius: if self.tool == Tool::RoundRect { self.props.corner_radius } else { 0.0 },
            dashed: self.props.dashed,
            dash_len: self.props.dash_len,
            gap_len: self.props.gap_len,
            ..Default::default()
        }
    }

    /// 以目前工具列設定建立線條/箭頭
    pub fn new_line_mark(&self, a: Pt, b: Pt) -> Mark {
        Mark {
            kind: if self.tool == Tool::Arrow { MarkType::Arrow } else { MarkType::Line },
            page: self.page,
            color: self.tool_color(),
            p1: a,
            p2: b,
            width: self.props.line_width,
            arrow_size: self.props.arrow_size,
            both_ends: self.props.both_ends,
            dashed: self.props.dashed,
            dash_len: self.props.dash_len,
            gap_len: self.props.gap_len,
            ..Default::default()
        }
    }

    pub fn begin_add_text(&mut self, at: Pt) {
        if self.current_font().is_none() {
            self.error_box("找不到可用的字型，請從「字型」選單選擇字型檔。");
            return;
        }
        self.dialog = Some(Dialog::Text { title: "新增文字", text: String::new(), target: TextTarget::New(at), focus: true });
    }

    pub fn begin_edit_text(&mut self, index: usize) {
        let Some(m) = self.model.marks.get(index) else { return };
        self.dialog = Some(Dialog::Text { title: "編輯文字", text: m.text.clone(), target: TextTarget::Edit(index), focus: true });
    }

    pub fn finish_text(&mut self, target: TextTarget, text: String) {
        match target {
            TextTarget::New(at) => {
                if text.trim().is_empty() {
                    return;
                }
                let Some(f) = self.current_font().cloned() else { return };
                let m = Mark {
                    kind: MarkType::Text,
                    page: self.page,
                    color: self.tool_color(),
                    text,
                    font_path: f.path,
                    font_index: f.index,
                    font_size: self.props.font_size,
                    pos: at,
                    text_border: self.props.text_border,
                    border_color: self.props.border_color,
                    border_width: self.props.border_width,
                    text_fill: self.props.text_fill,
                    fill_color: self.props.fill_color,
                    ..Default::default()
                };
                self.add_mark(m);
            }
            TextTarget::Edit(i) => {
                let Some(old) = self.model.marks.get(i) else { return };
                if old.text == text {
                    return;
                }
                self.model.snapshot();
                if text.trim().is_empty() {
                    self.model.marks.remove(i);
                    self.selection.clear();
                } else if let Some(m) = self.model.marks.get_mut(i) {
                    m.text = text;
                }
                self.coalesce = None;
                self.touch();
            }
        }
    }

    /// 「其他字型檔…」：載入任意 TTF／OTF／TTC
    pub fn pick_font_file(&mut self) {
        let Some(p) = rfd::FileDialog::new().set_title("選擇字型檔").add_filter("字型", &["ttf", "otf", "ttc"]).pick_file() else { return };
        let path = p.to_string_lossy().into_owned();
        match pdfcore::fonts::font_entry_from_file(&path) {
            Some(entry) => {
                let i = match self.fonts.iter().position(|f| f.path == entry.path) {
                    Some(i) => i,
                    None => {
                        self.fonts.push(entry);
                        self.fonts.len() - 1
                    }
                };
                self.set_font(i);
            }
            None => self.error_box("無法載入此字型檔"),
        }
    }

    pub fn set_font(&mut self, i: usize) {
        let Some(f) = self.fonts.get(i).cloned() else { return };
        self.props.font = i;
        self.apply_to_selection("font", |m| {
            if m.kind != MarkType::Text || (m.font_path == f.path && m.font_index == f.index) {
                return false;
            }
            m.font_path = f.path.clone();
            m.font_index = f.index;
            true
        });
    }

    // ------------------------------------------------------------ 存檔

    fn base_name(&self) -> (std::path::PathBuf, String) {
        let path = self.doc.as_ref().map(|d| d.path().to_string()).unwrap_or_default();
        let p = Path::new(&path);
        let dir = p.parent().map(|d| d.to_path_buf()).unwrap_or_default();
        let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "document".into());
        (dir, stem)
    }

    pub fn save_as(&mut self) -> bool {
        if self.doc.is_none() || !self.editing {
            return false;
        }
        let (dir, stem) = self.base_name();
        let Some(out) = rfd::FileDialog::new()
            .set_title("另存新檔")
            .set_directory(&dir)
            .set_file_name(format!("{stem}_編輯.pdf"))
            .add_filter("PDF 檔案", &["pdf"])
            .save_file()
        else {
            return false;
        };
        self.save_to_path(&out.to_string_lossy())
    }

    /// 存到指定檔案；成功後開啟存好的檔案並回到檢視模式
    pub fn save_to_path(&mut self, out: &str) -> bool {
        let Some(doc) = &self.doc else { return false };
        if !self.editing {
            return false;
        }
        let out = out.to_string();
        match save(doc, &self.model.marks, &SaveOptions::default(), &out) {
            Ok(()) => {
                // 存檔後改開啟存好的檔案（成為新的原始狀態），回到檢視模式；保留目前頁碼與縮放
                let (page, zoom) = (self.page, self.zoom);
                self.model.dirty = false;
                self.open_path(&out);
                if self.doc.as_ref().is_some_and(|d| d.path() == out) {
                    self.fit_pending = false;
                    self.zoom = zoom;
                    self.page = page.min(self.page_count().saturating_sub(1));
                }
                self.message(format!("已儲存：{out}"), 5.0);
                true
            }
            Err(e) => {
                self.error_box(format!("儲存失敗：{e}"));
                false
            }
        }
    }

    pub fn open_export_dialog(&mut self) {
        if self.doc.is_none() {
            return;
        }
        self.dialog = Some(Dialog::Export(ExportForm::new(self.page, self.page_count())));
    }

    pub fn export(&mut self, opt: SaveOptions, range_text: &str) {
        let Some(doc) = &self.doc else { return };
        let (dir, stem) = self.base_name();
        let tag: String = range_text.chars().filter(|c| c.is_ascii_digit() || *c == ',' || *c == '-').map(|c| if c == ',' { '_' } else { c }).collect();
        let suggest = format!("{stem}_p{tag}{}.pdf", if opt.encrypt { "_加密" } else { "" });
        let Some(out) = rfd::FileDialog::new().set_title("匯出頁面").set_directory(&dir).set_file_name(suggest).add_filter("PDF 檔案", &["pdf"]).save_file()
        else {
            return;
        };
        let out = out.to_string_lossy().into_owned();
        match save(doc, &self.model.marks, &opt, &out) {
            Ok(()) => {
                let enc = if opt.encrypt { "（已加密）" } else { "" };
                self.dialog = Some(Dialog::Message { title: APP_TITLE.into(), text: format!("已匯出 {} 頁{enc}：\n{out}", opt.pages.len()) });
            }
            Err(e) => self.error_box(format!("匯出失敗：{e}")),
        }
    }

    /// 含所有註記的 PDF（與另存新檔相同的結果），供列印與預覽
    pub fn printable_bytes(&mut self) -> Option<Vec<u8>> {
        let doc = self.doc.as_ref()?;
        match save_to_bytes(doc, &self.model.marks, &SaveOptions::default()) {
            Ok(b) => Some(b),
            Err(e) => {
                self.error_box(format!("準備列印失敗：{e}"));
                None
            }
        }
    }

    pub fn print(&mut self) {
        if self.doc.is_none() || self.print_job.is_some() {
            return;
        }
        let Some(setup) = crate::print::print_dialog(self.hwnd, self.page_count(), self.page) else { return };
        let Some(bytes) = self.printable_bytes() else { return };
        let name = self.base_name().1;
        let progress = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let total = setup.sequence.len();
        let (p, c) = (progress.clone(), cancel.clone());
        let handle = std::thread::spawn(move || {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| crate::print::run_print_job(setup, bytes, name, p, c)))
                .unwrap_or_else(|_| Err("列印時發生錯誤".into()))
        });
        self.print_job = Some(PrintJob { progress, cancel, total, handle: Some(handle) });
    }

    pub fn open_preview(&mut self) {
        if self.doc.is_none() {
            return;
        }
        let Some(bytes) = self.printable_bytes() else { return };
        match Document::open_bytes(bytes, "preview", "") {
            Ok(d) => self.preview = Some(PrintPreview::new(d, self.page)),
            Err(e) => self.error_box(format!("準備列印失敗：{e}")),
        }
    }

    pub fn window_title(&self) -> String {
        match &self.doc {
            Some(d) => {
                let name = Path::new(d.path()).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                let mode = if self.editing { "（編輯中）" } else { "" };
                format!("{name}{}{mode} - {APP_TITLE}", if self.model.dirty { "*" } else { "" })
            }
            None => APP_TITLE.to_string(),
        }
    }
}
