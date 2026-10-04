package client

import (
	"ratas/internal/proto"
	"ratas/internal/world"
)

// Scene is the part of the client state a graphical renderer needs to draw
// the world. It is a snapshot: the renderer may keep it after Publish.
type Scene struct {
	Level     *world.Level // replaced only when LevelVer changes
	LevelVer  int
	Explored  world.Bitset
	Self      proto.SelfView
	YouID     uint32
	Entities  []proto.EntityView
	TimeOfDay float64
	Paused    bool
	// the world map is open: the renderer draws it instead of the world
	MapOpen bool
	Places  []proto.Place // known places for the world map
	// the map area in text cells; the renderer draws the world there and
	// the text interface on top of it
	MapX, MapY, MapW, MapH int
}

// Sink receives world state from the client. It is implemented by the
// graphical front-end; the terminal front-end has none.
type Sink interface {
	// Publish replaces the current scene; nil means no world is shown (menus).
	Publish(s *Scene)
	// Effects delivers new visual effects (damage numbers, flashes).
	Effects(fx []proto.FX)
}

// updateVisibility recomputes the field of view and marks explored cells.
func (p *play) updateVisibility() {
	lv := p.level
	s := p.snap.Self
	for i := range p.visible {
		p.visible[i] = false
	}
	if s.Dead {
		return
	}
	world.FOV(lv, s.X, s.Y, s.Vision, func(x, y int) {
		i := y*lv.W + x
		p.visible[i] = true
		p.explored.Set(i)
	})
}

func (p *play) publishScene(L layout) {
	if p.sceneLevel == nil || p.sceneVer != p.levelVer {
		cp := *p.level
		cp.Tiles = append([]uint8(nil), p.level.Tiles...)
		p.sceneLevel = &cp
		p.sceneVer = p.levelVer
	}
	p.a.gfx.Publish(&Scene{
		Level:     p.sceneLevel,
		LevelVer:  p.sceneVer,
		Explored:  append(world.Bitset(nil), p.explored...),
		Self:      p.snap.Self,
		YouID:     p.welcome.YouID,
		Entities:  append([]proto.EntityView(nil), p.snap.Entities...),
		TimeOfDay: p.snap.TimeOfDay,
		Paused:    p.paused,
		MapOpen:   p.mode == modeMap,
		Places:    p.places(),
		MapX:      0, MapY: 1, MapW: L.mapW, MapH: L.mapH,
	})
}

func (p *play) places() []proto.Place {
	if p.sheet == nil {
		return nil
	}
	return p.sheet.Places
}
