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
    opts.iter().map(|o| format!("{}: {}", o.key, o.name)).collect::<Vec<_>>().join("; ")
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

pub(super) fn npc_prompt(r: &NpcRequest) -> String {
    let mut b = String::new();
    let _ = write!(b, "<character>\nName: {}, {} of the village {} in the realm of {}.\nPersonality: {}\n</character>\n\n", r.npc_name, r.role, r.village, r.world, r.persona);
    let _ = write!(b, "<situation>\nLanguage: {}.\nTime of day: {}.\nPlayer: {}, a level {} {}.\n", lang_name(&r.lang), r.time_of_day, r.player_name, r.player_level, r.player_class);
    if !r.region.is_empty() {
        let _ = writeln!(b, "Region: {}.", r.region);
    }
    if !r.mood.is_empty() {
        let _ = writeln!(b, "Your mood: {}.", r.mood);
    }
    if r.times_met > 0 {
        let _ = writeln!(b, "You have talked with this player {} times before.", r.times_met);
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
    for f in &r.facts {
        let _ = writeln!(b, "Known fact: {f}");
    }
    let _ = writeln!(b, "Your purse: {} gold. You can give quests: {}. You are a trader: {}.", r.purse, yes_no(r.can_give_quest), yes_no(r.trader));
    let _ = writeln!(b, "Gift list (key: name): {}", options(&r.gifts));
    if r.can_give_quest {
        let _ = writeln!(b, "Quest monster list (key: name): {}", options(&r.monsters));
    }
    b += "</situation>\n\n";
    if !r.history.is_empty() {
        b += "<conversation_so_far>\n";
        for t in &r.history {
            let who = if t.who == "npc" { r.npc_name.as_str() } else { "Player" };
            let _ = writeln!(b, "{who}: {}", t.text);
        }
        b += "</conversation_so_far>\n\n";
    }
    let _ = write!(b, "The player now says:\n<player_message>\n{}\n</player_message>", r.message);
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
    let _ = writeln!(b, "Opponent: {}, level {} {}, health {}%, {} steps away.", r.enemy, r.enemy_level, r.enemy_class, r.enemy_hp_pct, r.distance);
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
