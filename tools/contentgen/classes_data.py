# Five new starting classes, each with a class branch and three subclasses.
from core import *


def cls(out, key, name, color, desc, attrs, abilities, items, model):
    out.table("classes", {"key": key, "name": T(name), "color": color, "desc": T(desc), "attrs": attrs,
                          "abilities": abilities, "items": items, "model": model})


def build(out):
    # ================= Следопыт =================
    out.comment("Следопыт")
    cls(out, "ranger", ("Следопыт", "Ranger"), "#6ab04a",
        ("Лук, капканы и звери-спутники. Подклассы: Снайпер, Повелитель зверей, Лесной страж.",
         "Bow, snares and animal companions. Subclasses: Sharpshooter, Beastmaster, Forest Warden."),
        {"str": 4, "dex": 8, "int": 4, "vit": 6}, ["rng_aimed_shot"],
        ["hunting_bow", "leather_armor", "leather_cap", "leather_pants", "travel_cloak", "potion_health", "potion_health"],
        "ranger")
    subclass(out, "sharpshooter", ("Снайпер", "Sharpshooter"), "ranger", "#c0d080",
             ("Дальние смертельные выстрелы, пробивающие любую броню.", "Long deadly shots that pierce any armor."))
    subclass(out, "beastmaster", ("Повелитель зверей", "Beastmaster"), "ranger", "#b08a50",
             ("Звери-спутники сражаются рядом: волк, соколы и медведь.", "Animal companions fight at your side: a wolf, falcons and a bear."))
    subclass(out, "warden", ("Лесной страж", "Forest Warden"), "ranger", "#5a9a5a",
             ("Копьё и сила леса: ближний бой, шипы и стойкость.", "Spear and the strength of the forest: melee, thorns and endurance."))
    branch(out, "ranger", ("Следопыт", "Ranger"), "#6ab04a", ("Меткость, ловкость и знание троп.", "Aim, agility and knowledge of the trails."), "ranger")
    branch(out, "sharpshooter", ("Снайпер", "Sharpshooter"), "#c0d080", ("Выстрелы с убийственной точностью.", "Shots of deadly precision."), "ranger", "sharpshooter")
    branch(out, "beastmaster", ("Повелитель зверей", "Beastmaster"), "#b08a50", ("Звери-спутники.", "Animal companions."), "ranger", "beastmaster")
    branch(out, "warden", ("Лесной страж", "Forest Warden"), "#5a9a5a", ("Копьё и шипы леса.", "Spear and the thorns of the forest."), "ranger", "warden")

    skill(out, "rng_keen_eye", ("Меткий глаз", "Keen Eye"), "ranger", 1, 5, 1, ("Урон в дальнем бою и крит. шанс.", "Ranged damage and crit chance."), {"ranged_pct": 5, "crit": 1})
    skill(out, "rng_fieldcraft", ("Бывалый странник", "Fieldcraft"), "ranger", 1, 5, 1, ("Быстрее бег, чаще уклонение.", "Faster running, more dodges."), {"move_speed": 2, "dodge": 1})
    skill(out, "rng_volley", ("Залп", "Volley"), "ranger", 2, 1, 3, ("Веер из пяти стрел.", "A fan of five arrows."), grants="rng_volley", requires=["rng_keen_eye"], equip="bow")
    skill(out, "rng_snare", ("Капкан", "Snare"), "ranger", 2, 1, 4, ("Капканы вокруг вас ранят и держат врагов.", "Snares around you wound and hold enemies."), grants="rng_snare", requires=["rng_fieldcraft"])
    skill(out, "rng_survivor", ("Закалённый", "Hardened"), "ranger", 3, 3, 6, ("Здоровье и защита от яда.", "Health and poison protection."), {"max_hp": 8, "res_poison": 4}, requires=["rng_volley"])

    skill(out, "rng_deadeye", ("Мёртвый глаз", "Deadeye"), "sharpshooter", 1, 5, 5, ("С луком: урон и сила критических ударов.", "With a bow: damage and critical power."), {"ranged_pct": 6, "crit_mult": 0.1}, equip="bow")
    skill(out, "rng_piercing_shot", ("Бронебойный выстрел", "Piercing Shot"), "sharpshooter", 1, 1, 5, ("Стрела пробивает доспех и ослабляет его.", "An arrow that pierces armor and weakens it."), grants="rng_piercing_shot")
    skill(out, "rng_headshot", ("Выстрел в голову", "Headshot"), "sharpshooter", 2, 1, 7, ("Мощный выстрел, оглушающий цель.", "A powerful shot that stuns the target."), grants="rng_headshot", requires=["rng_deadeye"])
    skill(out, "rng_rain_of_arrows", ("Дождь стрел", "Rain of Arrows"), "sharpshooter", 3, 1, 10, ("Стрелы падают с неба на большую площадь.", "Arrows fall from the sky over a large area."), grants="rng_rain_of_arrows", requires=["rng_headshot"])

    skill(out, "rng_pack_bond", ("Узы стаи", "Pack Bond"), "beastmaster", 1, 5, 5, ("Здоровье и урон в ближнем бою.", "Health and melee damage."), {"max_hp": 6, "melee_pct": 2})
    skill(out, "rng_call_wolf", ("Зов волка", "Call of the Wolf"), "beastmaster", 1, 1, 5, ("Верный волк сражается рядом.", "A loyal wolf fights by your side."), grants="rng_call_wolf")
    skill(out, "rng_call_hawk", ("Соколы", "Falcons"), "beastmaster", 2, 1, 7, ("Два сокола клюют врагов с неба.", "Two falcons peck enemies from the sky."), grants="rng_call_hawk", requires=["rng_pack_bond"])
    skill(out, "rng_call_bear", ("Медведь-побратим", "Bear Brother"), "beastmaster", 3, 1, 10, ("Могучий медведь встаёт на вашу сторону.", "A mighty bear takes your side."), grants="rng_call_bear", requires=["rng_call_hawk"])

    skill(out, "rng_warden_might", ("Сила стража", "Warden's Might"), "warden", 1, 5, 5, ("Урон в ближнем бою и броня.", "Melee damage and armor."), {"melee_pct": 4, "armor": 1})
    skill(out, "rng_thorn_spear", ("Терновое копьё", "Thorn Spear"), "warden", 1, 1, 5, ("Два укола с ядом шипов.", "Two thrusts with thorn poison."), grants="rng_thorn_spear", equip="melee")
    skill(out, "rng_barkguard", ("Кора стража", "Barkguard"), "warden", 2, 1, 7, ("Броня, шипы и восстановление на время.", "Armor, thorns and regeneration for a while."), grants="rng_barkguard", requires=["rng_warden_might"])
    skill(out, "rng_forest_wrath", ("Ярость леса", "Forest Wrath"), "warden", 3, 1, 10, ("Удар по всем вокруг: колющий урон и яд.", "A blow at everyone around: piercing and poison damage."), grants="rng_forest_wrath", requires=["rng_barkguard"])

    ability(out, "rng_aimed_shot", ("Прицельный выстрел", "Aimed Shot"), "projectile", ("Выверенный выстрел из лука.", "A well-aimed bow shot."), "#d0e0a0",
            mana=3, cd=1500, damage=[6, 10], dmg="pierce", scale="dex", k=1.0, range_=10, speed=25, glyph="dir", equip="bow")
    ability(out, "rng_volley", ("Залп", "Volley"), "projectile", ("Пять стрел веером.", "Five arrows in a fan."), "#d0e0a0",
            mana=10, cd=6000, damage=[4, 7], dmg="pierce", scale="dex", k=0.6, range_=8, count=5, speed=25, glyph="dir", equip="bow")
    ability(out, "rng_snare", ("Капкан", "Snare"), "nova", ("Капканы в радиусе 2: урон и почти полная остановка на 4 с.", "Snares in radius 2: damage and nearly a full stop for 4 s."), "#a08a60",
            mana=10, cd=10000, damage=[4, 8], dmg="pierce", scale="dex", k=0.6, radius=2, glyph="#",
            on_hit=buff("snared", ("В капкане", "Snared"), 4000, {"move_speed": -75}, color="#a08a60"))
    ability(out, "rng_piercing_shot", ("Бронебойный выстрел", "Piercing Shot"), "projectile", ("Сильный выстрел: −8 брони и −15% сопротивления физическому урону на 6 с.", "A strong shot: −8 armor and −15% physical resistance for 6 s."), "#e0e0c0",
            mana=10, cd=5000, damage=[12, 18], dmg="pierce", scale="dex", k=1.3, range_=11, speed=20, glyph="dir",
            on_hit=buff("armor_pierced", ("Пробитый доспех", "Pierced Armor"), 6000, {"armor": -8, "res_physical": -15}, color="#e0e0c0"))
    ability(out, "rng_headshot", ("Выстрел в голову", "Headshot"), "projectile", ("Огромный урон и оглушение на 1,5 с.", "Huge damage and a 1.5 s stun."), "#ffffff",
            mana=16, cd=10000, damage=[20, 30], dmg="pierce", scale="dex", k=1.8, range_=12, speed=15, glyph="dir",
            on_hit=buff("headshot_stun", ("Оглушение", "Stun"), 1500, stun=True, color="#ffffff"))
    ability(out, "rng_rain_of_arrows", ("Дождь стрел", "Rain of Arrows"), "projectile", ("Стрелы накрывают круг радиусом 3.", "Arrows cover a circle of radius 3."), "#d0e0a0",
            mana=28, cd=14000, damage=[16, 26], dmg="pierce", scale="dex", k=1.5, range_=10, radius=3, speed=40, glyph="|")
    ability(out, "rng_call_wolf", ("Зов волка", "Call of the Wolf"), "summon", ("Волк-спутник на 60 с.", "A wolf companion for 60 s."), "#a0a0a0",
            mana=14, cd=30000, count=1, summon="rng_wolf_companion", duration=60000)
    ability(out, "rng_call_hawk", ("Соколы", "Falcons"), "summon", ("Два сокола на 40 с.", "Two falcons for 40 s."), "#c09a6a",
            mana=16, cd=30000, count=2, summon="rng_hawk", duration=40000)
    ability(out, "rng_call_bear", ("Медведь-побратим", "Bear Brother"), "summon", ("Медведь на 50 с.", "A bear for 50 s."), "#8a5a3a",
            mana=26, cd=50000, count=1, summon="rng_bear_companion", duration=50000)
    ability(out, "rng_thorn_spear", ("Терновое копьё", "Thorn Spear"), "strike", ("Два удара и яд на 5 с.", "Two blows and poison for 5 s."), "#7ab04a",
            mana=10, cd=5000, damage=[5, 9], dmg="pierce", scale="str", k=0.8, count=2, glyph="!", equip="melee",
            on_hit=buff("thorn_venom", ("Яд шипов", "Thorn Poison"), 5000, dot=3, dmg="poison", color="#7ab04a"))
    ability(out, "rng_barkguard", ("Кора стража", "Barkguard"), "buff", ("12 с: +10 брони, +8 шипов, +2 к восстановлению здоровья.", "12 s: +10 armor, +8 thorns, +2 health regeneration."), "#8a6a3a",
            mana=12, cd=25000, buff_=buff("barkguard", ("Кора стража", "Barkguard"), 12000, {"armor": 10, "thorns": 8, "hp_regen": 2}, color="#8a6a3a"))
    ability(out, "rng_forest_wrath", ("Ярость леса", "Forest Wrath"), "cleave", ("Удар по всем в радиусе 2: колющий урон и яд пополам.", "A blow at all in radius 2: piercing and poison damage in halves."), "#5aa03a",
            mana=20, cd=12000, damage=[12, 20], split=["pierce", "poison"], scale="str", k=1.2, radius=2, glyph="*")

    monster(out, "rng_wolf_companion", ("Волк-спутник", "Wolf Companion"), "w", "#a0a0a0", 70, [5, 9], 2, "pierce", 140, 800, "melee", "skirmisher", "wolf", {"cold": 25})
    monster(out, "rng_hawk", ("Ловчий сокол", "Hunting Falcon"), "v", "#c09a6a", 30, [3, 6], 0, "pierce", 110, 700, "melee", "skirmisher", "giant_bat")
    monster(out, "rng_bear_companion", ("Медведь-побратим", "Bear Brother"), "B", "#8a5a3a", 200, [10, 16], 6, "slash", 220, 1200, "melee", "frontline", "bear", {"cold": 25, "poison": 25})

    # ================= Паладин =================
    out.comment("Паладин")
    cls(out, "paladin", ("Паладин", "Paladin"), "#f0d060",
        ("Святой воин в латах: ауры, щит и свет. Подклассы: Защитник веры, Каратель, Светоносец.",
         "A holy warrior in plate: auras, shield and light. Subclasses: Protector, Avenger, Lightbringer."),
        {"str": 7, "dex": 3, "int": 5, "vit": 7}, ["pal_holy_strike"],
        ["mace", "wooden_shield", "chain_mail", "iron_helmet", "cloth_pants", "potion_health", "holy_water"], "paladin")
    subclass(out, "protector", ("Защитник веры", "Protector"), "paladin", "#a0c0ff",
             ("Щит, вызов и святилище: стоит насмерть за союзников.", "Shield, challenge and sanctuary: stands to the death for allies."))
    subclass(out, "avenger", ("Каратель", "Avenger"), "paladin", "#ffb040",
             ("Кара света: молот правосудия и судный день.", "Punishment of light: the hammer of justice and the day of judgement."))
    subclass(out, "lightbringer", ("Светоносец", "Lightbringer"), "paladin", "#fff4a0",
             ("Исцеляющий свет и маяк для союзников.", "Healing light and a beacon for allies."))
    branch(out, "paladin", ("Паладин", "Paladin"), "#f0d060", ("Вера, латы и ауры.", "Faith, plate and auras."), "paladin")
    branch(out, "protector", ("Защитник веры", "Protector"), "#a0c0ff", ("Щит и святилище.", "Shield and sanctuary."), "paladin", "protector")
    branch(out, "avenger", ("Каратель", "Avenger"), "#ffb040", ("Святая кара.", "Holy punishment."), "paladin", "avenger")
    branch(out, "lightbringer", ("Светоносец", "Lightbringer"), "#fff4a0", ("Исцеляющий свет.", "Healing light."), "paladin", "lightbringer")

    skill(out, "pal_devotion", ("Преданность", "Devotion"), "paladin", 1, 5, 1, ("Урон светом и броня.", "Holy damage and armor."), {"holy_pct": 5, "armor": 1})
    skill(out, "pal_faith_armor", ("Броня веры", "Armor of Faith"), "paladin", 1, 5, 1, ("Здоровье и защита от тьмы.", "Health and protection from darkness."), {"max_hp": 6, "res_shadow": 3})
    skill(out, "pal_lay_hands", ("Возложение рук", "Lay on Hands"), "paladin", 2, 1, 3, ("Сильное исцеление себя.", "A strong self-heal."), grants="pal_lay_hands", requires=["pal_faith_armor"])
    skill(out, "pal_aura_courage", ("Аура отваги", "Aura of Courage"), "paladin", 2, 1, 4, ("Вы и союзники рядом бьёте сильнее и стойче держитесь.", "You and nearby allies strike harder and stand firmer."), grants="pal_aura_courage", requires=["pal_devotion"])
    skill(out, "pal_steadfast", ("Непоколебимость", "Steadfast"), "paladin", 3, 3, 6, ("Блок и сопротивление всему.", "Block and resistance to everything."), {"block": 3, "res_all": 2}, requires=["pal_aura_courage"])

    skill(out, "pal_bulwark", ("Оплот", "Bulwark"), "protector", 1, 5, 5, ("Со щитом: блок и броня.", "With a shield: block and armor."), {"block": 3, "armor": 2}, equip="shield")
    skill(out, "pal_challenge", ("Вызов", "Challenge"), "protector", 1, 1, 5, ("Враги вокруг бросаются на вас, а вы крепнете.", "Enemies around rush at you, and you grow sturdier."), grants="pal_challenge")
    skill(out, "pal_divine_shield", ("Божественный щит", "Divine Shield"), "protector", 2, 1, 7, ("Почти полная неуязвимость на 6 с.", "Near invulnerability for 6 s."), grants="pal_divine_shield", requires=["pal_bulwark"])
    skill(out, "pal_sanctuary", ("Освящённая земля", "Hallowed Ground"), "protector", 3, 1, 10, ("Освящённая земля защищает и лечит союзников.", "Hallowed ground protects and heals allies."), grants="pal_sanctuary", requires=["pal_divine_shield"])

    skill(out, "pal_retribution", ("Возмездие", "Retribution"), "avenger", 1, 5, 5, ("Урон светом и в ближнем бою.", "Holy and melee damage."), {"holy_pct": 6, "melee_pct": 3})
    skill(out, "pal_hammer_of_justice", ("Молот правосудия", "Hammer of Justice"), "avenger", 1, 1, 5, ("Брошенный молот света оглушает врага.", "A thrown hammer of light stuns an enemy."), grants="pal_hammer_of_justice")
    skill(out, "pal_crusader_strike", ("Удар крестоносца", "Crusader Strike"), "avenger", 2, 1, 7, ("Два удара светом и сталью.", "Two blows of light and steel."), grants="pal_crusader_strike", requires=["pal_retribution"], equip="weapon")
    skill(out, "pal_judgement_day", ("Судный день", "Judgement Day"), "avenger", 3, 1, 10, ("Свет обрушивается на всех врагов вокруг.", "Light crashes down on every enemy around."), grants="pal_judgement_day", requires=["pal_crusader_strike"])

    skill(out, "pal_radiance", ("Сияние веры", "Radiant Faith"), "lightbringer", 1, 5, 5, ("Лечение сильнее, здоровье восстанавливается быстрее.", "Stronger healing, faster health regeneration."), {"heal_pct": 6, "hp_regen": 0.3})
    skill(out, "pal_holy_light", ("Свет небес", "Heavenly Light"), "lightbringer", 1, 1, 5, ("Исцеляет вас и союзников вокруг.", "Heals you and allies around."), grants="pal_holy_light")
    skill(out, "pal_beacon", ("Маяк света", "Beacon of Light"), "lightbringer", 2, 1, 7, ("Союзники рядом восстанавливают здоровье 10 секунд.", "Nearby allies regenerate health for 10 seconds."), grants="pal_beacon", requires=["pal_radiance"])
    skill(out, "pal_dawn", ("Рассвет", "Dawn"), "lightbringer", 3, 1, 10, ("Волна света жжёт врагов и лишает их защиты от света.", "A wave of light burns enemies and strips their protection from light."), grants="pal_dawn", requires=["pal_beacon"])

    ability(out, "pal_holy_strike", ("Святой удар", "Holy Strike"), "strike", ("Удар оружием, напоённым светом.", "A weapon blow filled with light."), "#ffe080",
            mana=4, cd=1500, damage=[5, 9], dmg="holy", scale="str", k=0.8, glyph="!", equip="weapon")
    ability(out, "pal_lay_hands", ("Возложение рук", "Lay on Hands"), "heal", ("Большое исцеление себя.", "A big self-heal."), "#fff0a0",
            mana=16, cd=30000, damage=[35, 50], scale="int", k=2.0, glyph="+")
    ability(out, "pal_aura_courage", ("Аура отваги", "Aura of Courage"), "buff", ("Вам и союзникам в радиусе 5: +10% урона в ближнем бою и +5% сопротивления всему на 15 с.", "You and allies in radius 5: +10% melee damage and +5% resistance to everything for 15 s."), "#ffd060",
            mana=12, cd=25000, radius=5, buff_=buff("aura_courage", ("Аура отваги", "Aura of Courage"), 15000, {"melee_pct": 10, "res_all": 5}, color="#ffd060"))
    ability(out, "pal_challenge", ("Вызов", "Challenge"), "taunt", ("Враги в радиусе 4 атакуют только вас; +8 брони на 8 с.", "Enemies in radius 4 attack only you; +8 armor for 8 s."), "#a0c0ff",
            mana=8, cd=12000, radius=4, on_hit=buff("challenged", ("Вызов", "Challenge"), 5000, color="#a0c0ff"),
            buff_=buff("challenge_guard", ("Стойкость", "Fortitude"), 8000, {"armor": 8}, color="#a0c0ff"))
    ability(out, "pal_divine_shield", ("Божественный щит", "Divine Shield"), "buff", ("6 с: +40% сопротивления всему и +15 брони.", "6 s: +40% resistance to everything and +15 armor."), "#f0f0ff",
            mana=18, cd=45000, buff_=buff("divine_shield", ("Божественный щит", "Divine Shield"), 6000, {"res_all": 40, "armor": 15}, color="#f0f0ff"))
    ability(out, "pal_sanctuary", ("Освящённая земля", "Hallowed Ground"), "buff", ("Вам и союзникам в радиусе 5: +20% сопротивления всему и +5 к восстановлению здоровья на 10 с.", "You and allies in radius 5: +20% resistance to everything and +5 health regeneration for 10 s."), "#fff8c0",
            mana=26, cd=40000, radius=5, buff_=buff("sanctuary", ("Освящённая земля", "Hallowed Ground"), 10000, {"res_all": 20, "hp_regen": 5}, color="#fff8c0"))
    ability(out, "pal_hammer_of_justice", ("Молот правосудия", "Hammer of Justice"), "projectile", ("Урон светом и оглушение на 1,5 с.", "Holy damage and a 1.5 s stun."), "#ffd040",
            mana=10, cd=8000, damage=[8, 14], dmg="holy", scale="str", k=1.0, range_=7, speed=40, glyph="T",
            on_hit=buff("justice_stun", ("Оглушение", "Stun"), 1500, stun=True, color="#ffd040"))
    ability(out, "pal_crusader_strike", ("Удар крестоносца", "Crusader Strike"), "strike", ("Два удара: урон светом и дробящий пополам.", "Two blows: holy and blunt damage in halves."), "#ffb040",
            mana=12, cd=6000, damage=[8, 13], split=["holy", "blunt"], scale="str", k=0.9, count=2, glyph="!", equip="weapon")
    ability(out, "pal_judgement_day", ("Судный день", "Judgement Day"), "nova", ("Свет в радиусе 4: огромный урон.", "Light in radius 4: huge damage."), "#fff0a0",
            mana=32, cd=16000, damage=[22, 34], dmg="holy", scale="str", k=1.5, radius=4, glyph="+")
    ability(out, "pal_holy_light", ("Свет небес", "Heavenly Light"), "heal", ("Лечит вас и союзников в радиусе 5.", "Heals you and allies in radius 5."), "#fff8c0",
            mana=14, cd=8000, damage=[18, 26], scale="int", k=1.4, radius=5, glyph="+")
    ability(out, "pal_beacon", ("Маяк света", "Beacon of Light"), "buff", ("Вам и союзникам в радиусе 5: восстановление 6 здоровья в секунду на 10 с.", "You and allies in radius 5: regain 6 health per second for 10 s."), "#fff4a0",
            mana=16, cd=20000, radius=5, buff_=buff("beacon", ("Маяк света", "Beacon of Light"), 10000, dot=-6, color="#fff4a0"))
    ability(out, "pal_dawn", ("Рассвет", "Dawn"), "nova", ("Свет в радиусе 5: урон и −25% сопротивления свету на 8 с.", "Light in radius 5: damage and −25% holy resistance for 8 s."), "#ffe8a0",
            mana=30, cd=16000, damage=[16, 24], dmg="holy", scale="int", k=1.4, radius=5, glyph="+",
            on_hit=buff("dawn_mark", ("Печать рассвета", "Mark of Dawn"), 8000, {"res_holy": -25}, color="#ffe8a0"))

    # ================= Монах =================
    out.comment("Монах")
    cls(out, "monk", ("Монах", "Monk"), "#f0a050",
        ("Боевые искусства, скорость и дух: удары посохом, уклонение и внутренняя сила. Подклассы: Путь ветра, Железное тело, Путь духа.",
         "Martial arts, speed and spirit: staff strikes, dodging and inner strength. Subclasses: Way of Wind, Iron Body, Way of Spirit."),
        {"str": 6, "dex": 7, "int": 4, "vit": 6}, ["mnk_palm_strike"],
        ["staff", "robe", "cloth_pants", "potion_health", "potion_health", "bread"], "monk")
    subclass(out, "way_of_wind", ("Путь ветра", "Way of Wind"), "monk", "#c0f0ff", ("Скорость, рывки и шквал ударов.", "Speed, dashes and a flurry of blows."))
    subclass(out, "iron_body", ("Железное тело", "Iron Body"), "monk", "#c0a080", ("Каменная стойка и алмазное тело: неприступная защита.", "Stone stance and diamond body: an impregnable defense."))
    subclass(out, "way_of_spirit", ("Путь духа", "Way of Spirit"), "monk", "#e0c0ff", ("Волны ци и просветление: тайная магия в ударах.", "Waves of chi and enlightenment: arcane magic in every blow."))
    branch(out, "monk", ("Монах", "Monk"), "#f0a050", ("Дисциплина тела и духа.", "Discipline of body and spirit."), "monk")
    branch(out, "way_of_wind", ("Путь ветра", "Way of Wind"), "#c0f0ff", ("Скорость и рывки.", "Speed and dashes."), "monk", "way_of_wind")
    branch(out, "iron_body", ("Железное тело", "Iron Body"), "#c0a080", ("Несокрушимость.", "Indestructibility."), "monk", "iron_body")
    branch(out, "way_of_spirit", ("Путь духа", "Way of Spirit"), "#e0c0ff", ("Сила ци.", "The power of chi."), "monk", "way_of_spirit")

    skill(out, "mnk_discipline", ("Дисциплина", "Discipline"), "monk", 1, 5, 1, ("Уклонение и скорость атаки.", "Dodge and attack speed."), {"dodge": 1.5, "attack_speed": 2})
    skill(out, "mnk_inner_strength", ("Внутренняя сила", "Inner Strength"), "monk", 1, 5, 1, ("Здоровье и восстановление маны.", "Health and mana regeneration."), {"max_hp": 5, "mp_regen": 0.15})
    skill(out, "mnk_flurry", ("Шквал ударов", "Flurry"), "monk", 2, 1, 3, ("Четыре быстрых удара по одной цели.", "Four quick blows at one target."), grants="mnk_flurry", requires=["mnk_discipline"])
    skill(out, "mnk_meditation", ("Медитация", "Meditation"), "monk", 2, 1, 4, ("Исцеление и быстрое восстановление маны.", "Healing and fast mana regeneration."), grants="mnk_meditation", requires=["mnk_inner_strength"])
    skill(out, "mnk_balance", ("Равновесие", "Balance"), "monk", 3, 3, 6, ("Уклонение и крит. шанс.", "Dodge and crit chance."), {"dodge": 2, "crit": 2}, requires=["mnk_flurry"])

    skill(out, "mnk_wind_step", ("Шаг ветра", "Wind Step"), "way_of_wind", 1, 5, 5, ("Скорость бега и уклонение.", "Running speed and dodge."), {"move_speed": 3, "dodge": 1.5})
    skill(out, "mnk_wind_dash", ("Порыв ветра", "Gust Dash"), "way_of_wind", 1, 1, 5, ("Рывок с ударом всего на пути.", "A dash that strikes everything in the way."), grants="mnk_wind_dash")
    skill(out, "mnk_cyclone_kick", ("Вихревой удар", "Cyclone Kick"), "way_of_wind", 2, 1, 7, ("Круговой удар ногой по всем рядом.", "A spinning kick at everyone nearby."), grants="mnk_cyclone_kick", requires=["mnk_wind_step"])
    skill(out, "mnk_storm_of_palms", ("Буря ладоней", "Storm of Palms"), "way_of_wind", 3, 1, 10, ("Семь стремительных ударов.", "Seven lightning-fast blows."), grants="mnk_storm_of_palms", requires=["mnk_cyclone_kick"])

    skill(out, "mnk_iron_flesh", ("Железная плоть", "Iron Flesh"), "iron_body", 1, 5, 5, ("Броня и стойкость к физическому урону.", "Armor and physical resistance."), {"armor": 2, "res_physical": 2})
    skill(out, "mnk_stone_stance", ("Каменная стойка", "Stone Stance"), "iron_body", 1, 1, 5, ("Броня и шипы ценой скорости.", "Armor and thorns at the cost of speed."), grants="mnk_stone_stance")
    skill(out, "mnk_earth_palm", ("Ладонь земли", "Earth Palm"), "iron_body", 2, 1, 7, ("Удар о землю оглушает врагов рядом.", "A blow to the ground stuns nearby enemies."), grants="mnk_earth_palm", requires=["mnk_iron_flesh"])
    skill(out, "mnk_diamond_body", ("Алмазное тело", "Diamond Body"), "iron_body", 3, 1, 10, ("Тело твёрже алмаза.", "A body harder than diamond."), grants="mnk_diamond_body", requires=["mnk_earth_palm"])

    skill(out, "mnk_chi_flow", ("Поток ци", "Chi Flow"), "way_of_spirit", 1, 5, 5, ("Тайная магия сильнее, маны больше.", "Stronger arcane magic, more mana."), {"arcane_pct": 6, "max_mp": 4})
    skill(out, "mnk_chi_blast", ("Волна ци", "Chi Blast"), "way_of_spirit", 1, 1, 5, ("Сгусток внутренней силы летит во врага.", "A burst of inner force flies at an enemy."), grants="mnk_chi_blast")
    skill(out, "mnk_spirit_palm", ("Духовная ладонь", "Spirit Palm"), "way_of_spirit", 2, 1, 7, ("Удар, похищающий жизнь.", "A blow that steals life."), grants="mnk_spirit_palm", requires=["mnk_chi_flow"])
    skill(out, "mnk_transcendence", ("Просветление", "Transcendence"), "way_of_spirit", 3, 1, 10, ("Волна ци ранит и лишает врагов магии.", "A wave of chi wounds enemies and strips their magic."), grants="mnk_transcendence", requires=["mnk_spirit_palm"])

    ability(out, "mnk_palm_strike", ("Удар ладонью", "Palm Strike"), "strike", ("Удар, сбивающий ритм врага: −20% скорости атаки на 3 с.", "A blow that breaks the enemy's rhythm: −20% attack speed for 3 s."), "#f0c080",
            mana=3, cd=1500, damage=[5, 9], dmg="blunt", scale="dex", k=0.9, glyph="!",
            on_hit=buff("staggered", ("Сбит с ритма", "Staggered"), 3000, {"attack_speed": -20}, color="#f0c080"))
    ability(out, "mnk_flurry", ("Шквал ударов", "Flurry"), "strike", ("Четыре удара по одной цели.", "Four blows at one target."), "#f0a050",
            mana=10, cd=5000, damage=[3, 6], dmg="blunt", scale="dex", k=0.5, count=4, glyph="!")
    ability(out, "mnk_meditation", ("Медитация", "Meditation"), "heal", ("Исцеление и +3 к восстановлению маны на 10 с.", "Healing and +3 mana regeneration for 10 s."), "#e0d0ff",
            mana=0, cd=25000, damage=[15, 22], scale="int", k=1.2, glyph="+",
            buff_=buff("meditation", ("Медитация", "Meditation"), 10000, {"mp_regen": 3}, color="#e0d0ff"))
    ability(out, "mnk_wind_dash", ("Порыв ветра", "Gust Dash"), "dash", ("Рывок на 5 клеток, удар по первому на пути.", "A 5-tile dash, striking the first in the way."), "#c0f0ff",
            mana=8, cd=5000, damage=[8, 14], dmg="blunt", scale="dex", k=1.0, range_=5, glyph="~")
    ability(out, "mnk_cyclone_kick", ("Вихревой удар", "Cyclone Kick"), "cleave", ("Удар по всем вокруг.", "A blow at everyone around."), "#c0f0ff",
            mana=12, cd=6000, damage=[8, 13], dmg="blunt", scale="dex", k=1.0, radius=1, glyph="*")
    ability(out, "mnk_storm_of_palms", ("Буря ладоней", "Storm of Palms"), "strike", ("Семь ударов по одной цели.", "Seven blows at one target."), "#a0e0ff",
            mana=22, cd=12000, damage=[4, 7], dmg="blunt", scale="dex", k=0.6, count=7, glyph="!")
    ability(out, "mnk_stone_stance", ("Каменная стойка", "Stone Stance"), "buff", ("10 с: +15 брони и +10 шипов, бег медленнее на 20%.", "10 s: +15 armor and +10 thorns, 20% slower running."), "#c0a080",
            mana=10, cd=20000, buff_=buff("stone_stance", ("Каменная стойка", "Stone Stance"), 10000, {"armor": 15, "thorns": 10, "move_speed": -20}, color="#c0a080"))
    ability(out, "mnk_earth_palm", ("Ладонь земли", "Earth Palm"), "nova", ("Удар в радиусе 2 и оглушение на 1 с.", "A blow in radius 2 and a 1 s stun."), "#a08060",
            mana=14, cd=9000, damage=[10, 16], dmg="blunt", scale="str", k=1.0, radius=2, glyph="*",
            on_hit=buff("earth_stun", ("Оглушение", "Stun"), 1000, stun=True, color="#a08060"))
    ability(out, "mnk_diamond_body", ("Алмазное тело", "Diamond Body"), "buff", ("10 с: +30% сопротивления всему и +5 к восстановлению здоровья.", "10 s: +30% resistance to everything and +5 health regeneration."), "#e0f0ff",
            mana=20, cd=40000, buff_=buff("diamond_body", ("Алмазное тело", "Diamond Body"), 10000, {"res_all": 30, "hp_regen": 5}, color="#e0f0ff"))
    ability(out, "mnk_chi_blast", ("Волна ци", "Chi Blast"), "projectile", ("Тайная магия во врага.", "Arcane magic at an enemy."), "#e0c0ff",
            mana=6, cd=2000, damage=[8, 13], dmg="arcane", scale="int", k=1.0, range_=8, speed=30, glyph="o")
    ability(out, "mnk_spirit_palm", ("Духовная ладонь", "Spirit Palm"), "strike", ("Удар тайной магией: 40% урона возвращается здоровьем.", "An arcane blow: 40% of the damage returns as health."), "#d0b0ff",
            mana=12, cd=6000, damage=[12, 18], dmg="arcane", scale="int", k=1.1, leech=40, glyph="!")
    ability(out, "mnk_transcendence", ("Просветление", "Transcendence"), "nova", ("Тайная магия в радиусе 4 и печать молчания на 3 с.", "Arcane magic in radius 4 and a 3 s seal of silence."), "#f0e0ff",
            mana=30, cd=16000, damage=[18, 28], dmg="arcane", scale="int", k=1.5, radius=4, glyph="o",
            on_hit=buff("enlightened_silence", ("Безмолвие", "Silence"), 3000, silence=True, color="#f0e0ff"))

    # ================= Скальд =================
    out.comment("Скальд")
    cls(out, "skald", ("Скальд", "Skald"), "#ff9ad0",
        ("Воин-сказитель: боевые песни усиливают союзников, а громовой голос оглушает врагов. Подклассы: Боевой скальд, Сказитель, Громогласый.",
         "A warrior-storyteller: battle songs strengthen allies and a thunderous voice stuns enemies. Subclasses: Battle Skald, Storyteller, Thundervoice."),
        {"str": 6, "dex": 5, "int": 6, "vit": 6}, ["skd_war_chant"],
        ["axe", "round_shield", "leather_armor", "fur_hat", "leather_pants", "potion_health", "potion_mana"], "skald")
    subclass(out, "battle_skald", ("Боевой скальд", "Battle Skald"), "skald", "#ff7060", ("Секира и песнь ярости в самой гуще боя.", "An axe and a song of fury in the thick of battle."))
    subclass(out, "storyteller", ("Сказитель", "Storyteller"), "skald", "#a0e0c0", ("Колыбельные для врагов, исцеляющие песни для друзей.", "Lullabies for enemies, healing songs for friends."))
    subclass(out, "thundervoice", ("Громогласый", "Thundervoice"), "skald", "#fff06a", ("Голос, подобный грому: молнии и оглушение.", "A voice like thunder: lightning and stuns."))
    branch(out, "skald", ("Скальд", "Skald"), "#ff9ad0", ("Песни и саги.", "Songs and sagas."), "skald")
    branch(out, "battle_skald", ("Боевой скальд", "Battle Skald"), "#ff7060", ("Песнь ярости.", "The song of fury."), "skald", "battle_skald")
    branch(out, "storyteller", ("Сказитель", "Storyteller"), "#a0e0c0", ("Песни покоя и сна.", "Songs of rest and sleep."), "skald", "storyteller")
    branch(out, "thundervoice", ("Громогласый", "Thundervoice"), "#fff06a", ("Громовой голос.", "The thunderous voice."), "skald", "thundervoice")

    skill(out, "skd_rhythm", ("Ритм битвы", "Battle Rhythm"), "skald", 1, 5, 1, ("Скорость атаки и урон в ближнем бою.", "Attack speed and melee damage."), {"attack_speed": 2, "melee_pct": 2})
    skill(out, "skd_lore", ("Знание саг", "Saga Lore"), "skald", 1, 5, 1, ("Мана и сила лечения.", "Mana and healing power."), {"max_mp": 4, "heal_pct": 2})
    skill(out, "skd_battle_hymn", ("Боевой гимн", "Battle Hymn"), "skald", 2, 1, 3, ("Вы и союзники рядом наносите больше урона.", "You and nearby allies deal more damage."), grants="skd_battle_hymn", requires=["skd_lore"])
    skill(out, "skd_shout", ("Устрашающий клич", "Daunting Shout"), "skald", 2, 1, 4, ("Клич ранит и замедляет атаки врагов вокруг.", "A shout that wounds and slows the attacks of enemies around."), grants="skd_shout", requires=["skd_rhythm"])
    skill(out, "skd_saga", ("Сага о героях", "Saga of Heroes"), "skald", 3, 3, 6, ("Здоровье и сопротивление всему.", "Health and resistance to everything."), {"max_hp": 7, "res_all": 2}, requires=["skd_battle_hymn"])

    skill(out, "skd_axe_song", ("Песня секиры", "Axe Song"), "battle_skald", 1, 5, 5, ("С оружием ближнего боя: урон и крит. шанс.", "With a melee weapon: damage and crit chance."), {"melee_pct": 5, "crit": 1}, equip="melee")
    skill(out, "skd_rending_verse", ("Рвущий куплет", "Rending Verse"), "battle_skald", 1, 1, 5, ("Два удара, оставляющие кровоточащие раны.", "Two blows that leave bleeding wounds."), grants="skd_rending_verse", equip="melee")
    skill(out, "skd_fury_ballad", ("Баллада ярости", "Ballad of Fury"), "battle_skald", 2, 1, 7, ("Скорость атаки и вампиризм на время.", "Attack speed and life leech for a while."), grants="skd_fury_ballad", requires=["skd_axe_song"])
    skill(out, "skd_last_verse", ("Последний куплет", "The Last Verse"), "battle_skald", 3, 1, 10, ("Сокрушительный удар по всем вокруг.", "A crushing blow at everyone around."), grants="skd_last_verse", requires=["skd_fury_ballad"])

    skill(out, "skd_soothing", ("Убаюкивающий голос", "Soothing Voice"), "storyteller", 1, 5, 5, ("Сила лечения и восстановление маны.", "Healing power and mana regeneration."), {"heal_pct": 6, "mp_regen": 0.15})
    skill(out, "skd_lullaby", ("Колыбельная", "Lullaby"), "storyteller", 1, 1, 5, ("Враги вокруг засыпают на 3 секунды.", "Enemies around fall asleep for 3 seconds."), grants="skd_lullaby")
    skill(out, "skd_song_of_rest", ("Песнь отдыха", "Song of Rest"), "storyteller", 2, 1, 7, ("Лечит вас и союзников вокруг.", "Heals you and allies around."), grants="skd_song_of_rest", requires=["skd_soothing"])
    skill(out, "skd_epic_tale", ("Эпическое сказание", "Epic Tale"), "storyteller", 3, 1, 10, ("Союзники рядом становятся героями сказания.", "Nearby allies become heroes of the tale."), grants="skd_epic_tale", requires=["skd_song_of_rest"])

    skill(out, "skd_resonance", ("Резонанс", "Resonance"), "thundervoice", 1, 5, 5, ("Урон молнией и сила заклинаний.", "Lightning damage and spell power."), {"lightning_pct": 6, "spell_pct": 2})
    skill(out, "skd_thunder_word", ("Громовое слово", "Thunder Word"), "thundervoice", 1, 1, 5, ("Слово, бьющее молнией.", "A word that strikes like lightning."), grants="skd_thunder_word")
    skill(out, "skd_echo_shout", ("Раскатистый клич", "Rolling Shout"), "thundervoice", 2, 1, 7, ("Клич перекатывается от врага к врагу.", "The shout rolls from enemy to enemy."), grants="skd_echo_shout", requires=["skd_resonance"])
    skill(out, "skd_voice_of_storm", ("Голос бури", "Voice of the Storm"), "thundervoice", 3, 1, 10, ("Гром вокруг вас оглушает врагов.", "Thunder around you stuns enemies."), grants="skd_voice_of_storm", requires=["skd_echo_shout"])

    ability(out, "skd_war_chant", ("Боевой напев", "War Chant"), "buff", ("Вам и союзникам в радиусе 4: +8% урона в ближнем бою и скорости атаки на 10 с.", "You and allies in radius 4: +8% melee damage and attack speed for 10 s."), "#ff9ad0",
            mana=6, cd=15000, radius=4, buff_=buff("war_chant", ("Боевой напев", "War Chant"), 10000, {"melee_pct": 8, "attack_speed": 8}, color="#ff9ad0"))
    ability(out, "skd_battle_hymn", ("Боевой гимн", "Battle Hymn"), "buff", ("Вам и союзникам в радиусе 5: +10% урона любого рода на 12 с.", "You and allies in radius 5: +10% damage of every kind for 12 s."), "#ffb0e0",
            mana=12, cd=20000, radius=5, buff_=buff("battle_hymn", ("Боевой гимн", "Battle Hymn"), 12000, {"melee_pct": 10, "spell_pct": 10, "ranged_pct": 10}, color="#ffb0e0"))
    ability(out, "skd_shout", ("Устрашающий клич", "Daunting Shout"), "nova", ("Тайная магия в радиусе 3 и −25% скорости атаки врагов на 5 с.", "Arcane magic in radius 3 and −25% enemy attack speed for 5 s."), "#ff80c0",
            mana=10, cd=8000, damage=[4, 8], dmg="arcane", scale="str", k=0.6, radius=3, glyph="!",
            on_hit=buff("daunted", ("Устрашение", "Daunted"), 5000, {"attack_speed": -25}, color="#ff80c0"))
    ability(out, "skd_rending_verse", ("Рвущий куплет", "Rending Verse"), "strike", ("Два удара и кровотечение на 5 с.", "Two blows and bleeding for 5 s."), "#ff7060",
            mana=10, cd=5000, damage=[6, 10], dmg="slash", scale="str", k=0.8, count=2, glyph="!", equip="melee",
            on_hit=buff("rent", ("Кровотечение", "Bleeding"), 5000, dot=4, dmg="slash", color="#ff7060"))
    ability(out, "skd_fury_ballad", ("Баллада ярости", "Ballad of Fury"), "buff", ("10 с: +30% скорости атаки и +5% вампиризма.", "10 s: +30% attack speed and +5% life leech."), "#ff5040",
            mana=14, cd=25000, buff_=buff("fury_ballad", ("Баллада ярости", "Ballad of Fury"), 10000, {"attack_speed": 30, "life_leech": 5}, color="#ff5040"))
    ability(out, "skd_last_verse", ("Последний куплет", "The Last Verse"), "cleave", ("Удар по всем в радиусе 2.", "A blow at all in radius 2."), "#ff4030",
            mana=24, cd=14000, damage=[16, 26], dmg="weapon", scale="str", k=1.4, radius=2, glyph="*", equip="melee")
    ability(out, "skd_lullaby", ("Колыбельная", "Lullaby"), "nova", ("Враги в радиусе 4 засыпают на 3 с.", "Enemies in radius 4 fall asleep for 3 s."), "#a0e0c0",
            mana=16, cd=18000, damage=[0, 0], radius=4, glyph="z",
            on_hit=buff("asleep", ("Сон", "Asleep"), 3000, stun=True, color="#a0e0c0"))
    ability(out, "skd_song_of_rest", ("Песнь отдыха", "Song of Rest"), "heal", ("Лечит вас и союзников в радиусе 5.", "Heals you and allies in radius 5."), "#a0f0c0",
            mana=14, cd=9000, damage=[16, 24], scale="int", k=1.3, radius=5, glyph="+")
    ability(out, "skd_epic_tale", ("Эпическое сказание", "Epic Tale"), "buff", ("Вам и союзникам в радиусе 6: +15% сопротивления всему и +40 здоровья на 15 с.", "You and allies in radius 6: +15% resistance to everything and +40 health for 15 s."), "#c0ffe0",
            mana=26, cd=40000, radius=6, buff_=buff("epic_tale", ("Эпическое сказание", "Epic Tale"), 15000, {"res_all": 15, "max_hp": 40}, color="#c0ffe0"))
    ability(out, "skd_thunder_word", ("Громовое слово", "Thunder Word"), "projectile", ("Молния и −15% сопротивления молнии на 5 с.", "Lightning and −15% lightning resistance for 5 s."), "#fff06a",
            mana=6, cd=2000, damage=[8, 13], dmg="lightning", scale="int", k=1.0, range_=8, speed=15, glyph="*",
            on_hit=buff("thunderstruck", ("Громовой удар", "Thunderstruck"), 5000, {"res_lightning": -15}, color="#fff06a"))
    ability(out, "skd_echo_shout", ("Раскатистый клич", "Rolling Shout"), "chain", ("Молния перескакивает на 4 врагов.", "Lightning leaps to 4 enemies."), "#fff8a0",
            mana=14, cd=6000, damage=[9, 14], dmg="lightning", scale="int", k=1.1, range_=8, count=4, glyph="*")
    ability(out, "skd_voice_of_storm", ("Голос бури", "Voice of the Storm"), "nova", ("Гром в радиусе 4: урон молнией и оглушение на 1 с.", "Thunder in radius 4: lightning damage and a 1 s stun."), "#ffff80",
            mana=30, cd=15000, damage=[18, 28], dmg="lightning", scale="int", k=1.5, radius=4, glyph="*",
            on_hit=buff("storm_voice_stun", ("Оглушение", "Stun"), 1000, stun=True, color="#ffff80"))

    # ================= Зельевар =================
    out.comment("Зельевар")
    cls(out, "brewer", ("Зельевар", "Brewer"), "#80e0a0",
        ("Бомбы, яды и чудесные эликсиры. Подклассы: Подрывник, Мутаген, Целитель-зельевар.",
         "Bombs, poisons and wondrous elixirs. Subclasses: Demolitionist, Mutagenist, Apothecary."),
        {"str": 4, "dex": 6, "int": 7, "vit": 6}, ["brw_firebomb"],
        ["dagger", "leather_armor", "leather_cap", "leather_pants", "potion_health", "potion_health", "potion_mana"], "brewer_hero")
    subclass(out, "demolitionist", ("Подрывник", "Demolitionist"), "brewer", "#ff8a3a", ("Кассетные, ледяные и огромные бомбы.", "Cluster, frost and enormous bombs."))
    subclass(out, "mutagenist", ("Мутаген", "Mutagenist"), "brewer", "#a0ff60", ("Пьёт мутагены и рвёт врагов голыми руками.", "Drinks mutagens and tears enemies apart bare-handed."))
    subclass(out, "apothecary", ("Целитель-зельевар", "Apothecary"), "brewer", "#80ffc0", ("Целебные туманы и эликсир жизни.", "Healing mists and the elixir of life."))
    branch(out, "brewer", ("Зельевар", "Brewer"), "#80e0a0", ("Склянки, смеси и выдержка.", "Flasks, mixtures and endurance."), "brewer")
    branch(out, "demolitionist", ("Подрывник", "Demolitionist"), "#ff8a3a", ("Взрывы.", "Explosions."), "brewer", "demolitionist")
    branch(out, "mutagenist", ("Мутаген", "Mutagenist"), "#a0ff60", ("Мутации.", "Mutations."), "brewer", "mutagenist")
    branch(out, "apothecary", ("Целитель-зельевар", "Apothecary"), "#80ffc0", ("Целебные смеси.", "Healing mixtures."), "brewer", "apothecary")

    skill(out, "brw_volatile", ("Летучие смеси", "Volatile Mixtures"), "brewer", 1, 5, 1, ("Урон огнём и ядом.", "Fire and poison damage."), {"fire_pct": 4, "poison_pct": 4})
    skill(out, "brw_tolerance", ("Закалка ядами", "Toxin Tolerance"), "brewer", 1, 5, 1, ("Защита от яда и огня.", "Protection from poison and fire."), {"res_poison": 4, "res_fire": 3})
    skill(out, "brw_acid_flask", ("Склянка кислоты", "Acid Flask"), "brewer", 2, 1, 3, ("Кислота разъедает доспехи врагов.", "Acid eats away at enemy armor."), grants="brw_acid_flask", requires=["brw_volatile"])
    skill(out, "brw_quick_draught", ("Глоток эликсира", "Quick Draught"), "brewer", 2, 1, 4, ("Быстрое исцеление себя.", "A quick self-heal."), grants="brw_quick_draught", requires=["brw_tolerance"])
    skill(out, "brw_experience", ("Опыт алхимика", "Alchemist's Experience"), "brewer", 3, 3, 6, ("Здоровье и сила лечения.", "Health and healing power."), {"max_hp": 6, "heal_pct": 5}, requires=["brw_acid_flask"])

    skill(out, "brw_blast_lore", ("Наука взрывов", "Blast Lore"), "demolitionist", 1, 5, 5, ("Урон огнём и сила заклинаний.", "Fire damage and spell power."), {"fire_pct": 6, "spell_pct": 2})
    skill(out, "brw_cluster_bomb", ("Кассетная бомба", "Cluster Bomb"), "demolitionist", 1, 1, 5, ("Три бомбы разом.", "Three bombs at once."), grants="brw_cluster_bomb")
    skill(out, "brw_frost_bomb", ("Ледяная бомба", "Frost Bomb"), "demolitionist", 2, 1, 7, ("Взрыв холода замедляет врагов.", "A blast of cold slows enemies."), grants="brw_frost_bomb", requires=["brw_blast_lore"])
    skill(out, "brw_great_blast", ("Великий взрыв", "Great Blast"), "demolitionist", 3, 1, 10, ("Огромная бомба с огромным радиусом.", "A huge bomb with a huge radius."), grants="brw_great_blast", requires=["brw_frost_bomb"])

    skill(out, "brw_mutation", ("Мутация", "Mutation"), "mutagenist", 1, 5, 5, ("Сила и урон в ближнем бою.", "Strength and melee damage."), {"str": 1, "melee_pct": 3})
    skill(out, "brw_mutagen_strength", ("Мутаген силы", "Strength Mutagen"), "mutagenist", 1, 1, 5, ("Мышцы раздуваются от силы.", "Muscles swell with strength."), grants="brw_mutagen_strength")
    skill(out, "brw_mutagen_speed", ("Мутаген скорости", "Speed Mutagen"), "mutagenist", 2, 1, 7, ("Движения становятся стремительными.", "Movements become lightning-fast."), grants="brw_mutagen_speed", requires=["brw_mutation"])
    skill(out, "brw_abomination", ("Облик чудовища", "Abomination Form"), "mutagenist", 3, 1, 10, ("Вы превращаетесь в неуязвимое чудовище.", "You turn into an invulnerable monster."), grants="brw_abomination", requires=["brw_mutagen_speed"])

    skill(out, "brw_herbalism", ("Травничество", "Herbalism"), "apothecary", 1, 5, 5, ("Сила лечения.", "Healing power."), {"heal_pct": 7})
    skill(out, "brw_healing_mist", ("Целебный туман", "Healing Mist"), "apothecary", 1, 1, 5, ("Туман лечит вас и союзников.", "A mist heals you and allies."), grants="brw_healing_mist")
    skill(out, "brw_antidote_cloud", ("Облако противоядия", "Antidote Cloud"), "apothecary", 2, 1, 7, ("Защита от яда и восстановление для всех рядом.", "Poison protection and regeneration for everyone nearby."), grants="brw_antidote_cloud", requires=["brw_herbalism"])
    skill(out, "brw_elixir_of_life", ("Эликсир жизни", "Elixir of Life"), "apothecary", 3, 1, 10, ("Огромное исцеление и долгое восстановление.", "A huge heal and long regeneration."), grants="brw_elixir_of_life", requires=["brw_antidote_cloud"])

    ability(out, "brw_firebomb", ("Огненная бомба", "Firebomb"), "projectile", ("Бомба взрывается огнём в радиусе 1.", "A bomb bursts into fire in radius 1."), "#ff8a3a",
            mana=4, cd=1500, damage=[5, 9], dmg="fire", scale="int", k=1.0, range_=7, radius=1, speed=45, glyph="o")
    ability(out, "brw_acid_flask", ("Склянка кислоты", "Acid Flask"), "projectile", ("Яд в радиусе 1 и −6 брони на 6 с.", "Poison in radius 1 and −6 armor for 6 s."), "#a0e040",
            mana=8, cd=5000, damage=[7, 11], dmg="poison", scale="int", k=1.0, range_=7, radius=1, speed=45, glyph="o",
            on_hit=buff("acid", ("Кислота", "Acid"), 6000, {"armor": -6}, dot=2, dmg="poison", color="#a0e040"))
    ability(out, "brw_quick_draught", ("Глоток эликсира", "Quick Draught"), "heal", ("Быстрое исцеление.", "A quick heal."), "#80e0a0",
            mana=8, cd=20000, damage=[20, 30], scale="int", k=1.3, glyph="+")
    ability(out, "brw_cluster_bomb", ("Кассетная бомба", "Cluster Bomb"), "projectile", ("Три огненные бомбы веером.", "Three fire bombs in a fan."), "#ff9a3a",
            mana=12, cd=6000, damage=[6, 10], dmg="fire", scale="int", k=0.8, range_=7, radius=1, count=3, speed=45, glyph="o")
    ability(out, "brw_frost_bomb", ("Ледяная бомба", "Frost Bomb"), "projectile", ("Холод в радиусе 2 и замедление на 50% на 5 с.", "Cold in radius 2 and a 50% slow for 5 s."), "#8ad8ff",
            mana=14, cd=8000, damage=[9, 14], dmg="cold", scale="int", k=1.0, range_=8, radius=2, speed=45, glyph="o",
            on_hit=buff("frost_bombed", ("Обморожение", "Frostbite"), 5000, {"move_speed": -50, "attack_speed": -20}, color="#8ad8ff"))
    ability(out, "brw_great_blast", ("Великий взрыв", "Great Blast"), "projectile", ("Огромный взрыв огня в радиусе 3.", "A huge fire blast in radius 3."), "#ff6a1a",
            mana=32, cd=16000, damage=[26, 40], dmg="fire", scale="int", k=2.0, range_=8, radius=3, speed=70, glyph="@")
    ability(out, "brw_mutagen_strength", ("Мутаген силы", "Strength Mutagen"), "buff", ("20 с: +8 силы, +20% урона в ближнем бою, +30 здоровья.", "20 s: +8 strength, +20% melee damage, +30 health."), "#a0ff60",
            mana=12, cd=30000, buff_=buff("mutagen_strength", ("Мутаген силы", "Strength Mutagen"), 20000, {"str": 8, "melee_pct": 20, "max_hp": 30}, color="#a0ff60"))
    ability(out, "brw_mutagen_speed", ("Мутаген скорости", "Speed Mutagen"), "buff", ("20 с: +25% скорости бега и атаки, +10% уклонения.", "20 s: +25% running and attack speed, +10% dodge."), "#c0ff80",
            mana=12, cd=30000, buff_=buff("mutagen_speed", ("Мутаген скорости", "Speed Mutagen"), 20000, {"move_speed": 25, "attack_speed": 25, "dodge": 10}, color="#c0ff80"))
    ability(out, "brw_abomination", ("Облик чудовища", "Abomination Form"), "buff", ("15 с: +25% сопротивления всему, +10 брони, +30% урона в ближнем бою, +5 к восстановлению здоровья.", "15 s: +25% resistance to everything, +10 armor, +30% melee damage, +5 health regeneration."), "#60c040",
            mana=24, cd=50000, buff_=buff("abomination", ("Облик чудовища", "Abomination Form"), 15000, {"res_all": 25, "armor": 10, "melee_pct": 30, "hp_regen": 5}, color="#60c040"))
    ability(out, "brw_healing_mist", ("Целебный туман", "Healing Mist"), "heal", ("Лечит вас и союзников в радиусе 5.", "Heals you and allies in radius 5."), "#80ffc0",
            mana=14, cd=8000, damage=[16, 24], scale="int", k=1.3, radius=5, glyph="+")
    ability(out, "brw_antidote_cloud", ("Облако противоядия", "Antidote Cloud"), "buff", ("Вам и союзникам в радиусе 5: +30% сопротивления яду, +10% всему и +3 к восстановлению здоровья на 12 с.", "You and allies in radius 5: +30% poison resistance, +10% to everything and +3 health regeneration for 12 s."), "#a0ffd0",
            mana=16, cd=25000, radius=5, buff_=buff("antidote_cloud", ("Облако противоядия", "Antidote Cloud"), 12000, {"res_poison": 30, "res_all": 10, "hp_regen": 3}, color="#a0ffd0"))
    ability(out, "brw_elixir_of_life", ("Эликсир жизни", "Elixir of Life"), "heal", ("Огромное исцеление и +8 к восстановлению здоровья на 10 с.", "A huge heal and +8 health regeneration for 10 s."), "#c0ffe0",
            mana=26, cd=45000, damage=[45, 65], scale="int", k=2.5, glyph="+",
            buff_=buff("elixir_of_life", ("Эликсир жизни", "Elixir of Life"), 10000, {"hp_regen": 8}, color="#c0ffe0"))
