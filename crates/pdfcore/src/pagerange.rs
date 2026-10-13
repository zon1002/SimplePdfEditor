// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Fu Project

//! 頁碼範圍："1,3,5-8" -> 0 起算的頁碼（去重、保持順序）。

pub fn parse_page_range(text: &str, page_count: usize) -> Result<Vec<usize>, String> {
    let mut out: Vec<usize> = Vec::new();
    let normalized = text.replace('\u{FF0C}', ",").replace('\u{2013}', "-").replace('\u{FF0D}', "-");
    for part in normalized.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let bad = || format!("頁碼「{part}」不正確（有效範圍 1–{page_count}）");
        let parse = |s: &str| s.trim().parse::<usize>().ok().filter(|&v| v >= 1 && v <= page_count);
        let (a, b) = match part.split_once('-') {
            Some((x, y)) => (parse(x).ok_or_else(bad)?, parse(y).ok_or_else(bad)?),
            None => {
                let v = parse(part).ok_or_else(bad)?;
                (v, v)
            }
        };
        let range: Box<dyn Iterator<Item = usize>> = if a <= b { Box::new(a..=b) } else { Box::new((b..=a).rev()) };
        for p in range {
            if !out.contains(&(p - 1)) {
                out.push(p - 1);
            }
        }
    }
    if out.is_empty() {
        return Err("請輸入要匯出的頁碼".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges() {
        assert_eq!(parse_page_range("1,3,5-8", 10).unwrap(), vec![0, 2, 4, 5, 6, 7]);
        assert_eq!(parse_page_range("3-1", 5).unwrap(), vec![2, 1, 0]);
        assert_eq!(parse_page_range("2，2, 1", 5).unwrap(), vec![1, 0]);
        assert!(parse_page_range("0", 5).is_err());
        assert!(parse_page_range("6", 5).is_err());
        assert!(parse_page_range("a", 5).is_err());
        assert!(parse_page_range(" ", 5).is_err());
    }
}
