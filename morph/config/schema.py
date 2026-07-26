from dataclasses import dataclass, field


@dataclass
class WindowConfig:
    width:  int = 800
    height: int = 600
    title:  str = "Morph App"


@dataclass
class MorphConfig:
    name:         str          = "my-app"
    entry:        str          = "src/App.mx"      # .html → .mx
    output:       str          = ".morph/"
    window:       WindowConfig = field(default_factory=WindowConfig)
    dependencies: dict         = field(default_factory=dict)
    cpp_sources:  list         = field(default_factory=list)
    node_bridge:  bool         = False

    @staticmethod
    def from_dict(d: dict) -> "MorphConfig":
        win = d.get("window", {})
        return MorphConfig(
            name=d.get("name", "my-app"),
            entry=d.get("entry", "src/App.mx"),
            output=d.get("output", ".morph/"),
            window=WindowConfig(
                width=win.get("width", 800),
                height=win.get("height", 600),
                title=win.get("title", "Morph App"),
            ),
            dependencies=d.get("dependencies", {}),
            cpp_sources=d.get("cpp_sources", []),
            node_bridge=d.get("node_bridge", False),
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
            "dependencies": self.dependencies,
            "cpp_sources":  self.cpp_sources,
        }
