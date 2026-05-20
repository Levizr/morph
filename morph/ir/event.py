from dataclasses import dataclass


@dataclass
class IREvent:
    trigger: str        # "click", "hover", "change"
    action: str         # "open", "close", "navigate", "call"
    target: str = ""    # window id, page id, or function name
    args: list = None

    def __post_init__(self):
        if self.args is None:
            self.args = []
