from bs4 import BeautifulSoup, Tag, NavigableString, Comment

class HTMLLexer:
    def __init__(self, html):
        self.html_content = html

    def tokenize(self):
        """
        Transforms HTML source into a sequential stream of lexical tokens 
        by walking the abstract components processed by BeautifulSoup.
        """
        # Parse the content using the default engine
        soup = BeautifulSoup(self.html_content, 'html.parser')
        tokens = []
        
        # Recursively traverse all components down the tree
        def tokenize_element(element):
            if isinstance(element, Comment):
                tokens.append(('COMMENT', f"<!--{element}-->"))
                
            elif isinstance(element, NavigableString):
                # Capture structural text while stripping out empty formatting whitespace
                text = str(element).strip()
                if text:
                    tokens.append(('TEXT', text))
                    
            elif isinstance(element, Tag):
                # 1. Tokenize the Opening Tag configuration
                tokens.append(('TAG_OPEN', element.name))
                
                # 2. Extract key-value structural attributes
                for attr_name, attr_value in element.attrs.items():
                    # Handle cases where attributes have list formats (like classes)
                    val_str = " ".join(attr_value) if isinstance(attr_value, list) else attr_value
                    tokens.append(('ATTR_NAME', attr_name))
                    tokens.append(('ATTR_VALUE', val_str))
                    
                # 3. Step inside to tokenize nested child tags or text strings
                for child in element.children:
                    tokenize_element(child)
                    
                # 4. Tokenize the closing boundary (ignore self-closing empty tags)
                if not element.is_empty_element:
                    tokens.append(('TAG_CLOSE', element.name))

        # Begin scanning the entire document structural wrapper
        for child in soup.children:
            tokenize_element(child)
            
        return tokens
