// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Fufeng Project

//! 存檔：在原始檔的乾淨副本上套用註記（lopdf），可選頁、可加密（AES-256）。
//!
//! - 高亮、底線 → 標準 Highlight / Underline 註解（附外觀串流）
//! - 線條、箭頭 → 標準 Line 註解，外觀自行繪製（可調箭頭大小與虛線，與編輯畫面一致）
//! - 文字 → 寫入頁面內容（Form XObject），嵌入字型子集

use crate::document::Document;
use crate::fonts::{self, FontFace, layout_text};
use crate::geom::{Affine, Pt, Rect, Rgba};
use crate::marks::{Mark, MarkType, Seg, line_geometry, shape_path};
use lopdf::encryption::crypt_filters::{Aes256CryptFilter, CryptFilter};
use lopdf::{Dictionary, EncryptionState, EncryptionVersion, LoadOptions, Object, ObjectId, Permissions, Stream, StringFormat, dictionary};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct SaveOptions {
    /// 0 起算的頁碼（依序輸出）；空 = 全部頁面
    pub pages: Vec<usize>,
    pub include_marks: bool,
    pub encrypt: bool,
    /// 開啟密碼
    pub user_password: String,
    /// 權限密碼（空 = 同開啟密碼）
    pub owner_password: String,
    pub allow_print: bool,
    pub allow_copy: bool,
    pub allow_modify: bool,
}

impl Default for SaveOptions {
    fn default() -> Self {
        Self {
            pages: Vec::new(),
            include_marks: true,
            encrypt: false,
            user_password: String::new(),
            owner_password: String::new(),
            allow_print: true,
            allow_copy: true,
            allow_modify: true,
        }
    }
}

pub fn save(doc: &Document, marks: &[Mark], opt: &SaveOptions, out_path: &str) -> Result<(), String> {
    let bytes = save_to_bytes(doc, marks, opt)?;
    std::fs::write(out_path, bytes).map_err(|e| format!("無法寫入檔案：{e}"))
}

pub fn save_to_bytes(doc: &Document, marks: &[Mark], opt: &SaveOptions) -> Result<Vec<u8>, String> {
    let mut pdf = lopdf::Document::load_mem_with_options(doc.bytes(), LoadOptions::with_password(doc.password()))
        .map_err(|e| format!("無法讀取 PDF：{e}"))?;
    // 解密後的副本：移除原本的加密設定（另存的檔案預設不加密）
    if let Ok(enc) = pdf.trailer.get(b"Encrypt").and_then(Object::as_reference) {
        pdf.objects.remove(&enc);
    }
    pdf.trailer.remove(b"Encrypt");
    pdf.encryption_state = None;

    let page_ids: Vec<ObjectId> = pdf.get_pages().values().copied().collect();
    let page_total = page_ids.len();
    if page_total != doc.page_count() {
        return Err(format!("頁數不一致（{page_total} / {}），無法安全寫入", doc.page_count()));
    }
    let transforms = doc.view_transforms();

    if opt.include_marks {
        apply_marks(&mut pdf, &page_ids, &transforms, marks)?;
    }

    let keep: Vec<usize> = {
        let mut seen = HashSet::new();
        opt.pages.iter().copied().filter(|&p| p < page_total && seen.insert(p)).collect()
    };
    if !keep.is_empty() && keep != (0..page_total).collect::<Vec<_>>() {
        rearrange_pages(&mut pdf, &page_ids, &keep)?;
    }

    pdf.prune_objects();
    pdf.compress();
    ensure_id(&mut pdf);

    if opt.encrypt {
        let owner = if opt.owner_password.is_empty() { &opt.user_password } else { &opt.owner_password };
        if opt.user_password.len() >= 127 || owner.len() >= 127 {
            return Err("密碼過長".into());
        }
        let mut perm = Permissions::COPYABLE_FOR_ACCESSIBILITY;
        if opt.allow_print {
            perm |= Permissions::PRINTABLE | Permissions::PRINTABLE_IN_HIGH_QUALITY;
        }
        if opt.allow_copy {
            perm |= Permissions::COPYABLE;
        }
        if opt.allow_modify {
            perm |= Permissions::MODIFIABLE | Permissions::ANNOTABLE | Permissions::FILLABLE | Permissions::ASSEMBLABLE;
        }
        let mut key = [0u8; 32];
        getrandom::fill(&mut key).map_err(|e| format!("無法產生加密金鑰：{e}"))?;
        let mut filters: BTreeMap<Vec<u8>, Arc<dyn CryptFilter>> = BTreeMap::new();
        filters.insert(b"StdCF".to_vec(), Arc::new(Aes256CryptFilter));
        let version = EncryptionVersion::V5 {
            encrypt_metadata: true,
            crypt_filters: filters,
            file_encryption_key: &key,
            stream_filter: b"StdCF".to_vec(),
            string_filter: b"StdCF".to_vec(),
            owner_password: owner,
            user_password: &opt.user_password,
            permissions: perm,
        };
        let state = EncryptionState::try_from(version).map_err(|e| format!("加密失敗：{e}"))?;
        pdf.encrypt(&state).map_err(|e| format!("加密失敗：{e}"))?;
    }

    let mut out = Vec::with_capacity(doc.bytes().len() + 4096);
    pdf.save_to(&mut out).map_err(|e| format!("寫入 PDF 失敗：{e}"))?;
    Ok(out)
}

// ---------------------------------------------------------------- 共用

/// 內容串流中的數字：最多 3 位小數，去掉多餘的 0
fn n(v: f64) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    let s = format!("{:.3}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() { "0".into() } else { s.to_string() }
}

fn real(v: f64) -> Object {
    Object::Real(v as f32)
}

fn rgb_array(c: Rgba) -> Object {
    Object::Array(vec![real(c.rf()), real(c.gf()), real(c.bf())])
}

fn rect_array(r: &Rect) -> Object {
    Object::Array(vec![real(r.x0), real(r.y0), real(r.x1), real(r.y1)])
}

fn rgb_op(c: Rgba, stroke: bool) -> String {
    format!("{} {} {} {}", n(c.rf()), n(c.gf()), n(c.bf()), if stroke { "RG" } else { "rg" })
}

/// PDF 日期字串（UTC）
fn pdf_date() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let days = secs.div_euclid(86400);
    let rem = secs.rem_euclid(86400);
    // civil_from_days（Howard Hinnant 的演算法）
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("D:{:04}{:02}{:02}{:02}{:02}{:02}Z", y, m, d, rem / 3600, rem % 3600 / 60, rem % 60)
}

fn ensure_id(pdf: &mut lopdf::Document) {
    if pdf.trailer.get(b"ID").is_ok() {
        return;
    }
    let mut id = [0u8; 16];
    if getrandom::fill(&mut id).is_err() {
        id = *b"SimplePdfEditor!";
    }
    let s = Object::String(id.to_vec(), StringFormat::Hexadecimal);
    pdf.trailer.set("ID", Object::Array(vec![s.clone(), s]));
}

fn resolve<'a>(pdf: &'a lopdf::Document, obj: &'a Object) -> Option<&'a Object> {
    match obj {
        Object::Reference(id) => pdf.get_object(*id).ok(),
        o => Some(o),
    }
}

/// 頁面（或繼承自上層節點）的屬性
fn inherited(pdf: &lopdf::Document, page: ObjectId, key: &[u8]) -> Option<Object> {
    let mut id = page;
    for _ in 0..64 {
        let dict = pdf.get_dictionary(id).ok()?;
        if let Ok(v) = dict.get(key) {
            return Some(v.clone());
        }
        id = dict.get(b"Parent").and_then(Object::as_reference).ok()?;
    }
    None
}

fn add_stream(pdf: &mut lopdf::Document, dict: Dictionary, content: Vec<u8>) -> ObjectId {
    pdf.add_object(Stream::new(dict, content))
}

/// 外觀串流（Form XObject，座標為 PDF 使用者空間）
fn add_appearance(pdf: &mut lopdf::Document, bbox: &Rect, resources: Option<Dictionary>, content: String) -> ObjectId {
    let mut d = dictionary! {
        "Type" => "XObject",
        "Subtype" => "Form",
        "FormType" => 1,
        "BBox" => rect_array(bbox),
        "Matrix" => Object::Array(vec![1.into(), 0.into(), 0.into(), 1.into(), 0.into(), 0.into()]),
    };
    if let Some(r) = resources {
        d.set("Resources", r);
    }
    add_stream(pdf, d, content.into_bytes())
}

fn push_annot(pdf: &mut lopdf::Document, page: ObjectId, annot: ObjectId) -> Result<(), String> {
    let existing = pdf.get_dictionary(page).map_err(|e| e.to_string())?.get(b"Annots").ok().cloned();
    match existing {
        Some(Object::Reference(arr_id)) => {
            if let Ok(Object::Array(a)) = pdf.get_object_mut(arr_id) {
                a.push(Object::Reference(annot));
                return Ok(());
            }
            let page_dict = pdf.get_dictionary_mut(page).map_err(|e| e.to_string())?;
            page_dict.set("Annots", Object::Array(vec![Object::Reference(annot)]));
        }
        Some(Object::Array(mut a)) => {
            a.push(Object::Reference(annot));
            pdf.get_dictionary_mut(page).map_err(|e| e.to_string())?.set("Annots", Object::Array(a));
        }
        _ => {
            pdf.get_dictionary_mut(page).map_err(|e| e.to_string())?.set("Annots", Object::Array(vec![Object::Reference(annot)]));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- 註記

fn apply_marks(pdf: &mut lopdf::Document, page_ids: &[ObjectId], transforms: &[Affine], marks: &[Mark]) -> Result<(), String> {
    let date = pdf_date();

    // 文字：先收集每個字型用到的字形，建好字型子集，再寫入各頁
    let mut texts_per_page: BTreeMap<usize, Vec<&Mark>> = BTreeMap::new();
    let mut font_usage: BTreeMap<(String, u32), BTreeMap<u16, char>> = BTreeMap::new();
    for m in marks.iter().filter(|m| m.kind == MarkType::Text && m.page < page_ids.len()) {
        if m.text.trim().is_empty() {
            continue;
        }
        if fonts::font(&m.font_path, m.font_index).is_none() {
            return Err(format!("無法載入字型：{}", m.font_path));
        }
        let used = font_usage.entry((m.font_path.clone(), m.font_index)).or_default();
        for line in layout_text(m).lines {
            for (gid, c, _) in line.glyphs {
                used.entry(gid).or_insert(c);
            }
        }
        texts_per_page.entry(m.page).or_default().push(m);
    }
    let mut font_objs: HashMap<(String, u32), ObjectId> = HashMap::new();
    for (key, used) in &font_usage {
        let face = fonts::font(&key.0, key.1).ok_or_else(|| format!("無法載入字型：{}", key.0))?;
        let (id, remap) = embed_font(pdf, &face, used)?;
        font_objs.insert(key.clone(), id);
        FONT_REMAP.with(|r| r.borrow_mut().insert(key.clone(), remap));
    }

    for m in marks {
        let Some(&page) = page_ids.get(m.page) else { continue };
        let tf = transforms.get(m.page).copied().unwrap_or(Affine::IDENTITY);
        match m.kind {
            MarkType::Highlight | MarkType::Underline => {
                if let Some(id) = markup_annot(pdf, page, &tf, m, &date) {
                    push_annot(pdf, page, id)?;
                }
            }
            MarkType::Line | MarkType::Arrow => {
                if let Some(id) = line_annot(pdf, page, &tf, m, &date) {
                    push_annot(pdf, page, id)?;
                }
            }
            MarkType::Rect | MarkType::Ellipse => {
                if let Some(id) = shape_annot(pdf, page, &tf, m, &date) {
                    push_annot(pdf, page, id)?;
                }
            }
            MarkType::Text | MarkType::Ocr => {}
        }
    }

    // OCR：每頁一個看不見的文字層
    let ocr_pages: BTreeMap<usize, Vec<&Mark>> = marks.iter().filter(|m| m.kind == MarkType::Ocr && m.page < page_ids.len() && !m.ocr.is_empty()).fold(
        BTreeMap::new(),
        |mut acc, m| {
            acc.entry(m.page).or_insert_with(Vec::new).push(m);
            acc
        },
    );
    if !ocr_pages.is_empty() {
        let font = glyphless_font(pdf, marks)?;
        for (page_no, ms) in ocr_pages {
            let Some(&page) = page_ids.get(page_no) else { continue };
            let tf = transforms.get(page_no).copied().unwrap_or(Affine::IDENTITY);
            stamp_ocr(pdf, page, page_no, &tf, &ms, font)?;
        }
    }

    for (page_no, texts) in texts_per_page {
        let Some(&page) = page_ids.get(page_no) else { continue };
        let tf = transforms.get(page_no).copied().unwrap_or(Affine::IDENTITY);
        stamp_texts(pdf, page, page_no, &tf, &texts, &font_objs)?;
    }
    FONT_REMAP.with(|r| r.borrow_mut().clear());
    Ok(())
}

thread_local! {
    /// 字型子集的字形編號對照（原 GID -> 新 GID = CID），只在單次存檔期間使用
    static FONT_REMAP: std::cell::RefCell<HashMap<(String, u32), HashMap<u16, u16>>> = std::cell::RefCell::new(HashMap::new());
}

/// 高亮／底線註解
fn markup_annot(pdf: &mut lopdf::Document, page: ObjectId, tf: &Affine, m: &Mark, date: &str) -> Option<ObjectId> {
    let quads: Vec<[Pt; 4]> = m.quads.iter().map(|q| q.map(|p| tf.apply(p + m.offset))).collect();
    if quads.is_empty() {
        return None;
    }
    let mut rect = Rect::EMPTY;
    let mut qp = Vec::new();
    for q in &quads {
        for p in q {
            rect.include(*p);
        }
        // QuadPoints 順序：ul, ur, ll, lr
        for p in [q[0], q[1], q[3], q[2]] {
            qp.push(real(p.x));
            qp.push(real(p.y));
        }
    }
    let highlight = m.kind == MarkType::Highlight;
    let mut content = String::new();
    let resources;
    if highlight {
        // 相乘混色：像螢光筆一樣不遮住底下文字
        let _ = writeln!(content, "/H gs {}", rgb_op(m.color, false));
        for q in &quads {
            let _ = writeln!(content, "{} {} m {} {} l {} {} l {} {} l h f", n(q[0].x), n(q[0].y), n(q[1].x), n(q[1].y), n(q[2].x), n(q[2].y), n(q[3].x), n(q[3].y));
        }
        resources = Some(dictionary! {
            "ExtGState" => dictionary! { "H" => dictionary! { "Type" => "ExtGState", "BM" => "Multiply", "ca" => 1, "CA" => 1 } },
        });
    } else {
        let _ = writeln!(content, "{} 0 J", rgb_op(m.color, true));
        let s = tf.scale_factor();
        for q in &m.quads {
            // 與編輯畫面相同：線寬為行高的 1/14，畫在四邊形底邊略上方
            let (ul, lr, ll) = (q[0] + m.offset, q[2] + m.offset, q[3] + m.offset);
            let h = ul.distance(ll);
            let t = (h / 14.0).max(0.8);
            let up = (ul - ll) * (t / h.max(0.01)) * 0.5;
            let (a, b) = (tf.apply(ll + up), tf.apply(lr + up));
            let _ = writeln!(content, "{} w {} {} m {} {} l S", n(t * s), n(a.x), n(a.y), n(b.x), n(b.y));
        }
        resources = None;
    }
    let rect = rect.expand(2.0);
    let ap = add_appearance(pdf, &rect, resources, content);
    let annot = dictionary! {
        "Type" => "Annot",
        "Subtype" => if highlight { "Highlight" } else { "Underline" },
        "Rect" => rect_array(&rect),
        "QuadPoints" => Object::Array(qp),
        "C" => rgb_array(m.color),
        "F" => 4,
        "P" => Object::Reference(page),
        "M" => Object::string_literal(date),
        "CreationDate" => Object::string_literal(date),
        "AP" => dictionary! { "N" => Object::Reference(ap) },
    };
    Some(pdf.add_object(annot))
}

/// 線條／箭頭：/L、/LE、/BS（含虛線）照標準填寫，讓其他閱讀器仍視為可編輯的線條；
/// 外觀串流自行繪製，才能支援可調的箭頭大小與虛線，並與編輯畫面完全一致。
fn line_annot(pdf: &mut lopdf::Document, page: ObjectId, tf: &Affine, m: &Mark, date: &str) -> Option<ObjectId> {
    let g = line_geometry(m);
    let fp = |p: Pt| tf.apply(p + m.offset);
    let (a, b) = (fp(m.p1), fp(m.p2));
    let (s0, s1) = (fp(g.shaft_start), fp(g.shaft_end));
    let heads: Vec<[Pt; 3]> = g.heads.iter().map(|h| h.map(fp)).collect();
    let k = tf.scale_factor();
    let width = m.width * k;
    let (dash, gap) = (m.dash_len.max(0.1) * k, m.gap_len.max(0.1) * k);

    let mut r = Rect::EMPTY;
    r.include(s0);
    r.include(s1);
    for h in &heads {
        for p in h {
            r.include(*p);
        }
    }
    let r = r.expand(width + 2.0);

    let mut c = String::new();
    let _ = writeln!(c, "{} {} w 0 j", rgb_op(m.color, true), n(width));
    if m.dashed {
        let _ = writeln!(c, "0 J [{} {}] 0 d", n(dash), n(gap));
    } else {
        let _ = writeln!(c, "1 J");
    }
    let _ = writeln!(c, "{} {} m {} {} l S", n(s0.x), n(s0.y), n(s1.x), n(s1.y));
    if !heads.is_empty() {
        let _ = writeln!(c, "{}", rgb_op(m.color, false));
        for h in &heads {
            let _ = writeln!(c, "{} {} m {} {} l {} {} l h", n(h[0].x), n(h[0].y), n(h[1].x), n(h[1].y), n(h[2].x), n(h[2].y));
        }
        let _ = writeln!(c, "f");
    }
    let ap = add_appearance(pdf, &r, None, c);

    let le = |arrow: bool| Object::Name(if arrow { b"ClosedArrow".to_vec() } else { b"None".to_vec() });
    let mut bs = dictionary! { "Type" => "Border", "W" => real(width), "S" => if m.dashed { "D" } else { "S" } };
    if m.dashed {
        bs.set("D", Object::Array(vec![real(dash), real(gap)]));
    }
    let mut annot = dictionary! {
        "Type" => "Annot",
        "Subtype" => "Line",
        "Rect" => rect_array(&r),
        "L" => Object::Array(vec![real(a.x), real(a.y), real(b.x), real(b.y)]),
        "BS" => bs,
        "C" => rgb_array(m.color),
        "LE" => Object::Array(vec![le(heads.len() > 1), le(!heads.is_empty())]),
        "F" => 4,
        "P" => Object::Reference(page),
        "M" => Object::string_literal(date),
        "CreationDate" => Object::string_literal(date),
        "AP" => dictionary! { "N" => Object::Reference(ap) },
    };
    if !heads.is_empty() {
        annot.set("IC", rgb_array(m.color));
    }
    Some(pdf.add_object(annot))
}

/// 矩形／圓角矩形／橢圓框：標準 Square／Circle 註解（中間鏤空，不填色），
/// 外觀串流自行繪製，才能有圓角與可調虛線，並與編輯畫面一致。
fn shape_annot(pdf: &mut lopdf::Document, page: ObjectId, tf: &Affine, m: &Mark, date: &str) -> Option<ObjectId> {
    let r0 = crate::marks::shape_rect(m);
    if r0.width() < 0.5 && r0.height() < 0.5 {
        return None;
    }
    let fp = |p: Pt| tf.apply(p + m.offset);
    let k = tf.scale_factor();
    let width = m.width * k;
    let (dash, gap) = (m.dash_len.max(0.1) * k, m.gap_len.max(0.1) * k);

    let mut c = String::new();
    let _ = writeln!(c, "{} {} w 0 j 0 J", rgb_op(m.color, true), n(width));
    if m.dashed {
        let _ = writeln!(c, "[{} {}] 0 d", n(dash), n(gap));
    }
    for seg in shape_path(m) {
        match seg {
            Seg::Move(a) => {
                let a = fp(a);
                let _ = write!(c, "{} {} m ", n(a.x), n(a.y));
            }
            Seg::Line(a) => {
                let a = fp(a);
                let _ = write!(c, "{} {} l ", n(a.x), n(a.y));
            }
            Seg::Cubic(c1, c2, e) => {
                let (c1, c2, e) = (fp(c1), fp(c2), fp(e));
                let _ = write!(c, "{} {} {} {} {} {} c ", n(c1.x), n(c1.y), n(c2.x), n(c2.y), n(e.x), n(e.y));
            }
            Seg::Close => {
                let _ = write!(c, "h ");
            }
        }
    }
    let _ = writeln!(c, "S");
    // 外框（含線寬）
    let outer = tf.transform_rect(&r0.translate(m.offset)).expand(width / 2.0 + 1.0);
    let ap = add_appearance(pdf, &outer, None, c);

    let mut bs = dictionary! { "Type" => "Border", "W" => real(width), "S" => if m.dashed { "D" } else { "S" } };
    if m.dashed {
        bs.set("D", Object::Array(vec![real(dash), real(gap)]));
    }
    let annot = dictionary! {
        "Type" => "Annot",
        "Subtype" => if m.kind == MarkType::Ellipse { "Circle" } else { "Square" },
        "Rect" => rect_array(&outer),
        "BS" => bs,
        "C" => rgb_array(m.color),
        "F" => 4,
        "P" => Object::Reference(page),
        "M" => Object::string_literal(date),
        "CreationDate" => Object::string_literal(date),
        "AP" => dictionary! { "N" => Object::Reference(ap) },
    };
    Some(pdf.add_object(annot))
}

// ---------------------------------------------------------------- 文字

/// 嵌入字型子集（Type0 / Identity-H）；回傳字型物件與「原 GID -> CID」對照
fn embed_font(pdf: &mut lopdf::Document, face: &FontFace, used: &BTreeMap<u16, char>) -> Result<(ObjectId, HashMap<u16, u16>), String> {
    let mut remapper = subsetter::GlyphRemapper::new();
    let mut remap = HashMap::new();
    remap.insert(0u16, remapper.remap(0));
    for &gid in used.keys() {
        remap.insert(gid, remapper.remap(gid));
    }
    let subset = subsetter::subset(&face.data, face.index, &remapper).map_err(|e| format!("字型子集化失敗：{e}"))?;
    let ttf = face.face().ok_or("無法解析字型")?;
    let upem = face.units_per_em;
    let k = 1000.0 / upem;

    // 字型名稱：6 個大寫字母的子集標記 + PostScript 名稱
    let ps_name: String = ttf
        .names()
        .into_iter()
        .filter(|nm| nm.name_id == ttf_parser::name_id::POST_SCRIPT_NAME)
        .filter_map(|nm| nm.to_string())
        .next()
        .unwrap_or_else(|| "Font".into())
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();
    let mut tag_seed: u64 = 0xcbf29ce484222325;
    for (&g, _) in used {
        tag_seed = (tag_seed ^ g as u64).wrapping_mul(0x100000001b3);
    }
    let tag: String = (0..6).map(|i| (b'A' + ((tag_seed >> (i * 5)) % 26) as u8) as char).collect();
    let base_font = format!("{tag}+{}", if ps_name.is_empty() { "Font" } else { &ps_name });

    let bbox = ttf.global_bounding_box();
    let (asc, desc) = (face.ascent * k, -face.descent * k);
    let cap = ttf.capital_height().map(|c| c as f64 * k).unwrap_or(asc);

    let compressed = {
        use std::io::Write;
        let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
        e.write_all(&subset).map_err(|e| e.to_string())?;
        e.finish().map_err(|e| e.to_string())?
    };
    let mut file_dict = dictionary! { "Filter" => "FlateDecode" };
    if face.truetype {
        file_dict.set("Length1", subset.len() as i64);
    } else {
        file_dict.set("Subtype", "OpenType");
    }
    let mut file_stream = Stream::new(file_dict, compressed);
    file_stream.allows_compression = false;
    let file_id = pdf.add_object(file_stream);

    let descriptor = dictionary! {
        "Type" => "FontDescriptor",
        "FontName" => Object::Name(base_font.clone().into_bytes()),
        "Flags" => 4,
        "FontBBox" => Object::Array(vec![
            real(bbox.x_min as f64 * k), real(bbox.y_min as f64 * k), real(bbox.x_max as f64 * k), real(bbox.y_max as f64 * k),
        ]),
        "ItalicAngle" => 0,
        "Ascent" => real(asc),
        "Descent" => real(desc),
        "CapHeight" => real(cap),
        "StemV" => 80,
        if face.truetype { "FontFile2" } else { "FontFile3" } => Object::Reference(file_id),
    };
    let descriptor_id = pdf.add_object(descriptor);

    // 寬度表：依 CID 排序
    let mut widths: Vec<(u16, f64)> = remap.iter().map(|(&old, &cid)| (cid, FontFace::advance(&ttf, old) * k)).collect();
    widths.sort_by_key(|w| w.0);
    let mut w_arr = Vec::new();
    for (cid, w) in &widths {
        w_arr.push(Object::Integer(*cid as i64));
        w_arr.push(Object::Array(vec![real(*w)]));
    }
    let mut cid_font = dictionary! {
        "Type" => "Font",
        "Subtype" => if face.truetype { "CIDFontType2" } else { "CIDFontType0" },
        "BaseFont" => Object::Name(base_font.clone().into_bytes()),
        "CIDSystemInfo" => dictionary! {
            "Registry" => Object::string_literal("Adobe"),
            "Ordering" => Object::string_literal("Identity"),
            "Supplement" => 0,
        },
        "FontDescriptor" => Object::Reference(descriptor_id),
        "DW" => 1000,
        "W" => Object::Array(w_arr),
    };
    if face.truetype {
        cid_font.set("CIDToGIDMap", "Identity");
    }
    let cid_id = pdf.add_object(cid_font);

    // ToUnicode：讓複製／搜尋文字時取得正確字元
    let mut cmap = String::from(
        "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
    );
    let pairs: Vec<(u16, char)> = used.iter().filter_map(|(old, c)| remap.get(old).map(|cid| (*cid, *c))).collect();
    for chunk in pairs.chunks(100) {
        let _ = writeln!(cmap, "{} beginbfchar", chunk.len());
        for (cid, c) in chunk {
            let mut buf = [0u16; 2];
            let hex: String = c.encode_utf16(&mut buf).iter().map(|u| format!("{u:04X}")).collect();
            let _ = writeln!(cmap, "<{cid:04X}> <{hex}>");
        }
        let _ = writeln!(cmap, "endbfchar");
    }
    cmap.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    let cmap_id = add_stream(pdf, dictionary! {}, cmap.into_bytes());

    let type0 = dictionary! {
        "Type" => "Font",
        "Subtype" => "Type0",
        "BaseFont" => Object::Name(base_font.into_bytes()),
        "Encoding" => "Identity-H",
        "DescendantFonts" => Object::Array(vec![Object::Reference(cid_id)]),
        "ToUnicode" => Object::Reference(cmap_id),
    };
    Ok((pdf.add_object(type0), remap))
}

/// 把一頁的所有文字註記畫成 Form XObject，再以 q/Q 包住原內容後附加到頁面。
/// 包住原內容可避免原頁面未還原的繪圖狀態（座標轉換、顏色等）影響新文字。
fn stamp_texts(
    pdf: &mut lopdf::Document,
    page: ObjectId,
    page_no: usize,
    tf: &Affine,
    texts: &[&Mark],
    font_objs: &HashMap<(String, u32), ObjectId>,
) -> Result<(), String> {
    let mut fonts_res = Dictionary::new();
    let mut gs_res = Dictionary::new();
    let mut font_names: HashMap<(String, u32), String> = HashMap::new();
    // 以透明度命名的 ExtGState（背景與外框可半透明）
    let gs_name = |gs_res: &mut Dictionary, alpha: u8| -> String {
        let name = format!("GA{alpha}");
        if !gs_res.has(name.as_bytes()) {
            let v = alpha as f64 / 255.0;
            gs_res.set(name.as_bytes(), dictionary! { "Type" => "ExtGState", "ca" => real(v), "CA" => real(v) });
        }
        name
    };

    let mut c = String::new();
    let [a, b, cc, d, e, f] = tf.0;
    let _ = writeln!(c, "q {} {} {} {} {} {} cm", n(a), n(b), n(cc), n(d), n(e), n(f));
    for m in texts {
        let mut mm = (*m).clone();
        mm.pos = m.pos + m.offset;
        let layout = layout_text(&mm);
        let key = (m.font_path.clone(), m.font_index);
        let Some(&font_id) = font_objs.get(&key) else { continue };
        let fname = {
            let len = font_names.len();
            font_names.entry(key.clone()).or_insert_with(|| format!("F{len}")).clone()
        };
        fonts_res.set(fname.as_bytes(), Object::Reference(font_id));

        // 背景與外框（畫在文字底下）
        let bx = layout.content.expand(fonts::TEXT_PADDING);
        if m.text_fill {
            let g = gs_name(&mut gs_res, m.fill_color.a);
            let _ = writeln!(c, "q /{g} gs {} {} {} {} {} re f Q", rgb_op(m.fill_color, false), n(bx.x0), n(bx.y0), n(bx.width()), n(bx.height()));
        }
        if m.text_border {
            let g = gs_name(&mut gs_res, m.border_color.a);
            let _ = writeln!(
                c,
                "q /{g} gs {} {} w 0 j 2 J {} {} {} {} re S Q",
                rgb_op(m.border_color, true),
                n(m.border_width),
                n(bx.x0),
                n(bx.y0),
                n(bx.width()),
                n(bx.height())
            );
        }
        let remap = FONT_REMAP.with(|r| r.borrow().get(&key).cloned()).unwrap_or_default();
        let _ = writeln!(c, "BT {} /{fname} 1 Tf", rgb_op(m.color, false));
        for line in &layout.lines {
            if line.glyphs.is_empty() {
                continue;
            }
            let _ = write!(c, "{} 0 0 {} {} {} Tm <", n(m.font_size), n(-m.font_size), n(mm.pos.x), n(line.baseline));
            // 以 Tj 逐行輸出；字距依字型寬度表自動前進，與排版時使用的 advance 相同
            for (gid, _, _) in &line.glyphs {
                let cid = remap.get(gid).copied().unwrap_or(0);
                let _ = write!(c, "{cid:04X}");
            }
            let _ = writeln!(c, "> Tj");
        }
        let _ = writeln!(c, "ET");
    }
    let _ = writeln!(c, "Q");

    let mut res = dictionary! { "Font" => fonts_res };
    if !gs_res.is_empty() {
        res.set("ExtGState", gs_res);
    }
    attach_form(pdf, page, page_no, c, res)
}

/// 把內容畫成 Form XObject，再以 q/Q 包住原內容後附加到頁面
fn attach_form(pdf: &mut lopdf::Document, page: ObjectId, page_no: usize, c: String, res: Dictionary) -> Result<(), String> {
    // 表單範圍：頁面可見範圍（PDF 座標）
    let crop = inherited(pdf, page, b"CropBox").or_else(|| inherited(pdf, page, b"MediaBox"));
    let bbox = crop
        .as_ref()
        .and_then(|o| resolve(pdf, o))
        .and_then(|o| o.as_array().ok())
        .and_then(|a| {
            let v: Vec<f64> = a.iter().filter_map(|x| resolve(pdf, x)).filter_map(|x| x.as_float().ok()).map(|x| x as f64).collect();
            (v.len() == 4).then(|| Rect::new(v[0].min(v[2]), v[1].min(v[3]), v[0].max(v[2]), v[1].max(v[3])))
        })
        .unwrap_or(Rect::new(0.0, 0.0, 612.0, 792.0));
    let xobj = add_appearance(pdf, &bbox, Some(res), c);

    // 把 XObject 掛到頁面資源（資源若是繼承或共用的，先複製一份到本頁）
    let mut resources = match inherited(pdf, page, b"Resources") {
        Some(Object::Reference(id)) => pdf.get_dictionary(id).cloned().unwrap_or_default(),
        Some(Object::Dictionary(d)) => d,
        _ => Dictionary::new(),
    };
    let mut xobjects = match resources.get(b"XObject").ok().cloned() {
        Some(Object::Reference(id)) => pdf.get_dictionary(id).cloned().unwrap_or_default(),
        Some(Object::Dictionary(d)) => d,
        _ => Dictionary::new(),
    };
    let mut serial = page_no;
    let name = loop {
        let nm = format!("FuEdit{serial}");
        if !xobjects.has(nm.as_bytes()) {
            break nm;
        }
        serial += 1;
    };
    xobjects.set(name.as_bytes(), Object::Reference(xobj));
    resources.set("XObject", xobjects);

    let pre = add_stream(pdf, dictionary! {}, b"q\n".to_vec());
    let post = add_stream(pdf, dictionary! {}, format!("\nQ\nq /{name} Do Q\n").into_bytes());
    let page_dict = pdf.get_dictionary_mut(page).map_err(|e| e.to_string())?;
    page_dict.set("Resources", resources);
    let mut contents = vec![Object::Reference(pre)];
    match page_dict.get(b"Contents").ok().cloned() {
        Some(Object::Array(a)) => contents.extend(a),
        Some(o @ Object::Reference(_)) => contents.push(o),
        _ => {}
    }
    contents.push(Object::Reference(post));
    // 內容是間接的陣列時，需取出陣列內容
    let contents = flatten_contents(pdf, contents);
    pdf.get_dictionary_mut(page).map_err(|e| e.to_string())?.set("Contents", Object::Array(contents));
    Ok(())
}

// ---------------------------------------------------------------- OCR 文字層

/// OCR 用的「無字形」字型：Type0／Identity-H，CID 即 Unicode 碼位，所有 CID 都對應到空白字形。
/// 文字以不可見模式（3 Tr）寫入，只用於搜尋、選取與複製，因此不需要任何中日韓字型。
fn glyphless_font(pdf: &mut lopdf::Document, marks: &[Mark]) -> Result<ObjectId, String> {
    // 用到的最大 CID（決定 CIDToGIDMap 的長度）與用到的高位元組（ToUnicode 範圍）
    let mut max_cid = 0u32;
    let mut highs = BTreeSet::new();
    for m in marks.iter().filter(|m| m.kind == MarkType::Ocr) {
        for l in &m.ocr {
            for g in &l.glyphs {
                for ch in g.text.chars() {
                    let cid = ocr_cid(ch) as u32;
                    max_cid = max_cid.max(cid);
                    highs.insert(cid >> 8);
                }
            }
        }
    }
    // 字型程式：自行產生的最小 TrueType（一個空白字形，寬 1 em、上升 0.8 em、下降 0.2 em），
    // 字的基線放在框底上方 0.2 處，選取範圍就剛好等於每個字的四邊形；不依賴任何系統字型
    let ttf = glyphless_ttf();
    let mut st = Stream::new(dictionary! { "Length1" => ttf.len() as i64 }, ttf);
    st.compress().ok();
    let font_file = Some(pdf.add_object(st));
    let mut descriptor = dictionary! {
        "Type" => "FontDescriptor",
        "FontName" => "GlyphLessFont",
        "Flags" => 5,
        "FontBBox" => Object::Array(vec![0.into(), (-200).into(), 1000.into(), 800.into()]),
        "ItalicAngle" => 0,
        "Ascent" => 800,
        "Descent" => -200,
        "CapHeight" => 800,
        "StemV" => 80,
    };
    if let Some(id) = font_file {
        descriptor.set("FontFile2", Object::Reference(id));
    }
    let descriptor_id = pdf.add_object(descriptor);
    // CIDToGIDMap：所有 CID 都對應到字形 0
    let mut map = Stream::new(dictionary! {}, vec![0u8; (max_cid as usize + 1) * 2]);
    map.compress().ok();
    let map_id = pdf.add_object(map);
    let cid_font = pdf.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "CIDFontType2",
        "BaseFont" => "GlyphLessFont",
        "CIDSystemInfo" => dictionary! {
            "Registry" => Object::string_literal("Adobe"),
            "Ordering" => Object::string_literal("Identity"),
            "Supplement" => 0,
        },
        "FontDescriptor" => Object::Reference(descriptor_id),
        "DW" => 1000,
        "CIDToGIDMap" => Object::Reference(map_id),
    });
    // ToUnicode：CID 就是 Unicode 碼位，以每 256 個一段的範圍對應
    let mut cmap = String::from(
        "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
    );
    let highs: Vec<u32> = highs.into_iter().collect();
    for chunk in highs.chunks(100) {
        let _ = writeln!(cmap, "{} beginbfrange", chunk.len());
        for h in chunk {
            let _ = writeln!(cmap, "<{h:02X}00> <{h:02X}FF> <{h:02X}00>");
        }
        let _ = writeln!(cmap, "endbfrange");
    }
    cmap.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    let cmap_id = add_stream(pdf, dictionary! {}, cmap.into_bytes());
    Ok(pdf.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type0",
        "BaseFont" => "GlyphLessFont",
        "Encoding" => "Identity-H",
        "DescendantFonts" => Object::Array(vec![Object::Reference(cid_font)]),
        "ToUnicode" => Object::Reference(cmap_id),
    }))
}

/// 最小的 TrueType 字型：只有一個空白字形，unitsPerEm 1000、字寬 1000、上升 800、下降 200
fn glyphless_ttf() -> Vec<u8> {
    fn be16(v: &mut Vec<u8>, x: u16) {
        v.extend_from_slice(&x.to_be_bytes());
    }
    fn be32(v: &mut Vec<u8>, x: u32) {
        v.extend_from_slice(&x.to_be_bytes());
    }
    let mut head = Vec::new();
    be32(&mut head, 0x0001_0000); // version
    be32(&mut head, 0x0001_0000); // fontRevision
    be32(&mut head, 0); // checkSumAdjustment（最後填入）
    be32(&mut head, 0x5F0F_3CF5); // magicNumber
    be16(&mut head, 0x000B); // flags
    be16(&mut head, 1000); // unitsPerEm
    head.extend_from_slice(&[0; 16]); // created, modified
    for v in [0i16, -200, 1000, 800] {
        be16(&mut head, v as u16); // xMin, yMin, xMax, yMax
    }
    be16(&mut head, 0); // macStyle
    be16(&mut head, 8); // lowestRecPPEM
    be16(&mut head, 2); // fontDirectionHint
    be16(&mut head, 0); // indexToLocFormat（short）
    be16(&mut head, 0); // glyphDataFormat

    let mut hhea = Vec::new();
    be32(&mut hhea, 0x0001_0000);
    for v in [800i16, -200, 0, 1000, 0, 0, 1000, 1, 0, 0, 0, 0, 0, 0, 0, 1].map(|v| v as u16) {
        // ascender, descender, lineGap, advanceWidthMax, minLSB, minRSB, xMaxExtent,
        // caretSlopeRise, caretSlopeRun, caretOffset, reserved×4, metricDataFormat, numberOfHMetrics
        be16(&mut hhea, v);
    }

    let mut maxp = Vec::new();
    be32(&mut maxp, 0x0001_0000);
    for v in [1u16, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0] {
        // numGlyphs, maxPoints, maxContours, maxCompositePoints, maxCompositeContours, maxZones, …
        be16(&mut maxp, v);
    }

    let mut hmtx = Vec::new();
    be16(&mut hmtx, 1000); // advanceWidth
    be16(&mut hmtx, 0); // lsb
    let loca = vec![0u8; 4]; // 兩個 0：字形 0 沒有外框
    let glyf = vec![0u8; 4];
    let mut cmap = Vec::new();
    for v in [0u16, 1, 3, 1] {
        be16(&mut cmap, v); // version, numTables, platformID, encodingID
    }
    be32(&mut cmap, 12); // offset
    for v in [4u16, 24, 0, 2, 2, 0, 0, 0xFFFF, 0, 0xFFFF, 1, 0] {
        // format 4：只有結尾的 0xFFFF 區段
        be16(&mut cmap, v);
    }

    let mut tables: Vec<(&[u8; 4], Vec<u8>)> =
        vec![(b"cmap", cmap), (b"glyf", glyf), (b"head", head), (b"hhea", hhea), (b"hmtx", hmtx), (b"loca", loca), (b"maxp", maxp)];
    let checksum = |d: &[u8]| -> u32 {
        d.chunks(4).fold(0u32, |acc, c| {
            let mut w = [0u8; 4];
            w[..c.len()].copy_from_slice(c);
            acc.wrapping_add(u32::from_be_bytes(w))
        })
    };
    let n = tables.len() as u16;
    let mut out = Vec::new();
    be32(&mut out, 0x0001_0000);
    be16(&mut out, n);
    be16(&mut out, 64); // searchRange（4 個 × 16）
    be16(&mut out, 2); // entrySelector
    be16(&mut out, n * 16 - 64); // rangeShift
    let mut offset = 12 + 16 * n as u32;
    let mut head_offset = 0;
    for (tag, data) in &tables {
        if *tag == b"head" {
            head_offset = offset;
        }
        out.extend_from_slice(*tag);
        be32(&mut out, checksum(data));
        be32(&mut out, offset);
        be32(&mut out, data.len() as u32);
        offset += (data.len() as u32 + 3) & !3;
    }
    for (_, data) in tables.iter_mut() {
        let pad = (4 - data.len() % 4) % 4;
        data.extend(std::iter::repeat_n(0, pad));
        out.extend_from_slice(data);
    }
    let adj = 0xB1B0_AFBAu32.wrapping_sub(checksum(&out));
    let at = head_offset as usize + 8;
    out[at..at + 4].copy_from_slice(&adj.to_be_bytes());
    out
}

/// 字元 → CID（Unicode 碼位；超出 BMP 或控制字元以 U+FFFD 代替）
fn ocr_cid(c: char) -> u16 {
    let v = c as u32;
    if v <= 0xFFFF && !(0xD800..=0xDFFF).contains(&v) && v >= 0x20 { v as u16 } else { 0xFFFD }
}

/// 一頁的 OCR 文字層：每個字以不可見文字放在它的四邊形上（字寬、字高與方向都與原圖相符）
fn stamp_ocr(pdf: &mut lopdf::Document, page: ObjectId, page_no: usize, tf: &Affine, marks: &[&Mark], font: ObjectId) -> Result<(), String> {
    let mut c = String::from("BT 3 Tr /OCR 1 Tf\n");
    let mut count = 0usize;
    for m in marks {
        for l in &m.ocr {
            for g in &l.glyphs {
                let chars: Vec<char> = g.text.chars().collect();
                if chars.is_empty() {
                    continue;
                }
                // X 軸沿底邊（閱讀方向）、Y 軸沿左邊（往上），每個字平分寬度；
                // 字型的下降為 0.2 em，基線放在框底上方 0.2 個字高
                let bl = tf.apply(g.quad[3] + m.offset);
                let x = tf.apply(g.quad[2] + m.offset) - bl;
                let y = tf.apply(g.quad[0] + m.offset) - bl;
                let o = bl + y * 0.2;
                let n = chars.len() as f64;
                let (xx, xy) = (x.x / n, x.y / n);
                for (k, ch) in chars.iter().enumerate() {
                    let (ox, oy) = (o.x + xx * k as f64, o.y + xy * k as f64);
                    let _ = writeln!(c, "{} {} {} {} {} {} Tm <{:04X}> Tj", n_(xx), n_(xy), n_(y.x), n_(y.y), n_(ox), n_(oy), ocr_cid(*ch));
                    count += 1;
                }
            }
        }
    }
    if count == 0 {
        return Ok(());
    }
    c.push_str("ET\n");
    let res = dictionary! { "Font" => dictionary! { "OCR" => Object::Reference(font) } };
    attach_form(pdf, page, page_no, c, res)
}

fn n_(v: f64) -> String {
    n(v)
}

/// /Contents 若指向一個陣列物件（而非串流），展開成串流的參照
fn flatten_contents(pdf: &lopdf::Document, items: Vec<Object>) -> Vec<Object> {
    let mut out = Vec::new();
    for o in items {
        if let Object::Reference(id) = o {
            if let Ok(Object::Array(a)) = pdf.get_object(id) {
                out.extend(a.iter().cloned());
                continue;
            }
        }
        out.push(o);
    }
    out
}

// ---------------------------------------------------------------- 頁面重排

/// 只保留 `keep` 中的頁面（依序）。移除的頁面若仍被其他物件參照（目的地、表單欄位等），
/// 一律改成 null，讓它們在清理時真正從檔案中移除，避免把未匯出的頁面內容一併帶出。
fn rearrange_pages(pdf: &mut lopdf::Document, page_ids: &[ObjectId], keep: &[usize]) -> Result<(), String> {
    let root_pages = pdf
        .catalog()
        .and_then(|c| c.get(b"Pages"))
        .and_then(Object::as_reference)
        .map_err(|e| format!("找不到頁面樹：{e}"))?;

    let kept: Vec<ObjectId> = keep.iter().filter_map(|&i| page_ids.get(i).copied()).collect();
    for &id in &kept {
        // 先把繼承的屬性寫入頁面本身
        for key in [&b"Resources"[..], b"MediaBox", b"CropBox", b"Rotate"] {
            if pdf.get_dictionary(id).map(|d| d.has(key)).unwrap_or(true) {
                continue;
            }
            if let Some(v) = inherited(pdf, id, key) {
                if let Ok(d) = pdf.get_dictionary_mut(id) {
                    d.set(key, v);
                }
            }
        }
        if let Ok(d) = pdf.get_dictionary_mut(id) {
            d.set("Parent", Object::Reference(root_pages));
        }
    }
    {
        let pages = pdf.get_dictionary_mut(root_pages).map_err(|e| e.to_string())?;
        pages.set("Kids", Object::Array(kept.iter().map(|&id| Object::Reference(id)).collect()));
        pages.set("Count", kept.len() as i64);
    }

    // 文件結構與大綱、頁碼標籤只適用於原本的完整文件
    if let Ok(cat) = pdf.catalog_mut() {
        for key in [&b"Outlines"[..], b"StructTreeRoot", b"MarkInfo", b"PageLabels", b"Dests"] {
            cat.remove(key);
        }
    }

    let dropped: HashSet<ObjectId> = page_ids.iter().copied().filter(|id| !kept.contains(id)).collect();
    let mut intermediate: BTreeSet<ObjectId> = BTreeSet::new();
    for &id in page_ids {
        let mut cur = pdf.get_dictionary(id).ok().and_then(|d| d.get(b"Parent").ok()).and_then(|p| p.as_reference().ok());
        let mut guard = 0;
        while let Some(p) = cur {
            if p == root_pages || guard > 64 || !intermediate.insert(p) {
                break;
            }
            guard += 1;
            cur = pdf.get_dictionary(p).ok().and_then(|d| d.get(b"Parent").ok()).and_then(|x| x.as_reference().ok());
        }
    }
    let ids: Vec<ObjectId> = pdf.objects.keys().copied().collect();
    for id in ids {
        if dropped.contains(&id) || intermediate.contains(&id) {
            continue;
        }
        if let Some(obj) = pdf.objects.get_mut(&id) {
            null_refs(obj, &dropped, 0);
        }
    }
    // 指向已移除頁面的連結註解（目的地第一個元素變成 null）一併移除
    for &id in &kept {
        let annots = match pdf.get_dictionary(id).ok().and_then(|d| d.get(b"Annots").ok()).cloned() {
            Some(Object::Array(a)) => a,
            Some(Object::Reference(r)) => match pdf.get_object(r) {
                Ok(Object::Array(a)) => a.clone(),
                _ => continue,
            },
            _ => continue,
        };
        let filtered: Vec<Object> = annots.into_iter().filter(|a| !is_dead_link(pdf, a)).collect();
        if let Ok(d) = pdf.get_dictionary_mut(id) {
            d.set("Annots", Object::Array(filtered));
        }
    }
    Ok(())
}

fn null_refs(obj: &mut Object, dropped: &HashSet<ObjectId>, depth: usize) {
    if depth > 256 {
        return;
    }
    match obj {
        Object::Reference(id) if dropped.contains(id) => *obj = Object::Null,
        Object::Array(a) => a.iter_mut().for_each(|o| null_refs(o, dropped, depth + 1)),
        Object::Dictionary(d) => d.iter_mut().for_each(|(_, o)| null_refs(o, dropped, depth + 1)),
        Object::Stream(s) => s.dict.iter_mut().for_each(|(_, o)| null_refs(o, dropped, depth + 1)),
        _ => {}
    }
}

fn is_dead_link(pdf: &lopdf::Document, annot: &Object) -> bool {
    let Some(Object::Dictionary(d)) = resolve(pdf, annot) else { return false };
    let dest_dead = |o: &Object| matches!(resolve(pdf, o), Some(Object::Array(a)) if matches!(a.first(), Some(Object::Null)));
    if let Ok(dest) = d.get(b"Dest") {
        if dest_dead(dest) {
            return true;
        }
    }
    if let Ok(action) = d.get(b"A") {
        if let Some(Object::Dictionary(a)) = resolve(pdf, action) {
            if let Ok(dest) = a.get(b"D") {
                return dest_dead(dest);
            }
        }
    }
    false
}
