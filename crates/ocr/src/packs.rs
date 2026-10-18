// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Fufeng Project

//! OCR 語言包：清單（manifest）、安裝位置、完整性檢查、安裝／移除／從檔案加入。
//!
//! 每個語言包是一個資料夾 `packs/<id>/`，內含 `pack.json` 與模型等檔案。
//! 安裝程式裝的放在 `{程式資料夾}\ocr\packs\`；事後下載或匯入的放在
//! `%LOCALAPPDATA%\SimplePdfEditor\ocr\packs\`（不需系統管理員權限），兩處都會讀取。

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};

/// 語言包種類
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackKind {
    /// 辨識引擎（onnxruntime.dll）與共用的文字方向模型
    Engine,
    /// 一組偵測＋辨識模型（中英日，標準／高精度／最高精度）
    Tier,
    /// 額外語言的辨識模型（韓文、拉丁語系…），使用已安裝的偵測模型
    Language,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PackFile {
    pub name: String,
    pub sha256: String,
    pub size: u64,
    /// 模型用途：det、rec、cls；其他檔案（授權等）為空
    #[serde(default)]
    pub role: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PackInfo {
    pub id: String,
    pub kind: PackKind,
    pub name: String,
    /// 正確率與速度說明（顯示在安裝程式與語言管理）
    pub description: String,
    pub version: u32,
    /// 辨識的語言（BCP-47）
    #[serde(default)]
    pub languages: Vec<String>,
    pub files: Vec<PackFile>,
    /// 語言包 zip 的下載網址、大小與 SHA-256
    pub url: String,
    pub zip_size: u64,
    pub zip_sha256: String,
    /// 安裝 OCR 時預設勾選
    #[serde(default)]
    pub default: bool,
    /// 排序：同種類中數字小的在前
    #[serde(default)]
    pub order: u32,
}

impl PackInfo {
    pub fn total_size(&self) -> u64 {
        self.files.iter().map(|f| f.size).sum()
    }
    pub fn file(&self, role: &str) -> Option<&PackFile> {
        self.files.iter().find(|f| f.role == role)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub version: u32,
    pub packs: Vec<PackInfo>,
}

/// 隨程式附帶的清單（離線也能列出可安裝的語言包）
pub const BUNDLED_MANIFEST: &str = include_str!("../../../ocr/ocr-packs.json");

/// 最新清單的網址（GitHub Releases）
pub const MANIFEST_URL: &str = "https://github.com/zon1002/SimplePdfEditor/releases/download/ocr-packs-v1/ocr-packs.json";

pub fn bundled_manifest() -> Manifest {
    serde_json::from_str(BUNDLED_MANIFEST).unwrap_or(Manifest { version: 0, packs: Vec::new() })
}

pub fn parse_manifest(text: &str) -> Result<Manifest, String> {
    serde_json::from_str(text).map_err(|e| format!("語言包清單格式錯誤：{e}"))
}

/// 已安裝的語言包
#[derive(Clone, Debug, PartialEq)]
pub struct InstalledPack {
    pub info: PackInfo,
    pub dir: PathBuf,
    /// 位於使用者資料夾（可由程式移除）；否則為安裝程式安裝
    pub user: bool,
}

impl InstalledPack {
    pub fn path(&self, role: &str) -> Option<PathBuf> {
        self.info.file(role).map(|f| self.dir.join(&f.name))
    }
}

/// 安裝程式的語言包資料夾：`{程式資料夾}\ocr\packs`
pub fn system_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    Some(exe.parent()?.join("ocr").join("packs"))
}

/// 使用者的語言包資料夾：`%LOCALAPPDATA%\SimplePdfEditor\ocr\packs`
pub fn user_dir() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")?;
    Some(PathBuf::from(base).join("SimplePdfEditor").join("ocr").join("packs"))
}

/// 額外的搜尋位置（開發與測試用）：環境變數 SPE_OCR_PACKS
fn extra_dirs() -> Vec<PathBuf> {
    std::env::var_os("SPE_OCR_PACKS").map(|v| std::env::split_paths(&v).collect()).unwrap_or_default()
}

/// 讀取一個語言包資料夾：pack.json 存在且所有檔案大小相符才算安裝完成
pub fn read_pack(dir: &Path) -> Option<PackInfo> {
    let text = std::fs::read_to_string(dir.join("pack.json")).ok()?;
    let info: PackInfo = serde_json::from_str(&text).ok()?;
    let complete = info.files.iter().all(|f| std::fs::metadata(dir.join(&f.name)).map(|m| m.len() == f.size).unwrap_or(false));
    complete.then_some(info)
}

/// 所有已安裝的語言包（使用者資料夾優先，同一個 id 只取一份）
pub fn installed() -> Vec<InstalledPack> {
    let mut out: Vec<InstalledPack> = Vec::new();
    let mut roots: Vec<(PathBuf, bool)> = Vec::new();
    for d in extra_dirs() {
        roots.push((d, true));
    }
    if let Some(d) = user_dir() {
        roots.push((d, true));
    }
    if let Some(d) = system_dir() {
        roots.push((d, false));
    }
    for (root, user) in roots {
        let Ok(entries) = std::fs::read_dir(&root) else { continue };
        for e in entries.flatten() {
            let dir = e.path();
            let Some(info) = read_pack(&dir) else { continue };
            if out.iter().any(|p| p.info.id == info.id) {
                continue;
            }
            out.push(InstalledPack { info, dir, user });
        }
    }
    out.sort_by_key(|p| (p.info.kind as u8, p.info.order));
    out
}

pub fn find_installed(id: &str) -> Option<InstalledPack> {
    installed().into_iter().find(|p| p.info.id == id)
}

/// 計算檔案的 SHA-256（小寫十六進位）
pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// 檢查資料夾中的檔案是否與清單完全相符（大小與 SHA-256）
pub fn verify_dir(dir: &Path, info: &PackInfo) -> Result<(), String> {
    for f in &info.files {
        let p = dir.join(&f.name);
        let size = std::fs::metadata(&p).map_err(|_| format!("缺少檔案：{}", f.name))?.len();
        if size != f.size {
            return Err(format!("檔案大小不符：{}", f.name));
        }
        let h = sha256_file(&p).map_err(|e| format!("無法讀取 {}：{e}", f.name))?;
        if !h.eq_ignore_ascii_case(&f.sha256) {
            return Err(format!("檔案檢查碼不符（可能已損壞或遭竄改）：{}", f.name));
        }
    }
    Ok(())
}

/// 從 zip 安裝到使用者資料夾：解壓縮到暫存資料夾、逐檔驗證，成功後才換上
pub fn install_zip(zip_path: &Path, expected: Option<&PackInfo>) -> Result<PackInfo, String> {
    let root = user_dir().ok_or("找不到使用者資料夾（LOCALAPPDATA）")?;
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let file = std::fs::File::open(zip_path).map_err(|e| format!("無法開啟語言包：{e}"))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|_| "不是有效的語言包（zip）檔案".to_string())?;
    let staging = root.join(format!(".staging-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
    let result = (|| -> Result<PackInfo, String> {
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
            if entry.is_dir() {
                continue;
            }
            // 只取檔名（忽略 zip 內的資料夾），防止路徑跳脫
            let Some(name) = entry.enclosed_name().and_then(|p| p.file_name().map(|n| n.to_owned())) else { continue };
            let mut out = std::fs::File::create(staging.join(&name)).map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut out).map_err(|e| format!("解壓縮失敗：{e}"))?;
        }
        let info = read_pack_unchecked(&staging).ok_or("語言包缺少 pack.json")?;
        if let Some(exp) = expected {
            if exp.id != info.id {
                return Err(format!("語言包不符（預期 {}，實際 {}）", exp.id, info.id));
            }
        }
        // 以隨程式附帶的清單為準：已知語言包的檔案檢查碼必須與清單相同
        let reference = bundled_manifest().packs.into_iter().find(|p| p.id == info.id && p.version == info.version);
        let check = reference.as_ref().unwrap_or(&info);
        verify_dir(&staging, check)?;
        let dest = root.join(&info.id);
        let _ = std::fs::remove_dir_all(&dest);
        std::fs::rename(&staging, &dest).map_err(|e| format!("無法安裝語言包：{e}"))?;
        Ok(info)
    })();
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&staging);
    }
    result
}

fn read_pack_unchecked(dir: &Path) -> Option<PackInfo> {
    serde_json::from_str(&std::fs::read_to_string(dir.join("pack.json")).ok()?).ok()
}

/// 移除使用者資料夾中的語言包（安裝程式安裝的請以安裝程式移除）
pub fn remove(pack: &InstalledPack) -> Result<(), String> {
    if !pack.user {
        return Err("此語言包由安裝程式安裝，請重新執行安裝程式取消勾選以移除。".into());
    }
    std::fs::remove_dir_all(&pack.dir).map_err(|e| format!("無法移除：{e}"))
}

/// HTTP 連線：使用 Windows 內建的 TLS（SChannel）與系統的根憑證
pub fn agent() -> ureq::Agent {
    let tls = ureq::tls::TlsConfig::builder()
        .provider(ureq::tls::TlsProvider::NativeTls)
        .root_certs(ureq::tls::RootCerts::PlatformVerifier)
        .build();
    ureq::Agent::config_builder().tls_config(tls).user_agent("SimplePdfEditor").build().into()
}

/// 連線錯誤轉成使用者看得懂的說明
fn http_error(e: &ureq::Error, what: &str) -> String {
    match e {
        ureq::Error::StatusCode(404) => format!("{what}：伺服器上找不到檔案（HTTP 404），語言包可能尚未發布或網址已變更。可改用「從檔案加入」。"),
        ureq::Error::StatusCode(code) => format!("{what}：伺服器回應錯誤（HTTP {code}），請稍後再試。"),
        ureq::Error::Io(_) | ureq::Error::HostNotFound | ureq::Error::ConnectionFailed | ureq::Error::Timeout(_) => {
            format!("{what}：無法連線（{e}）。請確認網路連線與 Proxy 設定，或改用「從檔案加入」。")
        }
        _ => format!("{what}：{e}"),
    }
}

/// 下載進度回呼：(已下載, 總大小)；回傳 false 表示取消
pub type Progress<'a> = &'a mut dyn FnMut(u64, u64) -> bool;

/// 下載檔案到指定路徑（邊下載邊計算 SHA-256，最後比對）
pub fn download(url: &str, dest: &Path, sha256: &str, size: u64, progress: Progress<'_>) -> Result<(), String> {
    let resp = agent().get(url).call().map_err(|e| http_error(&e, "下載失敗"))?;
    let mut reader = resp.into_body().into_reader();
    let mut out = std::fs::File::create(dest).map_err(|e| e.to_string())?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    let mut done = 0u64;
    loop {
        let n = reader.read(&mut buf).map_err(|e| format!("下載中斷：{e}"))?;
        if n == 0 {
            break;
        }
        std::io::Write::write_all(&mut out, &buf[..n]).map_err(|e| e.to_string())?;
        h.update(&buf[..n]);
        done += n as u64;
        if !progress(done, size) {
            drop(out);
            let _ = std::fs::remove_file(dest);
            return Err("已取消下載".into());
        }
    }
    drop(out);
    let got: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if !got.eq_ignore_ascii_case(sha256) {
        let _ = std::fs::remove_file(dest);
        return Err("下載的檔案檢查碼不符（可能已損壞或遭竄改），已刪除".into());
    }
    Ok(())
}

/// 下載並安裝語言包到使用者資料夾
pub fn download_and_install(info: &PackInfo, progress: Progress<'_>) -> Result<PackInfo, String> {
    let root = user_dir().ok_or("找不到使用者資料夾（LOCALAPPDATA）")?;
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let tmp = root.join(format!(".download-{}.zip", info.id));
    download(&info.url, &tmp, &info.zip_sha256, info.zip_size, progress)?;
    let r = install_zip(&tmp, Some(info));
    let _ = std::fs::remove_file(&tmp);
    r
}

/// 取得最新的語言包清單
pub fn fetch_manifest() -> Result<Manifest, String> {
    let resp = agent().get(MANIFEST_URL).call().map_err(|e| http_error(&e, "無法取得語言包清單"))?;
    let text = resp.into_body().read_to_string().map_err(|e| e.to_string())?;
    parse_manifest(&text)
}
