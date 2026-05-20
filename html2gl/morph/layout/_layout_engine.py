
class LayoutEngine:
    def __init__(self, html_ast, css_ast, window_width, window_height):
        self.html_ast = html_ast
        self.css_ast = css_ast
        self.window_width = window_width
        self.window_height = window_height

    def compute(self):
        mx = 200
        my = 300

        x = (mx / self.window_width - 0.5) * 2.0;
        y = (0.5 - my / self.window_height) * 2.0;

        print(x, y)