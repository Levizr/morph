import tinycss2

class CSSLexer:
    def __init__(self, css):
        self.css_content = css

    def tokenize(self):
        tokens = []

        rules = tinycss2.parse_stylesheet(
            self.css_content,
            skip_whitespace=True,
            skip_comments=True
        )

        for rule in rules:
            tokens.extend(self.handle_rule(rule))

        return tokens

    def handle_rule(self, rule):
        tokens = []

        if rule.type == "qualified-rule":
            selector = tinycss2.serialize(rule.prelude).strip()

            tokens.append(("SELECTOR", selector))
            tokens.append(("BRACE_OPEN", "{"))

            declarations = tinycss2.parse_declaration_list(
                rule.content,
                skip_whitespace=True,
                skip_comments=True
            )

            for decl in declarations:
                if decl.type != "declaration":
                    continue

                name = decl.name
                value = tinycss2.serialize(decl.value).strip()

                tokens.append(("PROPERTY", name))
                tokens.append(("COLON", ":"))
                tokens.append(("VALUE", value))

                if decl.important:
                    tokens.append(("IMPORTANT", "!important"))

                tokens.append(("SEMICOLON", ";"))

            tokens.append(("BRACE_CLOSE", "}"))

        elif rule.type == "at-rule":
            at_name = "@" + rule.at_keyword
            prelude = tinycss2.serialize(rule.prelude).strip()

            tokens.append(("AT_RULE", at_name))
            if prelude:
                tokens.append(("AT_VALUE", prelude))

            if rule.content:
                tokens.append(("BRACE_OPEN", "{"))

                nested_rules = tinycss2.parse_rule_list(
                    rule.content,
                    skip_whitespace=True,
                    skip_comments=True
                )

                for nested in nested_rules:
                    tokens.extend(self.handle_rule(nested))

                tokens.append(("BRACE_CLOSE", "}"))
            else:
                tokens.append(("SEMICOLON", ";"))

        return tokens