package game

import (
	"fmt"
	"slices"
	"strings"

	"ratas/internal/content"
)

// MulticlassLevel is the character level needed to start a second class.
const MulticlassLevel = 5

// ClassLevel is the number of skill points invested in a class (its base
// branch and its subclass branch).
func ClassLevel(p *PlayerState, class string) int {
	n := 0
	for key, r := range p.Skills {
		if sd := content.Skill(key); sd != nil {
			if b := content.Branch(sd.Branch); b != nil && b.Class == class {
				n += r
			}
		}
	}
	return n
}

// HasClass reports whether the character develops a class.
func (p *PlayerState) HasClass(class string) bool {
	return slices.Contains(p.Classes, class) || (len(p.Classes) == 0 && p.Class == class)
}

// Unlocked reports whether a secret class, subclass or skill has been opened.
func (p *PlayerState) Unlocked(kind, key string) bool {
	return slices.Contains(p.Unlocks, kind+":"+key)
}

// CanLearn explains why a skill cannot be learned ("" = it can).
func CanLearn(p *PlayerState, sd *content.SkillDef) string {
	if sd == nil {
		return "нет такого навыка"
	}
	if p.Skills[sd.Key] >= sd.MaxRank {
		return "максимальный ранг"
	}
	if b := content.Branch(sd.Branch); b != nil {
		if b.Secret {
			return "этому учат лишь уникальные мастера мира"
		}
		if b.Class != "" && !p.HasClass(b.Class) {
			name := b.Class
			if c := content.Class(b.Class); c != nil {
				name = c.Name
			}
			return fmt.Sprintf("класс «%s» не начат", name)
		}
		if b.Subclass != "" && p.Subclasses[b.Class] != b.Subclass {
			name := b.Subclass
			if sc := content.Subclass(b.Subclass); sc != nil {
				name = sc.Name
			}
			return fmt.Sprintf("нужен подкласс «%s»", name)
		}
	}
	if p.Level < sd.Level {
		return fmt.Sprintf("нужен уровень персонажа %d", sd.Level)
	}
	for _, r := range sd.Requires {
		if p.Skills[r] < 1 {
			name := r
			if rd := content.Skill(r); rd != nil {
				name = rd.Name
			}
			return "требуется: " + name
		}
	}
	if p.SkillPoints <= 0 {
		return "нет очков навыков"
	}
	return ""
}

// CanStartClass explains why a class cannot be started ("" = it can).
func CanStartClass(p *PlayerState, class string) string {
	c := content.Class(class)
	switch {
	case c == nil:
		return "нет такого класса"
	case p.HasClass(class):
		return "класс уже начат"
	case c.Secret && !p.Unlocked("class", class):
		return "секретный класс: его открывает задание уникального персонажа"
	case p.Level < MulticlassLevel:
		return fmt.Sprintf("второй класс можно начать с %d-го уровня", MulticlassLevel)
	}
	return ""
}

// CanChooseSubclass explains why a subclass cannot be chosen ("" = it can).
func CanChooseSubclass(p *PlayerState, key string) string {
	sc := content.Subclass(key)
	switch {
	case sc == nil:
		return "нет такого подкласса"
	case !p.HasClass(sc.Class):
		return "класс не начат"
	case p.Subclasses[sc.Class] != "":
		return "подкласс этого класса уже выбран"
	case sc.Secret && !p.Unlocked("subclass", key):
		return "секретный подкласс: его открывает задание уникального персонажа"
	case ClassLevel(p, sc.Class) < sc.Level:
		return fmt.Sprintf("нужен %d-й уровень класса", sc.Level)
	}
	return ""
}

func (g *Game) startClass(e *Entity, class string) {
	p := e.Player
	if why := CanStartClass(p, class); why != "" {
		g.Log(e, "#ff8080", "Нельзя начать класс: %s.", why)
		return
	}
	c := content.Class(class)
	p.Classes = append(p.Classes, class)
	for _, a := range c.Abilities {
		g.unlockAbility(e, a)
	}
	e.Recalc()
	p.Dirty = true
	g.Log(e, "#80ffff", "Вы начинаете путь класса «%s». Его навыки — в окне K.", c.Name)
	g.FX(e.Level, e.Pos, c.Name+"!", 0, c.Color, 1500)
}

func (g *Game) chooseSubclass(e *Entity, key string) {
	p := e.Player
	if why := CanChooseSubclass(p, key); why != "" {
		g.Log(e, "#ff8080", "Нельзя выбрать подкласс: %s.", why)
		return
	}
	sc := content.Subclass(key)
	if p.Subclasses == nil {
		p.Subclasses = map[string]string{}
	}
	p.Subclasses[sc.Class] = key
	p.Dirty = true
	g.Log(e, "#ffd24a", "Ваш путь: %s! Его навыки открыты в окне K.", sc.Name)
	g.FX(e.Level, e.Pos, sc.Name+"!", 0, sc.Color, 1800)
}

// unlock opens a secret class, subclass or skill, or gives an artifact.
func (g *Game) unlock(e *Entity, reward string) string {
	p := e.Player
	kind, key, _ := strings.Cut(reward, ":")
	switch kind {
	case "item":
		st := ItemStack{Key: key, Qty: 1}
		if !g.addItem(e, st) {
			g.dropItem(e.Level, e.Pos, st)
		}
		g.Log(e, "#ff80ff", "Артефакт: %s!", st.Name())
		return st.Name()
	case "class", "subclass":
		if !slices.Contains(p.Unlocks, reward) {
			p.Unlocks = append(p.Unlocks, reward)
		}
		name := key
		if kind == "class" {
			if c := content.Class(key); c != nil {
				name = fmt.Sprintf("класс «%s»", c.Name)
			}
			g.Log(e, "#ff80ff", "Открыт секретный %s! Начать его можно в окне навыков (K) с %d-го уровня.", name, MulticlassLevel)
		} else {
			if sc := content.Subclass(key); sc != nil {
				name = fmt.Sprintf("подкласс «%s»", sc.Name)
				if !p.HasClass(sc.Class) {
					p.Classes = append(p.Classes, sc.Class)
				}
			}
			g.Log(e, "#ff80ff", "Открыт секретный %s! Выбрать его можно в окне навыков (K).", name)
		}
		p.Dirty = true
		return name
	case "skill":
		sd := content.Skill(key)
		if sd == nil {
			return key
		}
		if p.Skills[key] < sd.MaxRank {
			p.Skills[key] = sd.MaxRank
		}
		if sd.Grants != "" {
			g.unlockAbility(e, sd.Grants)
		}
		e.Recalc()
		p.Dirty = true
		g.Log(e, "#ff80ff", "Тайное знание: %s!", sd.Name)
		return fmt.Sprintf("навык «%s»", sd.Name)
	}
	return reward
}
