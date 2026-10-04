package game

import (
	"math"
	"slices"

	"ratas/internal/content"
	"ratas/internal/world"
)

// itemEffect applies the buff and special effect of a used consumable.
func (g *Game) itemEffect(e *Entity, d *content.ItemDef) {
	p := e.Player
	switch d.Effect {
	case "cleanse":
		g.cleanse(e)
	case "return":
		ow := g.Levels["overworld"]
		g.FX(e.Level, e.Pos, "", '*', "#e0d0a0", 400)
		g.changeLevel(e, ow, g.Start)
		g.FX(e.Level, e.Pos, "", '*', "#e0d0a0", 600)
	case "respec":
		g.respec(e)
	case "xp":
		g.GiveXP(e, max(1, int(float64(XPForLevel(p.Level))*d.Amount/100)))
	}
	if d.Buff != nil {
		g.applyBuff(e, d.Buff, e.ID)
		g.FX(e.Level, e.Pos, d.Buff.Name+"!", 0, d.Buff.Color, 1000)
	}
}

func (g *Game) canReturn(e *Entity) bool {
	if e.Dead {
		return false
	}
	if e.Level == "overworld" && e.Pos.Dist(g.Start) < 6 {
		g.Log(e, "#808080", "Вы и так у колодца.")
		return false
	}
	return true
}

// cleanse removes harmful effects applied by others.
func (g *Game) cleanse(e *Entity) {
	kept := e.Buffs[:0]
	removed := 0
	for _, b := range e.Buffs {
		if b.Source != e.ID && isDebuff(&b.Def) {
			removed++
			continue
		}
		kept = append(kept, b)
	}
	e.Buffs = kept
	e.Recalc()
	if removed > 0 {
		g.FX(e.Level, e.Pos, "очищение", 0, "#c0ffc0", 900)
	}
}

// respec returns every skill and attribute point, forgets subclasses and
// additional classes. Secret skills taught by unique masters stay.
func (g *Game) respec(e *Entity) {
	p := e.Player
	kept := map[string]int{}
	for k, r := range p.Skills {
		if sd := content.Skill(k); sd != nil {
			if b := content.Branch(sd.Branch); b != nil && b.Secret {
				kept[k] = r
				continue
			}
		}
		p.SkillPoints += r
	}
	p.Skills = kept
	p.Subclasses = map[string]string{}
	p.Classes = []string{p.Class}
	if c := content.Class(p.Class); c != nil {
		for k, v := range p.Attrs {
			if base := c.Attrs[k]; v > base {
				p.AttrPoints += int(math.Round(v - base))
				p.Attrs[k] = base
			}
		}
		p.Abilities = nil
		p.Hotbar = [HotbarSize]string{}
		for _, a := range c.Abilities {
			g.unlockAbility(e, a)
		}
	}
	for k := range kept {
		if sd := content.Skill(k); sd != nil && sd.Grants != "" {
			g.unlockAbility(e, sd.Grants)
		}
	}
	e.Recalc()
	g.Log(e, "#c0c0ff", "Память очищена: %d очк. навыков и %d очк. характеристик можно распределить заново (K и C).", p.SkillPoints, p.AttrPoints)
	g.FX(e.Level, e.Pos, "Забвение", 0, "#c0c0ff", 1200)
}

// blessings granted by shrines.
var blessings = []content.BuffDef{
	{Key: "bless_stone", Name: "Благословение камня", DurationMs: 180000, Stats: map[string]float64{"res_all": 15, "armor": 3}, Color: "#d0d0ff"},
	{Key: "bless_bear", Name: "Благословение медведя", DurationMs: 180000, Stats: map[string]float64{"str": 4, "melee_pct": 15}, Color: "#ff8a5a"},
	{Key: "bless_owl", Name: "Благословение совы", DurationMs: 180000, Stats: map[string]float64{"int": 4, "spell_pct": 15, "mp_regen": 1}, Color: "#8ab0ff"},
	{Key: "bless_lynx", Name: "Благословение рыси", DurationMs: 180000, Stats: map[string]float64{"dex": 4, "crit": 5, "move_speed": 10}, Color: "#9ae07a"},
	{Key: "bless_oak", Name: "Благословение дуба", DurationMs: 180000, Stats: map[string]float64{"max_hp": 40, "hp_regen": 1.5}, Color: "#7ad08a"},
	{Key: "bless_sun", Name: "Благословение солнца", DurationMs: 180000, Stats: map[string]float64{"res_shadow": 30, "add_holy": 5, "holy_pct": 20}, Color: "#ffe08a"},
}

func (g *Game) prayAtShrine(e *Entity, l *world.Level, c world.Pos, def *content.TileDef) {
	b := blessings[g.rng.IntN(len(blessings))]
	g.applyBuff(e, &b, e.ID)
	e.HP, e.MP = e.MaxHP, e.MaxMP
	if def.Becomes != "" {
		g.SetTile(l, c.X, c.Y, content.TileID(def.Becomes))
	}
	g.Log(e, "#ffe08a", "Вы молитесь у святилища. %s на 3 минуты! Силы восстановлены.", b.Name)
	g.FX(e.Level, e.Pos, b.Name, 0, b.Color, 1500)
	g.FX(e.Level, c, "", '*', "#ffe08a", 600)
}

var dangerNames = []string{"спокойный край", "здесь бывает опасно", "опасные земли", "смертельно опасные земли"}

// checkSurroundings announces regions and discovers landmarks around a player.
func (g *Game) checkSurroundings(e *Entity) {
	p := e.Player
	if e.Level != "overworld" {
		p.region = 0
		return
	}
	if r := g.regionIndex(e.Pos); r != 0 && r != p.region {
		p.region = r
		reg := g.Regions[r-1]
		g.Log(e, "#e0d0a0", "Регион: %s — %s.", reg.Name, dangerNames[min(len(dangerNames)-1, reg.Danger)])
	}
	l := g.Levels["overworld"]
	for i, lm := range g.Landmarks {
		if slices.Contains(p.Found, i) || e.Pos.Dist(lm.Pos) > 7 || !world.LOS(l, e.Pos, lm.Pos) {
			continue
		}
		p.Found = append(p.Found, i)
		xp := 10 + 5*p.Level
		g.Log(e, "#ffe08a", "Открытие: %s (+%d опыта).", lm.Name, xp)
		g.FX(e.Level, e.Pos, "Открытие!", 0, "#ffe08a", 1200)
		g.GiveXP(e, xp)
	}
}

// regionIndex returns the 1-based region of an overworld cell (0 = none).
func (g *Game) regionIndex(p world.Pos) int {
	l := g.Levels["overworld"]
	if l == nil || len(g.RegionMap) != l.W*l.H || !l.In(p.X, p.Y) {
		return 0
	}
	r := int(g.RegionMap[p.Y*l.W+p.X])
	if r > len(g.Regions) {
		return 0
	}
	return r
}

// RegionName is the name of the overworld region at p ("" outside).
func (g *Game) RegionName(p world.Pos) string {
	if r := g.regionIndex(p); r != 0 {
		return g.Regions[r-1].Name
	}
	return ""
}

var biomeDanger = map[string]int{"swamp": 1, "hills": 1, "desert": 1, "tundra": 2, "cursed": 2, "ash": 3}
