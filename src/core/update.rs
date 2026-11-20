//! 检查更新：GitHub Releases 查询 + 下载安装包

use crate::core::http::{http_download, http_get, http_get_location};
use crate::core::json::json_str;
use crate::core::{last_error, set_error};
use std::path::Path;

const UPDATE_REPO: &str = "TimKuitu/kuaiyi-ng";

/// 版本大小比较：`remote > local` 返回 `true`（支持 `N.N` / `N.N.N` 及可选 `v`/`V` 前缀）
pub fn version_newer(remote: &str, local: &str) -> bool {
    let parse_ver = |s: &str| -> [u32; 3] {
        let trimmed = s.strip_prefix(['v', 'V']).unwrap_or(s);
        let mut nums = [0u32; 3];
        for (i, part) in trimmed.split('.').take(3).enumerate() {
            let digits: String = part.chars().take_while(|c| c.is_ascii_digit()).collect();
            nums[i] = digits.parse().unwrap_or(0);
        }
        nums
    };
    let r = parse_ver(remote);
    let l = parse_ver(local);
    for i in 0..3 {
        if r[i] != l[i] {
            return r[i] > l[i];
        }
    }
    false
}

/// 从 `releases/latest` 页面 HTML 中提取 `/releases/tag/vX.Y.Z`
pub fn tag_from_html(html: &str) -> Option<String> {
    let marker = "/releases/tag/";
    let mut pos = 0usize;
    while let Some(hit) = html[pos..].find(marker) {
        let start = pos + hit + marker.len();
        let end_rel = html[start..]
            .find(['"', '\'', '?', '/', '<', '#'])
            .unwrap_or(html.len() - start);
        let tag = &html[start..start + end_rel];
        if (2..32).contains(&tag.len()) {
            return Some(tag.to_string());
        }
        pos = start + end_rel;
    }
    None
}

/// 检查 GitHub Releases 最新版本；成功返回 `Ok((is_newer, tag, setup_url))`
pub fn check_update(local_ver: &str) -> Result<(bool, String, String), String> {
    let api_url = format!("https://api.github.com/repos/{UPDATE_REPO}/releases/latest");
    let page_url = format!("https://github.com/{UPDATE_REPO}/releases/latest");

    let mut tag = String::new();
    if let Some(resp) = http_get(&api_url, Some("Accept: application/vnd.github+json\r\n")) {
        if let Some(t) = json_str(&resp, "tag_name") {
            tag = t;
        }
    }

    if tag.is_empty() {
        if let Some(loc) = http_get_location(&page_url) {
            if let Some(t) = tag_from_html(&loc) {
                tag = t;
            }
        }
        if tag.is_empty() {
            let Some(page) = http_get(&page_url, Some("Accept: text/html\r\n")) else {
                set_error("无法连接 GitHub（网络受限或超时）");
                return Err(last_error());
            };
            let Some(t) = tag_from_html(&page) else {
                set_error("版本信息解析失败（可能被限流）");
                return Err(last_error());
            };
            tag = t;
        }
    }

    let ver_digits = tag.strip_prefix(['v', 'V']).unwrap_or(&tag);
    if !ver_digits.starts_with(|c: char| c.is_ascii_digit()) {
        set_error("版本信息解析失败");
        return Err(last_error());
    }

    let url = format!(
        "https://github.com/{UPDATE_REPO}/releases/download/{tag}/KuaiYiNG_v{ver_digits}_Setup.exe"
    );
    Ok((version_newer(&tag, local_ver), tag, url))
}

/// 下载安装包到本地路径；支持可选字节进度回调
pub fn download_update<P: AsRef<Path>>(
    url: &str,
    dest: P,
    progress: Option<fn(i64, i64)>,
) -> Result<(), String> {
    let n = http_download(url, dest, progress);
    if n <= 0 {
        set_error("下载失败（网络错误或写入失败）");
        return Err(last_error());
    }
    if n < 100_000 {
        set_error("下载的文件异常（太小）");
        return Err(last_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_newer_and_tag_from_html() {
        assert!(version_newer("v1.4.0", "1.3.9"));
        assert!(version_newer("1.2.1", "1.2"));
        assert!(!version_newer("v1.4.0", "v1.4.0"));
        assert!(!version_newer("1.3.5", "1.4"));

        let html = r#"<a href="/TimKuitu/kuaiyi-ng/releases/tag/v2.6.1">v2.6.1</a>"#;
        assert_eq!(tag_from_html(html), Some("v2.6.1".to_string()));
        assert_eq!(tag_from_html("no release tag here"), None);
    }
}
