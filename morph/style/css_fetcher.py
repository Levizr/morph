from __future__ import annotations
import hashlib
import os
import re
import urllib.request

CACHE_DIR = ".morph/css-cache"


class CSSFetcher:
    """
    Fetches remote CSS files and caches them locally.
    Also handles @font-face — downloads font files and rewrites URLs.
    """

    def fetch(self, url: str) -> str:
        """Fetch CSS string, using local cache after first download."""
        cached = self._cache_path(url)

        if os.path.exists(cached):
            return open(cached, encoding="utf-8").read()

        print(f"[morph] Fetching {url}")
        try:
            req = urllib.request.Request(
                url,
                headers={"User-Agent": "levizr-morph/0.1"}
            )
            with urllib.request.urlopen(req, timeout=10) as r:
                css = r.read().decode("utf-8")
        except Exception as e:
            print(f"[morph] warn: failed to fetch {url} — {e}")
            return ""

        os.makedirs(CACHE_DIR, exist_ok=True)
        open(cached, "w", encoding="utf-8").write(css)
        return css

    def fetch_with_fonts(self, url: str) -> str:
        """
        Fetch CSS and also download any @font-face font files.
        Rewrites remote font URLs to local cache paths.
        """
        css = self.fetch(url)
        if not css:
            return css

        # find all url(...) references inside the CSS
        font_urls = re.findall(
            r'url\(["\'\?](https?://[^"\')\s]+)["\'\?]?\)', css
        )


        for font_url in font_urls:
            local = self._cache_binary(font_url)
            if local:
                css = css.replace(font_url, local)

        return css

    def clear_cache(self) -> None:
        import shutil
        if os.path.exists(CACHE_DIR):
            shutil.rmtree(CACHE_DIR)

    # ── Internal ──────────────────────────────────────────────

    def _cache_path(self, url: str) -> str:
        h = hashlib.md5(url.encode()).hexdigest()[:12]
        return os.path.join(CACHE_DIR, f"{h}.css")

    def _cache_binary(self, url: str) -> str | None:
        ext  = os.path.splitext(url.split("?")[0])[1] or ".bin"
        h    = hashlib.md5(url.encode()).hexdigest()[:12]
        path = os.path.join(CACHE_DIR, f"{h}{ext}")

        if os.path.exists(path):
            return path

        try:
            req = urllib.request.Request(
                url, headers={"User-Agent": "levizr-morph/0.1"}
            )
            with urllib.request.urlopen(req, timeout=10) as r:
                data = r.read()
            os.makedirs(CACHE_DIR, exist_ok=True)
            open(path, "wb").write(data)
            return path
        except Exception as e:
            print(f"[morph] warn: failed to fetch font {url} — {e}")
            return None
