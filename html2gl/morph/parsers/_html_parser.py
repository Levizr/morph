class HTMLParser:
    def __init__(self, tokens):
        self.tokens = tokens

    def parse(self):

        # Initialize the base abstract syntax tree structure
        tree = [{
            "root": {
                "type": "Document",
                "children": []
            }
        }]
        
        # The stack tracks our position in the tree. 
        # It starts with a pointer to the root's children list.
        stack = [tree[0]["root"]["children"]]
        
        # We use an iterator so we can manually advance and peek at upcoming tokens
        token_iter = iter(self.tokens)
        
        for token_type, value in token_iter:
            current_scope = stack[-1]  # The children list of the current active parent tag
            
            if token_type == "TAG_OPEN":
                # Create a new element node
                new_node = {
                    "type": "Element",
                    "name": value,
                    "attributes": {},
                    "children": []
                }
                
                # Look ahead for attributes belonging to this specific tag
                # We peek or pull tokens until we hit something that isn't an attribute
                # Note: For this basic parser, we assume attributes immediately follow TAG_OPEN
                # To avoid dropping tokens, we look ahead carefully.
                
                # Append the node to the current parent's children list
                current_scope.append(new_node)
                
                # Push this new node's children list onto the stack. 
                # Any tokens found next will become children of this tag.
                stack.append(new_node["children"])
                
            elif token_type == "ATTR_NAME":
                # If your lexer provides ATTR_NAME followed by ATTR_VALUE
                # We need to find the node we just added to the tree to append attributes
                # Since stack[-1] is the children of the new node, the node itself is in stack[-2][-1]
                try:
                    attr_name = value
                    # Advance iterator to grab the next token, which should be ATTR_VALUE
                    next_token_type, next_value = next(token_iter)
                    if next_token_type == "ATTR_VALUE":
                        # Go up one level in the stack to find the element dictionary
                        parent_children_list = stack[-2]
                        current_tag_node = parent_children_list[-1]
                        current_tag_node["attributes"][attr_name] = next_value
                except StopIteration:
                    break
                    
            elif token_type == "TEXT":
                # Text nodes are leaf nodes and do not create a new nesting scope
                text_node = {
                    "type": "Text",
                    "value": value
                }
                current_scope.append(text_node)
                
            elif token_type == "COMMENT":
                comment_node = {
                    "type": "Comment",
                    "value": value
                }
                current_scope.append(comment_node)
                
            elif token_type == "TAG_CLOSE":
                # When a tag closes, we exit its scope by popping it off the stack
                # Safety check: Don't pop the root document scope
                if len(stack) > 1:
                    stack.pop()
                    
        return tree
