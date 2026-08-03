import os
import threading
import time
from watchdog.observers import Observer
from watchdog.events import FileSystemEventHandler


class SourceWatcher:
    def __init__(self, path: str, on_change):
        self._path = path
        self._on_change = on_change
        self._last = 0.0
        self._handler = None

    def start(self) -> None:
        self._handler = _Handler(self._on_change, debounce=0.3)
        self._observer = Observer()
        self._observer.schedule(self._handler, self._path, recursive=True)
        self._observer.start()

    def stop(self) -> None:
        self._observer.stop()
        self._observer.join()
        if self._handler is not None:
            self._handler.stop()


class _Handler(FileSystemEventHandler):
    EXTS = {".mx", ".html", ".css", ".js"}

    def __init__(self, callback, debounce: float):
        self._cb = callback
        self._debounce = debounce
        self._timer: threading.Timer | None = None
        self._lock = threading.Lock()
        self._pending: str | None = None

    def _wanted(self, path: str) -> bool:
        return os.path.splitext(path)[1] in self.EXTS

    def _notify(self, src_path: str) -> None:
        # Trailing-edge debounce: keep resetting the timer while the editor is
        # still writing, then fire once it goes quiet for `debounce` seconds.
        with self._lock:
            self._pending = src_path
            if self._timer is not None:
                self._timer.cancel()
            self._timer = threading.Timer(self._debounce, self._fire)
            self._timer.daemon = True
            self._timer.start()

    def _fire(self) -> None:
        with self._lock:
            self._timer = None
            path = self._pending
            self._pending = None
        if path:
            self._cb(path)

    # Editors that "save" via write-then-rename emit on_moved/on_created (and
    # sometimes on_deleted) instead of on_modified, so we must handle all of
    # them or reloads are silently dropped.
    def on_modified(self, event) -> None:
        if not event.is_directory and self._wanted(event.src_path):
            self._notify(event.src_path)

    def on_created(self, event) -> None:
        if not event.is_directory and self._wanted(event.src_path):
            self._notify(event.src_path)

    def on_moved(self, event) -> None:
        if not event.is_directory and self._wanted(event.dest_path):
            self._notify(event.dest_path)

    def on_deleted(self, event) -> None:
        if not event.is_directory and self._wanted(event.src_path):
            self._notify(event.src_path)

    def stop(self) -> None:
        with self._lock:
            if self._timer is not None:
                self._timer.cancel()
                self._timer = None
