# BBRadmin

Co-op seamless do Bloodborne (port **bbport** para Windows) entre dois ou mais jogadores na
**mesma rede Radmin VPN**, com o **servidor de co-op rodando no PC de um dos jogadores**.
Uma janela, um executável, nada para instalar.

**Baixar:** pegue o `BBRadmin.exe` na página de releases
(<https://github.com/shiki305/BBRadmin/releases>) — é um arquivo só, não precisa instalar nada.
Para compilar do zero, veja [Compilar](#compilar).

O jogo é iniciado pelo **próprio motor do bbport** (`bin\bb-probe-seamless.exe`), com o co-op
da **BBSC: Yharnam Together 1.0.0** — a mesma release de que o bbport 1.9.8.6 é feito. É co-op
de verdade (time/menu de time, sem Hunt Bell nem Kin Bell), do tipo "jogar o jogo todo junto".

---

## Como jogar

1. **Os dois no mesmo Radmin VPN** (mesma rede; cada um tem um endereço `26.x.x.x`).
2. **Quem hospeda:** aba **Jogar → Vou hospedar → Hospedar e jogar**. Anote o endereço que
   aparecer (é o seu `26.x.x.x`).
   - Na primeira vez o app instala o servidor do co-op **sozinho** (ele vem dentro do exe) e,
     se você permitir, abre o firewall (um único pedido de administrador).
3. **Quem entra:** aba **Jogar → Vou entrar**, digite o endereço do host e **Entrar e jogar**.
4. **No jogo:** abra o menu e use o **time** (o menu de co-op) para convidar/entrar.

Os dois precisam ter o **bbport com a pasta `seamless` já preparada** (abra o "Seamless co-op"
uma vez no launcher do bbport; o app avisa se faltar).

### A conta do co-op (nada para criar)

O co-op precisa de uma conta (o `sign-in` do servidor). O BBRadmin **não pede para criar uma**:
ele pega a que você **já tem no launcher do bbport**, na página *Seamless co-op* — a mesma com
que você entra por lá. Ela aparece na aba **Avançado → Conta do co-op** (e o botão
**Usar do bbport** a recarrega, caso você mude ela no launcher).

Cada PC usa a **sua própria** conta (o seu amigo usa a dele). O servidor do BBRadmin **cria a
conta sozinho no primeiro entrar** — não há cadastro nem site. Se o launcher do bbport ainda não
tiver uma conta sua, abra-o uma vez em *Seamless co-op* e faça o login; o BBRadmin passa a usá-la.

### A aba Co-op (regras do host)

Na tela do time dentro do jogo, só **Joining / Invading / Death mode** se mudam ali; o resto é
regra do **líder** — é o que o próprio jogo diz ("Host rules, set in the leader's launcher, on its
Co-op page"). Por isso quem hospeda escolhe essas regras **no BBRadmin, na aba Co-op**. Elas vão
junto no start e aparecem para o time.

Cada opção ali está escrita com **o que ela muda em relação ao jogo normal**, para não ter dúvida
("Normal" é o padrão, ou seja, o jogo como ele é):

| Opção | Padrão | O que muda |
|---|---|---|
| **Inimigos mais fortes** | Normal (×1) | `Um pouco` vida ×1,5 · dano ×1,25 · equilíbrio ×1,5 — `Mais` ×3 · ×1,5 · ×3 — `Muito` ×5 · ×2 · ×5 — `Automático` sobe com cada caçador no mesmo mapa. `Personalizado` usa os seus números (1 a 10; 1 = o do jogo). |
| **Ritmo do chefe** | Normal | `Agressivo` metade das pausas e giros ×1,3 — `Implacável` um quarto das pausas e giros ×1,6. Golpes e dano não mudam. |
| **Podridão (rot)** | desligado | Cada morte de verdade dá uma penalidade (até 5). Lâmpada, nível, chefe, seu assassino ou sua mancha de sangue tiram. |
| **Viajar juntos** | desligado | Uma lâmpada ou lápide pergunta se leva o time inteiro; desligado, cada um viaja sozinho. |
| **Com votação** | desligado | O time tem 20 s; um "não" cancela a viagem. Precisa de **Viajar juntos**. |
| **Reviver os caídos** | **ligado** | Os companheiros levantam um caçador caído. Desligado, morre de verdade, como no jogo normal. |

O quadro começa com o que você já escolheu na página *Seamless co-op* do launcher do bbport; o
botão **Pegar do bbport** recarrega essas escolhas. Depois disso, valem as do BBRadmin. Só o
**líder** (quem hospeda) escolhe: um convidado entra nas regras do time dele.

### A aba Controles (teclado+mouse ↔ controle)

O jogo aceita **teclado/mouse e controle ao mesmo tempo** — teclas e botões respondem sempre. O que
se escolhe ali é só o **mouse** (o mesmo "You play with" do launcher do bbport):

- **Teclado e mouse** — o mouse gira a câmera (layout Dark Souls III) e fica capturado no jogo.
- **Controle (gamepad)** — o mouse não mexe a câmera e o ponteiro fica livre; as teclas continuam
  valendo.

A troca grava na hora no `input.ini` do bbport (uma linha só, `mouse_to_joystick`, com um backup
`input.ini.bbradmin-bak` antes de mexer; **Restaurar input.ini** devolve o seu arquivo). Durante o
jogo, **F7** liga/desliga o mouse e **F8** recarrega o arquivo — dá para alternar sem reiniciar.
Para trocar **tecla por tecla** ou **botão por botão**, o botão **Abrir launcher do bbport** leva
à página *Controles* dele, que tem um campo por tecla.

---

## O que ele faz — e o que **não** faz

- Inicia o jogo por `Bloodborne.exe --run` com o ambiente do co-op do próprio bbport.
- Não usa Hunt Bell / Kin Bell.
- **Não** mexe em `C:\Windows`, no `hosts`, nem em qualquer arquivo do sistema.
- **Não** altera as configurações do bbport. Os dois únicos arquivos dele que o app escreve são
  `seamless\user\host_overrides.custom.json` (o servidor do co-op) e, **quando você manda** na aba
  Controles, a linha do mouse no `input.ini` — cada um com um backup ao lado antes de trocar. O
  `bbport.ini` e o `settings.json` nunca são escritos.
  Ele **lê** (só lê) o `settings.json` do launcher do bbport, para pegar a conta do co-op (e as
  regras do host, que entram só como valor inicial da aba **Co-op**).
- Só os arquivos do próprio app ficam na pasta dele (veja abaixo).

---

## Onde ficam as coisas

| Caminho | O quê |
|---|---|
| `%LOCALAPPDATA%\BBRadmin\config.ini` | as respostas do app (pasta do bbport, conta, papel). A senha é protegida com DPAPI (só o seu usuário lê). |
| `%LOCALAPPDATA%\BBRadmin\server\` | o servidor do co-op, extraído do próprio `BBRadmin.exe` no primeiro "Hospedar". |
| `%LOCALAPPDATA%\BBRadmin\backend\` | avisos que o servidor mostra no jogo. |
| `%LOCALAPPDATA%\BBRadmin\log\shadnet.txt` | o log do servidor (o do run anterior vira `shadnet.prev.txt`). |
| `%LOCALAPPDATA%\BBRadmin\bbradmin.log` | o log do app. |
| `%LOCALAPPDATA%\BBRadmin\dry-run.txt` | o resultado do `--dry-run`. |

Nada disso é do bbport: apagar a pasta `BBRadmin` não afeta o jogo.

---

## Teste rápido

```
BBRadmin.exe --dry-run
```

Escreve o que um start faria (pasta do bbport, endereço, overrides e variáveis do jogo) em
`%LOCALAPPDATA%\BBRadmin\dry-run.txt`, **sem iniciar nem gravar nada**. É o teste a rodar depois
de atualizar o bbport.

---

## Quando o bbport (ou a BBSC) muda

Todos os nomes que dependem do bbport estão em **um arquivo só**:

```
src\Core\CoopSchema.cs
```

— os arquivos do bbport (`bin\bb-probe-seamless.exe`, `seamless\user`, ...), as portas
(31314 link UDP, 31315 WebAPI, 20443 backend, 31313 loopback), a release do co-op (`1.0.0`),
o endereço do servidor da Sony e as variáveis de ambiente (`BB_PROBE`, `BB_SHADNET_*`, ...).

Se uma atualização renomear algo, corrija lá e recompile. O resto do código nunca escreve um
nome desse tipo à mão.

Do launcher do bbport, o BBRadmin lê mais três coisas, e todas numa classe só:
`src\Core\BbportLauncher.cs` — a **conta do co-op** (`supermedo_npid` / `supermedo_pass`), as
**regras do host** (`yt_*`) e o **que você joga com** (`play_with`); todos só como *valores
iniciais* (valem as escolhas feitas no BBRadmin). Se o launcher renomear essas chaves numa
atualização, corrija lá.

O **`input.ini`** (a linha `mouse_to_joystick`) fica em `src\Core\InputConfig.cs`; o nome do arquivo
está em `CoopSchema.InputFile`. Se o bbport mudar o nome da linha do mouse, corrija ali.

**Trocar o servidor embutido:** o bbport 1.9.8.6 só traz o **cliente** do co-op(1); por isso o
BBRadmin carrega o servidor **1.0.0** dentro de si e o extrai no "Hospedar". Para outra versão,
ponha os arquivos em `vendor\server\` (veja `build.cmd`) e recompile — o servidor é copiado por
cima do já instalado, preservando o `db\` e o `link_key.bin` dele.

---

## Compilar

```
build.cmd
```

Usa o `csc.exe` que **já vem no Windows** (.NET Framework 4.x) — nada para instalar. O
`vendor\` é empacotado num zip e embutido no exe como recurso. Saída: `build\BBRadmin.exe`.

O `vendor\server\` (o shadNet da BBSC, ~19 MB) está **neste repositório**, então um clone já compila
direto. Ele é GPL-2.0 como o resto do projeto: veja [Licença e créditos](#licença-e-créditos).

---

## Licença e créditos

O **BBRadmin é software livre sob a GPL-2.0** (veja `LICENSE`), porque carrega e redistribui, dentro
do próprio exe, o **shadNet** da *BBSC: Yharnam Together 1.0.0*
(`version=1.0.0;revision=cdb58df0bbb0;protocol=9`), **GPL-2.0**, da equipe:

- CLS_AMG — <https://github.com/CLSAMG22> (release: `BBSC-Yharnam-Together-1.0.0-BBport.zip`)
- Yharnam Together (Kaniel) — <https://github.com/lKaniel/bloodborne-server> (fonte do servidor)

A licença do servidor viaja junto do app (`vendor\LICENSE-BBSC.txt`, extraída ao lado do servidor).
O código-fonte completo do BBRadmin está neste repositório, como a GPL-2.0 exige. Não há arquivos de
jogo aqui; o port **bbport** é do Supermedo e cada jogador usa a sua cópia do jogo.

---

(1) A pasta `seamless` do bbport 1.9.8.6 não tem `server\`: o launcher dele só sabe **entrar** num
servidor. Um servidor de outra release é recusado (`the server refused this game`), porque cada
release só aceita jogos da mesma versão — é o motivo pelo qual o servidor 1.5.0 do RadminBell não
funciona aqui.
