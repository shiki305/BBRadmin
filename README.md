# BBRadmin (Rust)

Fork em **Rust** de [shiki305/BBRadmin](https://github.com/shiki305/BBRadmin): co-op seamless do
Bloodborne (port **bbport** para Windows) entre dois ou mais jogadores na **mesma rede Radmin VPN**,
com o **servidor de co-op rodando no PC de um dos jogadores**.
Uma janela, um executável nativo, nada para instalar.

O jogo é iniciado pelo **próprio motor do bbport** (`bin\bb-probe-seamless.exe`), com o co-op
da **BBSC: Yharnam Together 1.0.0** — a mesma release de que o bbport 1.9.8.6 é feito. É co-op
de verdade (time/menu de time, sem Hunt Bell nem Kin Bell), do tipo "jogar o jogo todo junto".

---

## Correção do bug de tela cheia (por que o jogo não abria em tela cheia no BBRadmin original)

No **bbport**, as opções de janela/tela cheia **não ficam no `bbport.ini`** — elas ficam em
`%APPDATA%\bbport-launcher\settings.json` (`fullscreen`, `exclusive_fullscreen`,
`borderless_fullscreen`, `game_monitor`, `window_size`, `hdr`, `present_mode`, etc.) e funcionam
por dois mecanismos:

1. **Variáveis de ambiente (`BB_FULLSCREEN` e `BB_WINDOW_MODE`):** o motor `bb-probe-seamless.exe`
   (`gpu/shim/window.cpp`) lê `BB_FULLSCREEN=1` (tela cheia SDL) e `BB_WINDOW_MODE=fullscreen`
   (tela cheia exclusiva) diretamente do ambiente do processo. Se não forem passadas, ele sempre
   abre em modo janela.
2. **Tela cheia sem bordas (`borderless_fullscreen: true`):** no bbport, esse modo é aplicado pelo
   próprio launcher via Win32 (`bbport_display.Placer`), que encontra o `HWND` do jogo, remove as
   bordas/barra de título (`WS_CAPTION | WS_THICKFRAME | WS_MAXIMIZEBOX` e estilos estendidos),
   desativa cantos arredondados no Windows 11 (`DWMWA_WINDOW_CORNER_PREFERENCE`) e redimensiona a
   janela em pixels físicos (`PER_MONITOR_AWARE_V2`) para cobrir o monitor inteiro.

O BBRadmin original em C# assumia que o `bbport.ini` cuidava da tela: não lia esses campos do
`settings.json`, não passava `BB_FULLSCREEN`/`BB_WINDOW_MODE` e não tinha gerenciador de janela
para `borderless_fullscreen`.

Nesta versão em **Rust**:
- Todas as configurações de tela e execução do `%APPDATA%\bbport-launcher\settings.json` são lidas
  e encaminhadas no ambiente do jogo (`src/core/bbport_launcher.rs` e `src/core/game_environment.rs`).
- Um `WindowPlacer` Win32 nativo (`src/core/display.rs`) roda em background enquanto o jogo está
  aberto, monitorando `bb-probe-seamless.exe` e `bb-probe.exe` para aplicar **borderless fullscreen**,
  posicionamento em múltiplos monitores e tamanho de janela.
- Na aba **Avançado → Modo de tela**, é possível seguir automaticamente o launcher do bbport
  (padrão) ou sobrescrever diretamente pelo BBRadmin (*Tela cheia sem bordas*, *Tela cheia SDL*,
  *Tela cheia exclusiva* ou *Janela*).
- **Tema escuro / claro automático:** segue o tema do Windows por padrão (com alternância rápida
  entre **Sistema / Escuro / Claro** na barra inferior ou em **Avançado → Opções**).
- **Atualização automática ao abrir:** verifica em segundo plano a release mais recente no GitHub
  (`anreren/BBRadmin`), baixa e valida o novo `BBRadmin.exe` e reinicia automaticamente quando
  possível (também disponível manualmente em **Avançado → Verificar atualizações**).

---

## Como jogar

1. **Os dois no mesmo Radmin VPN** (mesma rede; cada um tem um endereço `26.x.x.x`).
2. **Quem hospeda:** aba **Jogar → Eu hospedo → Jogar co-op**. Anote o endereço que aparecer
   (é o seu `26.x.x.x`).
   - Na primeira vez o app extrai o servidor do co-op **sozinho** (embutido no `BBRadmin.exe`) e,
     se você permitir, abre o firewall (um único pedido de administrador).
3. **Quem entra:** aba **Jogar → Eu entro**, digite o endereço do host e clique em **Jogar co-op**.
4. **No jogo:** abra o menu e use o **time** (o menu de co-op) para convidar/entrar.

Os dois precisam ter o **bbport com a pasta `seamless` já preparada** (abra o "Seamless co-op"
uma vez no launcher do bbport; o app avisa se faltar).

---

## Onde ficam as coisas

| Caminho | O quê |
|---|---|
| `%LOCALAPPDATA%\BBRadmin\config.ini` | preferências do app (compatível com o BBRadmin original; senha protegida com Windows DPAPI). |
| `%LOCALAPPDATA%\BBRadmin\server\` | servidor do co-op (`shadnet.exe` + DLLs Qt6), extraído do `BBRadmin.exe` no primeiro "Hospedar". |
| `%LOCALAPPDATA%\BBRadmin\backend\` | avisos que o servidor mostra no jogo. |
| `%LOCALAPPDATA%\BBRadmin\log\shadnet.txt` | log do servidor (o anterior vira `shadnet.prev.txt`). |
| `%LOCALAPPDATA%\BBRadmin\bbradmin.log` | log do app. |
| `%LOCALAPPDATA%\BBRadmin\dry-run.txt` | resultado do `--dry-run`. |

---

## Teste rápido

```cmd
BBRadmin.exe --dry-run
```

Escreve tudo o que um start faria (pasta do bbport, endereço, modo de tela, overrides e variáveis do
jogo) em `%LOCALAPPDATA%\BBRadmin\dry-run.txt` e imprime no console, **sem iniciar nem gravar nada**.

---

## Compilar

Requisitos: toolchain Rust para Windows MSVC (`rustup` — `x86_64-pc-windows-msvc`).

```cmd
build.cmd
```

Ou diretamente via Cargo:

```cmd
cargo build --release
```

O `build.rs` empacota `vendor\` em `server.zip` (embutido via `include_bytes!`) e incorpora o
`app.manifest` diretamente no executável final (`target\release\BBRadmin.exe` / `build\BBRadmin.exe`).

---

## Licença e créditos

O **BBRadmin é software livre sob a GPL-2.0** (veja `LICENSE`), porque carrega e redistribui, dentro
do próprio exe, o **shadNet** da *BBSC: Yharnam Together 1.0.0*
(`version=1.0.0;revision=cdb58df0bbb0;protocol=9`), **GPL-2.0**, da equipe:

- Projeto original BBRadmin: <https://github.com/shiki305/BBRadmin>
- CLS_AMG — <https://github.com/CLSAMG22> (release: `BBSC-Yharnam-Together-1.0.0-BBport.zip`)
- Yharnam Together (Kaniel) — <https://github.com/lKaniel/bloodborne-server> (fonte do servidor)
