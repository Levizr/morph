import time
from watchdog.observers import Observer
from watchdog.events import FileSystemEventHandler


class SourceWatcher:
    def __init__(self, path: str, on_change):
        self._path = path
        self._on_change = on_change
        self._last = 0.0

    def start(self) -> None:
        handler = _Handler(self._on_change, debounce=0.3)
        self._observer = Observer()
        self._observer.schedule(handler, self._path, recursive=True)
        self._observer.start()

    def stop(self) -> None:
        self._observer.stop()
        self._observer.join()


class _Handler(FileSystemEventHandler):
    EXTS = {".html", ".css", ".js"}

    def __init__(self, callback, debounce: float):
        self._cb = callback
        self._debounce = debounce
        self._last = 0.0

    def on_modified(self, event):
        if event.is_directory:
            return
        import os
        if os.path.splitext(event.src_path)[1] not in self.EXTS:
            return
        now = time.time()
        if now - self._last < self._debounce:
            return
        self._last = now
        self._cb(event.src_path)
