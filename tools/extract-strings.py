#!/usr/bin/env python3
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SOURCES = ROOT / "src"
TEMPLATE = ROOT / "po" / "wazzup.pot"

CALL = re.compile(r'\bt(?:_with)?\(\s*"((?:[^"\\]|\\.)*)"')
PLURAL = re.compile(r'\bt_count\(\s*"((?:[^"\\]|\\.)*)"\s*,\s*"((?:[^"\\]|\\.)*)"')
HELPER = re.compile(
    r'\b(?:switch_row|choice_row|text_row|group|page|secret_row|entry)\(\s*(?:store,\s*)?"((?:[^"\\]|\\.)*)"(?:\s*,\s*"((?:[^"\\]|\\.)*)")?'
)
TABLE_ROW = re.compile(r'\(\s*[A-Za-z0-9_:]+\s*,\s*"((?:[^"\\]|\\.)*)"\s*\)')
CONSTANT = re.compile(r'const RESTART_NOTE: &str = "((?:[^"\\]|\\.)*)"')
UNTRANSLATED = {"HTTP", "SOCKS5", "English", "Italiano", "Français", "Deutsch", "Español"}


def collect():
    singles, plurals = {}, {}
    for path in sorted(SOURCES.rglob("*.rs")):
        text = path.read_text(encoding="utf-8").split("#[cfg(test)]")[0]
        relative = path.relative_to(ROOT)
        for match in CALL.finditer(text):
            singles.setdefault(match.group(1), relative)
        for match in PLURAL.finditer(text):
            plurals.setdefault((match.group(1), match.group(2)), relative)
        if path.name in {"preferences_dialog.rs", "mod.rs"}:
            for match in HELPER.finditer(text):
                for group in match.groups():
                    if group:
                        singles.setdefault(group, relative)
            for match in TABLE_ROW.finditer(text):
                if path.name == "preferences_dialog.rs" or "i18n" in str(relative):
                    singles.setdefault(match.group(1), relative)
            for match in CONSTANT.finditer(text):
                singles.setdefault(match.group(1), relative)
    for key in [message for message in singles if message.endswith("-symbolic")]:
        singles.pop(key, None)
    for key in UNTRANSLATED:
        singles.pop(key, None)
    for singular, _ in plurals:
        singles.pop(singular, None)
    return singles, plurals


def read_catalog(path):
    known = set()
    current = None
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.startswith("msgid "):
            current = line[len("msgid "):].strip()[1:-1]
        elif line.startswith('"') and current is not None and not line.startswith('"Content'):
            current += line.strip()[1:-1]
        elif line.startswith("msgstr") and current:
            known.add(current)
            current = None
    return known


def main():
    singles, plurals = collect()
    lines = ['msgid ""', 'msgstr ""', '"Content-Type: text/plain; charset=UTF-8\\n"', ""]
    for message, origin in sorted(singles.items()):
        lines += [f"#: {origin}", f'msgid "{message}"', 'msgstr ""', ""]
    for (singular, plural), origin in sorted(plurals.items()):
        lines += [f"#: {origin}", f'msgid "{singular}"', f'msgid_plural "{plural}"', 'msgstr[0] ""', 'msgstr[1] ""', ""]
    TEMPLATE.write_text("\n".join(lines), encoding="utf-8")
    expected = set(singles) | {singular for singular, _ in plurals}
    missing_any = False
    for catalog in sorted(TEMPLATE.parent.glob("*.po")):
        missing = sorted(expected - read_catalog(catalog))
        if missing:
            missing_any = True
            print(f"{catalog.name}: {len(missing)} missing")
            for message in missing:
                print(f"  {message}")
    print(f"{len(expected)} messages written to {TEMPLATE.relative_to(ROOT)}")
    return 1 if missing_any else 0


if __name__ == "__main__":
    sys.exit(main())
