//! Remote CSS + font fetching with a local disk cache.
//! Mirrors `morph/style/css_fetcher.py` (`CSSFetcher`).
//!
//! Remote stylesheets (`import "https://..."`, `CSS.load("https://...")`)
//! are downloaded once into `<project>/.morph/css-cache`, `@font-face`
//! font binaries alongside them, and remote `url(...)` references are
//! rewritten to the local cache paths so builds work offline afterwards.
//! Fetch failures are warnings, never hard errors: like a browser that
//! cannot reach a stylesheet, the build continues without it.

use std::path::PathBuf;

/// User-Agent sent for stylesheet and font downloads.
const USER_AGENT: &str = "levizr-morph/0.1";

pub struct CssFetcher {
    cache_dir: PathBuf,
    silent: bool,
}

impl CssFetcher {
    pub fn new(cache_dir: PathBuf) -> Self {
        Self { cache_dir, silent: false }
    }

    pub fn silent(mut self) -> Self {
        self.silent = true;
        self
    }

    fn warn(&self, msg: &str) {
        if !self.silent {
            eprintln!("  ⚠ {}", msg);
        }
    }

    fn note(&self, msg: &str) {
        if !self.silent {
            eprintln!("  … {}", msg);
        }
    }

    /// Fetch a CSS string, using the local cache after the first download.
    /// Returns `None` when the stylesheet cannot be obtained.
    pub fn fetch(&self, url: &str) -> Option<String> {
        let cached = self.cache_path(url);
        if let Ok(text) = std::fs::read_to_string(&cached) {
            return Some(text);
        }
        self.note(&format!("Fetching {}", url));
        let bytes = http_get(url, USER_AGENT).ok()?;
        let css = String::from_utf8_lossy(&bytes).into_owned();
        if std::fs::create_dir_all(&self.cache_dir).is_ok() {
            let _ = std::fs::write(&cached, &css);
        }
        Some(css)
    }

    /// Fetch CSS and also download any remote `@font-face` font files,
    /// rewriting the remote `url(...)` references to local cache paths.
    pub fn fetch_with_fonts(&self, url: &str) -> Option<String> {
        let css = self.fetch(url)?;
        if css.is_empty() {
            return Some(css);
        }
        let downloader = |font_url: &str| self.cache_binary(font_url);
        Some(rewrite_remote_urls(&css, &downloader))
    }

    fn cache_path(&self, url: &str) -> PathBuf {
        self.cache_dir.join(format!("{}.css", &md5_hex(url)[..12]))
    }

    fn cache_binary(&self, url: &str) -> Option<PathBuf> {
        let ext = url
            .split('?')
            .next()
            .unwrap_or(url)
            .rsplit('.')
            .next()
            .filter(|e| e.len() <= 5 && !e.contains('/'))
            .map(|e| format!(".{}", e))
            .unwrap_or_else(|| ".bin".to_string());
        let path = self.cache_dir.join(format!("{}{}", &md5_hex(url)[..12], ext));
        if path.exists() {
            return Some(path);
        }
        let data = http_get(url, USER_AGENT).ok()?;
        if std::fs::create_dir_all(&self.cache_dir).is_err() {
            return None;
        }
        std::fs::write(&path, &data).ok()?;
        Some(path)
    }
}

/// Rewrite every remote `url(https?://...)` in `css` to a locally cached
/// path via `fetch`. Unresolvable URLs keep their original text so the
/// stylesheet still applies when the network is back.
pub fn rewrite_remote_urls(css: &str, fetch: &dyn Fn(&str) -> Option<PathBuf>) -> String {
    let mut out = String::with_capacity(css.len());
    let bytes = css.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if css[i..].starts_with("url(") {
            let inner_start = i + 4;
            if let Some(close) = css[inner_start..].find(')') {
                let raw = css[inner_start..inner_start + close].trim();
                let unquoted = raw.trim_matches(|c| c == '"' || c == '\'');
                if unquoted.starts_with("http://") || unquoted.starts_with("https://") {
                    if let Some(local) = fetch(unquoted) {
                        out.push_str("url(\"");
                        out.push_str(&local.display().to_string());
                        out.push_str("\")");
                        i = inner_start + close + 1;
                        continue;
                    }
                }
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn http_get(url: &str, user_agent: &str) -> Result<Vec<u8>, String> {
    // curl keeps this dependency-free and matches the "no root / no package
    // manager" goal of the UPX and static-deps downloaders.
    let out = std::process::Command::new("curl")
        .args(["-fsSL", "--max-time", "10", "-A", user_agent, url])
        .output()
        .map_err(|e| format!("failed to fetch {} — {}", url, e))?;
    if !out.status.success() {
        return Err(format!("failed to fetch {} (curl error)", url));
    }
    Ok(out.stdout)
}

/// Lowercase hex MD5 (public-domain algorithm, compact inline version so the
/// cache filenames match Python's `hashlib.md5(url).hexdigest()[:12]`).
pub fn md5_hex(input: &str) -> String {
    let mut state: [u32; 4] = [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476];
    let mut msg = input.as_bytes().to_vec();
    let bit_len = (msg.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_le_bytes());
    const S: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5,
        9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10,
        15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];
    // Per-round constants T[i] = floor(2^32 * abs(sin(i+1))), computed rather
    // than hardcoded so a transcription typo is impossible.
    let mut k = [0u32; 64];
    for (i, slot) in k.iter_mut().enumerate() {
        *slot = ((f64::from(i as u32 + 1).sin().abs() * 4294967296.0) as u64 & 0xffff_ffff) as u32;
    }
    for chunk in msg.chunks_exact(64) {
        let mut m = [0u32; 16];
        for (i, w) in m.iter_mut().enumerate() {
            *w = u32::from_le_bytes([
                chunk[4 * i],
                chunk[4 * i + 1],
                chunk[4 * i + 2],
                chunk[4 * i + 3],
            ]);
        }
        let (mut a, mut b, mut c, mut d) = (state[0], state[1], state[2], state[3]);
        for i in 0..64 {
            let (f, g) = match i {
                0..=15 => ((b & c) | ((!b) & d), i),
                16..=31 => ((d & b) | ((!d) & c), (5 * i + 1) % 16),
                32..=47 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | (!d)), (7 * i) % 16),
            };
            let tmp = d;
            d = c;
            c = b;
            b = b.wrapping_add(
                a.wrapping_add(f).wrapping_add(k[i]).wrapping_add(m[g]).rotate_left(S[i]),
            );
            a = tmp;
        }
        state[0] = state[0].wrapping_add(a);
        state[1] = state[1].wrapping_add(b);
        state[2] = state[2].wrapping_add(c);
        state[3] = state[3].wrapping_add(d);
    }
    let mut hex = String::with_capacity(32);
    for word in state {
        for byte in word.to_le_bytes() {
            hex.push_str(&format!("{:02x}", byte));
        }
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn md5_matches_python_hashlib() {
        // `python3 -c "import hashlib; print(hashlib.md5(b'...').hexdigest())"`
        assert_eq!(md5_hex(""), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(md5_hex("abc"), "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(md5_hex("https://example.com/app.css"), "bdeef555fa64a016c0543b683223e307");
    }

    #[test]
    fn rewrite_swaps_remote_urls_and_keeps_rest() {
        let css = "@font-face{src:url(https://x.test/a.woff2);} .a{background:url(\"local.png\");}";
        let out = rewrite_remote_urls(css, &|url| {
            assert_eq!(url, "https://x.test/a.woff2");
            Some(PathBuf::from(".morph/css-cache/abc123.woff2"))
        });
        assert!(out.contains("url(\".morph/css-cache/abc123.woff2\")"));
        assert!(out.contains("url(\"local.png\")"));
    }

    #[test]
    fn rewrite_keeps_unresolvable_urls_verbatim() {
        let css = ".a{background:url(https://down.test/b.png);}";
        let out = rewrite_remote_urls(css, &|_| None);
        assert_eq!(out, css);
    }

    #[test]
    fn cached_fetch_never_touches_network() {
        let dir = std::env::temp_dir().join(format!("morph_css_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let url = "https://unreachable.invalid/style.css";
        std::fs::write(dir.join(format!("{}.css", &md5_hex(url)[..12])), ".a{color:red}").unwrap();
        let fetcher = CssFetcher::new(dir.clone()).silent();
        assert_eq!(fetcher.fetch(url).as_deref(), Some(".a{color:red}"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
