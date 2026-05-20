from morph.ir.node import IRNode


def emit_node(node: IRNode, indent: int = 0) -> str:
    """Returns C++ source lines for a single IR node."""
    pad = "    " * indent
    lines = []
    # TODO: generate per-node C++ instantiation
    lines.append(f"{pad}// {node.node_type} node_{node.node_id}")
    return "\n".join(lines)
