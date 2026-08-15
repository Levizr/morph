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


@dataclass
class MorphConfig:
    name:         str          = "my-app"
    entry:        str          = "src/App.mx"      # .html → .mx
    output:       str          = ".morph/"
    window:       WindowConfig = field(default_factory=WindowConfig)
    renderer:     str          = "flash"           # "flash" (default) | "forge"
    dependencies: dict         = field(default_factory=dict)
    cpp_sources:  list         = field(default_factory=list)
    node_bridge:  bool         = False
    build:        BuildConfig  = field(default_factory=BuildConfig)

    @staticmethod
    def from_dict(d: dict) -> "MorphConfig":
        win = d.get("window", {})
        bl = d.get("build", {})
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
            node_bridge=d.get("node_bridge", False),
            build=BuildConfig(
                wayland=bl.get("wayland", False),
                system_freetype=bl.get("system_freetype", False),
                upx=bl.get("upx", True),
                upx_version=bl.get("upx_version", ""),
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
            "build":        {
                "wayland":         self.build.wayland,
                "system_freetype": self.build.system_freetype,
                "upx":             self.build.upx,
                "upx_version":     self.build.upx_version,
            },
        }
