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
    pub modules: HashMap<PathBuf, ResolvedModule>,
    pub order: Vec<PathBuf>,
}

/// C++ namespace segments for a module, relative to the entry file's
/// directory (`src/components/shop/ShopStore.mx` →
/// `["components", "shop", "shopstore"]`).
///
/// Hard `mx-naming` gates: every segment is lowercased and must match
/// `[a-z0-9_]` (CamelCase complies automatically; spaces/dashes/dots do
/// not — rename the file). A leading digit is `_`-prefixed for C++
/// validity. Modules outside the entry directory tree (`..`) are
/// rejected. Two modules normalizing to the same segments are a
/// collision (checked by the linter/builder, not here).
pub fn module_ns_segments(entry: &Path, module: &Path) -> Result<Vec<String>, String> {
    let entry_dir = entry.parent().unwrap_or_else(|| Path::new("."));
    let rel = module.strip_prefix(entry_dir).map_err(|_| {
        format!(
            "module {} is outside the entry directory tree {} (mx-naming)",
            module.display(),
            entry_dir.display()
        )
    })?;
    let mut segs: Vec<String> = Vec::new();
    for comp in rel.components().take(rel.components().count().saturating_sub(1)) {
        let s = comp.as_os_str().to_string_lossy().to_lowercase();
        if s.is_empty()
            || !s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
            return Err(format!(
                "directory `{s}` in {} violates mx-naming (lowercase `[a-z0-9_]` only): rename it",
                module.display()
            ));
        }
        segs.push(if s.starts_with(|c: char| c.is_ascii_digit()) { format!("_{s}") } else { s });
    }
    let stem = module
        .file_stem()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_else(|| "m".to_string());
    // Strip a leftover compound suffix (`foo.test` from `foo.test.mx` can
    // never be a clean segment).
    if stem.is_empty()
        || !stem.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return Err(format!(
            "file stem `{stem}` in {} violates mx-naming (lowercase `[a-z0-9_]` only): rename it",
            module.display()
        ));
    }
    segs.push(if stem.starts_with(|c: char| c.is_ascii_digit()) {
        format!("_{stem}")
    } else {
        stem
    });
    Ok(segs)
}

/// C++ namespace path (`a::b::c`) for a module, or an `mx-naming` error.
pub fn module_ns_path(entry: &Path, module: &Path) -> Result<String, String> {
    module_ns_segments(entry, module).map(|s| s.join("::"))
}

impl ModuleGraph {
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
    let mut graph =
        ModuleGraph { entry: entry_canon.clone(), modules: HashMap::new(), order: Vec::new() };
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
}
