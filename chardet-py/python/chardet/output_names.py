"""Public-API encoding-name remapping (pure-Python shim).

The Rust extension does not perform these transforms; this module mirrors
`chardet.output_names` for callers that manipulate result dicts directly.
"""

from __future__ import annotations

import codecs

#: Preferred superset name for each encoding (`prefer_superset` option).
PREFERRED_SUPERSET: dict[str, str] = {
    "ascii": "cp1252",
    "euc_kr": "cp949",
    "iso8859-1": "cp1252",
    "iso8859-2": "cp1250",
    "iso8859-5": "cp1251",
    "iso8859-6": "cp1256",
    "iso8859-7": "cp1253",
    "iso8859-8": "cp1255",
    "iso8859-9": "cp1254",
    "iso8859-11": "cp874",
    "iso8859-13": "cp1257",
    "tis-620": "cp874",
}

#: Python codec name -> chardet 5.x/6.x compatible display name.
_COMPAT_NAMES: dict[str, str] = {
    "big5hkscs": "Big5",
    "cp855": "IBM855",
    "cp866": "IBM866",
    "cp874": "CP874",
    "cp932": "CP932",
    "cp949": "CP949",
    "euc_jis_2004": "EUC-JP",
    "euc_kr": "EUC-KR",
    "gb18030": "GB18030",
    "hz": "HZ-GB-2312",
    "iso2022_jp_2": "ISO-2022-JP",
    "iso2022_kr": "ISO-2022-KR",
    "iso8859-1": "ISO-8859-1",
    "iso8859-2": "ISO-8859-2",
    "iso8859-5": "ISO-8859-5",
    "iso8859-6": "ISO-8859-6",
    "iso8859-7": "ISO-8859-7",
    "iso8859-8": "ISO-8859-8",
    "iso8859-9": "ISO-8859-9",
    "iso8859-13": "ISO-8859-13",
    "johab": "Johab",
    "koi8-r": "KOI8-R",
    "mac-cyrillic": "MacCyrillic",
    "mac-roman": "MacRoman",
    "shift_jis_2004": "SHIFT_JIS",
    "tis-620": "TIS-620",
    "utf-16": "UTF-16",
    "utf-32": "UTF-32",
    "utf-8-sig": "UTF-8-SIG",
    "cp1250": "Windows-1250",
    "cp1251": "Windows-1251",
    "cp1252": "Windows-1252",
    "cp1253": "Windows-1253",
    "cp1254": "Windows-1254",
    "cp1255": "Windows-1255",
    "cp1256": "Windows-1256",
    "cp1257": "Windows-1257",
    "kz1048": "KZ1048",
    "mac-greek": "MacGreek",
    "mac-iceland": "MacIceland",
    "mac-latin2": "MacLatin2",
    "mac-turkish": "MacTurkish",
}


def _decodes_without_error(data: bytes, encoding: str) -> bool:
    try:
        decoder = codecs.getincrementaldecoder(encoding)()
        decoder.decode(data, final=False)
    except (LookupError, UnicodeError, ValueError):
        return False
    return True


def _remap_encoding(result: dict, mapping: dict[str, str]) -> dict:
    enc = result.get("encoding")
    if isinstance(enc, str):
        result["encoding"] = mapping.get(enc, enc)
    return result


def apply_preferred_superset(result: dict, data: bytes | None = None) -> dict:
    """Replace the encoding name with its preferred Windows/CP superset."""
    enc = result.get("encoding")
    if not isinstance(enc, str):
        return result
    superset = PREFERRED_SUPERSET.get(enc)
    if superset is None:
        return result
    if data is None or _decodes_without_error(data, superset):
        result["encoding"] = superset
    return result


#: Deprecated alias kept for external consumers.
apply_legacy_rename = apply_preferred_superset


def apply_compat_names(result: dict) -> dict:
    """Convert internal codec names to chardet 5.x/6.x compatible names."""
    return _remap_encoding(result, _COMPAT_NAMES)
