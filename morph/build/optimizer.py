import os


def report_size(path: str) -> str:
    """Return human-readable file size."""
    if not os.path.exists(path):
        return "N/A"
    size = os.path.getsize(path)
    if size < 1024:
        return f"{size} B"
    if size < 1024**2:
        return f"{size/1024:.1f} KB"
    return f"{size/1024**2:.1f} MB"
