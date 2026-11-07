# ⌨️ aula-f75-rs

🦀 Ferramenta para **Linux x86_64** em *Rust* para configurar o teclado mecânico **AULA F75** — um substituto completo e independente do software oficial `OemDrv.exe` (exclusivo do Windows). Desenvolvida e testada em **Arch Linux** (niri + Wayland).

> 🎯 **Binário único. Sem driver. Sem instalador. Sem dependências externas.**
> Toda a comunicação é feita por HID padrão (`hidapi`) — remaps, macros e iluminação são gravados **dentro do teclado** e valem em qualquer computador depois.

📦 **Repo:** `https://github.com/antraxbr666/aula-f75-rs.git` · 🧠 **Documentação técnica completa:** [`AGENTS.md`](AGENTS.md)

---

## ✨ Destaques

| 🏆 | Recurso |
|---|---|
| 🖥️ | **GUI** (egui) com desenho do teclado, legendas fixas e navbar de ícones |
| 🔀 | **Remapeamento** por tecla (base + Fn) com *combo*, mídia, macro ou `none` |
| 📋 | **Active remaps**: lista ao vivo do que difere da fábrica, com *undo* por tecla e *undo all* |
| ⏺️ | **Macros** gravadas no teclado (16 slots) com combos, holds e pausas |
| 🌈 | **RGB por tecla** + temas prontos (*rainbow, ocean, fire, matrix, sunset, off*) |
| 🎵 | **Animações em tempo real** a ~14 fps direto do PC (CLI) |
| 🔋 | **Bateria**, sleep, debounce, win-lock, factory reset — tudo pela GUI |
| 🧵 | Toda IO HID em *thread* própria — **a interface nunca trava** |
| 🐧 | **Linux** com regra udev incluída (sem sudo) |

---

## 📡 Como funciona

O teclado escuta num **canal HID vendor**: report `0x13`, pacotes de **20 bytes** com *checksum* aditivo de 8 bits, payloads de até 14 B e **ACK por eco** (o teclado devolve cada pacote). Blocos grandes (até 512 B) são remontados por índice. O protocolo foi documentado pela comunidade em [aula-f75-hub/docs/protocol.md](https://github.com/bakiakyol/aula-f75-hub/blob/main/docs/protocol.md) e **revalidado aqui, byte a byte, contra o hardware real** — inclusive onde a doc da comunidade erra (ex.: índices *53 = Fn* e *59 = RCtrl*).

| 🔌 Conexão | VID:PID | Canal | Status |
|---|---|---|---|
| 📶 Dongle 2.4G | `3554:fa09` | usage page `0xFF02` | ✅ **verificado** (leitura, escrita, bateria, reset) |
| 🟦 Bluetooth | `3554:fa07` | usage page `0xFF02` | ✅ mesmo canal, verificado |
| 🔗 Cabo USB | `258a:010c` | usage page `0xFF00` (feature report 520 B) | ❌ *transporte diferente — no roadmap* |

> 💡 **Importante:** efeitos de firmware (1-31) são calculados *dentro* do teclado — não há tabela para ler. Só o efeito **32 (custom)** renderiza a tabela RGB por tecla (`0x02`). Ao pintar teclas ou aplicar um tema, o app **troca para 32 automaticamente**.

---

## 🛠️ Compilar

**Pré-requisitos:** Rust estável + headers do libudev (Arch: já vêm com o `systemd`; Debian/Ubuntu: `libudev-dev`).

```bash
git clone https://github.com/antraxbr666/aula-f75-rs.git
cd aula-f75-rs
cargo build --release        # → target/release/aula-f75
```

### 🐧 Linux: permissão sem sudo

Copie a regra udev incluída e reconecte o dongle:

```bash
sudo cp udev/70-aula-f75.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger
```

---

## 🖥️ GUI

```bash
aula-f75 gui
```

Navbar vertical de ícones à esquerda; cada aba abre no painel direito. O desenho do teclado no centro mostra a **camada custom** (editor) com as cores reais lidas do hardware.

> 🖴 **Bandeja:** ícone residente (StatusNotifierItem; aparece no quickshell, waybar etc.) com menu **Show** (abre ou foca a janela) e **Quit** (encerra tudo). **Clique esquerdo no ícone = Show.** O ícone vive no daemon `aula-f75 tray`, que roda a janela como **processo filho** — fechar a janela (X, niri `Mod+C`) fecha **só a janela**, o ícone continua e **Show** traz ela de volta.
>
> 🪟 **App ID** da janela: `aula-f75-rs` — use em window rules do niri, ex.: `match app-id="aula-f75-rs"`

```bash
# residente no autostart (niri/quickshell/waybar): só o ícone, janela pelo menu
aula-f75 tray --no-window
# com janela já no startup
aula-f75 tray
```

```kdl
// ~/.config/niri/config.kdl
spawn-at-startup "aula-f75" "tray" "--no-window"
```

| Ícone | Aba | O que faz |
|---|---|---|
| 💡 | **Lighting** | efeito (17 de hardware), brilho 0-9, velocidade 0-4, cor 0-7 |
| ⌨ | **Remap** | selecione teclas no board → aplique target; **Active remaps** com *undo* individual e *undo all* |
| ⏺ | **Macros** | salve/apague slots e atribua a teclas com modos de execução |
| 🌈 | **Themes** | temas prontos com um clique (aplicar = troca para custom) |
| ⚙ | **Settings** | 💤 sleep (tempos do app original), ⏱ debounce, ♻ factory reset com confirmação |
| ℹ | **Device** | transporte, VID:PID, strings HID, bateria |

**Destaques do board:**

- 🏷️ Legendas *sempre visíveis* (abreviações reais de keycap: `bksp`, `caps`, `shift`…)
- 🟠 Borda **laranja** = tecla com remap ativo na camada selecionada (sai na hora ao desfazer)
- ⬜ Clique seleciona · **Ctrl+clique** adiciona à seleção
- 🟢 Status + 🔋 bateria no cabeçalho; reconexão automática (5 s) após reset/replug

---

## ⌨️ CLI

### 💡 Iluminação

```bash
aula-f75 list                 # 🔍 dispositivos conectados (usage page + path)
aula-f75 info                 # 📊 efeito, brilho, velocidade, bateria, settings
aula-f75 effect 6             # 🌈 efeito (números de hardware; ver `info`)
aula-f75 brightness 5         # ☀️ 0-9
aula-f75 speed 2              # ⚡ 0-4
aula-f75 color blue           # 🎨 0-7 ou nome (red green blue yellow purple cyan white multicolor)
aula-f75 key-color w=ff0000 a=00ff00 s=00ff00 d=00ff00   # 🖌️ pinta teclas (troca p/ custom)
aula-f75 preset fire          # 🌠 rainbow · ocean · fire · matrix · sunset · off
aula-f75 live rainbow 15      # 🎵 animação em tempo real: rainbow · wave · fire · #rrggbb
```

### 🔀 Remap (camadas 0 = base, 1 = Fn)

```bash
aula-f75 remap 0                     # 📋 keymap decodificado
aula-f75 remap 0 --raw               # 📄 bloco bruto em hex (1008 chars)
aula-f75 remap 0 capslock=esc        # ✅ aplica + read-back
aula-f75 remap 0 f1=ctrl+shift+t f2=none
aula-f75 remap 1 f5=macro:0:2        # Fn: slot 0, repetir enquanto segura
```

**Targets aceitos:**

| Forma | Exemplo | Resultado |
|---|---|---|
| tecla | `esc`, `f5`, `pgup` | uso HID simples |
| modificadores + tecla | `ctrl+shift+t` | combo |
| duas teclas | `a+b` | combo sem modificador |
| `none` | `none` | tecla morta |
| `macro:IDX[:modo]` | `macro:0:2` | roda macro (modo 1, 2 ou 4) |
| `consumer:HEX` | `consumer:0E9` | mídia (volume+, `0EA` vol−, `0E2` mute…) |
| `special:HEX` / `fnfunc:B1.B2.B3` | `special:11` | funções internas (bateria etc.) |

### ⏺️ Macros

```bash
aula-f75 macro list                          # 📋 macros gravadas (decodificadas)
aula-f75 macro set 0 hello "h , e , l , l , o , enter"
aula-f75 macro assign pgup 0                 # ⌨️ liga à tecla (modos 1/2/4)
aula-f75 macro del 0                         # 🗑️ apaga slot (seguintes sobem)
```

**Sintaxe da sequência** (tokens por espaço/vírgula; atraso padrão 25 ms):

| Token | Significado |
|---|---|
| `h` · `enter` · `f5` | pressiona e solta |
| `ctrl+shift+t` | modificadores segurados + tecla |
| `^shift` | **segura** sem soltar (hold) |
| `shift` (após um `^shift`) | **solta** o hold |
| `wait:300` | pausa de 300 ms após o evento anterior |

### 🔧 Diagnóstico

```bash
aula-f75 battery             # 🔋 percentual (experimental)
aula-f75 strings             # 🏷️ descritores HID (debug)
aula-f75 dump                # 📄 bloco LED bruto (hex)
aula-f75 selftest            # ✅ lê → regrava idêntico → confere
aula-f75 keys                # ⌨️ nomes de teclas + índice LED + x/y
```

---

## ⚠️ Notas de segurança & comportamento

- 🖐️ **Key-release perdido:** escrever na flash com tecla pressionada pode deixar uma tecla *repetindo*. Por isso todo write espera **teclas soltas + 400 ms de idle** (`wait_keys_idle`) antes de agir.
- 🔄 **Factory reset** (`Fn+Esc` segurando, ou pela GUI): o teclado **reinicia e re-pareia** — leituras falham por alguns segundos *por design*; a GUI reconecta sozinha (5 s).
- 🚫 **GUI e CLI não ao mesmo tempo** — dois processos intercalam pacotes HID e corrompem leituras multi-pacote.
- 🔒 Nada aqui toca firmware/bootloader. Só blocos de configuração documentados.
- 🔤 A versão do firmware **não é exposta** pelo dongle 2.4G — a GUI informa isso honestamente.

---

## 🗺️ Roadmap

- [x] Leitura/escrita de blocos com ACK por eco e remontagem por índice
- [x] LED config (efeito, brilho, velocidade, cor, sleep, debounce)
- [x] RGB por tecla + temas
- [x] Guarda de teclas soltas antes de escrever (anti key-stuck)
- [x] 🖥️ GUI com desenho do teclado, navbar de seções e legendas fixas
- [x] 🔀 Remapeamento (`0x01`, 504 B/camada) com *Active remaps* + *undo*
- [x] ⏺️ Macros (`0x03`, 512 B) com gravação, atribuição e exclusão
- [x] 🎵 Iluminação em tempo real (`0x88`, ~14 fps, anti-flicker)
- [x] 🧠 `AGENTS.md` — referência completa do protocolo para desenvolvimento
- [x] 🖴 Ícone na bandeja (SNI/ksni) com Show/Quit
- [ ] 🔗 Transporte cabo `258a:010c` (usage page `0xFF00`, feature report 520 B)
- [ ] 🐧 Validação final no Linux (udev/hidraw, Bluetooth)
- [ ] ⏺️ Listar macros gravadas na GUI (hoje só via CLI)
- [ ] 🤖 Builds no GitHub Actions (Linux x86_64)

---

## 🤝 Créditos

- Protocolo base: [bakiakyol/aula-f75-hub](https://github.com/bakiakyol/aula-f75-hub) 📖
- Revalidação, correções e descobertas próprias: **este projeto** (ver [`AGENTS.md`](AGENTS.md)) 🔬
- Tabela de teclas extraída do `KB.ini` oficial do driver Windows

## 📄 Licença

**MIT**
