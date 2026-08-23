from dataclasses import dataclass, field


@dataclass
class WindowConfig:
    width:  int = 800
    height: int = 600
    title:  str = "Morph App"


@dataclass
class BuildConfig:
    # Static-build knobs. Defaults are tuned for the smallest binary / least
    # RAM. Every extra backend or bundled system lib adds size.
    wayland:         bool = False  # GLFW Wayland backend (adds ~150 KB + deps)
    system_freetype: bool = False  # use system libfreetype.a instead of the
                                   # trimmed self-built copy (saves build time,
                                   # but pulls zlib/png/brotli closures)
    upx:             bool   = True   # compress the final binary with UPX
                                     # (installs upx automatically if missing)
    upx_version:     str    = ""     # pin a specific UPX release to download,
                                     # e.g. "4.2.4"; empty = system upx or default

    # Toolchain overrides. Empty string = automatic selection:
    #   cxx     → production binary compiler   (auto: best g++)
    #   dev_cxx → hot-reload logic.so compiler (auto: newest clang++, else g++)
    #   cmake   → CMake binary for morph_devrt (auto: "cmake" on PATH)
    # Env equivalents: MORPH_CXX / MORPH_DEV_CXX / MORPH_CMAKE.
    cxx:      str = ""
    dev_cxx:  str = ""
    cmake:    str = ""


@dataclass
class LintConfig:
    """Per-project lint rule configuration.

    disable:     rule codes to turn off entirely, e.g. ["mx-list-key"].
    severities:  rule code → "error" | "warning" overrides,
                 e.g. {"mx-tag": "warning"}.
    """
    disable:    list[str]       = field(default_factory=list)
    severities: dict[str, str]  = field(default_factory=dict)


@dataclass
class NativeConfig:
    """Build options for user C++ files imported via `import {} from './x.cpp'`.

    Everything here is forwarded to g++ verbatim so native code can use
    external libraries (OpenCV, SQLite, ...).
    """
    include_dirs: list[str] = field(default_factory=list)  # -I paths for headers
    library_dirs: list[str] = field(default_factory=list)  # -L paths for archives
    libraries:   list[str] = field(default_factory=list)   # -l flags (name w/o -l)
    cflags:      list[str] = field(default_factory=list)   # extra compile flags
    ldflags:     list[str] = field(default_factory=list)   # extra link flags


@dataclass
class MorphConfig:
    name:         str          = "my-app"
    entry:        str          = "src/App.mx"      # .html → .mx
    output:       str          = ".morph/"
    window:       WindowConfig = field(default_factory=WindowConfig)
    renderer:     str          = "flash"           # "flash" (default) | "forge"
    dependencies: dict         = field(default_factory=dict)
    cpp_sources:  list         = field(default_factory=list)
    native:       NativeConfig = field(default_factory=NativeConfig)
    node_bridge:  bool         = False
    build:        BuildConfig  = field(default_factory=BuildConfig)
    lint:         LintConfig   = field(default_factory=LintConfig)

    @staticmethod
    def from_dict(d: dict) -> "MorphConfig":
        win = d.get("window", {})
        bl = d.get("build", {})
        nat = d.get("native", {})
        lt = d.get("lint", {})
        return MorphConfig(
            name=d.get("name", "my-app"),
            entry=d.get("entry", "src/App.mx"),
            output=d.get("output", ".morph/"),
            window=WindowConfig(
                width=win.get("width", 800),
                height=win.get("height", 600),
                title=win.get("title", "Morph App"),
            ),
            renderer=d.get("renderer", "flash"),
            dependencies=d.get("dependencies", {}),
            cpp_sources=d.get("cpp_sources", []),
            native=NativeConfig(
                include_dirs=nat.get("include_dirs", []),
                library_dirs=nat.get("library_dirs", []),
                libraries=nat.get("libraries", []),
                cflags=nat.get("cflags", []),
                ldflags=nat.get("ldflags", []),
            ),
            node_bridge=d.get("node_bridge", False),
            build=BuildConfig(
                wayland=bl.get("wayland", False),
                system_freetype=bl.get("system_freetype", False),
                upx=bl.get("upx", True),
                upx_version=bl.get("upx_version", ""),
                cxx=bl.get("cxx", ""),
                dev_cxx=bl.get("dev_cxx", ""),
                cmake=bl.get("cmake", ""),
            ),
            lint=LintConfig(
                disable=lt.get("disable", []),
                severities=lt.get("severities", {}),
            ),
        )

    def to_dict(self) -> dict:
        return {
            "name":         self.name,
            "entry":        self.entry,
            "output":       self.output,
            "window":       {
                "width":  self.window.width,
                "height": self.window.height,
                "title":  self.window.title,
            },
            "renderer":     self.renderer,
            "dependencies": self.dependencies,
            "cpp_sources":  self.cpp_sources,
            "native":       {
                "include_dirs": self.native.include_dirs,
                "library_dirs": self.native.library_dirs,
                "libraries":    self.native.libraries,
                "cflags":       self.native.cflags,
                "ldflags":      self.native.ldflags,
            },
            "build":        {
                "wayland":         self.build.wayland,
                "system_freetype": self.build.system_freetype,
                "upx":             self.build.upx,
                "upx_version":     self.build.upx_version,
                "cxx":             self.build.cxx,
                "dev_cxx":         self.build.dev_cxx,
                "cmake":           self.build.cmake,
            },
            "lint":         {
                "disable":    self.lint.disable,
                "severities": self.lint.severities,
            },
        }
