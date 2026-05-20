from morph.lexer.tokens import Token, TokenType
from bs4 import BeautifulSoup, Tag, NavigableString, Comment

class HTMLLexer:

    def tokenize(self, source: str) -> list[Token]:
        """
        Transforms HTML source into a sequential stream of lexical tokens 
        by walking the abstract components processed by BeautifulSoup.
        """
        tokens: list[Token] = []
        
        # Split the raw file source into a line cache to safely map exact positions
        source_lines = source.splitlines(keepends=True)
        soup = BeautifulSoup(source, 'html.parser')

        def find_precise_coords(parent, target_str: str) -> tuple[int, int]:
            """Iteratively finds the exact row/col of a string snippet near its parent."""
            if not parent or not hasattr(parent, 'sourceline') or parent.sourceline is None:
                return 1, 0
                
            start_line = parent.sourceline
            
            # Extract a safe sliding slice from the raw file starting from the parent's line.
            # We slice forward to pull enough code block context without hitting recursion.
            search_chunk = "".join(source_lines[start_line - 1 : start_line + 100])
            
            offset = search_chunk.find(target_str)
            if offset == -1:
                # Absolute fallback to parent tag coordinates if match fails
                return parent.sourceline, parent.sourcepos
                
            # Isolate the leading chunk to count multi-line breaks accurately
            leading = search_chunk[:offset]
            newlines = leading.count('\n')
            
            final_row = start_line + newlines
            if newlines > 0:
                final_col = len(leading) - leading.rfind('\n') - 1
            else:
                final_col = parent.sourcepos + offset
                
            return final_row, final_col
        
        def tokenize_element(element: Comment | NavigableString | Tag):
             # 1. Handle HTML Comments Safely
            if isinstance(element, Comment):
                comment_raw = f"<!--{element}-->"
                row, col = find_precise_coords(element.parent, comment_raw)
                tokens.append(Token(TokenType.COMMENT, str(element), row, col))
                
            # 2. Capture structural text blocks (Filtering out empty indentation whitespace)
            elif isinstance(element, NavigableString):
                text_content = str(element)
                if text_content.strip():
                    row, col = find_precise_coords(element.parent, text_content)
                    tokens.append(Token(TokenType.TEXT, text_content.strip(), row, col))

            # 3. Tokenize the Opening Tag configuration 
            elif isinstance(element, Tag):
                tokens.append(Token(TokenType.TAG_OPEN, element, element.sourceline, element.sourcepos))
 
                # Extract key-value structural attributes
                for attr_name, attr_value in element.attrs.items():
                    val_str = " ".join(attr_value) if isinstance(attr_value, list) else attr_value
                    tokens.append(Token(TokenType.ATTRIBUTE, f"{attr_name}='{val_str}'"))
                
                # Step inside to tokenize child tags or text strings
                for child in element.children:
                    tokenize_element(child)
                
                # Tokenize the closing boundary (ignore-closing empty tags)
                if not element.is_empty_element:
                    tokens.append(Token(TokenType.TAG_CLOSE, element, element.sourceline, element.sourcepos))

        # Main linear loop: Single pass scanning over all abstract elements
        for child in soup.children:
            tokenize_element(child)
           
            


        # Append End-Of-File designator marking the termination of tokenization
        last_line = len(source_lines)
        last_col = len(source_lines[-1]) if source_lines else 0
        tokens.append(Token(TokenType.EOF, "", last_line, last_col))
        print(tokens)
        return tokens
