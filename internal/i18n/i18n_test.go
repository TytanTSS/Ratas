package i18n

import "testing"

func TestTranslator(t *testing.T) {
	c := newCatalog("xx")
	for k, v := range map[string]string{
		"Зелье здоровья":            "Health Potion",
		"Кузнец":                    "Smith",
		"Регион: %s — %s.":          "Region: %s — %s.",
		"спокойный край":            "a quiet land",
		"Вы получили %d опыта.":     "You gained %d experience.",
		"%s пал в бою.":             "%s has fallen in battle.",
		" • выдал: ":                " • given by: ",
		"Прогресс":                  "Progress",
		"Каменные холмы":            "Stone Hills",
		"%-14s+%.0f%%":              "%-14s+%.0f%%",
		"Блок":                      "Block",
		"%[2]s у %[1]s":             "%[1]s's %[2]s",
		"меч":                       "sword",
		"Говорят, в %s неспокойно.": "They say %s is restless.",
		"Береги себя.":              "Take care.",
		"А, {player}! Рад тебя видеть, {player}.": "Ah, {player}! Glad to see you, {player}.",
	} {
		c.addLocked(k, v)
	}
	c.rebuild()
	for in, want := range map[string]string{
		"Зелье здоровья":    "Health Potion",
		"  Зелье здоровья ": "  Health Potion ",
		"зелье здоровья":    "health Potion",
		"Регион: Каменные холмы — спокойный край.":    "Region: Stone Hills — a quiet land.",
		"Вы получили 12 опыта.":                       "You gained 12 experience.",
		"Мирослава (Кузнец) пал в бою.":               "Miroslava (Smith) has fallen in battle.",
		"Прогресс: 3/5 • выдал: Иван":                 "Progress: 3/5 • given by: Ivan",
		"Блок          +5%":                           "Block         +5%",
		"меч у Ивана":                                 "Ivana's sword",
		"Говорят, в Каменке неспокойно. Береги себя.": "They say Kamenke is restless. Take care.",
		"[Зелье здоровья]":                            "[Health Potion]",
		"no cyrillic":                                 "no cyrillic",
		"А, Лира! Рад тебя видеть, Лира.":             "Ah, Lira! Glad to see you, Lira.",
	} {
		if got := c.translate(in); got != want {
			t.Errorf("%q → %q, want %q", in, got, want)
		}
	}
}

func TestTranslit(t *testing.T) {
	for in, want := range map[string]string{
		"Мирослава":    "Miroslava",
		"Щукин":        "Shchukin",
		"ЖАР":          "ZHAR",
		"Жар":          "Zhar",
		"Рыбачье":      "Rybache",
		"Ёлкино, 3":    "Yolkino, 3",
		"Медвежий Бор": "Medvezhiy Bor",
	} {
		if got := Translit(in); got != want {
			t.Errorf("Translit(%q) = %q, want %q", in, got, want)
		}
	}
}

func TestLanguageSwitch(t *testing.T) {
	defer SetLang(RU)
	SetLang("en_US.UTF-8")
	if Lang() != EN {
		t.Fatalf("lang %s", Lang())
	}
	if got := T("Новая игра"); got != "New game" {
		t.Errorf("T = %q", got)
	}
	SetLang(RU)
	if got := T("Новая игра"); got != "Новая игра" {
		t.Errorf("ru T = %q", got)
	}
}
