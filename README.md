# aula-f75-rs

Ferramenta multiplataforma (Windows/Linux) em Rust para configurar o teclado
**AULA F75** — substitui o software oficial `OemDrv.exe` (só Windows).

Protocolo: canal HID vendor (report `0x13`, pacotes de 20 bytes com checksum
de 8 bits, ACK por eco), documentado pela comunidade em
[aula-f75-hub/docs/protocol.md](https://github.com/bakiakyol/aula-f75-hub/blob/main/docs/protocol.md)
e revalidado aqui contra o hardware.

## Transportes suportados

| Conexão | VID:PID | Status |
|---|---|---|
| Dongle 2.4G | `3554:fa09` | ✅ verificado neste projeto (leitura, escrita, bateria) |
| Bluetooth | `3554:fa07` | ✅ (mesmo canal; leituras podem perder pacotes — o código remonta por índice) |
| Cabo | `258a:010c` | ❌ transporte diferente (feature report de 520 bytes, Usage Page `0xFF00`), não implementado |

## Uso

```
aula-f75 list                 # dispositivos conectados
aula-f75 info                 # efeito, brilho, velocidade, cor, bateria, etc.
aula-f75 effect 2             # efeito (2 = breathing; ver protocol.rs EFFECTS)
aula-f75 brightness 5         # brilho 0-9
aula-f75 speed 2              # velocidade 0-4
aula-f75 color blue           # cor 0-7 ou nome
aula-f75 battery              # bateria (experimental)
aula-f75 dump                 # bloco LED bruto (hex)
aula-f75 selftest             # lê, regrava idêntico e confere (valida escrita/ACK)
```

## Linux: permissão (sem sudo)

```
sudo cp udev/70-aula-f75.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger
```

## Notas de segurança

- O teclado pode **perder o key-release** de uma tecla pressionada no momento
  em que responde tráfego de configuração (a tecla fica repetindo até outra
  ser pressionada). O software oficial espera as teclas soltas; este CLI
  ainda não monitora — solte o teclado antes de executar um comando.
- Nada aqui toca firmware/bootloader. `Fn+Esc` (segurando) reseta o teclado
  de fábrica.

## Roadmap

- [ ] Monitorar interface de teclado e só enviar comandos com teclas soltas
- [ ] RGB por tecla (comando `0x02`, 378 bytes planos) + temas
- [ ] Remapeamento (comando `0x01`, 504 bytes/camada)
- [ ] Macros (comando `0x03`)
- [ ] Iluminação em tempo real (comando `0x88`) / modo música
- [ ] GUI (egui/iced) com desenho do teclado a partir do KB.ini
- [ ] Transporte cabo `258a:010c` (feature report 520 bytes — requer RE do
  `CDevG5KB::AccessData` no OemDrv.exe)
