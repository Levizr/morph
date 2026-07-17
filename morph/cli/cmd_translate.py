from __future__ import annotations

import os
import sys

from tree_sitter import Language, Parser
import tree_sitter_typescript as tsts

from morph.js.ast_builder import TSAstBuilder
from morph.js.codegen import TSToCppTranslator


def run(args):
    src = args.file
    if not os.path.isfile(src):
        print(f"error: file not found: {src}", file=sys.stderr)
        sys.exit(1)

    with open(src) as f:
        source = f.read()

    lang = Language(tsts.language_tsx())
    parser = Parser(lang)
    tree = parser.parse(source.encode("utf-8"))

    builder = TSAstBuilder()
    program = builder.build(tree.root_node)

    translator = TSToCppTranslator(indent_level=0)
    cpp = translator.translate(program)

    base, _ = os.path.splitext(src)
    out = base + ".cpp"
    with open(out, "w") as f:
        f.write(cpp)
        f.write("\n")

    print(f"translated: {src} -> {out}")
