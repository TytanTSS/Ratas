# Content generator: Python data with Russian and English text -> TOML content
# files and the English catalog. Every text is given as a (ru, en) pair.
import json

EN = {}  # ru -> en


def T(pair):
    """Register a (ru, en) pair, return the Russian text."""
    if isinstance(pair, str):
        return pair
    ru, en = pair
    if ru in EN and EN[ru] != en:
        raise SystemExit(f"two translations for {ru!r}: {EN[ru]!r} / {en!r}")
    EN[ru] = en
    return ru


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


class Out:
    def __init__(self, header):
        self.lines = [header.rstrip(), ""]

    def comment(self, text):
        self.lines += ["", f"# ---- {text} ----", ""]

    def table(self, kind, fields, subtables=None):
        self.lines.append(f"[[{kind}]]")
        for k, v in fields.items():
            if v is None or v == [] or v == {}:
                continue
            self.lines.append(f"{k} = {val(v)}")
        for name, sub in (subtables or {}).items():
            if sub is None:
                continue
            self.lines.append(f"[{kind}.{name}]")
            for k, v in sub.items():
                if v is None or v == [] or v == {}:
                    continue
                self.lines.append(f"{k} = {val(v)}")
        self.lines.append("")

    def write(self, path):
        with open(path, "w") as f:
            f.write("\n".join(self.lines).rstrip() + "\n")


def write_catalog(path, header):
    with open(path, "w") as f:
        f.write(f"# {header}\n")
        for ru, en in EN.items():
            f.write(f"{q(ru)} = {q(en)}\n")


# ---- builders ----

def buff(key, name, ms, stats=None, dot=None, dmg=None, stun=False, silence=False, stealth=False, color=None):
    return {"key": key, "name": T(name), "duration_ms": ms, "stats": stats, "dot_per_sec": dot,
            "dmg_type": dmg, "stun": stun or None, "silence": silence or None, "stealth": stealth or None,
            "color": color}


def ability(out, key, name, kind, desc, color, mana=0, cd=1000, damage=None, dmg=None, scale=None, k=None,
            range_=None, radius=None, count=None, speed=None, glyph=None, equip=None, split=None, leech=None,
            execute=None, backstab=None, summon=None, duration=None, buff_=None, on_hit=None):
    out.table("abilities", {
        "key": key, "name": T(name), "kind": kind, "mana": mana, "cooldown_ms": cd,
        "damage": damage, "dmg_type": dmg, "split": split, "scale": scale, "scale_k": k, "leech": leech,
        "execute": execute, "backstab": backstab, "range": range_, "radius": radius, "count": count,
        "speed_ms": speed, "summon": summon, "duration_ms": duration, "equip": equip,
        "glyph": glyph, "color": color, "desc": T(desc),
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
