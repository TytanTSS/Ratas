## Скачать

| Платформа | Файл |
|---|---|
| Windows 10/11 (x64) | `ratas-…-windows-amd64.zip` |
| Windows на ARM | `ratas-…-windows-arm64.zip` |
| macOS (Apple Silicon и Intel) | `ratas-…-macos-universal.tar.gz` |
| Linux (x64), терминал и окно | `ratas-…-linux-amd64.tar.gz` |
| Выделенный сервер для Linux | `ratas-…-linux-server-amd64.tar.gz` |

Контрольные суммы — в `SHA256SUMS.txt`.

**Windows.** Распакуйте архив и запустите `ratas.exe` (двойной клик откроет игру в консоли) или из терминала: `ratas.exe -gfx` — графический режим в окне. Лучше всего игра выглядит в Windows Terminal. SmartScreen может предупредить о неизвестном издателе: «Подробнее» → «Выполнить в любом случае».

**macOS.** Сборка не подписана сертификатом Apple, поэтому после распаковки снимите карантин:

```bash
tar xzf ratas-*-macos-universal.tar.gz
cd ratas-*-macos-universal
xattr -d com.apple.quarantine ratas
./ratas          # терминал
./ratas -gfx     # окно
```

**Linux.** Для графического режима нужны библиотеки OpenGL и X11, на обычном рабочем столе они уже есть.

Все флаги и управление — в `README.md` внутри архива.
