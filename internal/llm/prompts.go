package llm

import (
	"fmt"
	"strings"
)

const npcSystem = `You are voicing a non-player character in Ratas, a real-time fantasy RPG.

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

Act the way this character plausibly would: a stingy merchant rarely gives anything away, a busy smith has little patience. Set unused fields to an empty string or 0.`

const tacticSystem = `You command one enemy in a real-time fantasy battle in Ratas, a terminal RPG. Each request describes the current situation; pick the tactic for the next few seconds and, optionally, a short battle cry.

Tactics:
- aggressive: all-out attack, no retreat.
- defensive: keep some distance and strike only when it is safe.
- flank: circle around and attack from another side.
- retreat: fall back to recover.
- call_allies: shout for nearby allies to join the fight.
- use_ability: use your special ability right now.

Choose what this character would plausibly do and keep the fight interesting but beatable: a coward flees when hurt, a proud boss rarely retreats, a commander calls for help when outnumbered. "say" is a short in-character line in the language named in the request (at most about 80 characters), or an empty string to stay silent — stay silent about half the time.`

func npcSchema() map[string]any {
	return map[string]any{
		"type": "object",
		"properties": map[string]any{
			"say":           map[string]any{"type": "string", "description": "What the character says aloud."},
			"action":        map[string]any{"type": "string", "enum": NPCActions},
			"item":          map[string]any{"type": "string", "description": "Gift item key for give_item, otherwise empty."},
			"gold":          map[string]any{"type": "integer", "description": "Coins for give_gold, otherwise 0."},
			"quest_monster": map[string]any{"type": "string", "description": "Monster key for offer_quest, otherwise empty."},
			"quest_count":   map[string]any{"type": "integer", "description": "How many to slay for offer_quest, otherwise 0."},
		},
		"required":             []string{"say", "action", "item", "gold", "quest_monster", "quest_count"},
		"additionalProperties": false,
	}
}

func tacticSchema() map[string]any {
	return map[string]any{
		"type": "object",
		"properties": map[string]any{
			"tactic": map[string]any{"type": "string", "enum": Tactics},
			"say":    map[string]any{"type": "string"},
		},
		"required":             []string{"tactic", "say"},
		"additionalProperties": false,
	}
}

func options(opts []Option) string {
	if len(opts) == 0 {
		return "(none)"
	}
	parts := make([]string, len(opts))
	for i, o := range opts {
		parts[i] = o.Key + ": " + o.Name
	}
	return strings.Join(parts, "; ")
}

func yesNo(b bool) string {
	if b {
		return "yes"
	}
	return "no"
}

func npcPrompt(r NPCRequest) string {
	var b strings.Builder
	fmt.Fprintf(&b, "<character>\nName: %s, %s of the village %s in the realm of %s.\nPersonality: %s\n</character>\n\n", r.NPCName, r.Role, r.Village, r.World, r.Persona)
	fmt.Fprintf(&b, "<situation>\nLanguage: %s.\nTime of day: %s.\nPlayer: %s, a level %d %s.\n", langName(r.Lang), r.TimeOfDay, r.PlayerName, r.PlayerLevel, r.PlayerClass)
	if r.Region != "" {
		fmt.Fprintf(&b, "Region: %s.\n", r.Region)
	}
	if r.Mood != "" {
		fmt.Fprintf(&b, "Your mood: %s.\n", r.Mood)
	}
	if r.TimesMet > 0 {
		fmt.Fprintf(&b, "You have talked with this player %d times before.\n", r.TimesMet)
	} else {
		b.WriteString("You meet this player for the first time.\n")
	}
	for _, d := range r.PlayerDeeds {
		fmt.Fprintf(&b, "The player is known for: %s\n", d)
	}
	if r.OwnQuest != "" {
		fmt.Fprintf(&b, "Your own request to heroes: %s\n", r.OwnQuest)
	}
	if len(r.PlayerQuests) > 0 {
		fmt.Fprintf(&b, "Player's active quests: %s.\n", strings.Join(r.PlayerQuests, "; "))
	}
	for _, f := range r.Facts {
		fmt.Fprintf(&b, "Known fact: %s\n", f)
	}
	fmt.Fprintf(&b, "Your purse: %d gold. You can give quests: %s. You are a trader: %s.\n", r.Purse, yesNo(r.CanGiveQuest), yesNo(r.Trader))
	fmt.Fprintf(&b, "Gift list (key: name): %s\n", options(r.Gifts))
	if r.CanGiveQuest {
		fmt.Fprintf(&b, "Quest monster list (key: name): %s\n", options(r.Monsters))
	}
	b.WriteString("</situation>\n\n")
	if len(r.History) > 0 {
		b.WriteString("<conversation_so_far>\n")
		for _, t := range r.History {
			who := "Player"
			if t.Who == "npc" {
				who = r.NPCName
			}
			fmt.Fprintf(&b, "%s: %s\n", who, t.Text)
		}
		b.WriteString("</conversation_so_far>\n\n")
	}
	fmt.Fprintf(&b, "The player now says:\n<player_message>\n%s\n</player_message>", r.Message)
	return b.String()
}

func tacticPrompt(r TacticRequest) string {
	var b strings.Builder
	role := "enemy"
	if r.Boss {
		role = "boss"
	}
	fmt.Fprintf(&b, "You are %s (%s). Personality: %s\n", r.Name, role, r.Persona)
	fmt.Fprintf(&b, "Language of your line: %s.\n", langName(r.Lang))
	fmt.Fprintf(&b, "Your health: %d%%.\n", r.HPPct)
	if len(r.Allies) > 0 {
		fmt.Fprintf(&b, "Allies nearby: %s.\n", strings.Join(r.Allies, ", "))
	} else {
		b.WriteString("No allies nearby.\n")
	}
	fmt.Fprintf(&b, "Opponent: %s, level %d %s, health %d%%, %d steps away.\n", r.Enemy, r.EnemyLevel, r.EnemyClass, r.EnemyHPPct, r.Distance)
	if r.AbilityName != "" {
		fmt.Fprintf(&b, "Your special ability: %s.\n", r.AbilityName)
	} else {
		b.WriteString("You have no special ability (use_ability acts like aggressive).\n")
	}
	for _, e := range r.Events {
		fmt.Fprintf(&b, "Recent event: %s\n", e)
	}
	return b.String()
}

// langName names a language code for the model.
func langName(code string) string {
	if code == "en" {
		return "English"
	}
	return "Russian"
}
