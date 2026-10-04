package game

import (
	"fmt"
	"slices"
	"strings"

	"ratas/internal/content"
)

// Deeds are counters of what a hero has done: kills of every kind, damage of
// every type, crits, dodges, steps, deaths and more. A hidden skill (a skill
// with a deed) opens by itself once its deed is done, for a hero of its class.
//
// Deed keys:
//
//	kills, kill:<monster>, theme:<dungeon theme or biome>, bosses, elites,
//	night (kills at night), lowhp (kills below 25% health), deaths, gold,
//	crits, dmg:<damage type>, heal, dodge, block, casts, summons, potions,
//	steps, depth (deepest dungeon floor), uniques, quests, landmarks,
//	level, classlevel (of the skill's class)
//
// DeedText describes a deed for players: "Убить 100: нежить склепов".
func DeedText(deed string, count int) string {
	kind, arg, _ := strings.Cut(deed, ":")
	switch kind {
	case "kills":
		return fmt.Sprintf("Убить %d врагов", count)
	case "kill":
		name := arg
		if m := content.Monster(arg); m != nil {
			name = m.Name
		}
		return fmt.Sprintf("Убить %d: %s", count, name)
	case "theme":
		return fmt.Sprintf("Убить %d: %s", count, themeFoes(arg))
	case "bosses":
		return fmt.Sprintf("Сразить %d боссов подземелий", count)
	case "elites":
		return fmt.Sprintf("Убить %d элитных врагов или чемпионов", count)
	case "night":
		return fmt.Sprintf("Убить %d врагов ночью", count)
	case "lowhp":
		return fmt.Sprintf("Убить %d врагов, когда у вас меньше четверти здоровья", count)
	case "deaths":
		return fmt.Sprintf("Погибнуть %d раз и вернуться", count)
	case "gold":
		return fmt.Sprintf("Собрать %d золота", count)
	case "crits":
		return fmt.Sprintf("Нанести %d критических ударов", count)
	case "dmg":
		return fmt.Sprintf("Нанести %d урона: %s", count, DamageTypeName(arg))
	case "heal":
		return fmt.Sprintf("Восстановить умениями %d здоровья", count)
	case "dodge":
		return fmt.Sprintf("Увернуться от %d ударов", count)
	case "block":
		return fmt.Sprintf("Заблокировать %d ударов", count)
	case "casts":
		return fmt.Sprintf("Применить умения %d раз", count)
	case "summons":
		return fmt.Sprintf("Призвать союзников %d раз", count)
	case "potions":
		return fmt.Sprintf("Выпить %d зелий", count)
	case "steps":
		return fmt.Sprintf("Пройти %d шагов", count)
	case "depth":
		return fmt.Sprintf("Спуститься на %d-й ярус подземелья", count)
	case "uniques":
		return fmt.Sprintf("Выполнить %d заданий уникальных персонажей", count)
	case "quests":
		return fmt.Sprintf("Выполнить %d поручений", count)
	case "landmarks":
		return fmt.Sprintf("Найти %d достопримечательностей", count)
	case "level":
		return fmt.Sprintf("Достичь %d уровня", count)
	case "classlevel":
		return fmt.Sprintf("Достичь %d уровня класса", count)
	}
	return fmt.Sprintf("%s ×%d", deed, count)
}

// themeFoes names the creatures of a dungeon theme or a biome.
func themeFoes(theme string) string {
	switch theme {
	case "crypt":
		return "нежить склепов"
	case "cave":
		return "обитатели пещер"
	case "ice":
		return "твари льдов"
	case "volcano", "ash":
		return "огненные твари"
	case "temple", "desert":
		return "твари песков и гробниц"
	case "fortress":
		return "демоны и культисты"
	case "forest":
		return "лесные звери"
	case "plains", "hills":
		return "звери и разбойники равнин"
	case "swamp":
		return "болотные твари"
	case "tundra":
		return "твари севера"
	case "cursed":
		return "порождения проклятых земель"
	case "sand":
		return "твари побережий"
	}
	return theme
}

// DeedKnown reports whether a deed key is valid (for content validation).
func DeedKnown(deed string) bool {
	kind, arg, _ := strings.Cut(deed, ":")
	switch kind {
	case "kills", "bosses", "elites", "night", "lowhp", "deaths", "gold", "crits", "heal", "dodge", "block",
		"casts", "summons", "potions", "steps", "depth", "uniques", "quests", "landmarks", "level", "classlevel":
		return arg == ""
	case "kill":
		return content.Monster(arg) != nil
	case "theme":
		return arg != ""
	case "dmg":
		return content.DamageType(arg) != nil
	}
	return false
}

// deed adds to a counter of a hero (projectiles and summons count for their owner).
func (g *Game) deed(e *Entity, key string, n int) {
	if e != nil && e.Kind == KProjectile && e.Proj != nil {
		e = g.Entities[e.Proj.Owner]
	}
	if e == nil || e.Player == nil || n <= 0 {
		return
	}
	p := e.Player
	if p.Deeds == nil {
		p.Deeds = map[string]int{}
	}
	p.Deeds[key] += n
	p.deedCheck = true
}

// deedMax raises a counter that keeps the best value (the deepest floor).
func (g *Game) deedMax(e *Entity, key string, v int) {
	if e == nil || e.Player == nil {
		return
	}
	p := e.Player
	if p.Deeds == nil {
		p.Deeds = map[string]int{}
	}
	if v > p.Deeds[key] {
		p.Deeds[key] = v
		p.deedCheck = true
	}
}

// DeedProgress is how far a hero is in the deed of a hidden skill.
func DeedProgress(p *PlayerState, sd *content.SkillDef) int {
	switch sd.Deed {
	case "level":
		return p.Level
	case "classlevel":
		if b := content.Branch(sd.Branch); b != nil {
			return ClassLevel(p, b.Class)
		}
		return 0
	case "landmarks":
		return len(p.Found)
	}
	return p.Deeds[sd.Deed]
}

// HiddenFor reports whether a hidden skill belongs to a hero: its class is
// one the hero develops (or it belongs to no class).
func HiddenFor(p *PlayerState, sd *content.SkillDef) bool {
	if sd.Deed == "" {
		return false
	}
	b := content.Branch(sd.Branch)
	return b != nil && (b.Class == "" || p.HasClass(b.Class))
}

// checkDeeds opens the hidden skills whose deeds are done.
func (g *Game) checkDeeds(e *Entity) {
	p := e.Player
	p.deedCheck = false
	for i := range content.Skills() {
		sd := &content.Skills()[i]
		if !HiddenFor(p, sd) || p.Skills[sd.Key] > 0 || DeedProgress(p, sd) < sd.DeedCount {
			continue
		}
		p.Skills[sd.Key] = sd.MaxRank
		if sd.Grants != "" && !slices.Contains(p.Abilities, sd.Grants) {
			g.unlockAbility(e, sd.Grants)
		}
		e.Recalc()
		p.Dirty = true
		g.Log(e, "#ff80ff", "Скрытый навык открыт: %s! (%s)", sd.Name, DeedText(sd.Deed, sd.DeedCount))
		g.FX(e.Level, e.Pos, sd.Name+"!", 0, "#ff80ff", 2200)
	}
}

// killDeeds counts a slain monster for a hero who shared in the kill;
// slayer is the one who struck the blow.
func (g *Game) killDeeds(p, m *Entity, def *content.MonsterDef, slayer bool) {
	g.deed(p, "kills", 1)
	g.deed(p, "kill:"+m.Monster.Def, 1)
	if def != nil {
		for _, t := range def.Themes {
			g.deed(p, "theme:"+t, 1)
		}
		if def.Boss {
			g.deed(p, "bosses", 1)
		}
		if def.Elite || m.Monster.Champion != "" {
			g.deed(p, "elites", 1)
		}
	}
	if l := g.Levels[m.Level]; l != nil && l.Lit && g.IsNight() {
		g.deed(p, "night", 1)
	}
	if slayer && p.MaxHP > 0 && p.HP < p.MaxHP/4 {
		g.deed(p, "lowhp", 1)
	}
}
