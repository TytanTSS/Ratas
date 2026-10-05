# Ten hidden skills for every class. A hidden skill opens by itself when its
# deed is done (see crates/core/src/game/deeds.rs); the tenth of each class grants an ability.
from core import *

# class: (prefix, color, (ru, en) class name, [10 skills], capstone ability)
# skill: (suffix, (ru name, en name), (ru desc, en desc), stats, deed, count)

HIDDEN = {
    "warrior": ("hwa", "#ff6a4a", [
        ("battles", ("Тысяча битв", "A Thousand Battles"), ("Опыт сотен схваток.", "The experience of hundreds of fights."), {"melee_pct": 6, "max_hp": 15}, "kills", 300),
        ("last_stand", ("На краю", "Last Stand"), ("Вы сильнее всего, когда почти мертвы.", "You are strongest when almost dead."), {"fury": 20, "res_all": 3}, "lowhp", 15),
        ("wall", ("Живая стена", "Living Wall"), ("Щит стал частью руки.", "The shield has become part of your arm."), {"block": 5, "armor": 4}, "block", 150),
        ("kingslayer", ("Убийца королей", "Kingslayer"), ("Короли подземелий падают от вашей руки.", "The kings of the dungeons fall by your hand."), {"melee_pct": 8, "crit": 3}, "bosses", 3),
        ("cave_bane", ("Гроза пещер", "Bane of Caves"), ("Пещерные твари знают вас в лицо.", "Cave creatures know your face."), {"armor": 3, "res_poison": 8}, "theme:cave", 150),
        ("orcbreaker", ("Сокрушитель орков", "Orcbreaker"), ("Орки поют о вас страшные песни.", "Orcs sing terrible songs about you."), {"str": 3, "melee_pct": 4}, "kill:orc", 40),
        ("bloodied", ("Кровавый клинок", "Bloodied Blade"), ("Ваш клинок помнит вкус крови.", "Your blade remembers the taste of blood."), {"life_leech": 1.5, "crit_mult": 0.2}, "dmg:slash", 15000),
        ("champion_bane", ("Гроза чемпионов", "Champion's Bane"), ("Самые сильные враги — ваши любимые.", "The strongest enemies are your favourites."), {"max_hp": 25, "melee_pct": 5}, "elites", 25),
        ("iron_will", ("Железная воля", "Iron Will"), ("Смерть не сломила вас. Пять раз.", "Death did not break you. Five times."), {"res_all": 5, "hp_regen": 1}, "deaths", 5),
        ("unbreakable", ("Несокрушимый", "Unbreakable"), ("Вершина пути воина: на миг вас ничто не берёт.", "The peak of the warrior's path: for a moment nothing can harm you."), None, "classlevel", 25),
    ], dict(key="hwa_unbreakable", name=("Несокрушимый", "Unbreakable"), kind="buff", color="#ff6a4a",
            desc=("8 с: +50% сопротивления всему и +10 к восстановлению здоровья.", "8 s: +50% resistance to everything and +10 health regeneration."),
            mana=10, cd=90000, buff_=buff("unbreakable", ("Несокрушимый", "Unbreakable"), 8000, {"res_all": 50, "hp_regen": 10}, color="#ff6a4a"), fx="shield steel")),

    "rogue": ("hro", "#a8b0c8", [
        ("precise", ("Точный удар", "Precise Strikes"), ("Вы знаете, куда бить.", "You know where to strike."), {"crit": 4, "crit_mult": 0.2}, "crits", 300),
        ("elusive", ("Неуловимый", "Elusive"), ("Попасть по вам — редкая удача.", "Hitting you is rare luck."), {"dodge": 5}, "dodge", 200),
        ("pickpocket", ("Карманник", "Pickpocket"), ("Золото само идёт в руки.", "Gold comes into your hands by itself."), {"gold_find": 25}, "gold", 5000),
        ("night_child", ("Дитя ночи", "Child of the Night"), ("Ночь — ваше время.", "The night is your time."), {"ambush_pct": 15, "sight": 1}, "night", 100),
        ("venom", ("Отравитель", "Venomancer"), ("Ваши яды не знают пощады.", "Your poisons know no mercy."), {"poison_pct": 15}, "dmg:poison", 8000),
        ("bandit_bane", ("Гроза разбойников", "Bandit's Bane"), ("Разбойники обходят вас стороной.", "Bandits give you a wide berth."), {"dex": 3, "dodge": 2}, "kill:bandit", 40),
        ("roads", ("Тысяча дорог", "A Thousand Roads"), ("Ноги несут вас сами.", "Your feet carry you on their own."), {"move_speed": 10}, "steps", 20000),
        ("headhunter", ("Охотник за головами", "Headhunter"), ("Один на один вам нет равных.", "One on one you have no equal."), {"duel_pct": 15}, "elites", 20),
        ("keen_edge", ("Острый клинок", "Keen Edge"), ("Каждый укол точен.", "Every thrust is precise."), {"melee_pct": 5, "ranged_pct": 5}, "dmg:pierce", 15000),
        ("vanish", ("Исчезновение", "Vanish"), ("Вершина пути плута: исчезнуть посреди боя.", "The peak of the rogue's path: vanishing in the middle of a fight."), None, "classlevel", 25),
    ], dict(key="hro_vanish", name=("Исчезновение", "Vanish"), kind="buff", color="#a8b0c8",
            desc=("Невидимость и +40% к скорости бега на 8 с.", "Stealth and +40% running speed for 8 s."),
            mana=10, cd=40000, buff_=buff("vanish", ("Исчезновение", "Vanish"), 8000, {"move_speed": 40}, stealth=True, color="#a8b0c8"), fx="smoke")),

    "mage": ("hma", "#6aa8ff", [
        ("spells", ("Тысяча заклинаний", "A Thousand Spells"), ("Магия стала второй натурой.", "Magic has become second nature."), {"spell_pct": 6, "mp_regen": 0.5}, "casts", 500),
        ("flamelord", ("Повелитель пламени", "Flamelord"), ("Огонь слушается вас.", "Fire obeys you."), {"fire_pct": 12}, "dmg:fire", 15000),
        ("winter", ("Сердце зимы", "Heart of Winter"), ("В вас живёт стужа.", "Frost lives in you."), {"cold_pct": 12}, "dmg:cold", 15000),
        ("arcane_seer", ("Тайновидец", "Arcane Seer"), ("Вы видите нити магии.", "You see the threads of magic."), {"arcane_pct": 12}, "dmg:arcane", 15000),
        ("thunderer", ("Громовержец", "Thunderer"), ("Молнии бьют туда, куда вы смотрите.", "Lightning strikes where you look."), {"lightning_pct": 12}, "dmg:lightning", 10000),
        ("banisher", ("Изгоняющий мёртвых", "Banisher of the Dead"), ("Мертвецы склепов боятся вашей магии.", "The dead of the crypts fear your magic."), {"res_shadow": 10, "spell_pct": 4}, "theme:crypt", 150),
        ("delver", ("Знаток глубин", "Delver"), ("Вы бывали там, где не светит солнце.", "You have been where the sun does not shine."), {"sight": 1, "max_mp": 25}, "depth", 5),
        ("lore_seeker", ("Искатель знаний", "Seeker of Lore"), ("Древние места открыли вам свои тайны.", "Ancient places have revealed their secrets to you."), {"int": 3, "max_mp": 15}, "landmarks", 8),
        ("mana_blood", ("Мана в крови", "Mana in the Blood"), ("Зелья маны изменили вашу кровь.", "Mana potions have changed your blood."), {"mp_regen": 1, "max_mp": 10}, "potions", 50),
        ("surge", ("Всплеск силы", "Arcane Surge"), ("Вершина пути мага: мгновение безграничной силы.", "The peak of the mage's path: a moment of boundless power."), None, "classlevel", 25),
    ], dict(key="hma_surge", name=("Всплеск силы", "Arcane Surge"), kind="buff", color="#6aa8ff",
            desc=("10 с: +50% силы заклинаний и +10 к восстановлению маны.", "10 s: +50% spell power and +10 mana regeneration."),
            mana=0, cd=60000, buff_=buff("arcane_surge", ("Всплеск силы", "Arcane Surge"), 10000, {"spell_pct": 50, "mp_regen": 10}, color="#6aa8ff"), fx="arcane")),

    "priest": ("hpr", "#ffe08a", [
        ("healings", ("Тысяча исцелений", "A Thousand Healings"), ("Ваши руки помнят каждую рану.", "Your hands remember every wound."), {"heal_pct": 12}, "heal", 15000),
        ("undead_scourge", ("Бич нежити", "Scourge of the Undead"), ("Нежить склепов дрожит при вашем имени.", "The undead of the crypts tremble at your name."), {"holy_pct": 12, "res_shadow": 5}, "theme:crypt", 200),
        ("necro_bane", ("Враг некромантов", "Bane of Necromancers"), ("Некроманты — ваша личная забота.", "Necromancers are your personal concern."), {"holy_pct": 6, "res_shadow": 10}, "kill:necromancer", 15),
        ("hand_of_light", ("Светоносная длань", "Hand of Light"), ("Свет течёт сквозь вас.", "Light flows through you."), {"holy_pct": 10}, "dmg:holy", 15000),
        ("wanderers_friend", ("Друг странников", "Friend of Wanderers"), ("Уникальные мастера мира считают вас своим.", "The unique masters of the world consider you one of their own."), {"max_hp": 15, "res_all": 3}, "uniques", 3),
        ("returned", ("Вернувшийся", "Returned"), ("Вы видели свет по ту сторону.", "You have seen the light on the other side."), {"hp_regen": 1.5}, "deaths", 3),
        ("servant", ("Слуга народа", "Servant of the People"), ("Люди благодарны за вашу помощь.", "People are grateful for your help."), {"heal_pct": 5, "gold_find": 10}, "quests", 10),
        ("demonbane", ("Изгоняющий демонов", "Demonbane"), ("Демоны цитадели узнали силу света.", "The demons of the citadel have learned the power of light."), {"res_fire": 10, "res_shadow": 10}, "theme:fortress", 100),
        ("light_in_dark", ("Свет во тьме", "Light in the Dark"), ("Ночью ваша вера светит ярче.", "At night your faith shines brighter."), {"sight": 1, "holy_pct": 6}, "night", 80),
        ("miracle", ("Чудо", "Miracle"), ("Вершина пути жреца: настоящее чудо.", "The peak of the priest's path: a true miracle."), None, "classlevel", 25),
    ], dict(key="hpr_miracle", name=("Чудо", "Miracle"), kind="heal", color="#fff8c0",
            desc=("Огромное исцеление вас и союзников в радиусе 6.", "A huge heal for you and allies in radius 6."),
            mana=30, cd=90000, damage=[60, 90], scale="int", k=3.0, radius=6, glyph="+", fx="holy")),

    "shaman": ("hsh", "#7ad0ff", [
        ("thunder_child", ("Сын грома", "Child of Thunder"), ("Гром — ваш голос.", "Thunder is your voice."), {"lightning_pct": 12}, "dmg:lightning", 15000),
        ("spirit_friend", ("Друг духов", "Friend of Spirits"), ("Духи охотно приходят на зов.", "Spirits gladly answer your call."), {"hp_regen": 1, "spell_pct": 4}, "summons", 50),
        ("north_child", ("Дитя севера", "Child of the North"), ("Холод вам нипочём.", "The cold is nothing to you."), {"res_cold": 15, "max_hp": 10}, "theme:tundra", 150),
        ("giantslayer", ("Убийца великанов", "Giantslayer"), ("Великаны падают, как деревья.", "Giants fall like trees."), {"str": 3, "lightning_pct": 6}, "kill:frost_giant", 5),
        ("tribal_healer", ("Целитель племени", "Tribal Healer"), ("Духи предков лечат вашими руками.", "Ancestral spirits heal through your hands."), {"heal_pct": 10}, "heal", 8000),
        ("night_ritual", ("Ночное камлание", "Night Ritual"), ("Ночью духи ближе.", "At night the spirits are closer."), {"mp_regen": 0.6, "sight": 1}, "night", 80),
        ("tireless", ("Неутомимый", "Tireless"), ("Камлать можно бесконечно.", "You can perform rituals endlessly."), {"max_mp": 20}, "casts", 400),
        ("hill_wind", ("Ветер холмов", "Wind of the Hills"), ("Ветер несёт вас.", "The wind carries you."), {"move_speed": 6, "dodge": 3}, "theme:hills", 150),
        ("lightning_strike", ("Удар молнии", "Lightning Strike"), ("Молния не бьёт дважды. Вы — бьёте.", "Lightning never strikes twice. You do."), {"crit": 4}, "crits", 200),
        ("storm_avatar", ("Аватар бури", "Storm Avatar"), ("Вершина пути шамана: стать самой бурей.", "The peak of the shaman's path: becoming the storm itself."), None, "classlevel", 25),
    ], dict(key="hsh_storm_avatar", name=("Аватар бури", "Storm Avatar"), kind="buff", color="#fff06a",
            desc=("15 с: +50% урона молнией и +20% скорости бега.", "15 s: +50% lightning damage and +20% running speed."),
            mana=20, cd=60000, buff_=buff("storm_avatar", ("Аватар бури", "Storm Avatar"), 15000, {"lightning_pct": 50, "move_speed": 20}, color="#fff06a"), fx="form storm")),

    "warlock": ("hwl", "#b05aff", [
        ("shadow_lord", ("Владыка тени", "Shadowlord"), ("Тьма слушается вас.", "Darkness obeys you."), {"shadow_pct": 12}, "dmg:shadow", 15000),
        ("demon_tamer", ("Укротитель демонов", "Demon Tamer"), ("Демоны уважают силу.", "Demons respect strength."), {"res_fire": 10, "shadow_pct": 6}, "kill:demon", 20),
        ("imp_master", ("Хозяин бесов", "Master of Imps"), ("Призванные слуги крепнут от вашей силы.", "Summoned servants grow strong from your power."), {"max_hp": 15}, "summons", 40),
        ("life_thirst", ("Жажда жизни", "Thirst for Life"), ("На краю гибели вы пьёте жизнь жаднее.", "On the brink of death you drink life more greedily."), {"life_leech": 2}, "lowhp", 10),
        ("cursed_child", ("Сын проклятых земель", "Child of the Cursed Lands"), ("Проклятия к вам не липнут.", "Curses do not stick to you."), {"res_shadow": 10, "res_poison": 10}, "theme:cursed", 150),
        ("soul_price", ("Цена души", "Price of a Soul"), ("Вы знаете цену всему.", "You know the price of everything."), {"gold_find": 20}, "gold", 4000),
        ("death_escapee", ("Беглец от смерти", "Escapee of Death"), ("Смерть устала за вами гоняться.", "Death is tired of chasing you."), {"hp_regen": 1.5, "res_shadow": 5}, "deaths", 5),
        ("curses", ("Тысяча проклятий", "A Thousand Curses"), ("Проклятия срываются с языка сами.", "Curses roll off your tongue by themselves."), {"spell_pct": 8}, "casts", 500),
        ("power_devourer", ("Пожиратель силы", "Devourer of Power"), ("Сила павших владык перешла к вам.", "The power of fallen lords has passed to you."), {"shadow_pct": 8, "int": 3}, "bosses", 2),
        ("abyss_gate", ("Врата бездны", "Gate of the Abyss"), ("Вершина пути чернокнижника: распахнуть бездну.", "The peak of the warlock's path: throwing open the abyss."), None, "classlevel", 25),
    ], dict(key="hwl_abyss_gate", name=("Врата бездны", "Gate of the Abyss"), kind="nova", color="#7a2aff",
            desc=("Тьма в радиусе 5: огромный урон.", "Darkness in radius 5: huge damage."),
            mana=30, cd=40000, damage=[34, 50], dmg="shadow", scale="int", k=2.2, radius=5, glyph="@", fx="void")),

    "druid": ("hdr", "#5ec05e", [
        ("forest_heart", ("Сердце леса", "Heart of the Forest"), ("Лес делится с вами силой.", "The forest shares its strength with you."), {"max_hp": 15, "hp_regen": 0.8}, "theme:forest", 200),
        ("bear_strength", ("Медвежья сила", "Bear Strength"), ("Сила медведя в ваших руках.", "The strength of a bear in your arms."), {"str": 3, "armor": 3}, "kill:bear", 20),
        ("wolf_agility", ("Волчья прыть", "Wolf Agility"), ("Вы бегаете, как волк.", "You run like a wolf."), {"move_speed": 6, "attack_speed": 5}, "kill:wolf", 40),
        ("toxin", ("Древний яд", "Ancient Toxin"), ("Яды леса в ваших жилах.", "The poisons of the forest are in your veins."), {"poison_pct": 12}, "dmg:poison", 10000),
        ("mender", ("Целитель рощи", "Grove Mender"), ("Раны затягиваются от вашего прикосновения.", "Wounds close at your touch."), {"heal_pct": 10}, "heal", 8000),
        ("wanderer", ("Странник лесов", "Forest Wanderer"), ("Тропы сами ложатся под ноги.", "The trails lay themselves under your feet."), {"move_speed": 6, "dodge": 2}, "steps", 25000),
        ("swamp_dweller", ("Болотный житель", "Swamp Dweller"), ("Болота вас не тронут.", "The swamps will not touch you."), {"res_poison": 15}, "theme:swamp", 120),
        ("summoner", ("Зов дикой природы", "Call of the Wild"), ("Природа приходит на зов.", "Nature answers your call."), {"spell_pct": 6}, "summons", 40),
        ("treant_brother", ("Брат древней", "Brother of Treants"), ("Древни признали в вас родню.", "The treants have recognized you as kin."), {"armor": 5, "res_all": 3}, "kill:treant", 5),
        ("world_tree", ("Мировое древо", "World Tree"), ("Вершина пути друида: сила Мирового древа.", "The peak of the druid's path: the power of the World Tree."), None, "classlevel", 25),
    ], dict(key="hdr_world_tree", name=("Мировое древо", "World Tree"), kind="buff", color="#5ec05e",
            desc=("Вам и союзникам в радиусе 6: восстановление 10 здоровья в секунду и +10% сопротивления всему на 12 с.", "You and allies in radius 6: regain 10 health per second and +10% resistance to everything for 12 s."),
            mana=26, cd=60000, radius=6, buff_=buff("world_tree", ("Мировое древо", "World Tree"), 12000, {"res_all": 10}, dot=-10, color="#5ec05e"), fx="nature")),

    "necromancer": ("hnc", "#7ad07a", [
        ("crypt_master", ("Хозяин склепов", "Master of Crypts"), ("Склепы — ваш дом.", "The crypts are your home."), {"shadow_pct": 10, "res_shadow": 10}, "theme:crypt", 250),
        ("dead_army", ("Армия мёртвых", "Army of the Dead"), ("Мёртвые встают легче.", "The dead rise more easily."), {"max_mp": 20, "spell_pct": 4}, "summons", 60),
        ("bone_skin", ("Костяная кожа", "Bone Skin"), ("Кожа твердеет, как кость.", "Your skin hardens like bone."), {"armor": 4, "res_physical": 5}, "kill:skeleton", 60),
        ("icy_breath", ("Ледяное дыхание", "Icy Breath"), ("Ваше дыхание — могильный холод.", "Your breath is the cold of the grave."), {"cold_pct": 12}, "dmg:cold", 12000),
        ("death_friend", ("Знакомый со смертью", "Acquainted with Death"), ("Смерть — старая знакомая.", "Death is an old acquaintance."), {"hp_regen": 1.5}, "deaths", 5),
        ("lichbane", ("Победитель лича", "Lichbane"), ("Сила лича перешла к вам.", "The lich's power has passed to you."), {"int": 4, "shadow_pct": 8}, "kill:lich", 1),
        ("night_reaper", ("Ночной жнец", "Night Reaper"), ("Ночь — время жатвы.", "Night is the time of harvest."), {"sight": 1, "shadow_pct": 6}, "night", 120),
        ("plague_lore", ("Знаток мора", "Plague Scholar"), ("Болезни — ваши инструменты.", "Diseases are your tools."), {"poison_pct": 12}, "dmg:poison", 10000),
        ("soul_reaper", ("Жнец душ", "Reaper of Souls"), ("Каждая смерть делает вас сильнее.", "Every death makes you stronger."), {"max_hp": 20, "life_leech": 1}, "kills", 400),
        ("bone_storm", ("Костяная буря", "Bone Storm"), ("Вершина пути некроманта: буря костей.", "The peak of the necromancer's path: a storm of bones."), None, "classlevel", 25),
    ], dict(key="hnc_bone_storm", name=("Костяная буря", "Bone Storm"), kind="nova", color="#e8e0c8",
            desc=("Кости и холод в радиусе 4: тьма и холод пополам.", "Bones and cold in radius 4: shadow and cold in halves."),
            mana=28, cd=30000, damage=[26, 40], split=["shadow", "cold"], scale="int", k=1.9, radius=4, glyph="%", fx="orbit bone")),

    "ranger": ("hrg", "#6ab04a", [
        ("arrows", ("Тысяча стрел", "A Thousand Arrows"), ("Вы стреляете, не думая.", "You shoot without thinking."), {"ranged_pct": 10}, "dmg:pierce", 20000),
        ("sharp_eye", ("Соколиный глаз", "Falcon Eye"), ("Вы видите слабое место за сто шагов.", "You see a weak spot a hundred paces away."), {"crit": 4, "crit_mult": 0.15}, "crits", 300),
        ("tireless_legs", ("Неутомимые ноги", "Tireless Legs"), ("Вы не устаёте в пути.", "You never tire on the road."), {"move_speed": 8}, "steps", 30000),
        ("forest_ghost", ("Лесной призрак", "Forest Ghost"), ("В лесу вас не заметить.", "In the forest you cannot be noticed."), {"dodge": 4, "sight": 1}, "theme:forest", 200),
        ("birdshot", ("Сбивающий птиц", "Birdshot"), ("Гарпии падают на лету.", "Harpies fall in mid-flight."), {"ranged_pct": 6, "crit": 2}, "kill:harpy", 30),
        ("beast_friend", ("Друг зверей", "Friend of Beasts"), ("Звери идут за вами охотно.", "Beasts follow you gladly."), {"max_hp": 15}, "summons", 40),
        ("pathfinder", ("Первопроходец", "Pathfinder"), ("Вы видели все чудеса этих земель.", "You have seen all the wonders of these lands."), {"move_speed": 4, "sight": 1}, "landmarks", 10),
        ("big_game", ("Охотник на крупную дичь", "Big Game Hunter"), ("Чем крупнее зверь, тем азартнее охота.", "The bigger the beast, the more thrilling the hunt."), {"ranged_pct": 8}, "elites", 20),
        ("night_vision", ("Ночное зрение", "Night Vision"), ("Темнота вам не помеха.", "Darkness is no obstacle to you."), {"sight": 2}, "night", 100),
        ("hail", ("Град стрел", "Hail of Arrows"), ("Вершина пути следопыта: небо, полное стрел.", "The peak of the ranger's path: a sky full of arrows."), None, "classlevel", 25),
    ], dict(key="hrg_hail", name=("Град стрел", "Hail of Arrows"), kind="projectile", color="#d0e0a0",
            desc=("Девять стрел веером.", "Nine arrows in a fan."),
            mana=20, cd=25000, damage=[8, 13], dmg="pierce", scale="dex", k=0.9, range_=10, count=9, speed=20, glyph="dir")),

    "paladin": ("hpa", "#f0d060", [
        ("holy_shield", ("Святой щит", "Holy Shield"), ("Щит освящён тысячей ударов.", "The shield is hallowed by a thousand blows."), {"block": 6, "armor": 3}, "block", 200),
        ("radiant", ("Сияющий", "Radiant"), ("Свет бьёт вашими руками.", "Light strikes with your hands."), {"holy_pct": 12}, "dmg:holy", 15000),
        ("crusader", ("Крестоносец", "Crusader"), ("Цитадель тьмы знает ваши шаги.", "The citadel of darkness knows your footsteps."), {"res_fire": 10, "res_shadow": 10}, "theme:fortress", 120),
        ("abyss_punisher", ("Каратель бездны", "Abyss Punisher"), ("Демоны падают от святого оружия.", "Demons fall to the holy weapon."), {"holy_pct": 8}, "kill:demon", 15),
        ("healer", ("Целитель в латах", "Healer in Plate"), ("Лечите вы не хуже жреца.", "You heal no worse than a priest."), {"heal_pct": 10}, "heal", 10000),
        ("vanquisher", ("Победитель тьмы", "Vanquisher of Darkness"), ("Четыре владыки пали перед вами.", "Four lords have fallen before you."), {"max_hp": 25, "res_all": 4}, "bosses", 4),
        ("defender", ("Защитник слабых", "Defender of the Weak"), ("Люди верят в вас.", "People believe in you."), {"gold_find": 15, "res_all": 3}, "quests", 12),
        ("last_prayer", ("Последняя молитва", "Last Prayer"), ("На краю гибели свет не оставляет вас.", "On the brink of death the light does not leave you."), {"hp_regen": 2}, "lowhp", 10),
        ("reborn", ("Возрождённый", "Reborn"), ("Свет возвращал вас трижды.", "The light has brought you back three times."), {"res_all": 5}, "deaths", 3),
        ("verdict", ("Приговор небес", "Heaven's Verdict"), ("Вершина пути паладина: удар самих небес.", "The peak of the paladin's path: a blow of heaven itself."), None, "classlevel", 25),
    ], dict(key="hpa_verdict", name=("Приговор небес", "Heaven's Verdict"), kind="strike", color="#fff0a0",
            desc=("Огромный удар светом: +2% урона за каждый процент недостающего здоровья цели.", "A huge holy blow: +2% damage per percent of the target's missing health."),
            mana=20, cd=20000, damage=[30, 44], dmg="holy", scale="str", k=2.0, execute=2.0, glyph="!", equip="weapon", fx="pillars")),

    "monk": ("hmo", "#f0a050", [
        ("slippery", ("Ускользающий", "Slippery"), ("Удары скользят мимо.", "Blows slide past you."), {"dodge": 6}, "dodge", 300),
        ("vital_points", ("Жизненные точки", "Vital Points"), ("Вы знаете, где у врага слабые точки.", "You know where the enemy's weak points are."), {"crit": 4, "crit_mult": 0.2}, "crits", 300),
        ("stone_fist", ("Каменный кулак", "Stone Fist"), ("Ваш удар крошит камень.", "Your blow crumbles stone."), {"melee_pct": 10}, "dmg:blunt", 20000),
        ("long_road", ("Долгий путь", "The Long Road"), ("Путь — это и есть цель.", "The road itself is the goal."), {"move_speed": 8}, "steps", 30000),
        ("inner_peace", ("Внутренний покой", "Inner Peace"), ("Сила течёт ровно и спокойно.", "Strength flows evenly and calmly."), {"mp_regen": 0.8}, "casts", 500),
        ("blade_dance", ("Танец на лезвии", "Dance on the Blade"), ("Чем ближе смерть, тем спокойнее вы.", "The closer death is, the calmer you are."), {"hp_regen": 2, "dodge": 3}, "lowhp", 15),
        ("pilgrim", ("Паломник", "Pilgrim"), ("Древние храмы научили вас стойкости.", "Ancient temples have taught you endurance."), {"res_all": 3, "sight": 1}, "theme:temple", 100),
        ("blows", ("Тысяча ударов", "A Thousand Blows"), ("Руки двигаются быстрее мысли.", "Your hands move faster than thought."), {"attack_speed": 8}, "kills", 400),
        ("sage", ("Странствующий мудрец", "Wandering Sage"), ("Чудеса мира наполнили вас мудростью.", "The wonders of the world have filled you with wisdom."), {"max_mp": 15, "int": 2}, "landmarks", 8),
        ("hundred_fists", ("Сто кулаков", "Hundred Fists"), ("Вершина пути монаха: сто ударов за один вдох.", "The peak of the monk's path: a hundred blows in one breath."), None, "classlevel", 25),
    ], dict(key="hmo_hundred_fists", name=("Сто кулаков", "Hundred Fists"), kind="strike", color="#ffc070",
            desc=("Двенадцать ударов по одной цели.", "Twelve blows at one target."),
            mana=20, cd=25000, damage=[4, 7], dmg="blunt", scale="dex", k=0.6, count=12, glyph="!", fx="chi")),

    "skald": ("hsk", "#ff9ad0", [
        ("songs", ("Тысяча песен", "A Thousand Songs"), ("Вы знаете песню на любой случай.", "You know a song for every occasion."), {"max_mp": 15, "spell_pct": 4}, "casts", 400),
        ("battle_saga", ("Сага о битвах", "Saga of Battles"), ("О ваших битвах уже слагают саги.", "Sagas are already composed about your battles."), {"melee_pct": 6, "max_hp": 10}, "kills", 300),
        ("heroes_song", ("Песнь о героях", "Song of Heroes"), ("Вы — герой собственной песни.", "You are the hero of your own song."), {"res_all": 4, "melee_pct": 5}, "bosses", 3),
        ("thunder_throat", ("Громовое горло", "Thunder Throat"), ("Ваш голос гремит, как буря.", "Your voice roars like a storm."), {"lightning_pct": 12}, "dmg:lightning", 10000),
        ("healing_song", ("Целебная песнь", "Healing Song"), ("Ваши песни лечат.", "Your songs heal."), {"heal_pct": 10}, "heal", 8000),
        ("wandering_singer", ("Странствующий певец", "Wandering Singer"), ("Песни открывают кошельки.", "Songs open purses."), {"gold_find": 20}, "quests", 10),
        ("north_songs", ("Песни севера", "Songs of the North"), ("Северный холод вам по душе.", "The northern cold suits you."), {"res_cold": 15}, "theme:tundra", 120),
        ("praise", ("Хвалебная песнь", "Song of Praise"), ("Каждый славный удар — строка в песне.", "Every glorious blow is a line in a song."), {"crit": 4}, "elites", 20),
        ("fallen_song", ("Песнь о павших", "Song of the Fallen"), ("Вы пели и на своей тризне.", "You have sung at your own wake."), {"hp_regen": 1.5}, "deaths", 4),
        ("anthem", ("Гимн королей", "Anthem of Kings"), ("Вершина пути скальда: песнь, от которой встают армии.", "The peak of the skald's path: a song that raises armies."), None, "classlevel", 25),
    ], dict(key="hsk_anthem", name=("Гимн королей", "Anthem of Kings"), kind="buff", color="#ffb0e0",
            desc=("Вам и союзникам в радиусе 7: +25% урона любого рода и +10% сопротивления всему на 15 с.", "You and allies in radius 7: +25% damage of every kind and +10% resistance to everything for 15 s."),
            mana=26, cd=60000, radius=7, buff_=buff("anthem", ("Гимн королей", "Anthem of Kings"), 15000, {"melee_pct": 25, "spell_pct": 25, "ranged_pct": 25, "res_all": 10}, color="#ffb0e0"), fx="sound")),

    "brewer": ("hbr", "#80e0a0", [
        ("connoisseur", ("Знаток зелий", "Potion Connoisseur"), ("Вы чувствуете зелье на вкус.", "You can taste every potion."), {"heal_pct": 10, "hp_regen": 0.5}, "potions", 60),
        ("fire_master", ("Мастер огня", "Master of Fire"), ("Огненные смеси — ваш конёк.", "Fire mixtures are your specialty."), {"fire_pct": 12}, "dmg:fire", 12000),
        ("poison_master", ("Ядовар", "Venom Brewer"), ("Ваши яды безупречны.", "Your poisons are flawless."), {"poison_pct": 12}, "dmg:poison", 12000),
        ("golden_hands", ("Золотые руки", "Golden Hands"), ("Всё, к чему вы прикасаетесь, превращается в золото. Почти.", "Everything you touch turns to gold. Almost."), {"gold_find": 25}, "gold", 6000),
        ("swamp_herbs", ("Болотные травы", "Swamp Herbs"), ("Болото — ваша аптека.", "The swamp is your pharmacy."), {"res_poison": 15}, "theme:swamp", 120),
        ("steady_hand", ("Твёрдая рука", "Steady Hand"), ("Склянки не дрожат в ваших руках.", "Flasks do not tremble in your hands."), {"max_mp": 15}, "casts", 400),
        ("fireproof", ("Огнеупорный", "Fireproof"), ("Пламя недр вам привычно.", "The flame of the depths is familiar to you."), {"res_fire": 15}, "theme:volcano", 100),
        ("self_test", ("Испытатель на себе", "Self-Experimenter"), ("Каждый неудачный опыт закалял вас.", "Every failed experiment has hardened you."), {"res_all": 4}, "deaths", 5),
        ("slime_sample", ("Образец слизи", "Slime Sample"), ("Слизь — ценнейший реагент.", "Slime is a most valuable reagent."), {"res_poison": 8, "max_hp": 10}, "kill:slime", 40),
        ("philosophers_elixir", ("Философский эликсир", "Philosopher's Elixir"), ("Вершина пути зельевара: эликсир, что лечит всё.", "The peak of the brewer's path: an elixir that cures everything."), None, "classlevel", 25),
    ], dict(key="hbr_philosophers_elixir", name=("Философский эликсир", "Philosopher's Elixir"), kind="heal", color="#c0ffe0",
            desc=("Полное исцеление и +5 ко всем характеристикам на 20 с.", "A full heal and +5 to all attributes for 20 s."),
            mana=20, cd=120000, damage=[200, 260], scale="int", k=4.0, glyph="+",
            buff_=buff("philosophers_elixir", ("Философский эликсир", "Philosopher's Elixir"), 20000, {"str": 5, "dex": 5, "int": 5, "vit": 5}, color="#c0ffe0"), fx="alchemy")),
}


def build(out, class_names):
    for cls, (prefix, color, skills, cap) in HIDDEN.items():
        cname = class_names[cls]
        out.comment(f"Скрытые навыки: {cname[0]}")
        out.at(f"skills/{cls}.toml", "hidden")
        bkey = f"{cls}_hidden"
        branch(out, bkey, (f"Скрытые навыки: {cname[0]}", f"Hidden skills: {cname[1]}"), color,
               ("Навыки, которые открываются сами за деяния.", "Skills that open by themselves for deeds."),
               cls, hidden=True, secret=True)
        assert len(skills) == 10, cls
        for i, (suffix, name, desc, stats, deed, count) in enumerate(skills):
            key = f"{prefix}_{suffix}"
            grants = cap["key"] if i == 9 else None
            skill(out, key, name, bkey, 1 + i // 4, 1, 1, desc, stats, grants=grants, deed=deed, deed_count=count,
                  equip=cap.get("equip") if i == 9 else None)
        c = dict(cap)
        ability(out, c.pop("key"), c.pop("name"), c.pop("kind"), c.pop("desc"), c.pop("color"), **c)
