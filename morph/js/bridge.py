from morph.ir.event import IREvent


def intent_to_event(intent: dict) -> IREvent | None:
    """Convert a JS interpreter intent into an IREvent."""
    action = intent.get("action")
    if not action:
        return None
    return IREvent(
        trigger=intent.get("trigger", "click"),
        action=action,
        target=intent.get("target", ""),
        args=intent.get("args", []),
    )
