# AGENTS.md — AULA F75 reverse-engineered configuration tool

This document is the **single source of truth** for continuing development on this
project without the official driver (OemDrv.exe) or any external reference.
Everything below was verified against real hardware (2.4G dongle + Bluetooth)
unless explicitly marked otherwise. Read this file fully before touching
`src/` — it encodes weeks of hard-won protocol knowledge and non-obvious
firmware behavior that is **not** discoverable from the community doc alone.

- Repo: https://github.com/fredjunior54/aula-f75-rs.git (branch `main`)
- Stack: Rust, `hidapi` (HID IO), `egui`/`eframe` 0.29 (GUI), `ksni` (SNI tray),
  `clap` (CLI)
- Platform: **Linux x86_64 only** (Windows support dropped; developed and
  tested on shadowBox — Arch, niri + Wayland)
- All HID communication is hardcoded via `hidapi` — no vendor driver required.

---

## 1. Build & run

```bash
cargo build                 # debug
cargo build --release
cargo run -- gui            # graphical app
cargo run -- info           # CLI: current effect/brightness/battery/settings
cargo run -- remap 0        # CLI: dump base keymap (decoded)
```

> History: the project was bootstrapped on Windows; Windows support has
> been dropped and development now happens on **shadowBox** (Arch Linux,
> niri + Wayland). The GUI (eframe/winit) runs with the native Wayland
> backend. Shell note: `#` starts a comment in fish/bash too — quote it in
> arguments: `cargo run -- live '#ff0000'`.

### Linux requirements

- `hidapi` uses the **hidraw** backend. Without root you need a udev rule:
  ```
  # /etc/udev/rules.d/70-aula-f75.rules
  KERNEL=="hidraw*", ATTRS{idVendor}=="3554", MODE="0660", TAG+="uaccess"
  ```
  then `sudo udevadm control --reload-rules && sudo udevadm trigger` and
  re-plug the dongle. libudev headers are required at build time — on Arch
  they ship with `systemd` (already present on any normal install); on
  Debian/Ubuntu: `apt install libudev-dev libudev1`.
- The 2.4G dongle exposes **multiple hidraw nodes** (one per HID collection).
  `Keyboard::open` already selects the right one (see §3) — on Linux also
  verify `usage_page()` filtering works identically (hidraw exposes it via
  `HIDIOCGRAWINFO`/report descriptors; hidapi handles it).
- `wait_keys_idle()` opens the boot-keyboard collection (usage page 0x0001,
  usage 0x06) of the same VID:PID — on Linux this needs read access too
  (same udev rule covers it). If opening fails, the code warns and proceeds;
  do not turn that into a hard error.

---

## 2. Hardware & transports

| Transport | VID:PID | Usage page | Status |
|---|---|---|---|
| 2.4G dongle | `3554:FA09` | `0xFF02` (vendor config) | implemented, fully verified |
| Bluetooth | `3554:FA07` | `0xFF02` | implemented, verified |
| Wired (USB cable) | `258A:010C` | `0xFF00`, **520-byte feature report** | NOT implemented — different transport |

Known unknowns:

- The **firmware version is not exposed** over the 2.4G dongle. HID string
  descriptors exist (`strings` CLI) but carry no version. Do not promise a
  version display; the GUI states this honestly.
- Wired mode reuses the same data blocks (LED/keymap/RGB/macro) over a
  different transport (usage page 0xFF00, 520-byte feature reports, per the
  community doc). Implementing it = a second `Keyboard`-like transport in
  `device.rs`; the block layer above it stays unchanged.

---

## 3. Opening the device (critical details)

`Keyboard::open(&HidApi)` in `src/device.rs`:

1. Iterates `SUPPORTED` (dongle first, then Bluetooth).
2. For each matching VID:PID, **prefers the collection with
   `usage_page == 0xFF02`** (vendor config channel); falls back to any
   collection of that VID:PID.
3. Opens by path (`open_path`), not by VID:PID, to bind the right collection.

**Re-enumeration rule:** the keyboard re-enumerates after factory reset,
power cycles, replug, and dongle re-pairing. A stale `HidApi` device list
keeps opening dead paths (reads then time out). The GUI worker therefore
calls `api.refresh_devices()` before **every** job (`open_kb` helper in
`gui.rs`). Any new long-lived code path must do the same.
After closing an existing handle (e.g. killing the app), the dongle needs
**~3-5 seconds** before multi-packet reads succeed again — a fixed `sleep 2`
is NOT enough; build retries instead.

---

## 4. Wire protocol (report ID 0x13 channel)

Source: community doc (github.com/bakiakyol/aula-f75-hub/blob/main/docs/protocol.md)
re-validated on hardware. Where the two disagree, **this repo's code is right**.

### 4.1 Packet layout (20 bytes)

```text
[0]  0x13                 report id
[1]  command              write variant; read variant = write | 0x40
[2]  packet count         total packets in this transfer (bit 7 set in a
                          REPLY = keyboard-reported error)
[3]  packet index         0..count-1
[4]  (sub << 4) | len     len = payload length this packet (<= 14)
[5..18] payload           up to 14 bytes (PAYLOAD_MAX)
[19] checksum             8-bit additive sum of bytes 0..=18 (& 0xFF)
```

`protocol::make_packet()` builds these; `protocol::checksum()` is the sum.

### 4.2 Command numbers

| write | read | block | sub | size | meaning |
|---|---|---|---|---|---|
| 0x01 | 0x41 | keymap | layer (0 base, 1 Fn) | 504 | 126 entries × 4 bytes |
| 0x02 | 0x42 | per-key RGB | 0 | 378 | planar R[126] G[126] B[126] |
| 0x03 | 0x43 | macro store | block # (0 used) | 512 | see §6 |
| 0x04 | 0x44 | LED config | 0 | 128 | see §5 |
| 0x06 | — | factory reset | 0 | payload `[1]` | **no ack**, keyboard reboots |
| 0x09 | 0x49 | palette | ? | 490 | 23 palettes × 7 colors (unused) |
| 0x0A | 0x4A | profile | 0 | 1-2 | byte 0 = battery % (experimental) |
| 0x88 | — | real-time light | sub in high nibble of byte 4 | any | **no ack**, not stored |

### 4.3 Write flow (`write_block`)

- Split data into 14-byte chunks, send packet idx 0..count-1.
- The keyboard **echoes every packet back** as its acknowledgement. Match the
  echo on `(cmd, count, idx)`; if `echo[2] & 0x80` → keyboard reports error.
- **Stale-packet poisoning**: after a large read the keyboard sometimes
  retransmits stray reply packets. They sit in the IN queue and corrupt the
  next transfer's echo matching. `write_block` therefore **skips packets that
  don't match the expected echo** (16-read retry budget) instead of failing.
- 15 ms gap between packets (`WRITE_GAP`).

### 4.4 Read flow (`read_block`)

- Send one packet: `read_cmd`, count=1, idx=0, sub, empty payload.
- The keyboard streams the whole block back as `count` packets; reassemble by
  `idx` (each idx carries 14 bytes at `idx*14`).
- **Retransmission behavior**: re-sending the request makes the keyboard
  resend the ENTIRE reply. `read_block` retries up to 4× and then fails with
  `read 0xNN incomplete, missing packets [...]`.
- After complete reassembly, `drain()` discards duplicate tail packets
  (up to 64 reads with timeout 0) — without this the next write/read breaks.
- Read timeout: 500 ms per packet.

### 4.5 Commands with NO acknowledgement

- **0x06 factory reset** — send and pray. The keyboard **reboots and
  re-pairs for several seconds**; all reads fail in that window. The GUI
  handles this with a 5-second retry cadence while disconnected (60 s when
  connected). Expect `read 0x44 incomplete` errors right after a reset; they
  are normal, not a bug.
- **0x88 real-time** — bit 7 of the command byte means no echo/no flash
  storage. See §7.

### 4.6 Flash-write safety: `wait_keys_idle`

Before ANY flash write (keymap 0x01, macros 0x03, LED 0x04 in practice) call
`kb.wait_keys_idle(&api)`: it opens the boot-keyboard collection (usage page
0x0001, usage 0x06) non-blocking and waits until no keys are held and the
input has been quiet for 400 ms (5 s deadline, then proceeds with a warning).
Reason: a flash write during key activity can make the keyboard drop a
key-release report → key auto-repeats forever. Boot report parse quirk:
`0x01` in the keys array is ErrorRollOver, not a held key.

---

## 5. LED config block (0x04/0x44, 128 bytes)

Offsets that matter (`protocol::led`):

| off | meaning |
|---|---|
| 1 | debounce (ms), 0-8 |
| 10 | **active effect** (hardware numbering — see table below). Used as `effect.max(1)` when computing `effect_settings` offset. |
| 15 | win lock: 0 off, 1 locked |
| 18 | side light: 0 off, 1 fill, 2 breathing, 3 static |
| 24 | sleep timer in 30 s units, 0 = never |
| 26 | knob mode: 0 media, 1 lighting |
| 27 | OS mode: 0 Windows, 1 Android, 2 Mac |
| 58 + 2*(effect-1) | per-effect settings pair: `[brightness(0-9), (speed<<4)\|color(0-7)]` |
| 126-127 | signature `5A A5` — **read_led() validates this**; mismatch = wrong transport |

Sleep choices (exactly as the official app): 60s→2, 90s→3, 120s→4, 180s→6,
240s→8, 300s→10, 600s→20, 900s→30, 1200s→40 (units of 30 s).

**Effect numbering — hardware truth (the official app's numbers do NOT match
the firmware):**

```
1 static        2 breathing     3 spectrum      4 ripple-row   5 raindrops
6 wave          7 ripple        8 twinkle      10 snake       11 stream
12 reactive    13 center-wave  15 rainbow-wheel 16 waterfall  17 center-flash
32 custom (per-key RGB table, cmd 0x02)
```

Named colors 0-7: red, green, blue, yellow, purple, cyan, white, multicolor.
Brightness 0-9, speed 0-4.

**CRITICAL UX FACT:** effects 1-31 are computed **inside the keyboard** —
there is no table to read back. Only effect **32** renders the per-key RGB
table (0x02). The GUI's board drawing shows the 0x02 table and is, by
design, an *editor for the custom layer*, not a mirror of the keyboard.
Painting keys / applying a theme auto-switches the keyboard to effect 32
(`ACTIVE_EFFECT = 32` + write block). The factory default custom table has
esc/WASD/arrows blue ("game mode" highlight) — that's real factory data,
not a bug.

---

## 6. Keymap / remap (0x01/0x41)

504 bytes per layer = 126 entries × 4 bytes. `sub` = layer (0 base, 1 Fn;
layers 2-3 exist in the CLI but are unverified). Flash write → needs
`wait_keys_idle`.

### 6.1 Entry format

```text
type byte1                byte2          byte3
0x00 [modifier bits]      [usage2 opt]   [usage1]      plain key / combo
     all zero                                          = (none) — dead key
     b3==0, b1!=0                                       = pure modifier key
0x02 —                     usage16 = (byte2 << 8) | byte3              consumer/media
0x03 [mode 1|2|4]          [count]        [macro index] macro binding
0x07 24-bit code, big-endian across bytes 1..3                         special function
0x08 b1.b2.b3 triple                                                    Fn function
0x0D —                                                                  the Fn key itself
```

- Type 0x00 with two usages = two-key combo (byte2 pressed with byte3).
- Modifier bits (byte 1, and HID usages E0-E7):
  `01 lctrl, 02 lshift, 04 lalt, 08 lwin, 10 rctrl, 20 rshift, 40 ralt, 80 rwin`.
- Macro modes: 1 = run once, 2 = repeat while held, 4 = repeat until any key.

### 6.2 Special function codes (type 0x07), from the factory Fn layer

```
0x01 win-lock            0x04 factory-reset (hold)   0x05/06/07 bluetooth 1/2/3
0x08 2.4G                0x11 battery-gauge          0x18/19/1A windows/android/mac mode
0x1D knob-press (mute/media)
```

### 6.3 Fn function codes (type 0x08) — (b1, b2, b3) → meaning

```
(0,0,0) cycle-effect   (0,0,1) cycle-side-light  (2,0,0) cycle-color
(3,1,0) brightness-up  (3,2,0) brightness-down   (4,1,0) speed-up  (4,2,0) speed-down
```

### 6.4 Consumer usages seen in the factory Fn layer (type 0x02)

```
018A mail   0223 browser-home   00B6 media-prev   00CD media-play
00B5 media-next   00E2 mute   00EA volume-down   00E9 volume-up
```

### 6.5 Matrix / LED index (THE critical table)

**Matrix index == per-key RGB index == LED index.** Physical 75% board uses
indices 0..=89 plus the knob at 84 (values 90-125 in the RGB planes are
unused). Full table (`remap::MATRIX`, extracted from the official
`Dev/kb/wired/KB.ini` and proven by reading the factory keymap):

```
 0 esc      1 grave     2 tab      3 capslock 4 lshift   5 lctrl
 7 1        8 q         9 a       10 z       11 lwin    12 f1
13 2       14 w        15 s       16 x       17 lalt    18 f2
19 3       20 e        21 d       22 c       24 f3
25 4       26 r        27 f       28 v       30 f4
31 5       32 t        33 g       34 b      35 space    36 f5
37 6       38 y        39 h       40 n       42 f6
43 7       44 u        45 j       46 m       48 f7
49 8       50 i        51 k       52 comma   53 fn      54 f8
55 9       56 o        57 l       58 period  59 rctrl   60 f9
61 0       62 p        63 semicolon  64 slash 66 f10
67 minus   68 lbracket 69 quote   70 rshift  72 f11
73 equal   74 rbracket 77 left    78 f12
79 backspace 80 backslash 81 enter 82 up   83 down
84 knob    85 delete   86 pgup    87 pgdn    88 end    89 right
```

**Indices 53/59 gotcha:** the community keymap.py claims 53=ralt, 59=fn.
That is WRONG. The keyboard's own factory keymap has the Fn entry (0x0D) at
**53** and the RCtrl modifier at **59** — matching KB.ini. Trust the table
above.

`keymap::KEYS` additionally carries each key's x/y pixel position for the
board drawing (canvas ~670×290, key rect 36×27).

### 6.6 Factory baseline (`src/factory.rs`) — read this before touching it

`factory::BASE` / `factory::FN` are the two 504-byte factory keymaps,
captured from a factory-reset unit over the dongle. The GUI computes
"Active remaps" by diffing current layers against these tables, and the
`undo` buttons restore entries from them.

**Poisoning hazard (this bit us once):** the baseline was first captured
AFTER the user had remapped f8→enter, so "active remaps" showed 0 forever.
If you ever need to regenerate it:

1. Factory-reset the keyboard (CLI or Settings tab), wait for it to reboot
   and the GUI to reconnect (green + battery).
2. Kill every process using the device (GUI included!) and wait ~5 s.
3. `cargo run -- remap 0 --raw > l0.hex` and `remap 1 --raw > l1.hex`
   (each prints 1008 hex chars).
4. Sanity-check: entry 0 = `00000029` (esc identity), entry 53 =
   `0d000000` (Fn), f8 (54) = `00000041`, entry 59 = `00100000` (rctrl).
5. Regenerate `factory.rs` (hex → `0x.., ` joined, 16 per line).
6. Diff-test: `remap 0 capslock=esc`, re-read raw — exactly ONE byte must
   differ (byte 4*3+3 = 15). Then `remap 0 capslock=capslock` and confirm
   the raw read is byte-identical to the baseline again.

### 6.7 Remap target syntax (`remap::parse_target`)

Accepted forms: `none` · single usage (`esc`, `f5`) · `ctrl+shift+t`
(modifiers + one key) · `a+b` (two-key combo) · `macro:IDX[:mode]` ·
`consumer:HEX` (e.g. `0E9` volume-up) · `special:HEX` · `fnfunc:B1.B2.B3`.
Friendly modifier aliases: ctrl/control, shift, alt/opt/option,
win/gui/super/cmd/meta → left-hand bit.

---

## 7. Per-key RGB (0x02/0x42) and real-time lighting (0x88)

### 7.1 Stored per-key table (0x02)

378 bytes, **planar**: R[0..126] G[126..252] B[252..378]. Index = matrix
index (§6.5). Only drives effect **32** (custom). Writing it does not change
the active effect — set `ACTIVE_EFFECT=32` in the LED block too if you want
it visible immediately.

### 7.2 Real-time frames (0x88) — hardware-verified constraints

- **No acknowledgement** (bit 7 of cmd), nothing stored in flash.
- sub **2**: whole board one color, payload = `[R, G, B]`.
- sub **1**: per-key groups `R G B count idx…` repeated, payload ≤ 200 bytes
  per frame; keys NOT listed go dark.
- **Packet gap 5 ms** — 0-1 ms shows nothing, 2-3 ms drops frames.
- **~9 packets/frame, 70 ms frame period → ~14 fps**.
- The keyboard holds the last frame and falls back to its own effect
  **~1.5-2 s** after the last frame.
- **Flicker rule:** keys that change GROUP between frames flicker. Keep each
  key in a fixed band across the whole animation and only change colors
  (`live.rs` uses fixed column/row bands for exactly this reason).

---

## 8. Macro store (0x03/0x43, block 0, 512 bytes)

### 8.1 Layout

```text
header   per macro: [address u16 LE][size u16 LE]; the FIRST address equals
         the header size, giving the macro count (count = addr0 / 4)
macro    [name length][name bytes][event × n]
event    [(kind << 4) | delay.19..16 | 0x80 if release,
          delay bits 15..8, delay bits 7..0, code]
kind     0 key (HID usage), 1 modifier (usages E0-E7), 2 mouse button,
         3/4 mouse move X/Y, 5 wheel
delay    ms until the NEXT event (20-bit in hardware; we use 16)
```

Verified against the factory macro `M1` (= Ctrl+`).

### 8.2 FIRMWARE QUIRK — bytes 126-127 are corrupted

The firmware writes `5A A5` over store bytes 126-127, destroying whatever
lands there. `macros::Store::build()` relocates any macro that would cross
bytes 126-127 to offset 128 (`DODGE_TO`). Never remove this; a macro stored
across those bytes gets silently truncated/corrupted on the keyboard.

### 8.3 Sequence syntax (`macros::parse_sequence`)

Tokens separated by spaces or commas; default inter-event delay 25 ms:

- `h`, `enter`, `f5`, `up` — press + release
- `ctrl+shift+t` — modifiers held while the final key taps (one main key max
  per token)
- `^shift` — press WITHOUT release (hold); a later bare `shift` token
  RELEASES it
- `wait:300` — set delay on the PREVIOUS event (ms until next)

### 8.4 Capacity

16 slots share the ~500 usable bytes (minus the 4-byte-per-macro header and
the dodge gap). Long macros eat neighboring slots' space.

---

## 9. Battery (0x4A), strings, factory reset

- `battery()` reads the 2-byte profile block; byte 0 = percent
  (experimental, unverified at low charge).
- `strings()` reads HID manufacturer/product/serial descriptors — no
  firmware version lives there over the dongle.
- Factory reset = packet `0x06, count=1, idx=0, sub=0, payload [1]`.
  No ack; keyboard reboots + re-pairs (several seconds of failed reads are
  NORMAL); factory keymaps/macros/LED config all return to factory,
  including the factory custom RGB table (esc/WASD/arrows blue) and the
  factory firmware effect.

---

## 10. Codebase map

```
src/
  main.rs      clap CLI; every subcommand is a working protocol example
  protocol.rs  packets, checksum, commands, block lens, LED offsets,
               EFFECTS/COLORS tables, realtime constants
  device.rs    discovery/open, write_block/read_block/drain, wait_keys_idle,
               battery/strings/factory_reset, send_realtime
  keymap.rs    KEYS (name, x, y, led) for the board drawing; build_block()
               (per-key colors → planar 378); label() short legends
  remap.rs     MATRIX index table, HID usages, modifiers, SPECIALS,
               FN_FUNCS, CONSUMERS, entry_desc(), parse_target()
  macros.rs    Event/Store parse+build (with the 126-127 dodge),
               parse_sequence()
  themes.rs    built-in themes computed from key x/y (rainbow, ocean, fire,
               matrix, sunset, off) → per-key blocks
  live.rs      Anim::{Rainbow,Wave,Fire,Solid}, band construction, play()
  factory.rs   factory BASE/FN keymap constants (see §6.6!)
  gui.rs       egui app: worker thread + mpsc jobs, sections, board drawing
  fonts.rs     CaskaydiaCove Nerd Font embedded (primary family + text sizes)
  tray.rs      ksni SNI tray: Show/Quit menu, embedded icon (daemon side)
  daemon.rs    `aula-f75 tray`: supervises the GUI child + reaper thread
  ipc.rs       unix control socket (focus / single-instance lock)
```

### 10.1 GUI architecture rules (keep these)

- **All HID IO happens on one worker thread** (`worker()` in gui.rs) fed by
  an mpsc channel of `Job`s; replies come back as `Reply`s polled each frame.
  The UI thread never blocks on the device. Preserve this.
- The worker calls `api.refresh_devices()` before EVERY open (`open_kb`) —
  required for post-reset/replug reconnection.
- Reconnect cadence: 60 s refresh when connected, **5 s when disconnected**
  (keyboard is gone for seconds after reset/replug).
- After any job finishes, the GUI re-reads full state (`Job::Read`) to
  re-sync. Keymap-writing jobs ALSO return the written block in
  `Reply::KeymapsUpdated` and the GUI applies it **immediately** (optimistic
  update) so board markers/lists never go stale when the follow-up read
  fails on a flaky dongle. Don't regress this.
- Sections (left icon navbar): Lighting, Remap, Macros, Themes, Settings,
  Device. Live and Per-key were REMOVED at the user's request — do not
  re-add them. Animation streaming remains CLI-only (`live` subcommand).
- Board drawing = editor for the custom RGB layer. Keys with an active
  remap in the selected layer get an orange stroke; selection = white
  stroke; Ctrl-click adds to selection.
- **Tray (ksni/SNI, `src/tray.rs` + `src/daemon.rs`)**: resident icon with
  Show/Quit menu, hosted by the **`aula-f75 tray` daemon**, not by the GUI
  process. This is the close-to-tray model Wayland allows: close-to-tray via
  `CancelClose` + `Visible(false)` was tried and DOES NOT work (`winit`'s
  `set_visible` is a no-op on Wayland; commit `8cd62c6`, reverted in
  `0179265`) — do not resurrect it. Instead the daemon runs the window as a
  **child process** (`aula-f75 gui`): the compositor closing the window (X,
  niri Mod+C) kills only that child, the icon stays, and "Show" relaunches
  it. Windows outlive the daemon (an orphaned GUI keeps working, just
  without a tray).
- **Tray/GUI wiring**: the GUI **owns** a control socket
  (`$XDG_RUNTIME_DIR/aula-f75-<user>.sock`, `src/ipc.rs`, stdlib only) and
  `ipc::bind()` doubles as the single-instance lock — a second `aula-f75 gui`
  gets `AddrInUse`, sends `focus` to the live window and exits 0. Stale
  sockets left by a killed GUI are removed at bind time. The daemon only ever
  *connects* to it: `request_focus()` returns true when a window is alive
  (then it just focuses), false when it must spawn one. Messages are one
  `focus` line per connection; the watcher thread calls
  `Visible(true)`+`Focus`+`request_repaint` on the cloned `egui::Context`.
- **No HID in the daemon**: only the GUI process touches the device (keeps
  §11.1 "never two processes on the device" honest). `Tray`/`Gui` are
  dispatched in `main()` **before** `HidApi::new()` for that reason.
  Daemon crash detection is a 500 ms `try_wait` reaper thread (the window is
  closed by the compositor, not by us). "Quit" kills the child and exits the
  process (the SNI name dies with the D-Bus connection). ksni 0.3.7 blocking
  has no `wait()`; `daemon::run` idles on `handle.is_closed()`.
- **Window app id**: `gui::APP_ID` = `"aula-f75-rs"` (`.with_app_id(...)` on
  the root `ViewportBuilder`; egui-winit forwards it to
  `winit::platform::wayland::WindowAttributesExtWayland::with_name`). Without
  it the compositor reports the app id as unset. niri window rules target
  `app-id="aula-f75-rs"`.
- **Tray click**: `AulaTray::activate` (left click on the icon) sends `toggle`
  over the control socket (`src/ipc.rs`): with a window up the GUI closes
  itself (`ViewportCommand::Close` on the root viewport), with none the daemon
  spawns one. The menu **Show** item always means focus-or-open — it never
  closes. ksni's `MENU_ON_ACTIVATE` stays at its `false` default, so the host
  sends `Activate` instead of popping the menu; `secondary_activate` (middle
  click) is unused. Do not make `toggle` depend on focus state: the simple
  "alive = close" rule is the owner's choice.
- **Visuals**: the whole window uses **Catppuccin Mocha** via
  `catppuccin-egui` (`src/gui.rs`, `GuiApp::new`:
  `catppuccin_egui::set_theme(&cc.egui_ctx, catppuccin_egui::MOCHA)`). The
  crate gates each egui version behind a feature flag: `Cargo.toml` pins
  `default-features = false, features = ["egui29"]` — **when bumping
  eframe/egui, bump that feature** (`egui30`, `egui31`, …) or the theme link
  fails to compile. Colors still hardcoded in the drawing (orange remap
  stroke, white selection, key legends) are intentional; Mocha base is
  `#1e1e2e`, surface0 `#313244`.
- Icon bytes are pre-rendered straight-alpha RGBA from
  `assets/icon-source.png` (512x512 flat blue keyboard, transparent
  background — a saturated `#2f6ff5`-ish plate that reads on both dark and
  light panels): `assets/icon-256.rgba` feeds the window icon and the tray,
  `assets/icon-64.rgba` is published alongside it for small panels. `ksni`
  wants ARGB32, so `tray::to_argb()` rotates every pixel; egui wants
  straight RGBA. **Crop to the artwork, pad, then downscale**, otherwise the
  icon looks small next to other tray icons: the source has a wide asymmetric
  margin (artwork is 470x377 of the 512 canvas, offset +74 vertically).
  Measured after the pipeline: 96%x78% — the artwork is much wider than tall,
  so the vertical figure cannot get higher without distorting it.
  ```bash
  W=$(magick assets/icon-source.png -trim -format "%w" info:)
  H=$(magick assets/icon-source.png -trim -format "%h" info:)
  S=$(( W > H ? W : H )); P=$(( S * 3 / 100 ))   # 3% breathing room
  magick assets/icon-source.png -trim +repage -background none -gravity center \
    -extent ${S}x${S} -bordercolor none -border $P -filter Lanczos \
    -resize 256x256 -depth 8 rgba:assets/icon-256.rgba
  # same, with -resize 64x64 -> rgba:assets/icon-64.rgba
  ```
  Sizes must stay exact (`256*256*4 = 262144`, `64*64*4 = 16384` bytes).
  The tray publishes both pixmaps and lets the host pick (`icon_pixmap`);
  a 22 px panel downsamples the 256 one, which looks better than a pre-shrunk
  64 in most hosts. For an SVG source the renderer here is `rsvg-convert`
  (there is no `resvg`): render big first (`-w 1024 -h 1024`), then run the
  same trim/extent/border/downscale steps on the PNG.

### 10.2 Fonts and text sizes (read before writing UI strings)

The app ships **CaskaydiaCove Nerd Font Mono** compiled into the binary
(`assets/CaskaydiaCoveNerdFontMono-Regular.ttf`, ~2.9 MB, `fonts::install`):

- It is the **primary** family for both `Proportional` and `Monospace`; the
  fonts bundled with egui stay behind it as fallback, so emoji and any glyph
  Cascadia lacks still render.
- Text sizes: `Small` 11, **Body/Button/Monospace 14**, `Heading` 20 — egui's
defaults were 9/12.5/18/12 and were too small on the owner's display.
- Because of the Nerd Font, these now render (used by the icon navbar):
  `\u{F0EB}` bulb, `\u{F11C}` keyboard, `\u{F022}` list, `\u{F1FC}` brush,
  `\u{F013}` gear, `\u{F05A}` info, `\u{F021}` refresh, `\u{F241}` battery,
  `\u{F060}..\u{F063}` arrows.
- **Unicode arrows `U+2190-2193` are fine now** (Cascadia has them) — the old
  "use ASCII up/dn/lt/rt" rule applied to egui's default font and no longer
  needs to be followed.
- **U+FE0F (emoji variation selector) is still forbidden**: no font here
  handles it, so `emoji + FE0F` keeps producing a tofu square.
- Emojis render monochrome; if a label shows a box, prefer a Nerd codepoint
  over guessing.
- Board key legends are intentionally `FontId::proportional(10.0)` (hardcoded
  in the drawing), independent of the scales above — do not tie them to the
  global styles or the 36x27 key rects will clip.

---

## 11. Operational lessons (the fireproofing list)

1. **Never run the GUI and CLI against the device at the same time.** Two
   processes interleave HID packets and multi-packet reads collapse with
   `read 0xNN incomplete`. Kill one before using the other.
2. After killing a process that held the device, **wait ~5 s** before
   heavy reads (dongle re-settles).
3. `read 0x44/0x41 incomplete, missing packets [0,1,...]` right after
   factory reset/replug is EXPECTED — retry, don't "fix".
4. Before every flash write: `wait_keys_idle` (dropped key-release = stuck
   auto-repeat key).
5. Reads must `drain()` after reassembly; writes must skip non-matching
   queued packets (stale-packet poisoning — both already in device.rs).
6. Re-read the block after writing (read-back) and compare — the CLI does
   this (`updated and read-back OK`); keep the pattern in new code.
7. The 2.4G dongle occasionally flakes on a full read even when idle —
   worker-side retries and the GUI's optimistic keymap update exist for
   this reason.
8. `#` starts a comment in fish and bash too — always quote it in args:
   `cargo run -- live '#ff0000'`.
9. Baseline capture must happen ONLY at verified factory state (§6.6).
10. **Rebuilding while `aula-f75 tray` runs breaks its "Show"**: the daemon
    resolves `std::env::current_exe()` from `/proc/self/exe`, which reports
    `.../aula-f75 (deleted)` after cargo replaces the binary, so spawning the
    GUI fails with `os error 2`. Restart the daemon after every rebuild
    (`pkill -f 'target/debug/aula-f75 tray'` — quote it so the pattern does
    not match your own shell).

---

## 12. CLI reference (all hardware-verified workflows)

```
aula-f75 list                          # devices + usage page + hidraw path
aula-f75 info                          # effect/brightness/speed/battery/settings
aula-f75 effect 6                      # set active effect (hardware number)
aula-f75 brightness 5                  # 0-9
aula-f75 speed 2                       # 0-4
aula-f75 color blue                    # 0-7 or name
aula-f75 battery                       # experimental %
aula-f75 strings / dump / selftest     # descriptors / raw LED hex / write path
aula-f75 keys                          # key names + led idx + x/y
aula-f75 key-color w=ff0000 a=00ff00   # paint keys (switches to effect 32)
aula-f75 preset fire                   # theme: rainbow ocean fire matrix sunset off
aula-f75 remap 0                       # dump layer decoded
aula-f75 remap 0 --raw                 # dump layer as 1008-char hex
aula-f75 remap 0 capslock=esc f1=ctrl+shift+t
aula-f75 remap 1 f5=macro:0:2          # Fn layer, macro slot 0, repeat-while-held
aula-f75 macro list                    # stored macros decoded
aula-f75 macro set 0 name "h e l l o enter"
aula-f75 macro del 0
aula-f75 live rainbow 10               # stream animation ~14 fps
aula-f75 live '#ff0000'                # solid color (quote # in shells)
aula-f75 gui                          # window only (no tray); 2nd run just focuses
aula-f75 tray                         # resident tray + window (close-to-tray)
aula-f75 tray --no-window             # resident tray only (autostart); window via "Show"
```

Tray workflow: autostart with the **icon only** (`aula-f75 tray --no-window`,
e.g. `spawn-at-startup "aula-f75" "tray" "--no-window"` in
`~/.config/niri/config.kdl`); then `Mod+C` (`close-window`) closes the window
and leaves the icon; menu **Show** reopens it, **Quit** kills both.

Safe test cycle for keymap work (always restore!):

```
aula-f75 remap 0 capslock=esc     # apply
aula-f75 remap 0                  # verify decoded
aula-f75 remap 0 capslock=capslock  # restore identity
aula-f75 remap 0 --raw            # byte-compare vs factory::BASE if unsure
```

---

## 13. Roadmap / open work

- **Wired transport (258A:010C)** — usage page 0xFF00, 520-byte feature
  reports. Same blocks, different pipe. Needs a second transport impl +
  real hardware validation. This is the main protocol gap.
- **Linux hardware bring-up on shadowBox** — udev rule (§1) in daily use,
  confirm usage-page selection and `wait_keys_idle` behave identically on
  hidraw, test Bluetooth pairing path, GUI on niri/Wayland.
- **GUI macro listing** — the Macros tab can save/delete but does not LIST
  what's stored (CLI `macro list` does). Obvious UX gap, easy win.
- **GitHub Actions builds** (Linux x86_64) — deliberately deferred by the
  owner; ask before adding.
- Deliberately NOT wanted (owner's decision, do not resurrect): language
  switcher, startup entry, per-key GUI painting section, GUI Live
  section, macOS support, Windows support (dropped — Linux x86_64 only).
  (A tray icon was on this list until 2026-11, when the owner requested
  one — see §10.1.)
- PALETTE block (0x09, 490 B) is known but unused; only touch it with
  hardware in hand to validate.

## 14. Git conventions

- Dev machine is **shadowBox**, which has a global git identity
  (`AntraXClown <fredcox@gmail.com>`) — plain `git commit` uses it, no `-c`
  overrides needed. (History predating the Linux migration was committed as
  `fredc <fredc@local>` on Windows.)
- Push to `origin/main`. Commit messages in English, imperative, one topic
  per commit.
