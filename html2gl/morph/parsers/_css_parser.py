class CSSParser:
    def __init__(self, tokens):
        self.tokens = tokens
        self.pos = 0
        self.n = len(tokens)

    def current(self):
        if self.pos < self.n:
            return self.tokens[self.pos]
        return None

    def advance(self):
        self.pos += 1

    def parse(self):
        tree = {
            "root": {
                "type": "StyleSheet",
                "rules": []
            }
        }

        while self.pos < self.n:
            token = self.current()

            if token[0] == "SELECTOR":
                tree["root"]["rules"].append(self.parse_rule())

            elif token[0] == "AT_RULE":
                tree["root"]["rules"].append(self.parse_at_rule())

            else:
                self.advance()

        return tree

    # ---------------------------
    # NORMAL RULE
    # ---------------------------
    def parse_rule(self):
        selector = self.current()[1]
        self.advance()

        self.expect("BRACE_OPEN")

        declarations = {}

        while self.current() and self.current()[0] != "BRACE_CLOSE":
            token = self.current()

            if token[0] == "PROPERTY":
                name = token[1]
                self.advance()

                self.expect("COLON")

                value = ""
                if self.current() and self.current()[0] == "VALUE":
                    value = self.current()[1]
                    self.advance()

                # optional !important
                important = False
                if self.current() and self.current()[0] == "IMPORTANT":
                    important = True
                    self.advance()

                declarations[name] = {
                    "value": value,
                    "important": important
                }

                self.expect("SEMICOLON")

            else:
                self.advance()

        self.expect("BRACE_CLOSE")

        return {
            "type": "Rule",
            "selector": selector,
            "declarations": declarations
        }

    # ---------------------------
    # AT RULE (@media etc.)
    # ---------------------------
    def parse_at_rule(self):
        at_name = self.current()[1]
        self.advance()

        prelude = ""
        if self.current() and self.current()[0] == "AT_VALUE":
            prelude = self.current()[1]
            self.advance()

        rule = {
            "type": "AtRule",
            "name": at_name,
            "prelude": prelude,
            "rules": []
        }

        if self.current() and self.current()[0] == "BRACE_OPEN":
            self.advance()

            while self.current() and self.current()[0] != "BRACE_CLOSE":
                token = self.current()

                if token[0] == "SELECTOR":
                    rule["rules"].append(self.parse_rule())
                elif token[0] == "AT_RULE":
                    rule["rules"].append(self.parse_at_rule())
                else:
                    self.advance()

            self.expect("BRACE_CLOSE")
        else:
            self.expect("SEMICOLON")

        return rule

    # ---------------------------
    # HELPER
    # ---------------------------
    def expect(self, token_type):
        token = self.current()
        if token and token[0] == token_type:
            self.advance()
        else:
            raise SyntaxError(f"Expected {token_type}, got {token}")