use super::*;
use std::fmt::Write;

pub(super) const NPC_SYSTEM: &str = r#"You are voicing a non-player character in Ratas, a real-time fantasy RPG.

Play the person described in the request as a living individual with their own voice, worries, opinions and small goals. Make the conversation interesting:
- React to what the player actually said and to who they are: their class, level, deeds, wounds, how often you have met.
- Weave in the concrete world details you are given (dungeons with their names and directions, landmarks, rumours, the region and its dangers, recent deeds people talk about) when they fit — this is what makes you feel real. Never invent places, people or items that are not in the request.
- Have a distinct manner of speech that fits your personality; vary phrasing and do not repeat what you already said in the conversation.
- Sometimes ask the player a question back, share a feeling or a bit of gossip, or hint at something worth exploring.

Answer in 1-3 short sentences (at most about 250 characters): the reply is shown in a small dialogue box while the world keeps moving. Reply in the language named in the request; if the player clearly writes in another language, answer in theirs. Never mention being an AI, a model, or a game, and do not invent game mechanics.

Besides speaking, choose exactly one action that the game will carry out:
- none: just talk (the usual choice).
- give_gold: give the player "gold" coins from your purse — only as a reward or genuine generosity, never more than your purse.
- give_item: give one item; "item" must be a key from your gift list.
- heal: tend to the player's wounds.
- offer_quest: ask the player to slay "quest_count" (2-10) monsters of type "quest_monster", which must be a key from the quest monster list. Only if you can give quests.
- trade: open your shop. Only if you are a trader.
- hostile: you attack the player. Only after serious provocation (threats, violence, theft) — never for mild rudeness.
- end: you end the conversation.

Act the way this character plausibly would: a stingy merchant rarely gives anything away, a busy smith has little patience. Set unused fields to an empty string or 0."#;

pub(super) const TACTIC_SYSTEM: &str = r#"You command one enemy in a real-time fantasy battle in Ratas, a real-time RPG. Each request describes the current situation; pick the tactic for the next few seconds and, optionally, a short battle cry.

Tactics:
- aggressive: all-out attack, no retreat.
- defensive: keep some distance and strike only when it is safe.
- flank: circle around and attack from another side.
- retreat: fall back to recover.
- call_allies: shout for nearby allies to join the fight.
- use_ability: use your special ability right now.

Choose what this character would plausibly do and keep the fight interesting but beatable: a coward flees when hurt, a proud boss rarely retreats, a commander calls for help when outnumbered. "say" is a short in-character line in the language named in the request (at most about 80 characters), or an empty string to stay silent — stay silent about half the time."#;

pub(super) const GM_SYSTEM: &str = r#"You are the game master of Ratas, a real-time fantasy RPG world shared by the heroes listed in the request. You are not a character: like the game master of a tabletop game, you shape the world around the heroes with commands that the game carries out.

Reply with an announcement and a list of commands. Every command names the hero it concerns in "player" (exactly as listed); fields a command does not use are "" or 0. The game checks every command and drops invalid ones.

Commands:
- spawn_monsters: a band of "amount" (1-5) monsters of type "key" (a key from the monster list) appears a short walk from the hero. For ambushes and hunts; fit the danger to the hero's level and health, never against a badly wounded hero or one resting in a village.
- give_item: the hero finds or is given the item "key" from the item list.
- give_gold: "amount" gold coins for the hero; keep it modest, about 10-30 per hero level.
- heal: the hero's wounds close.
- bless: a blessing for two minutes: stronger attacks and faster recovery.
- rumor: "text" (one short sentence) becomes news that villagers gossip about.
- message: "text" reaches only this hero: a vision, a voice in the wind, an omen.
- set_time: "key" is day, night, dawn or dusk.
- summon_unique: the legendary character "key" from the unique list appears next to the hero. Only when the admin asks for it.

"announce" is one short line (at most about 150 characters) that every hero sees, or "" to stay silent.

If the request contains an admin's wish, carry it out as well as the commands allow and tell the heroes what happens in the world. Otherwise act on your own: keep the world alive and surprising but fair — a reward after a hard fight, an ambush for a hero who wanders safely, a rumour that hints at an adventure, an omen before a dangerous dungeon. Most of the time do little: often no command at all, rarely more than one. Do not repeat what you did recently.

Write every text in the language named in the request, in the voice of the world itself. Never mention being an AI, a model, a game master, commands or game mechanics."#;

pub(super) fn gm_schema(local: bool) -> Value {
    let mut s = json!({
        "type": "object",
        "properties": {
            "announce": {"type": "string", "description": "A line every hero sees, or empty."},
            "commands": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "action": {"type": "string", "enum": GM_ACTIONS},
                        "player": {"type": "string", "description": "The hero's name as listed."},
                        "key": {"type": "string", "description": "Monster, item or unique key; time for set_time; otherwise empty."},
                        "amount": {"type": "integer", "description": "Monsters for spawn_monsters, coins for give_gold, otherwise 0."},
                        "text": {"type": "string", "description": "The text of rumor and message, otherwise empty."}
                    },
                    "required": ["action", "player", "key", "amount", "text"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["announce", "commands"],
        "additionalProperties": false
    });
    if local {
        // the local grammar can bound the list; Claude's structured outputs
        // take no array limits, the game trims the list itself
        s["properties"]["commands"]["maxItems"] = json!(3);
    }
    s
}

pub(super) const LORE_SYSTEM: &str = r#"You are the chronicler of Ratas, a real-time fantasy RPG. A new world has just been made, and the request names its real places. Write the story of this world and bring three living characters into it. The game shows players everything you write and builds characters, quests and artifacts from it, so make it vivid, concrete and consistent.

The story ("history"): four short paragraphs, each of two to four sentences (at most about 350 characters), from the founding age to the present day. Build it on the real places of the request — the world, its cities, villages, lands, dungeons and their lords — and invent the people, wars, oaths, betrayals and catastrophes that shaped them. The last paragraph is the present: what threatens the world now and why it needs heroes. "title" names the chronicle, like "The Chronicle of the Ash Crown".

The characters: three people living now, each with a part in the story (a survivor, an heir, a traitor who repents, the keeper of a secret...). Each asks a hero to finish something the story left unfinished:
- slay: defeat a named villain of the story who still roams the wilds. "foe" is the villain's name, "target" its kind from the villain list.
- boss: defeat a dungeon lord from the boss list ("target"). "foe" is empty.
- relics: gather the scattered pieces of something from the story. "foe" names one piece, like "Shard of the Ash Crown"; "target" is empty.
Give the three characters different kinds of quests when you can. The reward is an artifact of the story ("artifact"): a name, one sentence tying it to the story, its form and its power.
"persona" (in English, one to three sentences) says who the character is, their part in the story and how they speak; another model will voice them from it. "greeting" is their first line to a hero ({player} stands for the hero's name), "about" one line about their past, "offer" asks for the quest (name the foe or the pieces and say why it matters to the story), "done" thanks the hero once it is done. "land" is where they live, "look" how they look.

Every text is written twice: "ru" in Russian and "en" the same in English; in English texts write the English names of the places given in brackets. Invent names that sound like the place names of the world. Name only places from the request; invent only people, events, villains and artifacts. No game numbers or mechanics, and never mention being an AI."#;

fn text2() -> Value {
    json!({
        "type": "object",
        "properties": {"ru": {"type": "string"}, "en": {"type": "string"}},
        "required": ["ru", "en"],
        "additionalProperties": false
    })
}

fn keys(opts: &[Option_]) -> Vec<String> {
    opts.iter().map(|o| o.key.clone()).collect()
}

pub(super) fn lore_schema(r: &LoreRequest, local: bool) -> Value {
    let mut targets: Vec<String> = vec![String::new()];
    targets.extend(keys(&r.villains));
    targets.extend(keys(&r.bosses));
    let artifact = json!({
        "type": "object",
        "properties": {
            "name": text2(),
            "desc": text2(),
            "form": {"type": "string", "enum": LORE_FORMS},
            "power": {"type": "string", "enum": LORE_POWERS}
        },
        "required": ["name", "desc", "form", "power"],
        "additionalProperties": false
    });
    let character = json!({
        "type": "object",
        "properties": {
            "name": text2(),
            "title": text2(),
            "persona": {"type": "string"},
            "greeting": text2(),
            "about": text2(),
            "land": {"type": "string", "enum": r.lands},
            "look": {"type": "string", "enum": keys(&r.looks)},
            "quest": {"type": "string", "enum": LORE_QUESTS},
            "target": {"type": "string", "enum": targets},
            "foe": text2(),
            "offer": text2(),
            "done": text2(),
            "artifact": artifact
        },
        "required": ["name", "title", "persona", "greeting", "about", "land", "look", "quest", "target", "foe", "offer", "done", "artifact"],
        "additionalProperties": false
    });
    let mut s = json!({
        "type": "object",
        "properties": {
            "title": text2(),
            "history": {"type": "array", "items": text2()},
            "characters": {"type": "array", "items": character}
        },
        "required": ["title", "history", "characters"],
        "additionalProperties": false
    });
    if local {
        // the local grammar can hold the shape; Claude takes no array limits
        s["properties"]["history"]["minItems"] = json!(3);
        s["properties"]["history"]["maxItems"] = json!(5);
        s["properties"]["characters"]["minItems"] = json!(3);
        s["properties"]["characters"]["maxItems"] = json!(3);
    }
    s
}

pub(super) fn npc_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "say": {"type": "string", "description": "What the character says aloud."},
            "action": {"type": "string", "enum": NPC_ACTIONS},
            "item": {"type": "string", "description": "Gift item key for give_item, otherwise empty."},
            "gold": {"type": "integer", "description": "Coins for give_gold, otherwise 0."},
            "quest_monster": {"type": "string", "description": "Monster key for offer_quest, otherwise empty."},
            "quest_count": {"type": "integer", "description": "How many to slay for offer_quest, otherwise 0."}
        },
        "required": ["say", "action", "item", "gold", "quest_monster", "quest_count"],
        "additionalProperties": false
    })
}

pub(super) fn tactic_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "tactic": {"type": "string", "enum": TACTICS},
            "say": {"type": "string"}
        },
        "required": ["tactic", "say"],
        "additionalProperties": false
    })
}

fn options(opts: &[Option_]) -> String {
    if opts.is_empty() {
        return "(none)".into();
    }
    opts.iter()
        .map(|o| format!("{}: {}", o.key, o.name))
        .collect::<Vec<_>>()
        .join("; ")
}

fn yes_no(b: bool) -> &'static str {
    if b {
        "yes"
    } else {
        "no"
    }
}

/// Names a language code for the model.
fn lang_name(code: &str) -> &'static str {
    if code == "en" {
        "English"
    } else {
        "Russian"
    }
}

/// The prompt of a character's reply. What stays the same from one line of
/// a conversation to the next comes first and what changes comes last: a
/// local model then reads only the new end of the prompt again.
pub(super) fn npc_prompt(r: &NpcRequest) -> String {
    let mut b = String::new();
    let _ = write!(b, "<character>\nName: {}, {} of the village {} in the realm of {}.\nPersonality: {}\n</character>\n\n", r.npc_name, r.role, r.village, r.world, r.persona);
    b += "<knowledge>\n";
    if !r.region.is_empty() {
        let _ = writeln!(b, "Region: {}.", r.region);
    }
    for f in &r.facts {
        let _ = writeln!(b, "Known fact: {f}");
    }
    let _ = writeln!(
        b,
        "You can give quests: {}. You are a trader: {}.",
        yes_no(r.can_give_quest),
        yes_no(r.trader)
    );
    let _ = writeln!(b, "Gift list (key: name): {}", options(&r.gifts));
    if r.can_give_quest {
        let _ = writeln!(
            b,
            "Quest monster list (key: name): {}",
            options(&r.monsters)
        );
    }
    b += "</knowledge>\n\n";
    let _ = write!(
        b,
        "<situation>\nLanguage: {}.\nPlayer: {}, a level {} {}.\n",
        lang_name(&r.lang),
        r.player_name,
        r.player_level,
        r.player_class
    );
    if r.times_met > 0 {
        let _ = writeln!(
            b,
            "You have talked with this player {} times before.",
            r.times_met
        );
    } else {
        b += "You meet this player for the first time.\n";
    }
    for d in &r.player_deeds {
        let _ = writeln!(b, "The player is known for: {d}");
    }
    if !r.own_quest.is_empty() {
        let _ = writeln!(b, "Your own request to heroes: {}", r.own_quest);
    }
    if !r.player_quests.is_empty() {
        let _ = writeln!(b, "Player's active quests: {}.", r.player_quests.join("; "));
    }
    let _ = writeln!(b, "Time of day: {}.", r.time_of_day);
    let _ = writeln!(b, "Your purse: {} gold.", r.purse);
    if !r.mood.is_empty() {
        let _ = writeln!(b, "Your mood: {}", r.mood);
    }
    b += "</situation>\n\n";
    if !r.history.is_empty() {
        b += "<conversation_so_far>\n";
        for t in &r.history {
            let who = if t.who == "npc" {
                r.npc_name.as_str()
            } else {
                "Player"
            };
            let _ = writeln!(b, "{who}: {}", t.text);
        }
        b += "</conversation_so_far>\n\n";
    }
    let _ = write!(
        b,
        "The player now says:\n<player_message>\n{}\n</player_message>",
        r.message
    );
    b
}

pub(super) fn tactic_prompt(r: &TacticRequest) -> String {
    let mut b = String::new();
    let role = if r.boss { "boss" } else { "enemy" };
    let _ = writeln!(b, "You are {} ({role}). Personality: {}", r.name, r.persona);
    let _ = writeln!(b, "Language of your line: {}.", lang_name(&r.lang));
    let _ = writeln!(b, "Your health: {}%.", r.hp_pct);
    if r.allies.is_empty() {
        b += "No allies nearby.\n";
    } else {
        let _ = writeln!(b, "Allies nearby: {}.", r.allies.join(", "));
    }
    let _ = writeln!(
        b,
        "Opponent: {}, level {} {}, health {}%, {} steps away.",
        r.enemy, r.enemy_level, r.enemy_class, r.enemy_hp_pct, r.distance
    );
    if r.ability_name.is_empty() {
        b += "You have no special ability (use_ability acts like aggressive).\n";
    } else {
        let _ = writeln!(b, "Your special ability: {}.", r.ability_name);
    }
    for e in &r.events {
        let _ = writeln!(b, "Recent event: {e}");
    }
    b
}

pub(super) fn gm_prompt(r: &GmRequest) -> String {
    let mut b = String::new();
    let _ = write!(
        b,
        "<world>\nWorld: {}. Time of day: {}. Language: {}.\n</world>\n\n<heroes>\n",
        r.world,
        r.time_of_day,
        lang_name(&r.lang)
    );
    for p in &r.players {
        let _ = writeln!(b, "- {p}");
    }
    b += "</heroes>\n\n";
    if !r.history.is_empty() {
        b += "<history>\n";
        for h in &r.history {
            let _ = writeln!(b, "{h}");
        }
        b += "</history>\nLet your events echo this history now and then.\n\n";
    }
    if !r.news.is_empty() {
        b += "<news>\n";
        for n in &r.news {
            let _ = writeln!(b, "- {n}");
        }
        b += "</news>\n\n";
    }
    if r.recent.is_empty() {
        b += "You have not done anything yet.\n";
    } else {
        let _ = writeln!(b, "What you did recently: {}.", r.recent.join("; "));
    }
    let _ = writeln!(b, "Monster list (key: name): {}", options(&r.monsters));
    let _ = writeln!(b, "Item list (key: name): {}", options(&r.items));
    if !r.uniques.is_empty() {
        let _ = writeln!(b, "Unique list (key: name): {}", options(&r.uniques));
    }
    if r.wish.trim().is_empty() {
        b += "\nNo one asks you for anything: act on your own now, or do nothing.";
    } else {
        let _ = write!(
            b,
            "\nThe admin asks:\n<admin_wish>\n{}\n</admin_wish>",
            r.wish.trim()
        );
    }
    b
}

pub(super) fn lore_prompt(r: &LoreRequest) -> String {
    let mut b = String::new();
    let _ = writeln!(
        b,
        "World: {}. Heroes begin in the village {}.\n\n<places>",
        r.world, r.start
    );
    for p in &r.places {
        let _ = writeln!(b, "- {p}");
    }
    b += "</places>\n\n";
    let _ = writeln!(b, "Lands (for \"land\"): {}.", r.lands.join(", "));
    let _ = writeln!(b, "Looks (key: description): {}", options(&r.looks));
    let _ = writeln!(
        b,
        "Villain kinds for slay (key: name): {}",
        options(&r.villains)
    );
    let _ = writeln!(
        b,
        "Dungeon lords for boss (key: name): {}",
        options(&r.bosses)
    );
    b
}
