# Иконка трея и `.exe` — дизайн

- **Дата:** 2026-06-06
- **Статус:** утверждён, готов к плану реализации
- **Платформа:** Windows 11 (целевой билд 26200)
- **Язык:** Rust

## 1. Контекст и проблема

Иконка трея сейчас — системная заглушка `LoadIconW(None, IDI_APPLICATION)`
(`badge.rs`): generic-лист, ничего не сообщает. Сам `desktag.exe` в Проводнике и
Диспетчере задач тоже без иконки (generic). Нужна осмысленная иконка.

Проект — постоянно видимый указатель текущего виртуального стола. Иконка трея
должна работать как **второй индикатор**: показывать номер текущего стола прямо в
трее, не глядя на бейдж.

## 2. Цель

1. **Трей:** динамическая иконка-«пилюля» с номером текущего стола; перерисовка
   при переключении.
2. **`.exe`:** статичная фирменная иконка-«пилюля» с глифом `D`, видимая в
   Проводнике / Диспетчере задач.

Обе в едином визуальном стиле бейджа (тёмный фон `0x202020`, светлый текст
`0xF0F0F0`).

## 3. Не-цели

- Детект светлой/тёмной темы панели задач (пилюля несёт свой фон → видна везде).
- Сглаживание краёв пилюли через premultiplied-alpha (на 16px не нужно).
- PNG-сжатие записей `.ico` (BMP-записей достаточно; внешний PNG-крейт не вводим).
- Конфигурируемые цвета/символы иконки.

## 4. Архитектура

Единый растеризатор пилюли, два потребителя. Новый модуль `src/icon.rs` —
единственное новое место логики.

```
icon.rs::rasterize(text, size) -> Vec<u8> (top-down RGBA)
        │
        ├── make_tray_hicon(text, size) -> HICON   (потребитель: badge.rs, runtime)
        └── write_ico(path, text)                  (потребитель: main.rs --gen-icon, dev)
```

Правки точечные: `badge.rs` (трей), `main.rs` (флаг `--gen-icon`), `Cargo.toml`
(`winresource`). Новые файлы: `src/icon.rs`, `build.rs`, ассет `assets/desktag.ico`.

## 5. Растеризатор (`icon.rs::rasterize`)

`rasterize(text: &str, size: u32) -> Vec<u8>` — возвращает top-down RGBA
(`size`×`size`×4):

1. 32-bit DIB-section (`CreateDIBSection`, top-down: `biHeight` отрицательный) +
   memory-DC; DIB выбран в DC.
2. Очистка прозрачным (0,0,0,0).
3. Пилюля: рисуем `RoundRect` (скруглённый квадрат) кистью `BG_COLOR`.
4. Текст: шрифт высотой от `size`, цвет `TEXT_COLOR`, по центру.
   Ширину меряем `GetTextExtentPoint32W`; двузначные («10») ужимаем под пилюлю.
5. **Фикс GDI-alpha:** классический GDI пишет пиксели с `alpha=0`. Один
   постпроход: строим регион пилюли (`CreateRoundRectRgn`), для каждого пикселя
   `PtInRegion` → внутри `alpha=255`, снаружи `alpha=0`. ≤256² пикселей —
   мгновенно.

Цвета/радиус — те же константы стиля, что у бейджа.

## 6. Трей — динамика и жизненный цикл `HICON`

- `make_tray_hicon(text, size) -> HICON`: `rasterize` → `hbmColor` (32-bpp из RGBA)
  + `hbmMask` (1-bpp, из альфы: `alpha>0` непрозрачно) → `CreateIconIndirect`.
- `badge.rs`: `thread_local! { CURRENT_TRAY_ICON: Cell<HICON> }`.
  - `install_tray` ставит иконку текущего номера (вместо `IDI_APPLICATION`).
  - Новый `update_tray_icon(hwnd)` — зовётся в `wndproc` при
    `WM_APP_DESKTOP_CHANGED` рядом с перечиткой label: `DestroyIcon` старый →
    собрать новый → `Shell_NotifyIconW(NIM_MODIFY)` → сохранить в `CURRENT_TRAY_ICON`.
  - `DestroyIcon` также при выходе (`remove_tray` / `WM_DESTROY`).
- Размер иконки: `GetSystemMetrics(SM_CXSMICON)` (16, либо 32 при HiDPI).
- Номер: `index0 + 1`, где `index0` из `desktop::current_index_and_name()`.

## 7. `.exe`-иконка — `build.rs` + генерация `.ico`

- `icon.rs::write_ico(path, text)`: для размеров 16/32/48/256 → `rasterize` →
  записи `.ico` формата BMP (`BITMAPINFOHEADER` с удвоенной высотой; 32-bpp BGRA
  bottom-up XOR + 1-bpp AND-маска из альфы); собрать `ICONDIR`; записать файл.
  Без PNG-крейта. 256² как BMP ≈ +260 КБ к `.exe` — приемлемо.
- `main.rs`: флаг `--gen-icon [path]` (по умолчанию `assets/desktag.ico`) —
  рисует `.ico` глифа `D` и выходит. Dev-разовый, вне потока демона.
- `build.rs`: `winresource::WindowsResource::new().set_icon("assets/desktag.ico").compile()`
  под `#[cfg(target_os = "windows")]`. `Cargo.toml` → `[build-dependencies] winresource = "0.1"`.
- Поток разработчика: один раз `cargo run -- --gen-icon` → коммит `assets/desktag.ico`
  → далее `build.rs` встраивает ресурс при сборке.

## 8. Что рисуем

- **Трей:** номер текущего стола `index0 + 1` (синхронно с бейджем).
- **`.exe`:** статичный глиф `D` (DeskTag), та же пилюля.

## 9. Обработка ошибок

- `make_tray_hicon` / GDI-вызовы могут вернуть null — при неудаче трей
  откатывается на `LoadIconW(None, IDI_APPLICATION)` (текущее поведение как
  фоллбэк, не паника).
- `write_ico` (dev-путь): ошибки I/O печатаем в stderr, ненулевой exit-код.
- `build.rs`: отсутствие `assets/desktag.ico` — ошибка сборки с понятным
  сообщением (ассет коммитится в репо, должен присутствовать).

## 10. Тесты

- `label.rs` — нетронут.
- `icon.rs` (под `#[cfg(windows)]`, GDI требует Windows):
  - smoke: `rasterize("1", 32)` → буфер длины `32*32*4`, есть пиксели `alpha=255`
    (пилюля непрозрачна) и есть `alpha=0` (углы прозрачны).
  - `.ico`-структура: `write_ico` в буфер → magic `00 00 01 00`, число записей = 4,
    заявленные размеры совпадают.
- CLAUDE.md: строку «cargo test — unit tests (label formatting only)» обновить —
  добавились icon-тесты под Windows.

## 11. Риски и допущения

- **Premultiplied alpha не делаем** — фон пилюли непрозрачный, лёгкая кайма
  антиалиаса цифры на 16px приемлема. Если визуально плохо — добавить premultiply
  отдельным проходом (изолированно в `rasterize`).
- **Утечки `HICON`** — строгий `DestroyIcon` при каждой замене и при выходе;
  единственный владелец — `CURRENT_TRAY_ICON`.
- **`winvd`-номер при удалении стола** — иконка перечитывается из того же
  `WM_APP_DESKTOP_CHANGED`, что и бейдж → синхронна, рассинхрона нет.
- **`winresource` и build-чувствительность** — это build-dependency, на runtime
  `winvd`-COM не влияет.
