#!/usr/bin/env python3
"""Compare the Rust port against Python chardet 7 on a generated corpus."""
from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

CHARDET_SRC = os.environ.get("CHARDET_SRC")
if CHARDET_SRC:
    sys.path.insert(0, CHARDET_SRC)

import chardet  # noqa: E402

OUT = Path("/tmp/opencode/parity_corpus")
RUST = os.environ.get("CHARDET_BIN", "target/debug/chardetect")

SAMPLES: list[tuple[str, str, str]] = [
    ("en", "The quick brown fox jumps over the lazy dog. Pack my box with five dozen liquor jugs. How vexingly quick daft zebras jump!", "ascii"),
    ("en", "The naïve approach doesn't always work in complex software systems, especially when internationalization is involved.", "utf-8"),
    ("en", "The naïve caf\xe9 r\xe9sum\xe9 was na\xefve but the fa\xe7ade still charmed everyone in the crowd.", "cp1252"),
    ("fr", "Le café est une boisson très populaire en France et dans le monde entier. Les Parisiens adorent déguster un bon expresso.", "cp1252"),
    ("fr", "Le café est une boisson très populaire en France et dans le monde entier, mais l'été reste chaud à Paris.", "iso8859-1"),
    ("fr", "Le café est une boisson très populaire en France et dans le monde entier, victime de son succès.", "iso8859-15"),
    ("de", "Die schnelle braune Füchsin springt über den faulen Hund. Zwölf Boxkämpfer jagen Eva quer über den großen Sylter Deich.", "cp1252"),
    ("de", "Über den Wolken muss die Freiheit wohl grenzenlos sein, alle Ängste, alle Sorgen sagt man, blieben darunter verborgen.", "iso8859-1"),
    ("es", "El veloz murciélago hindú comía feliz cardillo y kiwi. La cigüeña toca el saxofón detrás del palenque de paja.", "cp1252"),
    ("it", "Ma la volpe, col suo balzo, ha raggiunto il quieto Fido. Quel vituperabile xenofobo zelante assaggia il whisky ed esclama: alleluja!", "cp1252"),
    ("pt", "Um pequeno jabuti xereta viu dez cegonhas felizes. O próximo voo parte à tarde com muitos passageiros animados.", "cp1252"),
    ("nl", "Op brute wijze ving de schooljuffrouw de achtste vliegen. Zwijgende kabouters verstopten zich achter de dikke eik.", "cp1252"),
    ("sv", "Flygande bäckasiner söka hwila på mjuka tuvor. Yxskaftet var alldeles för långt för den lilla pojken.", "cp1252"),
    ("da", "Høj bly gom vandt fræk sexquiz på wc. Ærligt talt var det en sær præstation af den unge knægt.", "cp1252"),
    ("no", "Vår sære Zulu fra badeøya spilte jo whist på quiz. Blåbærsyltetøy er godt på brødskiver om morgenen.", "cp1252"),
    ("fi", "Törkylempijävongahdus on hyvä esimerkki suomen kielen pitkistä yhdyssanoista ja ääkkösistä.", "cp1252"),
    ("is", "Kæmi ný öxi hér ykist þjófum nú bæði víl og ádrepa. Það er gaman að læra íslensku í Reykjavík.", "iso8859-1"),
    ("pl", "Zażółć gęślą jaźń. Pchnąć w tę łódź jeża lub ośm skrzyń fig. Wszyscy ludzie rodzą się wolni i równi.", "cp1250"),
    ("cs", "Příliš žluťoučký kůň úpěl ďábelské ódy. Nové poznatky o českém jazyce jsou velmi zajímavé.", "cp1250"),
    ("hu", "Árvíztűrő tükörfúrógép. Az öreg halász és a nagyravágyó felesége minden reggel a tóhoz mentek.", "cp1250"),
    ("ro", "Știu că șirul acesta de caractere speciale românești este destul de lung pentru detectare.", "iso8859-16"),
    ("hr", "Čćčđšž su hrvatska slova. Ljubazni fenjerdžija čađavog lica hoće pokazati džep.", "cp1250"),
    ("tr", "Pijamalı hasta yağız şoföre çabucak güvendi. Türkçe karakterler ğüşiöç sembollerini içerir.", "cp1254"),
    ("el", "Ξεσκεπάζω την ψυχοφθόρα βδελυγμία. Η γλώσσα μου έδωσαν ελληνική και το σπίτι φτωχικό.", "iso8859-7"),
    ("ru", "Съешь же ещё этих мягких французских булок, да выпей чаю. Широкая электрификация улучшит жизнь.", "cp1251"),
    ("ru", "Съешь же ещё этих мягких французских булок, да выпей чаю. Широкая электрификация улучшит жизнь.", "koi8-r"),
    ("ru", "Съешь же ещё этих мягких французских булок, да выпей чаю. Широкая электрификация улучшит жизнь.", "iso8859-5"),
    ("uk", "Чуєш їх, доцю, га? Кумедна ж ти, прощайся без ґольфів! Українська мова багата на слова.", "koi8-u"),
    ("bg", "Жълтата дюля беше щастлива, че пухът, който цъфна, замръзна като гьон. Българският език е славянски.", "cp1251"),
    ("he", "דג סקרן שט בים מאוכזב ולפתע מצא חברה. העברית היא שפה שמית המדוברת בישראל.", "iso8859-8"),
    ("ar", "نص حكيم له سر قاطع وذو شأن عظيم مكتوب على ثوب بلا نجوم. اللغة العربية من أجمل اللغات.", "cp1256"),
    ("fa", "نص حكيم له سر قاطع وذو شأن عظيم مكتوب على ثوب بلا نجوم. زبان فارسی شیرین است.", "cp1256"),
    ("th", "เป็นมนุษย์สุดประเสริฐเลิศคุณค่า กว่าบรรดาฝูงสัตว์เดรัจฉาน. ภาษาไทยเป็นภาษาที่สวยงาม.", "tis-620"),
    ("vi", "Tôi thích ăn phở và bánh mì vào mỗi buổi sáng ở Hà Nội. Tiếng Việt có nhiều dấu thanh.", "cp1258"),
    ("ja", "日本語の文字コード検出テストです。このテキストは日本語で書かれています。", "cp932"),
    ("ja", "日本語の文字コード検出テストです。このテキストは日本語で書かれています。", "euc_jis_2004"),
    ("ja", "日本語の文字コード検出テストです。このテキストは日本語で書かれています。", "iso2022_jp_2"),
    ("ja", "日本語の文字コード検出テストです。このテキストは日本語で書かれています。", "utf-8"),
    ("ko", "한국어의 문자 코드 검출 테스트입니다. 이 텍스트는 한국어로 작성되었습니다.", "cp949"),
    ("ko", "한국어의 문자 코드 검출 테스트입니다. 이 텍스트는 한국어로 작성되었습니다.", "euc_kr"),
    ("zh", "中文字符编码检测测试文本。这段文字是用简体中文编写的，用于测试检测准确性。", "gb18030"),
    ("zh", "中文字符編碼檢測測試文本。這段文字是用繁體中文編寫的，用於測試檢測準確性。", "big5hkscs"),
    ("zh", "中文字符编码检测测试文本。这段文字是用简体中文编写的，用于测试检测准确性。", "utf-8"),
    ("en", "<html><head><meta charset=\"utf-8\"><title>Test</title></head><body>Hello world</body></html>", "ascii"),
    ("en", "<?xml version=\"1.0\" encoding=\"iso-8859-1\"?><root>caf\xe9</root>", "iso8859-1"),
    ("en", "#!/usr/bin/env python3\n# -*- coding: utf-8 -*-\nprint('héllo')\n", "utf-8"),
    ("en", "Hello world in UTF-16 little endian.", "utf-16"),
    ("en", "Hello world in UTF-16 big endian.", "utf-16-be"),
    ("en", "Hello world in UTF-32 little endian.", "utf-32"),
]


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    for f in OUT.iterdir():
        f.unlink()

    paths = []
    expected = {}
    for i, (lang, text, enc) in enumerate(SAMPLES):
        try:
            data = text.encode(enc)
        except Exception as e:  # noqa: BLE001
            print(f"skip {enc}: {e}")
            continue
        p = OUT / f"{i:04d}_{lang}_{enc.replace('/','_')}.bin"
        p.write_bytes(data)
        py = chardet.detect(data)
        expected[p.name] = (py["encoding"], py["confidence"], py["language"])
        paths.append(str(p))

    # Run Rust once with all paths.
    proc = subprocess.run([RUST, "--raw", *paths], capture_output=True, text=True, check=True)
    rust = {}
    for line in proc.stdout.splitlines():
        name, enc, conf, lang, _mime = line.split("\t")
        rust[Path(name).name] = (enc, float(conf), lang)

    total = len(expected)
    enc_ok = 0
    exact_ok = 0
    mismatches = []
    for name, (pe, pc, pl) in expected.items():
        re_, rc, rl = rust.get(name, ("MISSING", -1.0, "MISSING"))
        if pe == re_:
            enc_ok += 1
        if pe == re_ and abs(pc - rc) < 1e-12 and pl == rl:
            exact_ok += 1
        else:
            mismatches.append((name, (pe, pc, pl), (re_, rc, rl)))

    print(f"encoding match: {enc_ok}/{total} ({100*enc_ok/total:.1f}%)")
    print(f"exact match:    {exact_ok}/{total} ({100*exact_ok/total:.1f}%)")
    print("--- mismatches ---")
    for name, exp, got in mismatches:
        print(f"{name}\n   py   = {exp}\n   rust = {got}")
    sys.exit(1 if mismatches else 0)


if __name__ == "__main__":
    main()
