// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Fufeng Project

//! 製作 OCR 語言包（開發／發行用工具，不隨程式散布）
//!
//!   ocr-packs fetch <快取資料夾>               依 ocr/packs.src.json 下載上游模型與 ONNX Runtime，並驗證 SHA-256
//!   ocr-packs build <快取資料夾> <輸出資料夾>   組成各語言包資料夾與 zip，並寫入 ocr/ocr-packs.json（含大小與檢查碼）

use ocr::packs::{Manifest, PackFile, PackInfo, PackKind, sha256_file};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
struct Upstream {
    url: String,
    sha256: String,
}

#[derive(Deserialize)]
struct SrcFile {
    name: String,
    #[serde(default)]
    role: String,
    from: String,
}

#[derive(Deserialize)]
struct SrcPack {
    id: String,
    kind: PackKind,
    name: String,
    description: String,
    version: u32,
    #[serde(default)]
    order: u32,
    #[serde(default)]
    default: bool,
    #[serde(default)]
    languages: Vec<String>,
    files: Vec<SrcFile>,
}

#[derive(Deserialize)]
struct Src {
    release_url: String,
    upstream: BTreeMap<String, Upstream>,
    packs: Vec<SrcPack>,
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn load_src() -> Src {
    let text = std::fs::read_to_string(repo_root().join("ocr/packs.src.json")).expect("ocr/packs.src.json");
    serde_json::from_str(&text).expect("packs.src.json 格式錯誤")
}

fn fetch(cache: &Path) {
    let src = load_src();
    std::fs::create_dir_all(cache).unwrap();
    for (name, up) in &src.upstream {
        let dest = cache.join(name);
        if dest.exists() {
            let h = sha256_file(&dest).unwrap();
            if up.sha256.is_empty() || h.eq_ignore_ascii_case(&up.sha256) {
                println!("已存在 {name}  {h}");
                continue;
            }
            println!("檢查碼不符，重新下載 {name}");
        }
        println!("下載 {name} …");
        let mut last = 0;
        let mut progress = |done: u64, _: u64| {
            if done - last > 20 << 20 {
                last = done;
                println!("  {} MB", done >> 20);
            }
            true
        };
        if up.sha256.is_empty() {
            // 上游未公布檢查碼的檔案（ONNX Runtime 的 zip）：下載後印出檢查碼，請填入 packs.src.json
            let resp = ocr::packs::agent().get(&up.url).call().expect("下載失敗");
            let mut r = resp.into_body().into_reader();
            let mut f = std::fs::File::create(&dest).unwrap();
            std::io::copy(&mut r, &mut f).unwrap();
            println!("  SHA-256 = {}（請填入 packs.src.json）", sha256_file(&dest).unwrap());
        } else {
            ocr::packs::download(&up.url, &dest, &up.sha256, 0, &mut progress).expect("下載或驗證失敗");
            println!("  完成，檢查碼相符");
        }
    }
}

/// 取得來源檔案內容：一般檔名、`zip名稱:路徑`（從 zip 取出），或 `license:apache`
fn source_bytes(cache: &Path, src: &Src, from: &str) -> Vec<u8> {
    if from == "license:apache" {
        let apache = std::fs::read_to_string(repo_root().join("LICENSE-APACHE")).expect("LICENSE-APACHE");
        return format!(
            "本語言包的模型取自 PaddleOCR（Copyright (c) PaddlePaddle Authors），\n\
             ONNX 格式由 RapidOCR（RapidAI）轉換，皆以 Apache License 2.0 授權：\n\
             https://github.com/PaddlePaddle/PaddleOCR\n\
             https://github.com/RapidAI/RapidOCR\n\n{apache}"
        )
        .into_bytes();
    }
    if let Some((zip_name, inner)) = from.split_once(':') {
        let up = src.upstream.get(zip_name).expect("未知的上游檔案");
        let path = cache.join(zip_name);
        if !up.sha256.is_empty() {
            assert!(sha256_file(&path).unwrap().eq_ignore_ascii_case(&up.sha256), "{zip_name} 檢查碼不符");
        }
        let mut z = zip::ZipArchive::new(std::fs::File::open(&path).unwrap()).unwrap();
        let mut e = z.by_name(inner).unwrap_or_else(|_| panic!("{zip_name} 中找不到 {inner}"));
        let mut buf = Vec::new();
        e.read_to_end(&mut buf).unwrap();
        return buf;
    }
    let up = src.upstream.get(from).expect("未知的上游檔案");
    let path = cache.join(from);
    assert!(sha256_file(&path).unwrap().eq_ignore_ascii_case(&up.sha256), "{from} 檢查碼不符，請先執行 fetch");
    std::fs::read(path).unwrap()
}

fn build(cache: &Path, out: &Path) {
    let src = load_src();
    std::fs::create_dir_all(out).unwrap();
    let mut packs = Vec::new();
    for sp in &src.packs {
        let dir = out.join(&sp.id);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut files = Vec::new();
        for f in &sp.files {
            let bytes = source_bytes(cache, &src, &f.from);
            let path = dir.join(&f.name);
            std::fs::write(&path, &bytes).unwrap();
            files.push(PackFile { name: f.name.clone(), sha256: sha256_file(&path).unwrap(), size: bytes.len() as u64, role: f.role.clone() });
        }
        let mut info = PackInfo {
            id: sp.id.clone(),
            kind: sp.kind,
            name: sp.name.clone(),
            description: sp.description.clone(),
            version: sp.version,
            languages: sp.languages.clone(),
            files,
            url: String::new(),
            zip_size: 0,
            zip_sha256: String::new(),
            default: sp.default,
            order: sp.order,
        };
        // 語言包資料夾內的 pack.json 不含 zip 本身的資訊
        std::fs::write(dir.join("pack.json"), serde_json::to_string_pretty(&info).unwrap()).unwrap();
        let zip_name = format!("ocr-{}-v{}.zip", sp.id, sp.version);
        let zip_path = out.join(&zip_name);
        write_zip(&dir, &zip_path);
        info.url = format!("{}/{zip_name}", src.release_url);
        info.zip_size = std::fs::metadata(&zip_path).unwrap().len();
        info.zip_sha256 = sha256_file(&zip_path).unwrap();
        println!("{:8} {:>7.1} MB → {zip_name} {:>7.1} MB", sp.id, info.total_size() as f64 / 1048576.0, info.zip_size as f64 / 1048576.0);
        packs.push(info);
    }
    let manifest = Manifest { version: 1, packs };
    let text = serde_json::to_string_pretty(&manifest).unwrap();
    std::fs::write(out.join("ocr-packs.json"), &text).unwrap();
    std::fs::write(repo_root().join("ocr/ocr-packs.json"), &text).unwrap();
    println!("已寫入 ocr/ocr-packs.json");
}

fn write_zip(dir: &Path, zip_path: &Path) {
    let mut zw = zip::ZipWriter::new(std::fs::File::create(zip_path).unwrap());
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).last_modified_time(zip::DateTime::default());
    let mut names: Vec<_> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.file_name()).collect();
    names.sort();
    for n in names {
        let name = n.to_string_lossy().into_owned();
        zw.start_file(&name, opts).unwrap();
        zw.write_all(&std::fs::read(dir.join(&n)).unwrap()).unwrap();
    }
    zw.finish().unwrap();
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("fetch") if args.len() >= 3 => fetch(Path::new(&args[2])),
        Some("build") if args.len() >= 4 => build(Path::new(&args[2]), Path::new(&args[3])),
        _ => eprintln!("用法：ocr-packs fetch <快取資料夾> | ocr-packs build <快取資料夾> <輸出資料夾>"),
    }
}
