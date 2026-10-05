"""Backward-compatibility shim for ``chardet.equivalences`` (deprecated).

The public-API encoding-name remapping now lives in
:mod:`chardet.output_names`; this module re-exports it and emits a
deprecation warning on import, mirroring the reference package.
"""

from __future__ import annotations

import warnings

from chardet.output_names import (
    PREFERRED_SUPERSET,
    apply_compat_names,
    apply_legacy_rename,
    apply_preferred_superset,
)

__all__ = [
    "PREFERRED_SUPERSET",
    "apply_compat_names",
    "apply_legacy_rename",
    "apply_preferred_superset",
]

warnings.warn(
    "chardet.equivalences is deprecated; import from chardet.output_names instead. "
    "Will be removed in chardet 8.0.",
    DeprecationWarning,
    stacklevel=2,
)
