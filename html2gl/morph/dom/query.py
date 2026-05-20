def find_elements(node, tag_name, result=None):
    if result is None:
        result = []

    if not node:
        return result

    # Check current node
    if node.get("type") == "Element" and node.get("name") == tag_name:
        result.append(node)

    # Traverse children
    for child in node.get("children", []):
        find_elements(child, tag_name, result)

    return result
