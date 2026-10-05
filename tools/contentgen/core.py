# Content generator: Python data with Russian and English text -> regions of the
# TOML content files and of the English catalogs. Every text is given as a
# (ru, en) pair. A region lies between "# >>> contentgen: <name>" and
# "# <<< contentgen: <name>"; the rest of a file is written by hand and kept.
import contextlib
import json
import os
import re

EN = {}  # ru -> en of the current build: one text, one translation
ALL = {}  # ru -> en of every pair ever registered (also while the data modules load)


def T(pair):
    """Register a (ru, en) pair, return the Russian text."""
    if isinstance(pair, str):
        return pair
    ru, en = pair
    if ru in EN and EN[ru] != en:
        raise SystemExit(f"two translations for {ru!r}: {EN[ru]!r} / {en!r}")
    EN[ru] = en
    ALL.setdefault(ru, en)
    return ru


def _texts(v, out):
    """The translatable (Cyrillic) texts of a value."""
    if isinstance(v, str):
        if re.search("[Ѐ-ӿ]", v):
            out.append(v)
    elif isinstance(v, (list, tuple)):
        for x in v:
            _texts(x, out)
    elif isinstance(v, dict):
        for x in v.values():
            _texts(x, out)


def q(s):
    return json.dumps(s, ensure_ascii=False)


def val(v):
    if isinstance(v, bool):
        return "true" if v else "false"
    if isinstance(v, (int, float)):
        if isinstance(v, float) and v.is_integer():
            return str(int(v))
        return str(v)
    if isinstance(v, str):
        return q(v)
    if isinstance(v, (list, tuple)):
        return "[" + ", ".join(val(x) for x in v) + "]"
    if isinstance(v, dict):
        return "{ " + ", ".join(f"{k} = {val(x)}" for k, x in v.items()) + " }"
    raise TypeError(v)


def _mark(name):
    return f"# >>> contentgen: {name}"


def _end(name):
    return f"# <<< contentgen: {name}"


_REGION = re.compile(r"^# >>> contentgen: (\S+)\n.*?^# <<< contentgen: \1$\n?", re.S | re.M)


def strip_regions(text):
    """A file without its generated regions (what was written by hand)."""
    return _REGION.sub("", text)


def splice(path, regions, header):
    """Puts the regions [(name, body)] into a file: replaces them in place, appends
    the missing ones; a new file starts with the header."""
    text = header.rstrip() + "\n"
    if os.path.exists(path):
        with open(path) as f:
            text = f.read()
    for name, body in regions:
        block = _mark(name) + "\n" + (body + "\n" if body else "") + _end(name)
        pat = re.compile(rf"^{re.escape(_mark(name))}\n.*?^{re.escape(_end(name))}$", re.S | re.M)
        if pat.search(text):
            text = pat.sub(lambda m: block, text, count=1)
        else:
            text = text.rstrip("\n") + "\n\n" + block + "\n"
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w") as f:
        f.write(text)


class Part:
    def __init__(self):
        self.lines = []
        self.en = {}  # ru -> en of the texts its tables use


class Out:
    """Generated tables by region (file, name), in the order they were made."""

    def __init__(self):
        self.parts = {}
        self.cur = None
        self.pending = None

    def at(self, file, region):
        """What follows goes into a region of a content file."""
        self.cur = self.parts.setdefault((file, region), Part())
        self._flush()

    @contextlib.contextmanager
    def into(self, file, region):
        """Writes a few tables into another region and comes back."""
        prev, pending = self.cur, self.pending
        self.cur, self.pending = self.parts.setdefault((file, region), Part()), None
        try:
            yield
        finally:
            self.cur, self.pending = prev, pending

    def comment(self, text):
        """A section title for the tables that follow."""
        self.pending = text

    def _flush(self):
        if self.pending is not None and self.cur is not None:
            self.cur.lines += ["", f"# ---- {self.pending} ----", ""]
            self.pending = None

    def table(self, kind, fields, subtables=None):
        self._flush()
        found = []
        _texts([fields, subtables], found)
        for ru in found:
            en = EN.get(ru, ALL.get(ru))
            if en is not None:
                self.cur.en.setdefault(ru, en)
        lines = self.cur.lines
        lines.append(f"[[{kind}]]")
        for k, v in fields.items():
            if v is None or v == [] or v == {}:
                continue
            lines.append(f"{k} = {val(v)}")
        for name, sub in (subtables or {}).items():
            if sub is None:
                continue
            lines.append(f"[{kind}.{name}]")
            for k, v in sub.items():
                if v is None or v == [] or v == {}:
                    continue
                lines.append(f"{k} = {val(v)}")
        lines.append("")

    def text(self, file, region):
        part = self.parts.get((file, region))
        return "\n".join(part.lines) if part else ""

    def write(self, data, cat, existing, must_differ):
        """Writes the regions into data/<file> and their new translations into the
        same regions of cat/<file>. A translation that already exists outside the
        generated regions wins; returns the texts of must_differ that would get
        another meaning there."""
        content, catalogs = {}, {}
        conflicts, written, reused = {}, {}, set()
        for (file, region), part in self.parts.items():
            content.setdefault(file, []).append((region, "\n".join(part.lines).strip("\n")))
            lines = []
            for ru, en in part.en.items():
                have = existing.get(ru, written.get(ru))
                if have is not None:
                    if have != en and ru not in conflicts and ru not in reused:
                        reused.add(ru)
                        print(f"reuse {file}: {ru!r}: {have!r} (not {en!r})")
                        if ru in must_differ:
                            conflicts[ru] = en
                    continue
                written[ru] = en
                lines.append(f"{q(ru)} = {q(en)}")
            catalogs.setdefault(file, []).append((region, "\n".join(lines)))
        for file, regions in content.items():
            splice(f"{data}/{file}", regions, "# Создано tools/contentgen/run.py.")
        for file, regions in catalogs.items():
            splice(f"{cat}/{file}", regions, f"# content/{file}")
        return conflicts


# ---- builders ----

def buff(key, name, ms, stats=None, dot=None, dmg=None, stun=False, silence=False, stealth=False, color=None):
    return {"key": key, "name": T(name), "duration_ms": ms, "stats": stats, "dot_per_sec": dot,
            "dmg_type": dmg, "stun": stun or None, "silence": silence or None, "stealth": stealth or None,
            "color": color}


def ability(out, key, name, kind, desc, color, mana=0, cd=1000, damage=None, dmg=None, scale=None, k=None,
            range_=None, radius=None, count=None, speed=None, glyph=None, equip=None, split=None, leech=None,
            execute=None, backstab=None, summon=None, duration=None, buff_=None, on_hit=None, fx=None):
    out.table("abilities", {
        "key": key, "name": T(name), "kind": kind, "mana": mana, "cooldown_ms": cd,
        "damage": damage, "dmg_type": dmg, "split": split, "scale": scale, "scale_k": k, "leech": leech,
        "execute": execute, "backstab": backstab, "range": range_, "radius": radius, "count": count,
        "speed_ms": speed, "summon": summon, "duration_ms": duration, "equip": equip,
        "glyph": glyph, "color": color, "fx": fx, "desc": T(desc),
    }, {"buff": buff_, "on_hit": on_hit})


def skill(out, key, name, branch, tier, max_rank, level, desc, stats=None, grants=None, requires=None,
          equip=None, deed=None, deed_count=None):
    out.table("skills", {
        "key": key, "name": T(name), "branch": branch, "tier": tier, "max_rank": max_rank, "level": level,
        "requires": requires, "equip": equip, "grants": grants, "deed": deed, "deed_count": deed_count,
        "desc": T(desc), "stats": stats,
    })


def branch(out, key, name, color, desc, cls=None, sub=None, secret=False, hidden=False):
    out.table("branches", {"key": key, "name": T(name), "color": color, "desc": T(desc), "class": cls,
                           "subclass": sub, "secret": secret or None, "hidden": hidden or None})


def subclass(out, key, name, cls, color, desc, secret=False):
    out.table("subclasses", {"key": key, "name": T(name), "class": cls, "color": color, "desc": T(desc),
                             "secret": secret or None})


def monster(out, key, name, glyph, color, hp, damage, armor, dmg, move, attack, behavior, role, model,
            resist=None, ability_=None):
    out.table("monsters", {
        "key": key, "name": T(name), "glyph": glyph, "color": color, "hp": hp, "damage": damage, "armor": armor,
        "dmg_type": dmg, "move_ms": move, "attack_ms": attack, "xp": 0, "sight": 9, "behavior": behavior,
        "role": role, "ability": ability_, "model": model, "resist": resist, "depth": [0, 0], "themes": [],
        "gold": [0, 0], "weight": 0, "group": [1, 1], "ally": True,
    })


def relic(out, key, name, color, desc):
    out.table("items", {"key": key, "name": T(name), "glyph": "*", "color": color, "kind": "quest",
                        "value": 0, "depth": 99, "weight": 0, "desc": T(desc)})


def unique(out, key, name, title, color, model, persona, greeting, about, biomes, quest, offer, done, reward,
           target, champion=None, count=None, sources=None):
    out.table("uniques", {
        "key": key, "name": T(name), "title": T(title), "color": color, "model": model,
        "persona": T(persona), "greeting": T(greeting), "about": [T(a) for a in about], "biomes": biomes,
        "quest": quest, "target": target, "champion": T(champion) if champion else None, "count": count,
        "sources": sources, "offer": T(offer), "done": T(done), "reward": reward,
    })
