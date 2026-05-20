from dataclasses import dataclass, field


@dataclass
class StyleRule:
    selector: str
    declarations: dict[str, str] = field(default_factory=dict)


class StyleSheet:
    def __init__(self):
        self.rules: list[StyleRule] = []

    def add_rule(self, rule: StyleRule):
        self.rules.append(rule)

    def __repr__(self):
        return f"StyleSheet({len(self.rules)} rules)"
