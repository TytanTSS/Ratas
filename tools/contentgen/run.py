import glob
import os
import sys
import tomllib

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import core
import classes_data
import secrets_data
import hidden_data

# run from anywhere: python3 tools/contentgen/run.py
REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
DATA = f"{REPO}/data/content"
CAT = f"{REPO}/data/i18n/en"

MUST_DIFFER = {"Следопыт", "Сокол", "Святилище"}
GENERATED = {"classes_new.toml", "secret_paths.toml", "hidden_skills.toml"}

# the existing catalog (without the files generated here)
existing = {}
for f in glob.glob(f"{CAT}/*.toml"):
    if os.path.basename(f) in GENERATED:
        continue
    with open(f, "rb") as fh:
        existing.update(tomllib.load(fh))


def emit(name, header, build, *args):
    core.EN.clear()
    out = core.Out(header)
    build(out, *args)
    out.write(f"{DATA}/{name}")
    # the existing translation of a text wins; texts whose meaning differs
    # must get other Russian names (reviewed by hand)
    conflicts = {}
    for ru, en in core.EN.items():
        if ru in existing and existing[ru] != en:
            print(f"reuse {name}: {ru!r}: {existing[ru]!r} (not {en!r})")
            if ru in MUST_DIFFER:
                conflicts[ru] = en
    fresh = {ru: en for ru, en in core.EN.items() if ru not in existing}
    core.EN.clear()
    core.EN.update(fresh)
    core.write_catalog(f"{CAT}/{name}", f"content/data/{name}")
    existing.update(fresh)
    return conflicts


bad = {}
bad.update(emit("classes_new.toml", "# Пять новых классов: Следопыт, Паладин, Монах, Скальд и Зельевар, у каждого по три подкласса.\n"
                "# Файл создан генератором вместе с английским каталогом i18n/en/classes_new.toml.", classes_data.build))
bad.update(emit("secret_paths.toml", "# Секретные подклассы: ещё по одному каждому классу, по два новым классам; каждому учит уникальный персонаж.\n"
                "# Файл создан генератором вместе с английским каталогом i18n/en/secret_paths.toml.", secrets_data.build))

names = {}
for f in [f"{DATA}/classes.toml", f"{DATA}/secrets.toml", f"{DATA}/classes_new.toml"]:
    with open(f, "rb") as fh:
        for c in tomllib.load(fh).get("classes", []):
            names[c["key"]] = (c["name"], existing.get(c["name"], c["name"]))
bad.update(emit("hidden_skills.toml", "# Скрытые навыки: по десять на каждый класс. Навык открывается сам, когда герой этого класса\n"
                "# совершит деяние (deed, deed_count — см. crates/core/src/game/deeds.rs). Десятый навык каждого класса даёт умение.\n"
                "# Файл создан генератором вместе с английским каталогом i18n/en/hidden_skills.toml.", hidden_data.build, names))
if bad:
    sys.exit(1)
print("ok")
