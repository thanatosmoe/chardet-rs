"""Detection pipeline types (drop-in shim over the Rust extension)."""

from __future__ import annotations

import dataclasses
from typing import TypedDict

#: Confidence for deterministic (non-BOM) detection stages.
DETERMINISTIC_CONFIDENCE: float = 0.95


class DetectionDict(TypedDict):
    """Dictionary representation of a detection result."""

    encoding: str | None
    confidence: float
    language: str | None
    mime_type: str | None


@dataclasses.dataclass(frozen=True, slots=True)
class DetectionResult:
    """A single encoding detection result."""

    encoding: str | None
    confidence: float
    language: str | None
    mime_type: str | None = None

    def to_dict(self) -> DetectionDict:
        return {
            "encoding": self.encoding,
            "confidence": self.confidence,
            "language": self.language,
            "mime_type": self.mime_type,
        }


#: Sentinel result for "no detection".
_NONE_RESULT = DetectionResult(encoding=None, confidence=0.0, language=None)
