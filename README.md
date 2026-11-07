# ⌨️ aula-f75-rs

🦀 Ferramenta multiplataforma (**Windows + Linux**) em Rust para configurar o teclado mecânico **AULA F75** — substitui o software oficial `OemDrv.exe` (exclusivo do Windows).

> 🎯 Binário único, sem driver, sem instalador, sem dependências externas.

---

## 📡 Como funciona

O teclado escuta num canal HID vendor (report `0x13`, pacotes de 20 bytes com checksum de 8 bits e ACK por eco). O protocolo foi documentado pela comunidade em [aula-f75-hub/docs/protocol.md](https://github.com/bakiakyol/aula-f75-hub/blob/main/docs/protocol.md) e **revalidado aqui contra o hardware real**.

| 🔌 Conexão | VID:PID | Status |
|---|---|---|
| 📶 Dongle 2.4G | `3554:fa09` | ✅ verificado (leitura, escrita, bateria) |
| 🟦 Bluetooth | `3554:fa07` | ✅ mesmo canal (leituras remontadas por índice) |
| 🔗 Cabo | `258a:010c` | ❌ transporte diferente (feature report 520 B) — não implementado |

## 🚀 Uso

```bash
aula-f75 list                 # 🔍 dispositivos conectados
aula-f75 info                 # 💡 efeito, brilho, velocidade, cor, bateria…
aula-f75 effect 2             # 🌈 efeito (2 = breathing; ver EFFECTS)
aula-f75 brightness 5         # ☀️ brilho 0-9
aula-f75 speed 2              # ⚡ velocidade 0-4
aula-f75 color blue           # 🎨 cor 0-7 ou nome
aula-f75 battery              # 🔋 bateria (experimental)
aula-f75 keys                 # ⌨️ nomes de teclas para o key-color
aula-f75 key-color w=ff0000 a=ff0000 s=ff0000 d=ff0000   # 🔴 pinte teclas
aula-f75 preset rainbow       # 🌈 tema: rainbow, ocean, fire, matrix, sunset, off
aula-f75 remap 0              # ⌨️ mostra o keymap (0 = base, 1 = Fn)
aula-f75 remap 0 capslock=esc # 🔀 remapeia (esc, ctrl+shift+t, none, macro:0…)
aula-f75 macro list           # ⏺️ macros gravados no teclado
aula-f75 macro set 0 hello "h , i , enter"        # ⏺️ cria macro no slot 0
aula-f75 macro assign pgup 0  # ⌨️ liga o macro à tecla (modos 1/2/4)
aula-f75 live rainbow 15      # 🎵 animação em tempo real (~14 fps)
aula-f75 dump                 # 📄 bloco LED bruto (hex)
aula-f75 selftest             # ✅ lê → regrava idêntico → confere
aula-f75 gui                  # 🖥️ interface gráfica (egui)
```

## 🖥️ GUI

```
aula-f75 gui
```

- 🎨 **Teclado desenhado na tela** com as cores reais de cada tecla (lidas do hardware)
- 🖌️ Clique para selecionar, **Ctrl+clique** para várias, pinte com o color picker e aplique
- 💡 Controles de efeito / brilho / velocidade / cor
- 🌈 Temas com um clique (rainbow, ocean, fire, matrix, sunset, off)
- ⚙️ **Settings**: sleep (on/off + tempos do app original), debounce, factory reset (com confirmação)
- ⌨️ **Remap**: selecione teclas no desenho e remapeie (tecla, combo, `none`, macro)
- ⏺️ **Macros**: crie, apague e atribua macros a teclas (`ctrl+shift+t`, `wait:300`, `^shift`)
- 🎵 **Live**: animações em tempo real no teclado (rainbow, wave, fire, cor sólida)
- ℹ️ Device info (transporte, VID:PID, strings HID)
- 🔋 Bateria e status no cabeçalho
- ⚙️ Toda a comunicação HID numa thread separada — a UI nunca trava

## 🐧 Linux: permissão sem sudo

```bash
sudo cp udev/70-aula-f75.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger
```

## 🛠️ Compilar

```bash
cargo build --release    # binário em target/release/aula-f75
```

## ⚠️ Notas de segurança

- 🖐️ O teclado pode **perder o key-release** de uma tecla pressionada no momento de um comando (ela ficaria repetindo). Este CLI monitora a interface de teclado e só envia comandos com tudo solto e idle ~400 ms.
- 🔒 Nada aqui toca firmware/bootloader.
- 🔄 `Fn+Esc` (segurando) reseta o teclado de fábrica.

## 🗺️ Roadmap

- [x] Leitura/escrita de blocos com ACK e remontagem por índice
- [x] LED config (efeito, brilho, velocidade, cor)
- [x] RGB por tecla + temas
- [x] Guarda de teclas soltas antes de escrever
- [x] 🖥️ GUI (egui) com desenho do teclado, navbar de seções e temas
- [x] ⚙️ Settings: sleep, debounce, factory reset
- [x] 🔀 Remapeamento (comando `0x01`, 504 B/camada)
- [x] ⏺️ Macros (comando `0x03`)
- [x] 🎵 Iluminação em tempo real / modo música (`0x88`, ~14 fps)
- [ ] 🔗 Transporte cabo `258a:010c`

## 📄 Licença

MIT
