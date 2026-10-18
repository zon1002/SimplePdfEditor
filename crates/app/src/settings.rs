// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Fufeng Project

//! 使用者設定（工具列圖示、尺規）：存在 %APPDATA%\SimplePdfEditor\settings.ini。

use crate::icons::IconStyle;
use crate::ruler::RulerUnit;
use crate::sidebar::SidebarTab;
use std::path::PathBuf;

/// 工具列按鈕的顯示方式
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonMode {
    TextUnderIcon,
    TextBesideIcon,
    IconOnly,
    TextOnly,
}

impl ButtonMode {
    pub const ALL: [ButtonMode; 4] = [ButtonMode::TextUnderIcon, ButtonMode::TextBesideIcon, ButtonMode::IconOnly, ButtonMode::TextOnly];

    pub fn name(self) -> &'static str {
        match self {
            ButtonMode::TextUnderIcon => "圖示＋下方文字",
            ButtonMode::TextBesideIcon => "圖示＋右側文字",
            ButtonMode::IconOnly => "只顯示圖示",
            ButtonMode::TextOnly => "只顯示文字",
        }
    }
    fn key(self) -> &'static str {
        match self {
            ButtonMode::TextUnderIcon => "under",
            ButtonMode::TextBesideIcon => "beside",
            ButtonMode::IconOnly => "icon",
            ButtonMode::TextOnly => "text",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub icon_style: IconStyle,
    pub button_mode: ButtonMode,
    pub rulers: bool,
    pub ruler_unit: RulerUnit,
    /// 左側欄（頁面縮圖／書籤）
    pub sidebar: bool,
    pub sidebar_tab: SidebarTab,
    /// 上次使用的 OCR 精度與額外語言
    pub ocr_tier: String,
    pub ocr_langs: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            icon_style: IconStyle::Color,
            button_mode: ButtonMode::TextUnderIcon,
            rulers: true,
            ruler_unit: RulerUnit::Millimeter,
            sidebar: true,
            sidebar_tab: SidebarTab::Pages,
            ocr_tier: "std".into(),
            ocr_langs: Vec::new(),
        }
    }
}

fn path() -> Option<PathBuf> {
    let dir = std::env::var_os("APPDATA")?;
    Some(PathBuf::from(dir).join("SimplePdfEditor").join("settings.ini"))
}

impl Settings {
    pub fn load() -> Settings {
        let mut s = Settings::default();
        let Some(text) = path().and_then(|p| std::fs::read_to_string(p).ok()) else { return s };
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let v = v.trim();
            match k.trim() {
                "iconStyle" => s.icon_style = IconStyle::from_index(v.parse().unwrap_or(2)),
                "buttonStyle" => {
                    if let Some(m) = ButtonMode::ALL.iter().find(|m| m.key() == v) {
                        s.button_mode = *m;
                    }
                }
                "rulers" => s.rulers = v != "0",
                "sidebar" => s.sidebar = v != "0",
                "sidebarTab" => s.sidebar_tab = SidebarTab::from_key(v),
                "ocrTier" => s.ocr_tier = v.to_string(),
                "ocrLangs" => s.ocr_langs = v.split(',').filter(|x| !x.is_empty()).map(str::to_string).collect(),
                "rulerUnit" => {
                    if let Some(u) = RulerUnit::ALL.iter().find(|u| u.suffix() == v) {
                        s.ruler_unit = *u;
                    }
                }
                _ => {}
            }
        }
        s
    }

    pub fn save(&self) {
        let Some(p) = path() else { return };
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let text = format!(
            "iconStyle={}\nbuttonStyle={}\nrulers={}\nrulerUnit={}\nsidebar={}\nsidebarTab={}\nocrTier={}\nocrLangs={}\n",
            self.icon_style.to_index(),
            self.button_mode.key(),
            if self.rulers { 1 } else { 0 },
            self.ruler_unit.suffix(),
            if self.sidebar { 1 } else { 0 },
            self.sidebar_tab.key(),
            self.ocr_tier,
            self.ocr_langs.join(",")
        );
        let _ = std::fs::write(p, text);
    }
}
