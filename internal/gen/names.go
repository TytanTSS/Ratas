package gen

import "math/rand/v2"

var villageNames = []string{
	"Ольховка", "Берёзовка", "Каменка", "Дубки", "Сосновый Бор", "Ясная Поляна",
	"Тихий Брод", "Медвежий Угол", "Вороний Холм", "Рыбачье", "Красный Яр",
	"Светлая Заводь", "Мельничье", "Старая Гать", "Липовец", "Ключи",
	"Белый Камень", "Овражки", "Заречье", "Лисья Нора", "Туманный Дол", "Кленовка",
}

var maleNames = []string{
	"Борислав", "Ждан", "Мирон", "Остап", "Ратибор", "Святослав", "Тихон", "Фрол",
	"Гордей", "Добрыня", "Елисей", "Любомир", "Назар", "Прохор", "Яромир", "Вешняк",
}

var femaleNames = []string{
	"Аглая", "Белослава", "Веста", "Злата", "Любава", "Милена", "Олеся", "Рада",
	"Снежана", "Услада", "Ярина", "Дарёна", "Забава", "Лада", "Агафья", "Мирослава",
}

var dungeonNames = map[string][]string{
	"crypt":    {"Склеп Забытых Королей", "Катакомбы Морвен", "Гробница Безымянных", "Усыпальница Пепла", "Костяные Залы"},
	"cave":     {"Логово Гоблинов", "Пещера Эха", "Глубокие Норы", "Шахта Чёрного Камня", "Паучьи Гроты"},
	"ice":      {"Ледяные Чертоги", "Пещера Вечной Стужи", "Грот Инея", "Зал Ледобородов"},
	"volcano":  {"Огненные Недра", "Жерло Вулкана", "Кузня Пламени", "Пылающие Глубины"},
	"temple":   {"Гробница Фараона", "Затерянный Храм", "Песчаная Усыпальница", "Храм Солнца"},
	"fortress": {"Цитадель Бездны", "Чёрная Крепость", "Твердыня Культа", "Врата Преисподней"},
}

var worldNames = []string{"Ратас", "Вельмар", "Северный Предел", "Изумрудная Марка", "Пограничье", "Долина Туманов"}

func pickUnique(r *rand.Rand, list []string, used map[string]bool) string {
	for tries := 0; tries < 50; tries++ {
		n := list[r.IntN(len(list))]
		if !used[n] {
			used[n] = true
			return n
		}
	}
	return list[r.IntN(len(list))]
}

// PersonName returns a random first name.
func PersonName(r *rand.Rand) string {
	if r.IntN(2) == 0 {
		return maleNames[r.IntN(len(maleNames))]
	}
	return femaleNames[r.IntN(len(femaleNames))]
}

// WorldName returns a random realm name.
func WorldName(r *rand.Rand) string { return worldNames[r.IntN(len(worldNames))] }
