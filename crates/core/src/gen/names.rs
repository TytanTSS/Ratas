use crate::rng::Rng;
use std::collections::HashSet;

pub const VILLAGE_NAMES: &[&str] = &[
    "Ольховка",
    "Берёзовка",
    "Каменка",
    "Дубки",
    "Сосновый Бор",
    "Ясная Поляна",
    "Тихий Брод",
    "Медвежий Угол",
    "Вороний Холм",
    "Рыбачье",
    "Красный Яр",
    "Светлая Заводь",
    "Мельничье",
    "Старая Гать",
    "Липовец",
    "Ключи",
    "Белый Камень",
    "Овражки",
    "Заречье",
    "Лисья Нора",
    "Туманный Дол",
    "Кленовка",
];

pub const CITY_NAMES: &[&str] = &[
    "Белокаменск",
    "Златоград",
    "Высокий Престол",
    "Светлоград",
    "Твердислав",
    "Каменная Гавань",
    "Звенигород",
    "Старый Кремень",
];

const MALE_NAMES: &[&str] = &[
    "Борислав",
    "Ждан",
    "Мирон",
    "Остап",
    "Ратибор",
    "Святослав",
    "Тихон",
    "Фрол",
    "Гордей",
    "Добрыня",
    "Елисей",
    "Любомир",
    "Назар",
    "Прохор",
    "Яромир",
    "Вешняк",
];

const FEMALE_NAMES: &[&str] = &[
    "Аглая",
    "Белослава",
    "Веста",
    "Злата",
    "Любава",
    "Милена",
    "Олеся",
    "Рада",
    "Снежана",
    "Услада",
    "Ярина",
    "Дарёна",
    "Забава",
    "Лада",
    "Агафья",
    "Мирослава",
];

pub fn dungeon_names(theme: &str) -> &'static [&'static str] {
    match theme {
        "crypt" => &[
            "Склеп Забытых Королей",
            "Катакомбы Морвен",
            "Гробница Безымянных",
            "Усыпальница Пепла",
            "Костяные Залы",
        ],
        "cave" => &[
            "Логово Гоблинов",
            "Пещера Эха",
            "Глубокие Норы",
            "Шахта Чёрного Камня",
            "Паучьи Гроты",
        ],
        "ice" => &[
            "Ледяные Чертоги",
            "Пещера Вечной Стужи",
            "Грот Инея",
            "Зал Ледобородов",
        ],
        "volcano" => &[
            "Огненные Недра",
            "Жерло Вулкана",
            "Кузня Пламени",
            "Пылающие Глубины",
        ],
        "temple" => &[
            "Гробница Фараона",
            "Затерянный Храм",
            "Песчаная Усыпальница",
            "Храм Солнца",
        ],
        _ => &[
            "Цитадель Бездны",
            "Чёрная Крепость",
            "Твердыня Культа",
            "Врата Преисподней",
        ],
    }
}

const WORLD_NAMES: &[&str] = &[
    "Ратас",
    "Вельмар",
    "Северный Предел",
    "Изумрудная Марка",
    "Пограничье",
    "Долина Туманов",
];

/// A name from the list not used yet (if possible).
pub fn pick_unique(r: &mut Rng, list: &[&str], used: &mut HashSet<String>) -> String {
    for _ in 0..50 {
        let n = *r.pick(list);
        if !used.contains(n) {
            used.insert(n.to_string());
            return n.to_string();
        }
    }
    r.pick(list).to_string()
}

/// Parts of generated village names: an adjective in the three genders and a
/// noun with its gender ("Тихий Брод", "Светлая Заводь", "Старое Село").
const TOWN_ADJ: &[[&str; 3]] = &[
    ["Тихий", "Тихая", "Тихое"],
    ["Красный", "Красная", "Красное"],
    ["Светлый", "Светлая", "Светлое"],
    ["Старый", "Старая", "Старое"],
    ["Новый", "Новая", "Новое"],
    ["Верхний", "Верхняя", "Верхнее"],
    ["Нижний", "Нижняя", "Нижнее"],
    ["Дальний", "Дальняя", "Дальнее"],
    ["Белый", "Белая", "Белое"],
    ["Чёрный", "Чёрная", "Чёрное"],
    ["Зелёный", "Зелёная", "Зелёное"],
    ["Ясный", "Ясная", "Ясное"],
    ["Глухой", "Глухая", "Глухое"],
    ["Кривой", "Кривая", "Кривое"],
    ["Высокий", "Высокая", "Высокое"],
    ["Солнечный", "Солнечная", "Солнечное"],
    ["Лесной", "Лесная", "Лесное"],
    ["Речной", "Речная", "Речное"],
    ["Каменный", "Каменная", "Каменное"],
    ["Медвежий", "Медвежья", "Медвежье"],
    ["Волчий", "Волчья", "Волчье"],
    ["Лисий", "Лисья", "Лисье"],
    ["Вороний", "Воронья", "Воронье"],
    ["Сосновый", "Сосновая", "Сосновое"],
    ["Берёзовый", "Берёзовая", "Берёзовое"],
    ["Дубовый", "Дубовая", "Дубовое"],
    ["Ольховый", "Ольховая", "Ольховое"],
    ["Туманный", "Туманная", "Туманное"],
    ["Тёплый", "Тёплая", "Тёплое"],
    ["Сухой", "Сухая", "Сухое"],
];

/// 0 masculine, 1 feminine, 2 neuter.
const TOWN_NOUNS: &[(&str, usize)] = &[
    ("Брод", 0),
    ("Яр", 0),
    ("Холм", 0),
    ("Ключ", 0),
    ("Бор", 0),
    ("Луг", 0),
    ("Мост", 0),
    ("Камень", 0),
    ("Угол", 0),
    ("Двор", 0),
    ("Хутор", 0),
    ("Погост", 0),
    ("Затон", 0),
    ("Овраг", 0),
    ("Заводь", 1),
    ("Поляна", 1),
    ("Роща", 1),
    ("Слобода", 1),
    ("Падь", 1),
    ("Горка", 1),
    ("Балка", 1),
    ("Мельница", 1),
    ("Пристань", 1),
    ("Гать", 1),
    ("Поле", 2),
    ("Озеро", 2),
    ("Село", 2),
    ("Займище", 2),
    ("Городище", 2),
    ("Урочище", 2),
];

/// Generated city names: a root and an ending ("Бел" + "оград").
const CITY_ROOTS: &[&str] = &[
    "Бел",
    "Злат",
    "Свет",
    "Яр",
    "Добр",
    "Свят",
    "Рад",
    "Влад",
    "Мир",
    "Твер",
    "Крас",
    "Серебр",
    "Камен",
    "Высок",
    "Стар",
    "Нов",
    "Велик",
    "Сокол",
    "Орл",
    "Ясн",
    "Тих",
    "Бор",
    "Лес",
    "Зар",
    "Звон",
    "Слав",
    "Чист",
    "Гром",
    "Рус",
    "Сне",
    "Син",
    "Дубр",
];

const CITY_ENDS: &[&str] = &[
    "оград",
    "огорск",
    "одар",
    "омир",
    "ополь",
    "обор",
    "ослав",
    "огорье",
];

/// A village name not used yet: the classic names first, then generated ones.
pub fn village_name(r: &mut Rng, used: &mut HashSet<String>) -> String {
    let made = |r: &mut Rng| {
        let a = r.pick(TOWN_ADJ);
        let (n, g) = *r.pick(TOWN_NOUNS);
        format!("{} {}", a[g], n)
    };
    fresh_name(r, VILLAGE_NAMES, used, made)
}

/// A city name not used yet: the classic names first, then generated ones.
pub fn city_name(r: &mut Rng, used: &mut HashSet<String>) -> String {
    let made = |r: &mut Rng| format!("{}{}", r.pick(CITY_ROOTS), r.pick(CITY_ENDS));
    fresh_name(r, CITY_NAMES, used, made)
}

fn fresh_name(
    r: &mut Rng,
    classic: &[&str],
    used: &mut HashSet<String>,
    made: impl Fn(&mut Rng) -> String,
) -> String {
    let free: Vec<&str> = classic
        .iter()
        .copied()
        .filter(|n| !used.contains(*n))
        .collect();
    if !free.is_empty() {
        let n = r.pick(&free).to_string();
        used.insert(n.clone());
        return n;
    }
    let mut n = made(r);
    for _ in 0..300 {
        if !used.contains(&n) {
            break;
        }
        n = made(r);
    }
    used.insert(n.clone());
    n
}

/// A name from the list not used yet; when the list runs out, the name of a
/// place nearby tells namesakes apart ("Пещера Эха (Тихий Брод)").
pub fn place_name(r: &mut Rng, list: &[&str], near: &str, used: &mut HashSet<String>) -> String {
    let free: Vec<&str> = list
        .iter()
        .copied()
        .filter(|n| !used.contains(*n))
        .collect();
    if !free.is_empty() {
        let n = r.pick(&free).to_string();
        used.insert(n.clone());
        return n;
    }
    let start = r.usize_n(list.len());
    if near.is_empty() {
        return list[start].to_string();
    }
    for k in 0..list.len() {
        let n = format!("{} ({near})", list[(start + k) % list.len()]);
        if used.insert(n.clone()) {
            return n;
        }
    }
    format!("{} ({near})", list[start])
}

/// A random first name.
pub fn person_name(r: &mut Rng) -> String {
    if r.int_n(2) == 0 {
        r.pick(MALE_NAMES).to_string()
    } else {
        r.pick(FEMALE_NAMES).to_string()
    }
}

/// A random realm name.
pub fn world_name(r: &mut Rng) -> String {
    r.pick(WORLD_NAMES).to_string()
}
