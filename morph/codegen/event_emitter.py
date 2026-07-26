from morph.ir.event import IREvent


def emit_event(event: IREvent, node_var: str) -> str:
    """Returns the full C++ lambda for node->onClick = <result>."""
    if event.action == "open":
        return f'[&wm]() {{ wm.open("{event.target}"); }}'
    if event.action == "close":
        return f'[&wm]() {{ wm.close("{event.target}"); }}'
    if event.action == "navigate":
        return f'[&wm]() {{ wm.navigate("{event.target}"); }}'
    if event.action == "log":
        escaped = event.target.replace('"', '\\"')
        return f'[&wm]() {{ fprintf(stderr, "{escaped}\\n"); }}'
    if event.action == "call":
        # event.target is already a C++ lambda from the JS transpiler
        return event.target
    return f"[&wm]() {{ /* unhandled action: {event.action} */ }}"
