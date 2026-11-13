<p align="center">
  <img src="assets/icon-source.png" width="160" alt="aula-f75-rs logo">
</p>

<h1 align="center">aula-f75-rs</h1>

<p align="center"><b>v0.1.0</b></p>

<p align="center">
  🦀 <em>A Rust CLI + GUI for the AULA F75 mechanical keyboard on Linux x86_64</em> — a complete, independent replacement for the official
  <code>OemDrv.exe</code> (Windows-only). Developed and tested on Arch Linux (niri + Wayland).
</p>

> 🎯 **Single binary. No driver. No installer. No external dependencies.**
> Everything goes through standard HID (`hidapi`) — remaps, macros and lighting are written **inside the keyboard** and keep working on any computer afterwards.

📦 **Repo:** `https://github.com/fredjunior54/aula-f75-rs.git` · 🧠 **Full technical docs:** [`AGENTS.md`](AGENTS.md)

> 🔤 Embedded typeface: **CaskaydiaCove Nerd Font Mono** (`assets/CaskaydiaCoveNerdFontMono-Regular.ttf`), compiled in via `include_bytes!` — no system font required. Licenses in `assets/LICENSE-NerdFonts.txt` (Nerd Fonts / Cascadia Code, SIL OFL 1.1).

---

## ✨ Highlights

| 🏆 | Feature |
|---|---|
| 🖥️ | **GUI** (egui) with a keyboard drawing, fixed legends and an icon navbar |
| 🔀 | **Per-key remapping** (base + Fn layers) with *combo*, media, macro or `none` |
| 📋 | **Active remaps**: live list of everything that differs from factory, with per-key *undo* and *undo all* |
| ⏺️ | **Macros stored in the keyboard** (16 slots) with combos, holds and waits |
| 🌈 | **Per-key RGB** plus ready-made themes (*rainbow, ocean, fire, matrix, sunset, off*) |
| 🎵 | **Real-time animations** at ~14 fps straight from the PC (CLI) |
| 🔋 | **Battery**, sleep, debounce, win-lock, factory reset — all from the GUI |
| 🧵 | All HID IO lives on its own thread — **the interface never blocks** |
| 🐧 | **Linux** with a bundled udev rule (no sudo) |
| 🌿 | **Catppuccin Mocha** theme ([catppuccin/egui](https://github.com/catppuccin/egui)) |
| 🔤 | **CaskaydiaCove Nerd Font** embedded in the binary (14 px base text, monospace + Nerd icons) |

---

## 📡 How it works

The keyboard listens on a **vendor HID channel**: report `0x13`, **20-byte** packets with an 8-bit additive *checksum*, payloads of up to 14 B and **echo ACK** (the keyboard sends every packet back). Large blocks (up to 512 B) are reassembled by index. The protocol was documented by the community in [aula-f75-hub/docs/protocol.md](https://github.com/bakiakyol/aula-f75-hub/blob/main/docs/protocol.md) and **re-validated here, byte by byte, against real hardware** — including where the community doc is wrong (e.g. *53 = Fn* and *59 = RCtrl*).

| 🔌 Connection | VID:PID | Channel | Status |
|---|---|---|---|
| 📶 2.4G dongle | `3554:fa09` | usage page `0xFF02` | ✅ **verified** (read, write, battery, reset) |
| 🟦 Bluetooth | `3554:fa07` | usage page `0xFF02` | ✅ same channel, verified |
| 🔗 USB cable | `258a:010c` | usage page `0xFF00` (520 B feature report) | ❌ *different transport — on the roadmap* |

> 💡 **Important:** firmware effects (1-31) are computed *inside* the keyboard — there is no table to read back. Only effect **32 (custom)** renders the per-key RGB table (`0x02`). Painting keys or applying a theme **switches to 32 automatically**.

---

## 🛠️ Building

**Requirements:** stable Rust + libudev headers (Arch: already shipped with `systemd`; Debian/Ubuntu: `libudev-dev`).

```bash
git clone https://github.com/fredjunior54/aula-f75-rs.git
cd aula-f75-rs
cargo build --release        # → target/release/aula-f75
```

### 🐧 Linux: permissions without sudo

Copy the bundled udev rule and replug the dongle:

```bash
sudo cp udev/70-aula-f75.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger
```

---

## 🖥️ GUI

```bash
aula-f75 gui
```

Vertical icon navbar on the left; each tab opens in the right panel. The keyboard drawing in the centre shows the **custom layer** (editor) with the real colours read from the hardware.

> 🖴 **Tray:** resident icon (StatusNotifierItem; shows up in quickshell, waybar etc.) with a **Show** menu (opens or focuses the window) and **Quit** (ends everything). **Left click on the icon = toggle:** opens the window when closed, closes it when open. The icon lives in the `aula-f75 tray` daemon, which runs the window as a **child process** — closing the window (X, niri `Mod+C`) closes **only the window**, the icon stays and **Show** brings it back.
>
> 🪟 **Window app ID:** `aula-f75-rs` — use it in niri window rules, e.g. `match app-id="aula-f75-rs"`

```bash
# resident in autostart (niri/quickshell/waybar): icon only, window from the menu
aula-f75 tray --no-window
# with the window open at startup
aula-f75 tray
```

```kdl
// ~/.config/niri/config.kdl
spawn-at-startup "aula-f75" "tray" "--no-window"
```

| Icon | Tab | What it does |
|---|---|---|
| 💡 | **Lighting** | effect (17 hardware values), brightness 0-9, speed 0-4, colour 0-7 |
| ⌨ | **Remap** | select keys on the board → apply a target; **Active remaps** with per-key *undo* and *undo all* |
| ⏺ | **Macros** | save/delete slots and bind them to keys with run modes |
| 🌈 | **Themes** | ready-made themes in one click (applying switches to custom) |
| ⚙ | **Settings** | 💤 sleep (same values as the original app), ⏱ debounce, ♻ factory reset with confirmation |
| ℹ | **Device** | transport, VID:PID, HID strings, battery — and the app logo at the bottom |

**Board highlights:**

- 🏷️ Legends *always visible* (real keycap abbreviations: `bksp`, `caps`, `shift`…)
- 🟠 **Orange** stroke = key with an active remap on the selected layer (gone as soon as you undo)
- ⬜ Click selects · **Ctrl+click** adds to the selection
- 🟢 Status + 🔋 battery in the header; automatic reconnection (5 s) after a reset/replug

---

## ⌨️ CLI

### 💡 Lighting

```bash
aula-f75 list                 # 🔍 connected devices (usage page + path)
aula-f75 info                 # 📊 effect, brightness, speed, battery, settings
aula-f75 effect 6             # 🌈 effect (hardware numbers; see `info`)
aula-f75 brightness 5         # ☀️ 0-9
aula-f75 speed 2              # ⚡ 0-4
aula-f75 color blue           # 🎨 0-7 or name (red green blue yellow purple cyan white multicolor)
aula-f75 key-color w=ff0000 a=00ff00 s=00ff00 d=00ff00   # 🖌️ paint keys (switches to custom)
aula-f75 preset fire          # 🌠 rainbow · ocean · fire · matrix · sunset · off
aula-f75 live rainbow 15      # 🎵 real-time animation: rainbow · wave · fire · #rrggbb
```

### 🔀 Remap (layers 0 = base, 1 = Fn)

```bash
aula-f75 remap 0                     # 📋 decoded keymap
aula-f75 remap 0 --raw               # 📄 raw block as hex (1008 chars)
aula-f75 remap 0 capslock=esc        # ✅ applies + read-back
aula-f75 remap 0 f1=ctrl+shift+t f2=none
aula-f75 remap 1 f5=macro:0:2        # Fn: slot 0, repeat while held
```

**Accepted targets:**

| Form | Example | Result |
|---|---|---|
| key | `esc`, `f5`, `pgup` | plain HID usage |
| modifiers + key | `ctrl+shift+t` | combo |
| two keys | `a+b` | combo without a modifier |
| `none` | `none` | dead key |
| `macro:IDX[:mode]` | `macro:0:2` | runs a macro (mode 1, 2 or 4) |
| `consumer:HEX` | `consumer:0E9` | media (volume+, `0EA` vol−, `0E2` mute…) |
| `special:HEX` / `fnfunc:B1.B2.B3` | `special:11` | internal functions (battery etc.) |

### ⏺️ Macros

```bash
aula-f75 macro list                          # 📋 stored macros (decoded)
aula-f75 macro set 0 hello "h , e , l , l , o , enter"
aula-f75 macro assign pgup 0                 # ⌨️ bind it to a key (modes 1/2/4)
aula-f75 macro del 0                         # 🗑️ delete slot (later ones shift up)
```

**Sequence syntax** (tokens separated by spaces/commas; default delay 25 ms):

| Token | Meaning |
|---|---|
| `h` · `enter` · `f5` | press and release |
| `ctrl+shift+t` | modifiers held + key |
| `^shift` | **hold** without releasing |
| `shift` (after a `^shift`) | **release** the hold |
| `wait:300` | 300 ms pause after the previous event |

### 🔧 Diagnostics

```bash
aula-f75 battery             # 🔋 percentage (experimental)
aula-f75 strings             # 🏷️ HID descriptors (debug)
aula-f75 dump                # 📄 raw LED block (hex)
aula-f75 selftest            # ✅ read → write back identical → verify
aula-f75 keys                # ⌨️ key names + LED index + x/y
```

---

## ⚠️ Safety & behaviour notes

- 🖐️ **Lost key release:** writing to flash while a key is held can leave that key *auto-repeating*. Every write therefore waits for **no keys held + 400 ms idle** (`wait_keys_idle`) before acting.
- 🔄 **Factory reset** (hold `Fn+Esc`, or from the GUI): the keyboard **reboots and re-pairs** — reads fail for a few seconds *by design*; the GUI reconnects on its own (5 s).
- 🚫 **Never run the GUI and the CLI at the same time** — two processes interleave HID packets and corrupt multi-packet reads.
- 🔒 Nothing here touches firmware or the bootloader. Only documented configuration blocks.
- 🔤 The **firmware version is not exposed** over the 2.4G dongle — the GUI says so honestly.
- 💡 With brightness at `0` a theme or per-key paint is still written correctly, but **nothing lights up** — that is not a failed write.

---

## 🗺️ Roadmap

- [x] Block read/write with echo ACK and index reassembly
- [x] LED config (effect, brightness, speed, colour, sleep, debounce)
- [x] Per-key RGB + themes
- [x] Idle-key guard before writing (anti key-stuck)
- [x] 🖥️ GUI with keyboard drawing, section navbar and fixed legends
- [x] 🔀 Remapping (`0x01`, 504 B/layer) with *Active remaps* + *undo*
- [x] ⏺️ Macros (`0x03`, 512 B) with save, bind and delete
- [x] 🎵 Real-time lighting (`0x88`, ~14 fps, anti-flicker)
- [x] 🧠 `AGENTS.md` — complete protocol reference for development
- [x] 🖴 Tray icon (SNI/ksni) with Show/Quit
- [ ] 🔗 Cable transport `258a:010c` (usage page `0xFF00`, 520 B feature report)
- [ ] 🐧 Final Linux validation (udev/hidraw, Bluetooth)
- [ ] ⏺️ List stored macros in the GUI (CLI only today)
- [ ] 🤖 GitHub Actions builds (Linux x86_64)

---

## 🤝 Credits

- Base protocol: [bakiakyol/aula-f75-hub](https://github.com/bakiakyol/aula-f75-hub) 📖
- Re-validation, corrections and original findings: **this project** (see [`AGENTS.md`](AGENTS.md)) 🔬
- Key table extracted from the official Windows driver's `KB.ini`

## 📄 License

**MIT**
