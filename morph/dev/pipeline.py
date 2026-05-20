from morph.lexer import HTMLLexer, CSSLexer, JSLexer
from morph.parser import HTMLParser, CSSParser, JSParser
from morph.style.resolver import StyleResolver
from morph.ir.builder import IRBuilder
from morph.ir.serializer import IRSerializer
from morph.layout.engine import LayoutEngine
from morph.utils.logger import log_info, log_error


def run(config) -> dict | None:
    """Full source → IR dict. Returns None on failure."""
    try:
        import pathlib
        entry   = pathlib.Path(config.entry)
        css_f   = pathlib.Path("src/style.css")
        js_f    = pathlib.Path("src/app.js")

        html_src = entry.read_text()
        css_src  = css_f.read_text()  if css_f.exists()  else ""
        js_src   = js_f.read_text()   if js_f.exists()   else ""

        html_tok = HTMLLexer().tokenize(html_src)
        css_tok  = CSSLexer().tokenize(css_src)
        js_tok   = JSLexer().tokenize(js_src)

        dom      = HTMLParser().parse(html_tok)
        sheet    = CSSParser().parse(css_tok)
        js_ir    = JSParser().parse(js_tok)

        StyleResolver(sheet).resolve(dom)
        ir       = IRBuilder(config).build(dom, js_ir)
        LayoutEngine().compute(ir)

        return IRSerializer().to_dict(ir)

    except Exception as e:
        log_error(str(e))
        return None
