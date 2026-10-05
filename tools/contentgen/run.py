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

# the translations written by hand (outside the generated regions win)
existing = {}
for f in sorted(glob.glob(f"{CAT}/**/*.toml", recursive=True)):
    with open(f) as fh:
        existing.update(tomllib.loads(core.strip_regions(fh.read())))

out = core.Out()
# five new classes: the roster and skills/<class>.toml
classes_data.build(out)
core.EN.clear()
# secret subclasses: skills/<class>.toml, their teachers in masters.toml
secrets_data.build(out)
core.EN.clear()

# every class with its English name, for the hidden skills
with open(f"{DATA}/classes.toml") as fh:
    roster = core.strip_regions(fh.read()) + "\n" + out.text("classes.toml", "classes_new")
names = {}
for c in tomllib.loads(roster)["classes"]:
    names[c["key"]] = (c["name"], existing.get(c["name"]) or core.ALL.get(c["name"], c["name"]))
# ten hidden skills per class: skills/<class>.toml
hidden_data.build(out, names)

if out.write(DATA, CAT, existing, MUST_DIFFER):
    sys.exit(1)
print("ok")
