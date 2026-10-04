package game

import (
	"testing"

	"ratas/internal/proto"
	"ratas/internal/world"
)

// A mouse-aimed ability goes to the enemy under the cursor, not the nearest.
func TestMouseAim(t *testing.T) {
	g := setup(t)
	p, _, _ := g.Join("Тест", "mage")
	wild(t, g, p)
	l := g.Levels[p.Level]
	near := spawnAt(g, "wolf", l, p.Pos.Add(world.Pos{X: 2}))
	far := spawnAt(g, "wolf", l, p.Pos.Add(world.Pos{X: -4}))
	near.Monster.Target, far.Monster.Target = 0, 0
	p.MP = 100
	p.Player.Hotbar[0] = "magic_missile"
	g.SetInput(p, proto.Input{Ability: 1, Aim: true, AimX: int32(far.Pos.X), AimY: int32(far.Pos.Y)})
	run(g, 30)
	if far.HP >= far.MaxHP {
		t.Fatalf("the aimed wolf was not hit (player %v, far %v)", p.Pos, far.Pos)
	}
	if near.HP < near.MaxHP {
		t.Fatal("the nearest wolf was hit instead of the aimed one")
	}

	// aimed at empty ground, a projectile flies there anyway
	p.Cooldowns = map[string]float64{}
	g.Remove(near)
	g.Remove(far)
	p.MP = 100
	g.SetInput(p, proto.Input{Ability: 1, Aim: true, AimX: int32(p.Pos.X), AimY: int32(p.Pos.Y - 3)})
	g.Tick()
	flying := false
	for _, e := range g.Entities {
		if e.Proj != nil && e.Proj.Owner == p.ID && e.Proj.Path[len(e.Proj.Path)-1].X == p.Pos.X {
			flying = true
		}
	}
	if !flying || p.Facing != world.DirUp {
		t.Fatalf("no projectile to empty ground: facing %v", p.Facing)
	}
}

// The knight's long reach hits an enemy two tiles away and diagonally.
func TestKnightReach(t *testing.T) {
	g := setup(t)
	p, _, _ := g.Join("Тест", "warrior")
	wild(t, g, p)
	l := g.Levels[p.Level]
	far := spawnAt(g, "wolf", l, p.Pos.Add(world.Pos{X: 2}))
	if far.Pos != p.Pos.Add(world.Pos{X: 2}) {
		t.Skip("no free tile")
	}
	far.Monster.Target = 0
	g.indexLevels()
	g.attackFacing(p)
	if far.HP < far.MaxHP {
		t.Fatal("hit two tiles away without the skill")
	}
	p.Player.Skills["long_reach"] = 1
	p.Recalc()
	if p.stats.Reach != 1 {
		t.Fatalf("reach %d", p.stats.Reach)
	}
	for i := 0; i < 10 && far.HP >= far.MaxHP; i++ { // attacks can miss
		g.attackFacing(p)
	}
	if far.HP >= far.MaxHP {
		t.Fatal("the long reach did not hit two tiles away")
	}
	if !g.inReach(p, &Entity{Level: p.Level, Pos: p.Pos.Add(world.Pos{X: 1, Y: 1})}) {
		t.Fatal("no diagonal reach")
	}
	if g.inReach(p, &Entity{Level: p.Level, Pos: p.Pos.Add(world.Pos{X: 3})}) {
		t.Fatal("reach too long")
	}
	// strike abilities reach too
	before := far.HP
	for i := 0; i < 10 && far.HP >= before; i++ {
		p.MP, p.Cooldowns = 100, map[string]float64{}
		g.useAbility(p, "power_strike", nil)
	}
	if far.HP >= before {
		t.Fatal("power strike did not reach")
	}
	// a bow does not get the melee reach
	p.Player.Equip[SlotMain] = ItemStack{Key: "hunting_bow", Qty: 1}
	p.Player.Equip[SlotOff] = ItemStack{}
	delete(p.Player.Equip, SlotOff)
	p.Recalc()
	if p.stats.Reach != 0 {
		t.Fatal("reach with a bow")
	}
}
