// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Fufeng Project

//! 未嵌入字型的中日韓替代字型。
//!
//! 台灣的薪資、報表、公文系統常輸出只指定 `MSung-Light`、`MHei-Medium`、`STSong-Light`
//! 等名稱、卻沒有嵌入字型的 PDF。hayro 內建的替代字型只有拉丁字母，這些字會整段空白。
//! 這裡依字元集合（Adobe-CNS1／GB1／Japan1／Korea1）與字型名稱、襯線／粗細，
//! 改用 Windows 內建的對應字型（細明體、正黑體、標楷體、宋體、雅黑、明朝、Gothic、Batang、Malgun…）。
//! 只在需要時讀取字型檔，並快取在記憶體中；找不到任何字型時退回 hayro 原本的處理。

use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_interpret::font::{FallbackFontQuery, FontData, FontQuery};
use hayro::hayro_interpret::hayro_cmap::CidFamily;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Script {
    TraditionalChinese,
    SimplifiedChinese,
    Japanese,
    Korean,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// 明體／宋體／明朝／Batang（襯線）
    Serif,
    /// 黑體／Gothic／Dotum（無襯線）
    Sans,
    /// 楷書
    Kai,
}

/// 依字型名稱判斷文字種類（字元集合不明時使用）
fn script_from_name(name: &str) -> Option<Script> {
    let n = name.to_ascii_lowercase();
    let has = |keys: &[&str]| keys.iter().any(|k| n.contains(k));
    if has(&["msung", "mhei", "mingliu", "pmingliu", "dfkai", "kaiu", "jhenghei", "big5", "cns"]) {
        Some(Script::TraditionalChinese)
    } else if has(&["stsong", "stheiti", "stkaiti", "simsun", "simhei", "simkai", "kaiti", "yahei", "fangsong", "gb"]) {
        Some(Script::SimplifiedChinese)
    } else if has(&["heisei", "kozmin", "kozgo", "mincho", "gothic-medium", "msgothic", "ms-gothic", "meiryo", "yugoth", "ryumin", "gothicbbb", "hiragino"]) {
        Some(Script::Japanese)
    } else if has(&["hysmyeongjo", "hygothic", "batang", "dotum", "gulim", "malgun", "myeongjo", "hanyang"]) {
        Some(Script::Korean)
    } else {
        None
    }
}

fn style_of(q: &FallbackFontQuery) -> Style {
    let n = q.post_script_name.as_deref().unwrap_or_default().to_ascii_lowercase();
    let has = |keys: &[&str]| keys.iter().any(|k| n.contains(k));
    if has(&["kai", "kaiu"]) {
        Style::Kai
    } else if has(&["hei", "gothic", "dotum", "gulim", "malgun", "yahei", "jhenghei", "sans"]) {
        Style::Sans
    } else if has(&["sung", "song", "ming", "min", "batang", "myeongjo", "serif"]) || q.is_serif {
        Style::Serif
    } else {
        Style::Sans
    }
}

fn is_bold(q: &FallbackFontQuery) -> bool {
    let n = q.post_script_name.as_deref().unwrap_or_default().to_ascii_lowercase();
    q.is_bold || q.font_weight >= 600 || n.contains("bold")
}

/// 候選字型檔（Windows Fonts 資料夾內的檔名, 集合中的索引），依優先順序
fn candidates(script: Script, style: Style, bold: bool) -> Vec<(&'static str, u32)> {
    use Script::*;
    use Style::*;
    match (script, style) {
        (TraditionalChinese, Serif) => vec![("mingliu.ttc", 0), ("mingliub.ttc", 0), ("msjh.ttc", 0)],
        (TraditionalChinese, Kai) => vec![("kaiu.ttf", 0), ("mingliu.ttc", 0), ("msjh.ttc", 0)],
        (TraditionalChinese, Sans) => {
            if bold { vec![("msjhbd.ttc", 0), ("msjh.ttc", 0), ("mingliu.ttc", 0)] } else { vec![("msjh.ttc", 0), ("mingliu.ttc", 0)] }
        }
        (SimplifiedChinese, Serif) => vec![("simsun.ttc", 0), ("msyh.ttc", 0)],
        (SimplifiedChinese, Kai) => vec![("simkai.ttf", 0), ("simsun.ttc", 0), ("msyh.ttc", 0)],
        (SimplifiedChinese, Sans) => {
            if bold { vec![("msyhbd.ttc", 0), ("msyh.ttc", 0), ("simhei.ttf", 0), ("simsun.ttc", 0)] } else { vec![("msyh.ttc", 0), ("simhei.ttf", 0), ("simsun.ttc", 0)] }
        }
        (Japanese, Serif | Kai) => vec![("msmincho.ttc", 0), ("yumin.ttf", 0), ("msgothic.ttc", 0), ("YuGothM.ttc", 0)],
        (Japanese, Sans) => vec![("YuGothM.ttc", 0), ("msgothic.ttc", 0), ("meiryo.ttc", 0)],
        (Korean, Serif | Kai) => vec![("batang.ttc", 0), ("malgun.ttf", 0)],
        (Korean, Sans) => {
            if bold { vec![("malgunbd.ttf", 0), ("malgun.ttf", 0), ("gulim.ttc", 0)] } else { vec![("malgun.ttf", 0), ("gulim.ttc", 0)] }
        }
    }
}

fn fonts_dir() -> PathBuf {
    PathBuf::from(std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into())).join("Fonts")
}

type Cache = Mutex<HashMap<String, Option<FontData>>>;

/// 讀取字型檔（快取；讀不到時記為 None，不再重試）
fn load(file: &str) -> Option<FontData> {
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let mut map = CACHE.get_or_init(|| Mutex::new(HashMap::new())).lock().unwrap_or_else(|e| e.into_inner());
    if let Some(v) = map.get(file) {
        return v.clone();
    }
    let data = std::fs::read(fonts_dir().join(file)).ok().map(|b| Arc::new(b) as FontData);
    map.insert(file.to_string(), data.clone());
    data
}

/// 未嵌入字型查詢的中日韓替代字型；不是中日韓字型時回傳 None
pub fn cjk_substitute(q: &FallbackFontQuery) -> Option<(FontData, u32)> {
    let script = match q.character_collection.as_ref().map(|c| &c.family) {
        Some(CidFamily::AdobeCNS1) => Script::TraditionalChinese,
        Some(CidFamily::AdobeGB1) => Script::SimplifiedChinese,
        Some(CidFamily::AdobeJapan1) => Script::Japanese,
        Some(CidFamily::AdobeKorea1) => Script::Korean,
        _ => script_from_name(q.post_script_name.as_deref().or(q.font_name.as_deref())?)?,
    };
    candidates(script, style_of(q), is_bold(q)).into_iter().find_map(|(file, index)| load(file).map(|d| (d, index)))
}

/// 渲染與文字擷取共用的設定：中日韓未嵌入字型改用系統字型，其餘沿用 hayro 預設
pub fn interpreter_settings() -> InterpreterSettings {
    let base = InterpreterSettings::default();
    let standard = base.font_resolver.clone();
    InterpreterSettings {
        font_resolver: Arc::new(move |query: &FontQuery| match query {
            FontQuery::Fallback(f) => cjk_substitute(f).or_else(|| standard(query)),
            _ => standard(query),
        }),
        ..base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_guess_from_common_names() {
        assert_eq!(script_from_name("MSung-Light"), Some(Script::TraditionalChinese));
        assert_eq!(script_from_name("MHei-Medium"), Some(Script::TraditionalChinese));
        assert_eq!(script_from_name("STSong-Light"), Some(Script::SimplifiedChinese));
        assert_eq!(script_from_name("HeiseiMin-W3"), Some(Script::Japanese));
        assert_eq!(script_from_name("HYSMyeongJo-Medium"), Some(Script::Korean));
        assert_eq!(script_from_name("Helvetica"), None);
    }

    #[test]
    fn style_from_names() {
        let q = |n: &str| FallbackFontQuery { post_script_name: Some(n.into()), ..Default::default() };
        assert_eq!(style_of(&q("MSung-Light")), Style::Serif);
        assert_eq!(style_of(&q("MHei-Medium")), Style::Sans);
        assert_eq!(style_of(&q("DFKaiShu-SB-Estd-BF")), Style::Kai);
        assert_eq!(style_of(&q("HeiseiKakuGo-W5")), Style::Sans);
    }
}
