from morph.ir.event import IREvent


def emit_event(event: IREvent, node_var: str) -> str:
    """Returns C++ lambda for an IREvent."""
    if event.action == "open":
        return f'wm.open("{event.target}");'
    if event.action == "close":
        return f'wm.close("{event.target}");'
    if event.action == "navigate":
        return f'wm.navigate("{event.target}");'
    if event.action == "log":
        escaped = event.target.replace('"', '\\"')
        return f'fprintf(stderr, "{escaped}\\n");'
    return f"// unhandled action: {event.action}"
