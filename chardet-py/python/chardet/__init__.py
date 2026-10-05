"""Universal character encoding detector — Rust-backed drop-in for chardet 7."""

from __future__ import annotations

from ._chardet import (
    DEFAULT_MAX_BYTES,
    MINIMUM_THRESHOLD,
    UniversalDetector,
    __version__,
    detect,
    detect_all,
)
from .enums import EncodingEra, LanguageFilter
from .pipeline import DetectionDict, DetectionResult

__all__ = [
    "DEFAULT_MAX_BYTES",
    "MINIMUM_THRESHOLD",
    "DetectionDict",
    "DetectionResult",
    "EncodingEra",
    "LanguageFilter",
    "UniversalDetector",
    "__version__",
    "detect",
    "detect_all",
]


def __getattr__(name: str) -> object:
    """Resolve the deprecated ``chardet.equivalences`` submodule lazily."""
    if name == "equivalences":
        import importlib

        return importlib.import_module("chardet.equivalences")
    msg = f"module {__name__!r} has no attribute {name!r}"
    raise AttributeError(msg)
