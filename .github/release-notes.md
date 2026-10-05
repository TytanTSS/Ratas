## Скачать

| Платформа | Файл |
|---|---|
| Windows 10/11 (x64) | `ratas-…-windows-amd64.zip` |
| Windows на ARM | `ratas-…-windows-arm64.zip` |
| macOS (Apple Silicon и Intel) | `ratas-…-macos-universal.tar.gz` |
| Linux (x64), игра и сервер | `ratas-…-linux-amd64.tar.gz` |
| Выделенный сервер для Linux без графики | `ratas-…-linux-server-amd64.tar.gz` |

Контрольные суммы — в `SHA256SUMS.txt`.

**Windows.** Распакуйте архив и запустите `ratas.exe` — откроется окно игры. SmartScreen может предупредить о неизвестном издателе: «Подробнее» → «Выполнить в любом случае». Выделенный сервер: `ratas.exe --server` из терминала.

**macOS.** Сборка не подписана сертификатом Apple, поэтому после распаковки снимите карантин:

```bash
tar xzf ratas-*-macos-universal.tar.gz
cd ratas-*-macos-universal
xattr -d com.apple.quarantine ratas
./ratas              # игра
./ratas --server     # выделенный сервер
```

**Linux.** Для окна нужны OpenGL и X11 — на обычном рабочем столе они уже есть. Серверу (`ratas-server --server` или `ratas --server`) графика не нужна.

Все флаги и управление — в `README.md` внутри архива.
