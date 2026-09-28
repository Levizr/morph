//! Transitive `.mx`/`.ts`/`.tsx` module graph resolution for component imports.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};

use crate::ast_types::MxSource;

/// One parsed module in the graph.
#[derive(Debug, Clone)]
pub struct ResolvedModule {
    pub path: PathBuf,
    pub dir: PathBuf,
    pub source: MxSource,
    /// Resolved morph-module imports (`./x.mx`/`./x.ts`/`./x.tsx`):
    /// (raw path as written, canonical target).
    pub module_imports: Vec<(String, PathBuf)>,
}

/// Transitively-closed module graph rooted at the entry file, in
/// breadth-first discovery order (entry first).
#[derive(Debug)]
pub struct ModuleGraph {
    pub entry: PathBuf,
    /// Base directory C++ namespaces are computed relative to.
    ///
    /// Defaults to the entry file's parent directory (`src/` for
    /// `src/App.mx`). Route graphs rebase this to the source root so a
    /// route like `src/add/route.mx` can import shared siblings such as
    /// `src/components/NavBar.mx` via `../components/…` — without the
    /// rebase every shared import looks "outside the entry tree".
    pub ns_base: PathBuf,
    pub modules: HashMap<PathBuf, ResolvedModule>,
    pub order: Vec<PathBuf>,
}

/// Show a path relative to the source root (`src/…`) instead of an
/// absolute path. Falls back to the `src/…` suffix when the module is
/// outside the base, and to the file name when there is no `src`.
fn short_path(base: &Path, module: &Path) -> String {
    if let Ok(rel) = module.strip_prefix(base) {
        let root = base.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        if root.is_empty() {
            return rel.display().to_string();
        }
        return format!("{root}/{}", rel.display());
    }
    let comps: Vec<String> =
        module.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    if let Some(i) = comps.iter().rposition(|c| c == "src") {
        return comps[i..].join("/");
    }
    module
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| module.display().to_string())
}

/// Short display name for the namespace base itself (`src`, not `/abs/…/src`).
fn short_base(base: &Path) -> String {
    base.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| base.display().to_string())
}

/// C++ namespace segments for a module, relative to a base directory
/// (`src/components/shop/ShopStore.mx` with base `src/` →
/// `["components", "shop", "shopstore"]`).
///
/// Hard `mx-naming` gates: every segment is lowercased and must match
/// `[a-z0-9_]`. Pinpoint errors name the offending `src/…` path and its
/// fix, plus the docs URL.
pub fn module_ns_segments_from_base(base: &Path, module: &Path) -> Result<Vec<String>, String> {
    let rel = module.strip_prefix(base).map_err(|_| {
        format!(
            "mx-naming: `{}` is outside the source tree `{}` — move it under `{}/`. Learn more: https://morph.levizr.com/docs/errors/mx-naming",
            short_path(base, module),
            short_base(base),
            short_base(base)
        )
    })?;
    let mut segs: Vec<String> = Vec::new();
    for comp in rel.components().take(rel.components().count().saturating_sub(1)) {
        let raw = comp.as_os_str().to_string_lossy().to_string();
        let s = raw.to_lowercase();
        if s.is_empty()
            || !s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
            let fixed: String = s
                .chars()
                .map(|c| {
                    if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' {
                        c
                    } else if c == '-' || c == ' ' || c == '.' {
                        '_'
                    } else {
                        '_'
                    }
                })
                .collect();
            return Err(format!(
                "mx-naming: directory `{}` in `{}` should be `{}` (lowercase `[a-z0-9_]` only). Rename it. Learn more: https://morph.levizr.com/docs/errors/mx-naming",
                raw,
                short_path(base, module),
                fixed
            ));
        }
        segs.push(if s.starts_with(|c: char| c.is_ascii_digit()) { format!("_{s}") } else { s });
    }
    let raw_stem = module
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "m".to_string());
    let stem = raw_stem.to_lowercase();
    // Strip a leftover compound suffix (`foo.test` from `foo.test.mx` can
    // never be a clean segment).
    if stem.is_empty()
        || !stem.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        let fixed: String = stem
            .chars()
            .map(|c| {
                if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' {
                    c
                } else if c == '-' || c == ' ' || c == '.' {
                    '_'
                } else {
                    '_'
                }
            })
            .collect();
        return Err(format!(
            "mx-naming: file `{}` in `{}` should be `{}{}` (lowercase `[a-z0-9_]` only). Rename it. Learn more: https://morph.levizr.com/docs/errors/mx-naming",
            raw_stem,
            short_path(base, module),
            fixed,
            module.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default()
        ));
    }
    segs.push(if stem.starts_with(|c: char| c.is_ascii_digit()) {
        format!("_{stem}")
    } else {
        stem
    });
    Ok(segs)
}

/// C++ namespace segments for a module, relative to the entry file's
/// directory. Kept for single-entry builds; route builds use
/// [`module_ns_segments_from_base`] with the source root so shared
/// `src/components/…` imports are inside the tree.
pub fn module_ns_segments(entry: &Path, module: &Path) -> Result<Vec<String>, String> {
    let base = entry.parent().unwrap_or_else(|| Path::new("."));
    module_ns_segments_from_base(base, module)
}

/// C++ namespace path (`a::b::c`) for a module, or an `mx-naming` error.
pub fn module_ns_path(entry: &Path, module: &Path) -> Result<String, String> {
    module_ns_segments(entry, module).map(|s| s.join("::"))
}

/// C++ namespace path relative to an explicit source root, or an
/// `mx-naming` error. Route builds use this with `src/` as the base.
pub fn module_ns_path_from_base(base: &Path, module: &Path) -> Result<String, String> {
    module_ns_segments_from_base(base, module).map(|s| s.join("::"))
}

impl ModuleGraph {
    /// Base directory namespaces resolve against. Defaults to the entry
    /// file's parent; route graphs rebase it to `src/`.
    pub fn ns_base(&self) -> &Path {
        &self.ns_base
    }

    /// Rebase namespaces to the source root (route builds call this with
    /// `src/` so shared `src/components/…` imports stay in-tree).
    pub fn set_ns_base(&mut self, base: PathBuf) {
        self.ns_base = base;
    }

    pub fn entry_module(&self) -> Option<&ResolvedModule> {
        self.modules.get(&self.entry)
    }

    pub fn get(&self, path: &Path) -> Option<&ResolvedModule> {
        self.modules.get(path)
    }

    pub fn all_paths(&self) -> impl Iterator<Item = &PathBuf> {
        self.order.iter()
    }

    pub fn len(&self) -> usize {
        self.modules.len()
    }

    pub fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }
}

/// Resolve an import path against the importing file's directory, then the
/// project root.
pub fn resolve_import_path(from_dir: &Path, cwd: &Path, import_path: &str) -> Option<PathBuf> {
    for base in [from_dir, cwd] {
        let candidate = base.join(import_path);
        if candidate.is_file() {
            if let Ok(canon) = candidate.canonicalize() {
                return Some(canon);
            }
            return Some(candidate);
        }
    }
    None
}

/// Parse the entry file and every transitively imported `.mx`/`.ts`/`.tsx`
/// module.
///
/// Missing imports are hard errors; import cycles terminate without
/// re-parsing (render-time cycles are rejected by the IR builder).
pub fn resolve_graph(entry: &Path, cwd: &Path) -> anyhow::Result<ModuleGraph> {
    let entry_canon = entry
        .canonicalize()
        .map_err(|e| anyhow::anyhow!("cannot resolve entry {}: {}", entry.display(), e))?;
    let ns_base = entry_canon.parent().map_or_else(|| cwd.to_path_buf(), Path::to_path_buf);
    let mut graph = ModuleGraph {
        entry: entry_canon.clone(),
        ns_base,
        modules: HashMap::new(),
        order: Vec::new(),
    };
    let mut queue = VecDeque::from([entry_canon]);
    while let Some(path) = queue.pop_front() {
        if graph.modules.contains_key(&path) {
            continue;
        }
        let source = crate::parse_mx_file(&path)
            .map_err(|e| anyhow::anyhow!("parsing {}: {}", path.display(), e))?;
        let dir = path.parent().map_or_else(|| cwd.to_path_buf(), Path::to_path_buf);
        let mut module_imports = Vec::new();
        for imp in &source.imports {
            if !imp.kind.is_module_source() {
                continue;
            }
            let Some(import_path) = imp.kind.path() else {
                continue;
            };
            match resolve_import_path(&dir, cwd, import_path) {
                Some(resolved) => {
                    module_imports.push((import_path.to_string(), resolved.clone()));
                    if !graph.modules.contains_key(&resolved) {
                        queue.push_back(resolved);
                    }
                }
                None => {
                    anyhow::bail!(
                        "Component import not found: {} (imported from {})",
                        import_path,
                        path.display()
                    );
                }
            }
        }
        // Re-export targets join the graph too (missing targets are as
        // fatal as missing imports).
        for re in &source.re_exports {
            match resolve_import_path(&dir, cwd, &re.path) {
                Some(resolved) => {
                    module_imports.push((re.path.clone(), resolved.clone()));
                    if !graph.modules.contains_key(&resolved) {
                        queue.push_back(resolved);
                    }
                }
                None => {
                    anyhow::bail!(
                        "Re-export target not found: {} (re-exported from {})",
                        re.path,
                        path.display()
                    );
                }
            }
        }
        graph.order.push(path.clone());
        graph.modules.insert(path.clone(), ResolvedModule { path, dir, source, module_imports });
    }
    Ok(graph)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write as _;

    fn scratch(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("morph_resolve_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src").join("components")).unwrap();
        root
    }

    fn write(root: &Path, rel: &str, content: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        let mut f = String::new();
        write!(f, "{content}").unwrap();
        std::fs::write(p, f).unwrap();
    }

    const ENTRY: &str = r#"
import Hero from './components/Hero.mx'
import { Card } from './components/ui.mx'

export default function App() {
  return (
    <body>
      <Hero title="hi" />
      <Card t="x" />
    </body>
  )
}
"#;

    const HERO: &str = r"
import { morphState } from 'morph'

export function Hero(props: { title: string }) {
  const [count, setCount] = morphState(0)
  return <div>{props.title}</div>
}
";

    const UI: &str = r"
import Base from './Base.mx'

export function Card(props: { t: string }) {
  return <div><Base /></div>
}
";

    const BASE: &str = r"
export default function Base() {
  return <span>base</span>
}
";

    #[test]
    fn resolves_transitive_graph_in_order() {
        let root = scratch("basic");
        write(&root, "src/App.mx", ENTRY);
        write(&root, "src/components/Hero.mx", HERO);
        write(&root, "src/components/ui.mx", UI);
        write(&root, "src/components/Base.mx", BASE);
        let graph = resolve_graph(&root.join("src/App.mx"), &root).unwrap();
        assert_eq!(graph.len(), 4);
        // Entry first, then its direct imports, then nested.
        assert!(graph.order[0].ends_with("src/App.mx"));
        let names: Vec<String> = graph
            .order
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, vec!["App.mx", "Hero.mx", "ui.mx", "Base.mx"]);
        let entry = graph.entry_module().unwrap();
        assert_eq!(entry.source.components.len(), 1);
        assert!(entry.source.components[0].is_default);
        let hero = graph.modules.values().find(|m| m.path.ends_with("Hero.mx")).unwrap();
        assert_eq!(hero.source.components[0].props_param, "props");
        assert_eq!(hero.source.components[0].props.len(), 1);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_mx_import_is_hard_error() {
        let root = scratch("missing");
        write(
            &root,
            "src/App.mx",
            "import Nope from './Nope.mx'\nexport default function App() { return <div/> }",
        );
        let err = resolve_graph(&root.join("src/App.mx"), &root).unwrap_err();
        assert!(err.to_string().contains("Nope.mx"), "{err}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_typescript_import_is_hard_error() {
        let root = scratch("missing-ts");
        write(
            &root,
            "src/App.mx",
            "import { count } from './missing.ts'\nexport default function App() { return <div/> }",
        );
        let err = resolve_graph(&root.join("src/App.mx"), &root).unwrap_err();
        assert!(err.to_string().contains("missing.ts"), "{err}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn import_cycles_terminate() {
        let root = scratch("cycle");
        write(
            &root,
            "src/A.mx",
            "import B from './B.mx'\nexport default function A() { return <div><B /></div> }",
        );
        write(
            &root,
            "src/B.mx",
            "import A from './A.mx'\nexport function B() { return <span><A /></span> }",
        );
        let graph = resolve_graph(&root.join("src/A.mx"), &root).unwrap();
        assert_eq!(graph.len(), 2);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn typescript_module_sources_are_followed() {
        let root = scratch("ts");
        write(
            &root,
            "src/App.mx",
            "import { count, setCount } from './stores/cart.ts'\nimport Badge from './widgets/Badge.tsx'\nexport default function App() { return (<div>{count}<Badge /></div>) }",
        );
        write(
            &root,
            "src/stores/cart.ts",
            "export const [count, setCount] = morphShared<number>(0)\n",
        );
        write(
            &root,
            "src/widgets/Badge.tsx",
            "export default function Badge() { return (<span>badge</span>) }\n",
        );
        let graph = resolve_graph(&root.join("src/App.mx"), &root).unwrap();
        assert_eq!(graph.len(), 3);
        let names: Vec<String> = graph
            .order
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, vec!["App.mx", "cart.ts", "Badge.tsx"]);
        let entry = graph.entry_module().unwrap();
        assert_eq!(entry.module_imports.len(), 2);
        let store = graph.modules.values().find(|m| m.path.ends_with("cart.ts")).unwrap();
        assert_eq!(store.source.shared_bindings.len(), 1);
        assert_eq!(store.source.shared_bindings[0].getter, "count");
        let badge = graph.modules.values().find(|m| m.path.ends_with("Badge.tsx")).unwrap();
        assert_eq!(badge.source.components.len(), 1);
        assert!(badge.source.components[0].is_default);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn non_module_imports_are_not_followed() {
        let root = scratch("nonmx");
        write(
            &root,
            "src/App.mx",
            "import { morphState } from 'morph'\nimport './style.css'\nexport default function App() { return <div/> }",
        );
        let graph = resolve_graph(&root.join("src/App.mx"), &root).unwrap();
        assert_eq!(graph.len(), 1);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn route_graph_rebased_to_src_root_allows_shared_components() {
        let root = scratch("rebased");
        write(
            &root,
            "src/add/route.mx",
            "import NavBar from '../components/NavBar.mx'\nexport default function Add() { return (<body><NavBar /></body>) }",
        );
        write(&root, "src/components/NavBar.mx", "export function NavBar() { return (<div/>) }");
        let mut graph = resolve_graph(&root.join("src/add/route.mx"), &root).unwrap();
        // Without the rebase the shared sibling is "outside the tree".
        let err = module_ns_path(&graph.entry, &graph.order[1]).unwrap_err();
        assert!(err.contains("mx-naming"), "{err}");
        assert!(!err.contains(&root.display().to_string()), "{err}");
        // Rebasing to `src/` (what `morph build` does per route) fixes it
        // and keeps namespaces entry-consistent (`components::navbar`).
        graph.set_ns_base(root.join("src").canonicalize().unwrap_or(root.join("src")));
        let shared = graph.order[1].clone();
        assert_eq!(
            module_ns_path_from_base(graph.ns_base(), &shared).unwrap(),
            "components::navbar"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn naming_errors_pinpoint_src_relative_paths() {
        let root = scratch("pinpoint");
        write(&root, "src/Bad-Dir/list.mx", "export function List() { return (<div/>) }");
        let base = root.join("src").canonicalize().unwrap_or(root.join("src"));
        let module = base.join("Bad-Dir/list.mx");
        let err = module_ns_segments_from_base(&base, &module).unwrap_err();
        assert!(err.contains("src/Bad-Dir/list.mx"), "{err}");
        assert!(err.contains("bad_dir"), "{err}");
        assert!(err.contains("mx-naming"), "{err}");
        assert!(err.contains("https://morph.levizr.com/docs/errors/mx-naming"), "{err}");
        assert!(!err.contains(&root.display().to_string()), "{err}");
        let _ = std::fs::remove_dir_all(&root);
    }
}
