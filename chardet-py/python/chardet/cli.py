"""Command-line interface for chardet (Python shim over the Rust core)."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

import chardet
from chardet.enums import EncodingEra

DEFAULT_MAX_BYTES = chardet.DEFAULT_MAX_BYTES

_ERA_NAMES = [e.name.lower() for e in EncodingEra if e.bit_count() == 1] + ["all"]

_ISO_TO_LANGUAGE = {
    "ar": "arabic", "be": "belarusian", "bg": "bulgarian", "br": "breton",
    "cs": "czech", "cy": "welsh", "da": "danish", "de": "german", "el": "greek",
    "en": "english", "eo": "esperanto", "es": "spanish", "et": "estonian",
    "fa": "farsi", "fi": "finnish", "fr": "french", "ga": "irish", "gd": "gaelic",
    "he": "hebrew", "hr": "croatian", "hu": "hungarian", "id": "indonesian",
    "is": "icelandic", "it": "italian", "ja": "japanese", "kk": "kazakh",
    "ko": "korean", "lt": "lithuanian", "lv": "latvian", "mk": "macedonian",
    "ms": "malay", "mt": "maltese", "nl": "dutch", "no": "norwegian",
    "pl": "polish", "pt": "portuguese", "ro": "romanian", "ru": "russian",
    "sk": "slovak", "sl": "slovene", "sr": "serbian", "sv": "swedish",
    "tg": "tajik", "th": "thai", "tr": "turkish", "uk": "ukrainian",
    "und": "undetermined", "ur": "urdu", "vi": "vietnamese", "zh": "chinese",
}


def _print_result(result, label, *, minimal, language, mime_type):
    desc = str(result["encoding"])
    if minimal:
        if language:
            desc += f" {result['language'] or 'und'}"
        if mime_type:
            desc += f" {result['mime_type'] or 'application/octet-stream'}"
        print(desc)
    else:
        if language:
            iso = result["language"] or "und"
            name = _ISO_TO_LANGUAGE.get(iso, iso).title()
            desc += f" {iso} ({name})"
        if mime_type:
            desc += f" {result['mime_type'] or 'application/octet-stream'}"
        print(f"{label}: {desc} with confidence {result['confidence']}")


def main(argv: list[str] | None = None) -> None:
    parser = argparse.ArgumentParser(description="Detect character encoding of files.")
    parser.add_argument("files", nargs="*", help="Files to detect encoding of")
    parser.add_argument("--minimal", action="store_true", help="Output only the encoding name")
    parser.add_argument("-l", "--language", action="store_true", help="Include detected language")
    parser.add_argument("-m", "--mime-type", action="store_true", help="Include detected MIME type")
    parser.add_argument("-e", "--encoding-era", default=None, choices=_ERA_NAMES)
    parser.add_argument("-i", "--include-encodings", default=None)
    parser.add_argument("-x", "--exclude-encodings", default=None)
    parser.add_argument("--no-match-encoding", default="cp1252")
    parser.add_argument("--empty-input-encoding", default="utf-8")
    parser.add_argument("--version", action="version", version=f"chardet {chardet.__version__}")
    args = parser.parse_args(argv)

    era = EncodingEra[args.encoding_era.upper()] if args.encoding_era else EncodingEra.ALL
    include = [s.strip() for s in args.include_encodings.split(",")] if args.include_encodings else None
    exclude = [s.strip() for s in args.exclude_encodings.split(",")] if args.exclude_encodings else None

    def run(data, label):
        result = chardet.detect(
            data,
            encoding_era=era,
            include_encodings=include,
            exclude_encodings=exclude,
            no_match_encoding=args.no_match_encoding,
            empty_input_encoding=args.empty_input_encoding,
        )
        _print_result(result, label, minimal=args.minimal, language=args.language, mime_type=args.mime_type)

    if args.files:
        errors = 0
        for filepath in args.files:
            try:
                with Path(filepath).open("rb") as f:
                    data = f.read(DEFAULT_MAX_BYTES)
            except OSError as e:
                print(f"chardetect: {filepath}: {e}", file=sys.stderr)
                errors += 1
                continue
            run(data, filepath)
        if errors == len(args.files):
            sys.exit(1)
    else:
        run(sys.stdin.buffer.read(DEFAULT_MAX_BYTES), "stdin")


if __name__ == "__main__":  # pragma: no cover
    main()
