# Copyright (c) 2026 Levizr Technologies (levizr.com)!

# -----------

from morph.lexer import HTMLLexer, CSSLexer
from morph.parsers import HTMLParser, CSSParser
from morph.dom.query import find_elements
from morph.layout import LayoutEngine

if __name__ == "__main__":
    sample_html = None

    with open("/home/piyush/My_Projects/ProjectMorph/html2gl/test.html" , "r") as f:
        sample_html = f.read()
    
    # HTML
    tokens = HTMLLexer(sample_html).tokenize()
    html_ast = HTMLParser(tokens).parse()

    css = find_elements(html_ast[0]["root"], "style")[0]["children"][0]["value"]
    print("\n--- HTML AST ---")
    import json
    print(json.dumps(html_ast, indent=2))
    
    css_tokens = CSSLexer(css).tokenize()

    print(css_tokens)
    print("---- CSS AST ----")
    print(json.dumps(CSSParser(css_tokens).parse(), indent=2))

    