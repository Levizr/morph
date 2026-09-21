//! File-based routing manifest: index every `route.mx` file.
//!
//! A route id is the file's folder path relative to the source root
//! (`src/auth/login/route.mx` → `/auth/login`), not the filename.
//! Entries are sorted by id so RID assignment is deterministic across
//! builds. Files that fail to parse are skipped here — imported modules
//! error in their own pass; the manifest is a best-effort index.
use std::path::{Path, PathBuf};

/// One indexed route file.
#[derive(Debug, Clone)]
pub struct RouteEntry {
    /// Route id: `/auth/login` (folder path, always leading `/`).
    pub id: String,
    /// Interned route id: index in sorted-id order (the RID).
    pub rid: usize,
    /// Absolute path to the `route.mx` file.
    pub file: PathBuf,
    /// C++ const name for `app::routes::` (e.g. `kAuthLogin`).
    pub const_name: String,
    /// The route file's `windowConfig` export, when present.
    pub title: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// Owner id/route from `windowConfig.parent` (`""` = none).
    pub parent: String,
    /// `windowConfig.modal`.
    pub modal: bool,
    /// `windowConfig.role` literal (`""` = default; validated at lowering).
    pub role: String,
    /// Whether the file has a default-export component (`mx-route-no-export`).
    pub has_default_export: bool,
}

/// Scan `src_root` for files named exactly `route.mx` and index them.
/// Skips dot-directories, `node_modules`, and `_`-prefixed (private)
/// folders at any depth — those are never routes.
pub fn scan_routes(src_root: &Path) -> Vec<RouteEntry> {
    let mut found: Vec<(String, PathBuf)> = Vec::new();
    collect_routes(src_root, src_root, &mut found);
    found.sort_by(|a, b| a.0.cmp(&b.0));

    let mut entries = Vec::with_capacity(found.len());
    let mut used_consts = std::collections::HashSet::new();
    for (rid, (id, file)) in found.into_iter().enumerate() {
        let parsed = crate::parse_mx_file(&file).ok();
        let wc = parsed.as_ref().and_then(|s| s.window_config.clone());
        let has_default_export =
            parsed.as_ref().map(|s| s.components.iter().any(|c| c.is_default)).unwrap_or(false);
        let const_name = unique_const_name(&id, &mut used_consts);
        entries.push(RouteEntry {
            id,
            rid,
            file,
            const_name,
            title: wc.as_ref().map(|w| w.title.clone()),
            width: wc.as_ref().map(|w| w.width),
            height: wc.as_ref().map(|w| w.height),
            parent: wc.as_ref().map(|w| w.parent.clone()).unwrap_or_default(),
            modal: wc.as_ref().is_some_and(|w| w.modal),
            role: wc.as_ref().map(|w| w.role.clone()).unwrap_or_default(),
            has_default_export,
        });
    }
    entries
}

fn collect_routes(dir: &Path, src_root: &Path, out: &mut Vec<(String, PathBuf)>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if name.starts_with('.') || name == "node_modules" || name == "target" {
                continue;
            }
            collect_routes(&path, src_root, out);
            continue;
        }
        if name != "route.mx" {
            continue;
        }
        let parent = match path.parent().and_then(|p| p.strip_prefix(src_root).ok()) {
            Some(parent) => parent,
            None => continue,
        };
        // `_`-prefixed folders are private (Next.js rule) — never routes,
        // even when they contain a file literally named route.mx.
        if parent.components().any(|c| c.as_os_str().to_string_lossy().starts_with('_')) {
            continue;
        }
        let mut id = String::from("/");
        id.push_str(
            &parent
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/"),
        );
        if id.len() > 1 {
            id = id.trim_end_matches('/').to_string();
        }
        out.push((id, path));
    }
}

/// `/auth/login` → `kAuthLogin`; `/` (root) → `kRoot`. Anything outside
/// `[A-Za-z0-9]` becomes `_`; collisions get a numeric suffix.
fn unique_const_name(id: &str, used: &mut std::collections::HashSet<String>) -> String {
    let mut base = String::from("k");
    let mut any_segment = false;
    for segment in id.split('/') {
        if segment.is_empty() {
            continue;
        }
        any_segment = true;
        let mut chars = segment.chars();
        if let Some(first) = chars.next() {
            base.extend(first.to_uppercase());
            for c in chars {
                base.push(if c.is_ascii_alphanumeric() { c } else { '_' });
            }
        }
    }
    if !any_segment {
        base.push_str("Root");
    }
    if !used.contains(&base) {
        used.insert(base.clone());
        return base;
    }
    let mut n = 2;
    loop {
        let candidate = format!("{base}{n}");
        if !used.contains(&candidate) {
            used.insert(candidate.clone());
            return candidate;
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_tree(name: &str, files: &[&str]) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("morph-routes-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for file in files {
            let path = root.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, "export default function Page() { return (<div/>); }").unwrap();
        }
        root
    }

    #[test]
    fn indexes_nested_routes_with_sorted_rids() {
        let root = write_tree(
            "nested",
            &["settings/route.mx", "auth/login/route.mx", "auth/register/route.mx"],
        );
        let routes = scan_routes(&root);
        let ids: Vec<&str> = routes.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, vec!["/auth/login", "/auth/register", "/settings"]);
        assert_eq!(routes[0].rid, 0);
        assert_eq!(routes[2].rid, 2);
        assert_eq!(routes[0].const_name, "kAuthLogin");
        assert_eq!(routes[2].const_name, "kSettings");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn skips_non_routes_and_private_folders() {
        let root = write_tree(
            "skips",
            &[
                "components/Button.mx",
                "other/page.mx",
                "blog/_components/route.mx",
                "_hidden/route.mx",
                "shop/route.mx",
            ],
        );
        let routes = scan_routes(&root);
        let ids: Vec<&str> = routes.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, vec!["/shop"]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn root_route_and_const_collisions() {
        let root = write_tree("collisions", &["route.mx", "a-b/route.mx", "a_b/route.mx"]);
        let routes = scan_routes(&root);
        let by_id: std::collections::HashMap<&str, &str> =
            routes.iter().map(|r| (r.id.as_str(), r.const_name.as_str())).collect();
        assert_eq!(by_id["/"], "kRoot");
        // `/a-b` and `/a_b` sanitize identically — one wins `kA_b`,
        // the other takes the numeric suffix.
        let mut pair = [by_id["/a-b"], by_id["/a_b"]];
        pair.sort();
        assert_eq!(pair, ["kA_b", "kA_b2"]);
        let _ = std::fs::remove_dir_all(&root);
    }
}
