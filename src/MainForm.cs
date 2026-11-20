using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
using System.Drawing;
using System.Globalization;
using System.IO;
using System.Text;
using System.Windows.Forms;
using BBRadmin.Core;

namespace BBRadmin
{
    /// <summary>
    /// The window: four tabs and a log. "Jogar" is the whole game — choose who hosts, take the address,
    /// press one button. "Co-op" is the leader's rules, each one beside what it changes. "Controles"
    /// switches keyboard+mouse / controller and opens the port's own Controls page for rebinding.
    /// "Avançado" is everything that is not needed to play (the bbport folder, the account, the firewall
    /// rules, the official launcher).
    ///
    /// Nothing here writes to the port's settings: a start touches BBSC's own
    /// seamless\user\host_overrides.custom.json and, when the player chose, the one mouse camera line of
    /// input.ini — each with a backup beside it.
    /// </summary>
    internal sealed class MainForm : Form
    {
        private readonly AppConfig _config;
        private readonly List<NetworkChoice> _networks = new List<NetworkChoice>();
        private readonly ServerController _server = new ServerController();
        private readonly Timer _tick = new Timer();

        private BbportInfo _port = new BbportInfo();
        private Process _game;
        private bool _busy;

        // header
        private Label _portLabel;
        // play tab
        private RadioButton _roleHost;
        private RadioButton _roleGuest;
        private Label _ownCaption;
        private TextBox _ownAddress;
        private Button _copyAddress;
        private Label _hostCaption;
        private TextBox _hostIpBox;
        private Button _testServer;
        private Button _play;
        private Button _startServer;
        private Button _stopServer;
        private Label _playHint;
        private Label _steps;
        // co-op tab — the leader's rules
        private RadioButton[] _enemies;
        private CheckBox _ruleRot;
        private CheckBox _ruleTravel;
        private CheckBox _ruleVote;
        private CheckBox _ruleRevive;
        private ComboBox _ruleTempo;
        private TextBox _ruleCustomHp;
        private TextBox _ruleCustomDamage;
        private TextBox _ruleCustomPoise;
        private Label _ruleHint;
        // controls tab
        private RadioButton _playWithKeyboard;
        private RadioButton _playWithController;
        private Label _inputHint;
        private bool _uiLoading;

        // The rules' choices as the player sees them, beside what the engine's names them
        // (bbport_yharnam's ENEMIES keys and tempo words).
        private static readonly string[] EnemiesValues = { "off", "auto", "bit", "on", "much", "custom" };
        private static readonly string[] EnemiesLabels =
        {
            "Normal — como o jogo: vida ×1 · dano ×1 · equilíbrio ×1   (padrão)",
            "Automático — sobe com cada caçador no mesmo mapa (até \"Mais fortes\" com o time cheio)",
            "Um pouco mais fortes — vida ×1,5 · dano ×1,25 · equilíbrio ×1,5",
            "Mais fortes — vida ×3 · dano ×1,5 · equilíbrio ×3",
            "Muito mais fortes — vida ×5 · dano ×2 · equilíbrio ×5",
            "Personalizado — os meus números (1 a 10; 1 = o do jogo):"
        };
        private static readonly string[] TempoValues = { "", "aggressive", "relentless" };
        private static readonly string[] TempoLabels =
        {
            "Normal — como o jogo (padrão)",
            "Agressivo — metade das pausas, giros ×1,3",
            "Implacável — um quarto das pausas, giros ×1,6"
        };
        // advanced tab
        private TextBox _folderBox;
        private Label _portInfo;
        private TextBox _accountBox;
        private TextBox _passwordBox;
        private CheckBox _showPassword;
        private Label _accountHint;
        private CheckBox _keepServer;
        private CheckBox _stopWithGame;
        private CheckBox _autoFirewall;
        // bottom
        private Label _stateLabel;
        private Button _logToggle;
        private TextBox _logBox;
        private Panel _bottomBar;

        public MainForm(AppConfig config)
        {
            _config = config;
            BuildUi();
            LoadConfigIntoUi();
            RefreshDetection();

            _tick.Interval = 1500;
            _tick.Tick += delegate { RefreshState(); };
            _tick.Start();

            Log.Line("BBRadmin aberto.");
        }

        // ---------------------------------------------------------------- the window itself

        private void BuildUi()
        {
            Text = "BBRadmin · co-op em Radmin VPN";
            Font = new Font("Segoe UI", 9F);
            ClientSize = new Size(632, 680);
            MinimumSize = new Size(600, 520);
            StartPosition = FormStartPosition.CenterScreen;
            BackColor = SystemColors.Control;
            FormClosing += OnClosing;

            // header
            Panel header = new Panel();
            header.Dock = DockStyle.Top;
            header.Height = 58;
            header.BackColor = Color.White;

            Label title = new Label();
            title.Text = "BBRadmin";
            title.Font = new Font("Segoe UI", 13F, FontStyle.Bold);
            title.AutoSize = true;
            title.Location = new Point(14, 10);
            header.Controls.Add(title);

            Label subtitle = new Label();
            subtitle.Text = "co-op contínuo para Bloodborne, por Radmin VPN";
            subtitle.ForeColor = Color.DimGray;
            subtitle.AutoSize = true;
            subtitle.Location = new Point(16, 34);
            header.Controls.Add(subtitle);

            _portLabel = new Label();
            _portLabel.AutoSize = false;
            _portLabel.TextAlign = ContentAlignment.MiddleRight;
            _portLabel.ForeColor = Color.DimGray;
            _portLabel.Location = new Point(300, 12);
            _portLabel.Size = new Size(318, 36);
            _portLabel.Anchor = AnchorStyles.Top | AnchorStyles.Right;
            header.Controls.Add(_portLabel);

            // log (hidden until asked for)
            _logBox = new TextBox();
            _logBox.Multiline = true;
            _logBox.ReadOnly = true;
            _logBox.ScrollBars = ScrollBars.Vertical;
            _logBox.WordWrap = false;
            _logBox.Dock = DockStyle.Bottom;
            _logBox.Height = 200;
            _logBox.BackColor = Color.FromArgb(250, 250, 250);
            _logBox.Font = new Font("Consolas", 8.5F);
            _logBox.Visible = false;

            // bottom bar
            _bottomBar = new Panel();
            _bottomBar.Dock = DockStyle.Bottom;
            _bottomBar.Height = 36;

            _stateLabel = new Label();
            _stateLabel.AutoSize = true;
            _stateLabel.Location = new Point(14, 9);
            _bottomBar.Controls.Add(_stateLabel);

            _logToggle = new Button();
            _logToggle.Text = "Mostrar log";
            _logToggle.Size = new Size(120, 26);
            _logToggle.Anchor = AnchorStyles.Top | AnchorStyles.Right;
            _logToggle.Location = new Point(632 - 134, 5);
            _logToggle.Click += OnToggleLog;
            _bottomBar.Controls.Add(_logToggle);

            TabControl tabs = new TabControl();
            tabs.Dock = DockStyle.Fill;
            tabs.Padding = new Point(14, 6);
            tabs.TabPages.Add(BuildPlayTab());
            tabs.TabPages.Add(BuildCoopTab());
            tabs.TabPages.Add(BuildControlsTab());
            tabs.TabPages.Add(BuildAdvancedTab());

            // Docking is applied from the last added control to the first, so the header and the bottom
            // bar claim their edges before the tabs fill what is left.
            Controls.Add(tabs);
            Controls.Add(_logBox);
            Controls.Add(_bottomBar);
            Controls.Add(header);
        }

        private TabPage BuildPlayTab()
        {
            TabPage page = new TabPage();
            page.Text = "Jogar";
            page.UseVisualStyleBackColor = true;
            page.AutoScroll = true;   // the window may be short: everything stays reachable

            // --- who hosts
            GroupBox who = new GroupBox();
            who.Text = "Quem hospeda?";
            who.Location = new Point(12, 10);
            who.Size = new Size(596, 76);
            who.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right;

            _roleHost = new RadioButton();
            _roleHost.Text = "Eu hospedo (o servidor roda neste PC)";
            _roleHost.Location = new Point(14, 24);
            _roleHost.Size = new Size(320, 20);
            _roleHost.Checked = true;
            _roleHost.CheckedChanged += delegate { RefreshState(); };
            who.Controls.Add(_roleHost);

            _roleGuest = new RadioButton();
            _roleGuest.Text = "Eu entro (o servidor está no PC do amigo)";
            _roleGuest.Location = new Point(14, 48);
            _roleGuest.Size = new Size(320, 20);
            _roleGuest.CheckedChanged += delegate { RefreshState(); };
            who.Controls.Add(_roleGuest);
            page.Controls.Add(who);

            // --- addresses
            GroupBox address = new GroupBox();
            address.Text = "Endereço";
            address.Location = new Point(12, 94);
            address.Size = new Size(596, 126);
            address.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right;

            _ownCaption = new Label();
            _ownCaption.Text = "Este PC (dê este endereço ao amigo):";
            _ownCaption.AutoSize = true;
            _ownCaption.Location = new Point(14, 24);
            address.Controls.Add(_ownCaption);

            _ownAddress = new TextBox();
            _ownAddress.ReadOnly = true;
            _ownAddress.Location = new Point(16, 46);
            _ownAddress.Size = new Size(300, 24);
            _ownAddress.BackColor = Color.White;
            address.Controls.Add(_ownAddress);

            _copyAddress = new Button();
            _copyAddress.Text = "Copiar";
            _copyAddress.Location = new Point(324, 44);
            _copyAddress.Size = new Size(90, 27);
            _copyAddress.Click += OnCopyAddress;
            address.Controls.Add(_copyAddress);

            _hostCaption = new Label();
            _hostCaption.Text = "IP do host (o amigo te passa, 26.x.x.x):";
            _hostCaption.AutoSize = true;
            _hostCaption.Location = new Point(14, 80);
            address.Controls.Add(_hostCaption);

            _hostIpBox = new TextBox();
            _hostIpBox.Location = new Point(16, 100);
            _hostIpBox.Size = new Size(300, 24);
            address.Controls.Add(_hostIpBox);

            _testServer = new Button();
            _testServer.Text = "Testar";
            _testServer.Location = new Point(324, 98);
            _testServer.Size = new Size(90, 27);
            _testServer.Click += OnTestServer;
            address.Controls.Add(_testServer);
            page.Controls.Add(address);

            // --- the buttons
            GroupBox actions = new GroupBox();
            actions.Text = "Começar";
            actions.Location = new Point(12, 228);
            actions.Size = new Size(596, 92);
            actions.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right;

            _play = new Button();
            _play.Text = "Jogar co-op";
            _play.Font = new Font("Segoe UI", 12F, FontStyle.Bold);
            _play.Location = new Point(14, 22);
            _play.Size = new Size(230, 54);
            _play.Click += OnPlay;
            actions.Controls.Add(_play);

            _startServer = new Button();
            _startServer.Text = "Só iniciar o servidor";
            _startServer.Location = new Point(258, 22);
            _startServer.Size = new Size(160, 25);
            _startServer.Click += OnStartServerOnly;
            actions.Controls.Add(_startServer);

            _stopServer = new Button();
            _stopServer.Text = "Parar o servidor";
            _stopServer.Location = new Point(258, 51);
            _stopServer.Size = new Size(160, 25);
            _stopServer.Click += OnStopServer;
            actions.Controls.Add(_stopServer);

            _playHint = new Label();
            _playHint.Text = "Hospede ou entre. O jogo abre já com o co-op e a tela de time aparece dentro dele.";
            _playHint.ForeColor = Color.DimGray;
            _playHint.AutoSize = false;
            _playHint.Location = new Point(428, 22);
            _playHint.Size = new Size(158, 58);
            actions.Controls.Add(_playHint);
            page.Controls.Add(actions);

            // --- how it works
            GroupBox steps = new GroupBox();
            steps.Text = "Como funciona";
            steps.Location = new Point(12, 328);
            steps.Size = new Size(596, 158);
            steps.Anchor = AnchorStyles.Top | AnchorStyles.Bottom | AnchorStyles.Left | AnchorStyles.Right;

            _steps = new Label();
            _steps.AutoSize = false;
            _steps.Location = new Point(14, 22);
            _steps.Size = new Size(570, 124);
            _steps.Anchor = AnchorStyles.Top | AnchorStyles.Bottom | AnchorStyles.Left | AnchorStyles.Right;
            _steps.Text =
                "1.  Os dois ligam o Radmin VPN e entram na mesma rede (aparece um IP 26.x.x.x).\r\n" +
                "2.  Um clica em \"Jogar co-op\" com \"Eu hospedo\" e passa o endereço ao outro (botão Copiar).\r\n" +
                "3.  O outro escolhe \"Eu entro\", cola o endereço em \"IP do host\" e clica em \"Jogar co-op\".\r\n" +
                "4.  No jogo, o que hospeda abre o menu, cria o time e convida; o amigo aceita o convite.\r\n" +
                "5.  Para sair, feche o jogo. O servidor continua rodando para o próximo encontro.\r\n" +
                "\r\n" +
                "As regras do co-op ficam na aba \"Co-op\"; os controles, na aba \"Controles\".";
            steps.Controls.Add(_steps);
            page.Controls.Add(steps);

            return page;
        }

        private TabPage BuildCoopTab()
        {
            TabPage page = new TabPage();
            page.Text = "Co-op";
            page.UseVisualStyleBackColor = true;
            page.AutoScroll = true;

            Label intro = new Label();
            intro.AutoSize = false;
            intro.Location = new Point(12, 8);
            intro.Size = new Size(596, 34);
            intro.ForeColor = Color.DimGray;
            intro.Text = "As regras do líder do time: valem para o time inteiro e aparecem na tela do time " +
                         "dentro do jogo. Só Joining / Invading / Death mode se mudam ali. Mudanças contam " +
                         "do próximo start (marque antes de \"Jogar co-op\").";
            page.Controls.Add(intro);

            page.Controls.Add(BuildEnemiesGroup());
            page.Controls.Add(BuildTempoGroup());
            page.Controls.Add(BuildOtherRulesGroup());

            _ruleHint = new Label();
            _ruleHint.AutoSize = false;
            _ruleHint.Location = new Point(14, 512);
            _ruleHint.Size = new Size(430, 20);
            _ruleHint.ForeColor = Color.DimGray;
            page.Controls.Add(_ruleHint);

            Button fromLauncher = new Button();
            fromLauncher.Text = "Pegar do bbport";
            fromLauncher.Location = new Point(452, 508);
            fromLauncher.Size = new Size(156, 28);
            fromLauncher.Click += OnUseLauncherRules;
            page.Controls.Add(fromLauncher);

            return page;
        }

        /// <summary>
        /// "Inimigos mais fortes": the six choices the bbport launcher's Seamless co-op page has
        /// (bbport_yharnam.ENEMIES), each labelled with what it multiplies from the game's own numbers
        /// (×1 is exactly the game). "Personalizado" shows the three multipliers, each 1 to 10.
        /// </summary>
        private GroupBox BuildEnemiesGroup()
        {
            GroupBox group = new GroupBox();
            group.Text = "Inimigos mais fortes";
            group.Location = new Point(12, 46);
            group.Size = new Size(596, 192);
            group.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right;

            _enemies = new RadioButton[EnemiesValues.Length];
            int y = 22;
            for (int i = 0; i < EnemiesValues.Length; i++)
            {
                RadioButton radio = new RadioButton();
                radio.Text = EnemiesLabels[i];
                radio.AutoSize = true;
                radio.Location = new Point(14, y);
                radio.CheckedChanged += OnRulesChanged;
                group.Controls.Add(radio);
                _enemies[i] = radio;
                y += 22;
            }
            _enemies[0].Checked = true;

            Label hpCaption = new Label();
            hpCaption.Text = "Vida";
            hpCaption.AutoSize = true;
            hpCaption.Location = new Point(34, 160);
            group.Controls.Add(hpCaption);

            _ruleCustomHp = new TextBox();
            _ruleCustomHp.Location = new Point(68, 156);
            _ruleCustomHp.Size = new Size(46, 24);
            group.Controls.Add(_ruleCustomHp);

            Label damageCaption = new Label();
            damageCaption.Text = "Dano";
            damageCaption.AutoSize = true;
            damageCaption.Location = new Point(124, 160);
            group.Controls.Add(damageCaption);

            _ruleCustomDamage = new TextBox();
            _ruleCustomDamage.Location = new Point(162, 156);
            _ruleCustomDamage.Size = new Size(46, 24);
            group.Controls.Add(_ruleCustomDamage);

            Label poiseCaption = new Label();
            poiseCaption.Text = "Equilíbrio";
            poiseCaption.AutoSize = true;
            poiseCaption.Location = new Point(218, 160);
            group.Controls.Add(poiseCaption);

            _ruleCustomPoise = new TextBox();
            _ruleCustomPoise.Location = new Point(282, 156);
            _ruleCustomPoise.Size = new Size(46, 24);
            group.Controls.Add(_ruleCustomPoise);

            return group;
        }

        /// <summary>
        /// "Ritmo do chefe" (bbport_yharnam.BOSS_TEMPOS): how much rest the bosses get between attacks.
        /// Their moves and damage never change.
        /// </summary>
        private GroupBox BuildTempoGroup()
        {
            GroupBox group = new GroupBox();
            group.Text = "Ritmo do chefe";
            group.Location = new Point(12, 246);
            group.Size = new Size(596, 76);
            group.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right;

            Label caption = new Label();
            caption.AutoSize = true;
            caption.Location = new Point(14, 22);
            caption.Text = "Nos dois de baixo o chefe descansa menos e vira mais rápido (os golpes e o dano iguais).";
            group.Controls.Add(caption);

            _ruleTempo = new ComboBox();
            _ruleTempo.DropDownStyle = ComboBoxStyle.DropDownList;
            _ruleTempo.Location = new Point(14, 44);
            _ruleTempo.Size = new Size(320, 24);
            _ruleTempo.Items.AddRange(TempoLabels);
            _ruleTempo.SelectedIndexChanged += OnRulesChanged;
            group.Controls.Add(_ruleTempo);

            return group;
        }

        /// <summary>The switches of the launcher's Host rules section, each with what it does.</summary>
        private GroupBox BuildOtherRulesGroup()
        {
            GroupBox group = new GroupBox();
            group.Text = "Outras regras";
            group.Location = new Point(12, 330);
            group.Size = new Size(596, 172);
            group.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right;

            _ruleRot = new CheckBox();
            _ruleRot.Text = "Podridão (rot) — cada morte de verdade dá uma penalidade (até 5)";
            _ruleRot.AutoSize = true;
            _ruleRot.Location = new Point(14, 22);
            _ruleRot.CheckedChanged += OnRulesChanged;
            group.Controls.Add(_ruleRot);

            group.Controls.Add(Note("Uma lâmpada, um nível, um chefe, seu assassino ou sua mancha de sangue tiram.", 30, 42));

            _ruleTravel = new CheckBox();
            _ruleTravel.Text = "Viajar juntos — uma lâmpada ou lápide pergunta se leva o time inteiro";
            _ruleTravel.AutoSize = true;
            _ruleTravel.Location = new Point(14, 64);
            _ruleTravel.CheckedChanged += OnRulesChanged;
            group.Controls.Add(_ruleTravel);

            group.Controls.Add(Note("Desligado: cada um viaja sozinho.", 30, 84));

            _ruleVote = new CheckBox();
            _ruleVote.Text = "Com votação — o time tem 20 s; um \"não\" cancela a viagem (precisa de \"Viajar juntos\")";
            _ruleVote.AutoSize = true;
            _ruleVote.Location = new Point(30, 106);
            _ruleVote.CheckedChanged += OnRulesChanged;
            group.Controls.Add(_ruleVote);

            _ruleRevive = new CheckBox();
            _ruleRevive.Text = "Reviver os caídos — os companheiros levantam um caçador caído";
            _ruleRevive.AutoSize = true;
            _ruleRevive.Location = new Point(14, 128);
            _ruleRevive.CheckedChanged += OnRulesChanged;
            group.Controls.Add(_ruleRevive);

            group.Controls.Add(Note("Desligado: morre de verdade, como no jogo normal.  (Padrão: ligado.)", 30, 148));

            return group;
        }

        private static Label Note(string text, int x, int y)
        {
            Label note = new Label();
            note.Text = text;
            note.AutoSize = true;
            note.ForeColor = Color.DimGray;
            note.Location = new Point(x, y);
            return note;
        }

        /// <summary>
        /// The controls: the game takes keyboard, mouse and a controller side by side, so the only thing
        /// to choose is whether the mouse turns the camera (the port's own "You play with" choice, which
        /// is the one `mouse_to_joystick` line in input.ini). Rebinding a key or a controller button is
        /// the launcher's Controls page, opened from here.
        /// </summary>
        private TabPage BuildControlsTab()
        {
            TabPage page = new TabPage();
            page.Text = "Controles";
            page.UseVisualStyleBackColor = true;
            page.AutoScroll = true;

            Label intro = new Label();
            intro.AutoSize = false;
            intro.Location = new Point(12, 8);
            intro.Size = new Size(596, 32);
            intro.ForeColor = Color.DimGray;
            intro.Text = "O jogo aceita teclado/mouse e controle ao mesmo tempo: as teclas e os botões do " +
                         "controle respondem sempre. A escolha abaixo decide só o mouse.";
            page.Controls.Add(intro);

            GroupBox choice = new GroupBox();
            choice.Text = "Com o que você joga";
            choice.Location = new Point(12, 46);
            choice.Size = new Size(596, 152);
            choice.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right;

            _playWithKeyboard = new RadioButton();
            _playWithKeyboard.Text = "Teclado e mouse — o mouse gira a câmera (layout Dark Souls III) e fica capturado no jogo";
            _playWithKeyboard.AutoSize = true;
            _playWithKeyboard.Location = new Point(14, 24);
            _playWithKeyboard.CheckedChanged += OnPlayWithChanged;
            choice.Controls.Add(_playWithKeyboard);

            _playWithController = new RadioButton();
            _playWithController.Text = "Controle (gamepad) — o mouse não mexe a câmera; o ponteiro fica livre";
            _playWithController.AutoSize = true;
            _playWithController.Location = new Point(14, 48);
            _playWithController.CheckedChanged += OnPlayWithChanged;
            choice.Controls.Add(_playWithController);

            choice.Controls.Add(Note("Durante o jogo: F7 liga/desliga o mouse e F8 recarrega o arquivo. " +
                                     "As teclas continuam valendo nas duas opções.", 14, 74));

            _inputHint = new Label();
            _inputHint.AutoSize = false;
            _inputHint.Location = new Point(14, 108);
            _inputHint.Size = new Size(566, 34);
            _inputHint.ForeColor = Color.DimGray;
            choice.Controls.Add(_inputHint);
            page.Controls.Add(choice);

            Button apply = new Button();
            apply.Text = "Aplicar agora";
            apply.Location = new Point(12, 208);
            apply.Size = new Size(132, 28);
            apply.Click += OnApplyControls;
            page.Controls.Add(apply);

            Button restore = new Button();
            restore.Text = "Restaurar input.ini";
            restore.Location = new Point(152, 208);
            restore.Size = new Size(150, 28);
            restore.Click += OnRestoreControls;
            page.Controls.Add(restore);

            Button launcher = new Button();
            launcher.Text = "Abrir launcher do bbport";
            launcher.Location = new Point(310, 208);
            launcher.Size = new Size(180, 28);
            launcher.Click += OnOpenLauncherControls;
            page.Controls.Add(launcher);

            GroupBox rebind = new GroupBox();
            rebind.Text = "Trocar tecla por tecla, ou botão do controle";
            rebind.Location = new Point(12, 246);
            rebind.Size = new Size(596, 132);
            rebind.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right;

            Label rebindNote = new Label();
            rebindNote.AutoSize = false;
            rebindNote.Location = new Point(14, 22);
            rebindNote.Size = new Size(566, 100);
            rebindNote.Text =
                "O BBRadmin só alterna teclado+mouse / controle (acima). Para escolher cada tecla e cada " +
                "botão, use a página Controles do launcher do bbport (botão acima): lá dá para clicar num " +
                "campo e apertar a tecla ou o botão do controle.\r\n" +
                "\r\n" +
                "O arquivo é o input.ini da pasta do bbport. O BBRadmin guarda uma cópia dele " +
                "(input.ini.bbradmin-bak) antes de mexer, e \"Restaurar input.ini\" devolve a sua.";
            rebind.Controls.Add(rebindNote);
            page.Controls.Add(rebind);

            return page;
        }

        private TabPage BuildAdvancedTab()
        {
            TabPage page = new TabPage();
            page.Text = "Avançado";
            page.UseVisualStyleBackColor = true;

            // --- the port's folder
            GroupBox folder = new GroupBox();
            folder.Text = "Pasta do bbport";
            folder.Location = new Point(12, 10);
            folder.Size = new Size(596, 112);
            folder.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right;

            Label folderCaption = new Label();
            folderCaption.Text = "A que tem o Bloodborne.exe:";
            folderCaption.AutoSize = true;
            folderCaption.Location = new Point(14, 22);
            folder.Controls.Add(folderCaption);

            _folderBox = new TextBox();
            _folderBox.ReadOnly = true;
            _folderBox.Location = new Point(16, 44);
            _folderBox.Size = new Size(420, 24);
            _folderBox.BackColor = Color.White;
            folder.Controls.Add(_folderBox);

            Button browse = new Button();
            browse.Text = "Procurar...";
            browse.Location = new Point(442, 42);
            browse.Size = new Size(76, 27);
            browse.Click += OnBrowseFolder;
            folder.Controls.Add(browse);

            Button check = new Button();
            check.Text = "Verificar";
            check.Location = new Point(522, 42);
            check.Size = new Size(62, 27);
            check.Click += OnCheckFolder;
            folder.Controls.Add(check);

            _portInfo = new Label();
            _portInfo.AutoSize = false;
            _portInfo.Location = new Point(16, 74);
            _portInfo.Size = new Size(566, 32);
            _portInfo.ForeColor = Color.DimGray;
            folder.Controls.Add(_portInfo);
            page.Controls.Add(folder);

            // --- the account
            GroupBox account = new GroupBox();
            account.Text = "Conta do co-op";
            account.Location = new Point(12, 130);
            account.Size = new Size(596, 168);
            account.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right;

            Label accountCaption = new Label();
            accountCaption.Text = "Id do jogador (cada um dos dois usa um diferente):";
            accountCaption.AutoSize = true;
            accountCaption.Location = new Point(14, 22);
            account.Controls.Add(accountCaption);

            _accountBox = new TextBox();
            _accountBox.Location = new Point(16, 44);
            _accountBox.Size = new Size(240, 24);
            account.Controls.Add(_accountBox);

            Label passwordCaption = new Label();
            passwordCaption.Text = "Senha:";
            passwordCaption.AutoSize = true;
            passwordCaption.Location = new Point(14, 76);
            account.Controls.Add(passwordCaption);

            _passwordBox = new TextBox();
            _passwordBox.Location = new Point(16, 96);
            _passwordBox.Size = new Size(240, 24);
            _passwordBox.UseSystemPasswordChar = true;
            account.Controls.Add(_passwordBox);

            _showPassword = new CheckBox();
            _showPassword.Text = "Mostrar";
            _showPassword.Location = new Point(266, 97);
            _showPassword.Size = new Size(90, 22);
            _showPassword.CheckedChanged += delegate { _passwordBox.UseSystemPasswordChar = !_showPassword.Checked; };
            account.Controls.Add(_showPassword);

            Button newPassword = new Button();
            newPassword.Text = "Gerar nova";
            newPassword.Location = new Point(370, 94);
            newPassword.Size = new Size(96, 27);
            newPassword.Click += OnNewPassword;
            account.Controls.Add(newPassword);

            Button fromLauncher = new Button();
            fromLauncher.Text = "Usar do bbport";
            fromLauncher.Location = new Point(472, 94);
            fromLauncher.Size = new Size(110, 27);
            fromLauncher.Click += OnUseLauncherAccount;
            account.Controls.Add(fromLauncher);

            _accountHint = new Label();
            _accountHint.Text = "";
            _accountHint.AutoSize = false;
            _accountHint.Location = new Point(16, 124);
            _accountHint.Size = new Size(566, 38);
            _accountHint.ForeColor = Color.DimGray;
            account.Controls.Add(_accountHint);
            page.Controls.Add(account);

            // --- options
            GroupBox options = new GroupBox();
            options.Text = "Opções";
            options.Location = new Point(12, 306);
            options.Size = new Size(596, 104);
            options.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right;

            _keepServer = new CheckBox();
            _keepServer.Text = "Deixar o servidor rodando quando eu fechar o BBRadmin";
            _keepServer.Location = new Point(14, 24);
            _keepServer.Size = new Size(560, 20);
            options.Controls.Add(_keepServer);

            _stopWithGame = new CheckBox();
            _stopWithGame.Text = "Parar o servidor que eu iniciei quando o jogo fechar";
            _stopWithGame.Location = new Point(14, 48);
            _stopWithGame.Size = new Size(560, 20);
            options.Controls.Add(_stopWithGame);

            _autoFirewall = new CheckBox();
            _autoFirewall.Text = "Pedir as regras de firewall antes de hospedar";
            _autoFirewall.Location = new Point(14, 72);
            _autoFirewall.Size = new Size(560, 20);
            options.Controls.Add(_autoFirewall);
            page.Controls.Add(options);

            // --- tools
            Button firewall = new Button();
            firewall.Text = "Instalar regras de firewall";
            firewall.Location = new Point(12, 420);
            firewall.Size = new Size(186, 28);
            firewall.Click += OnFirewallButton;
            page.Controls.Add(firewall);

            Button restore = new Button();
            restore.Text = "Restaurar overrides";
            restore.Location = new Point(206, 420);
            restore.Size = new Size(150, 28);
            restore.Click += OnRestoreOverrides;
            page.Controls.Add(restore);

            Button official = new Button();
            official.Text = "Abrir launcher oficial";
            official.Location = new Point(364, 420);
            official.Size = new Size(160, 28);
            official.Click += OnOpenOfficialLauncher;
            page.Controls.Add(official);

            Button reload = new Button();
            reload.Text = "Recarregar";
            reload.Location = new Point(532, 420);
            reload.Size = new Size(76, 28);
            reload.Click += delegate { RefreshDetection(); RefreshState(); };
            page.Controls.Add(reload);

            Button logs = new Button();
            logs.Text = "Abrir pasta de logs";
            logs.Location = new Point(12, 456);
            logs.Size = new Size(150, 28);
            logs.Click += OnOpenLogs;
            page.Controls.Add(logs);

            Button portFolder = new Button();
            portFolder.Text = "Abrir pasta do bbport";
            portFolder.Location = new Point(170, 456);
            portFolder.Size = new Size(170, 28);
            portFolder.Click += OnOpenPortFolder;
            page.Controls.Add(portFolder);

            Button dry = new Button();
            dry.Text = "Ver o que um start faria";
            dry.Location = new Point(348, 456);
            dry.Size = new Size(186, 28);
            dry.Click += OnDryRun;
            page.Controls.Add(dry);

            return page;
        }

        // ---------------------------------------------------------------- state

        private void LoadConfigIntoUi()
        {
            _roleHost.Checked = _config.Role != AppConfig.RoleGuest;
            _roleGuest.Checked = !_roleHost.Checked;
            _accountBox.Text = _config.Account;
            _passwordBox.Text = _config.Password;
            if (_config.Rules == null) _config.Rules = new HostRules();
            LoadRulesIntoUi();
            LoadControlsIntoUi();
            _keepServer.Checked = _config.KeepServer;
            _stopWithGame.Checked = _config.StopServerWithGame;
            _autoFirewall.Checked = true;
        }

        /// <summary>Shows the leader's rules that are kept in the config.</summary>
        private void LoadRulesIntoUi()
        {
            _ruleRot.Checked = _config.Rules.Rot;
            _ruleTravel.Checked = _config.Rules.Travel;
            _ruleVote.Checked = _config.Rules.Vote;
            _ruleRevive.Checked = _config.Rules.Revive;
            _enemies[IndexOf(EnemiesValues, _config.Rules.Enemies)].Checked = true;
            _ruleTempo.SelectedIndex = IndexOf(TempoValues, _config.Rules.Tempo);
            _ruleCustomHp.Text = _config.Rules.CustomHp;
            _ruleCustomDamage.Text = _config.Rules.CustomDamage;
            _ruleCustomPoise.Text = _config.Rules.CustomPoise;
            OnRulesChanged(null, EventArgs.Empty);
        }

        /// <summary>Shows the controls the config keeps (empty means "not chosen yet": keyboard+mouse).</summary>
        private void LoadControlsIntoUi()
        {
            _uiLoading = true;
            try
            {
                bool keyboard = _config.PlayWith != InputConfig.Controller;
                _playWithKeyboard.Checked = keyboard;
                _playWithController.Checked = !keyboard;
            }
            finally
            {
                _uiLoading = false;
            }
        }

        /// <summary>What "Inimigos mais fortes" is set to right now.</summary>
        private string SelectedEnemies()
        {
            for (int i = 0; i < _enemies.Length; i++)
            {
                if (_enemies[i].Checked) return EnemiesValues[i];
            }
            return "off";
        }

        /// <summary>The rules as the controls now have them.</summary>
        private HostRules RulesFromUi()
        {
            HostRules rules = new HostRules();
            rules.Rot = _ruleRot.Checked;
            rules.Travel = _ruleTravel.Checked;
            rules.Vote = _ruleVote.Checked && _ruleTravel.Checked;   // a vote only travels together
            rules.Revive = _ruleRevive.Checked;
            rules.Enemies = SelectedEnemies();
            rules.Tempo = ValueOf(TempoValues, _ruleTempo.SelectedIndex, "");
            rules.CustomHp = _ruleCustomHp.Text.Trim();
            rules.CustomDamage = _ruleCustomDamage.Text.Trim();
            rules.CustomPoise = _ruleCustomPoise.Text.Trim();
            return rules;
        }

        private void SyncConfigFromUi()
        {
            _config.Role = _roleHost.Checked ? AppConfig.RoleHost : AppConfig.RoleGuest;
            _config.Account = _accountBox.Text.Trim();
            _config.Password = _passwordBox.Text;
            _config.Rules = RulesFromUi();
            _config.PlayWith = _playWithController.Checked ? InputConfig.Controller : InputConfig.Keyboard;
            _config.KeepServer = _keepServer.Checked;
            _config.StopServerWithGame = _stopWithGame.Checked;
            _config.Save();
        }

        private void RefreshDetection()
        {
            BbportLauncher.Invalidate();
            _networks.Clear();
            _networks.AddRange(Radmin.Detect());

            string folder = _config.BbportDir;
            if (string.IsNullOrEmpty(folder)) folder = Bbport.GuessFolder();
            _port = Bbport.Inspect(folder);

            if (_config.BbportDir != _port.Root && _port.Root.Length > 0)
            {
                _config.BbportDir = _port.Root;
                _config.Save();
            }

            _folderBox.Text = _port.Root;

            NetworkChoice pick = Radmin.Pick(_networks);
            _ownAddress.Text = pick != null ? pick.Ip : "";

            _portInfo.Text = _port.Root.Length > 0
                ? _port.Describe()
                : "Escolha a pasta onde você descompactou o bbport (a que tem Bloodborne.exe).";
            if (_port.Missing.Count > 0) _portInfo.Text = "Não deu para usar: " + string.Join(", ", _port.Missing.ToArray());
            else if (_port.Notes.Count > 0) _portInfo.Text = _port.Root + " · observe: " + string.Join(", ", _port.Notes.ToArray());

            string launcherAccount = BbportLauncher.Account();
            _accountHint.Text = launcherAccount.Length > 0
                ? "Conta do seu launcher do bbport: " + launcherAccount + " — a mesma do Seamless co-op."
                : "Sem conta no launcher do bbport: o servidor cria uma sozinho no primeiro entrar.";

            // The controls: if the player has not chosen in this window yet, take what the file already
            // says, so the first start changes nothing. RefreshDetection is also the "Recarregar" button,
            // which re-reads the file the launcher or the game may have changed.
            if (_config.PlayWith.Length == 0)
            {
                string current = InputConfig.Current(_port);
                if (current.Length > 0)
                {
                    _config.PlayWith = current;
                    _config.Save();
                }
            }
            LoadControlsIntoUi();
            RefreshInputHint();
        }

        /// <summary>One line under the controls switch: the file in use and what it says now.</summary>
        private void RefreshInputHint()
        {
            if (_inputHint == null) return;
            string file = _port.Ready ? InputConfig.PathIn(_port) : "";
            _inputHint.Text = "Agora: " + InputConfig.Describe(_port) + "." +
                              (file.Length > 0 ? "\r\nArquivo: " + file : "");
        }

        private void RefreshState()
        {
            bool host = _roleHost.Checked;
            _ownCaption.Visible = host;
            _ownAddress.Visible = host;
            _copyAddress.Visible = host;
            _hostCaption.Visible = !host;
            _hostIpBox.Visible = !host;
            _testServer.Visible = !host;

            bool serverUp = _server.Running();
            string state = "Servidor: " + (host ? _server.State() : "no PC do amigo");
            if (host && _port.Ready && !Radmin.IsVpn(KindOf(_ownAddress.Text)) && _networks.Count > 0)
            {
                state += "  ·  ligue o Radmin VPN";
            }
            if (_busy) state = "Trabalhando...  ·  " + state;
            _stateLabel.Text = state;

            _portLabel.Text = _port.Describe();
            _playHint.Text = host
                ? "Você hospeda: o servidor sobe e o jogo abre. Depois passe seu endereço ao amigo."
                : "Você entra: o jogo abre apontando para o servidor do amigo.";

            _play.Enabled = !_busy && _port.Ready;
            _startServer.Enabled = !_busy && host && _port.Ready && !serverUp;
            _stopServer.Enabled = !_busy && serverUp;
            _testServer.Enabled = !_busy && Validate(
                host ? _ownAddress.Text : _hostIpBox.Text) == "";

            if (_game != null)
            {
                try
                {
                    if (_game.HasExited)
                    {
                        Append("O jogo fechou.");
                        Log.Line("Jogo fechou.");
                        bool ours = _server.StartedByUs;
                        _game = null;
                        if (host && ours && _stopWithGame.Checked)
                        {
                            Append("Parando o servidor que eu iniciei (opção marcada).");
                            _server.Stop();
                        }
                    }
                }
                catch (Exception)
                {
                    _game = null;
                }
            }
        }

        private static string KindOf(string ip)
        {
            if (string.IsNullOrEmpty(ip)) return "";
            string[] parts = ip.Split('.');
            int first = 0;
            if (parts.Length == 4) int.TryParse(parts[0], out first);
            if (first == 26) return "Radmin VPN";
            if (first == 25) return "Hamachi";
            if (first == 100) return "Tailscale";
            if (first == 10 || first == 172 || first == 192) return "Rede local";
            return "Outra rede";
        }

        private static string Validate(string ip)
        {
            if (string.IsNullOrEmpty(ip)) return "sem endereço";
            string[] parts = ip.Split('.');
            if (parts.Length != 4) return "não é um endereço IPv4";
            foreach (string part in parts)
            {
                int value;
                if (!int.TryParse(part, NumberStyles.None, CultureInfo.InvariantCulture, out value)) return "não é um endereço IPv4";
                if (value < 0 || value > 255) return "não é um endereço IPv4";
            }
            return "";
        }

        // ---------------------------------------------------------------- actions

        private void OnPlay(object sender, EventArgs e)
        {
            SyncConfigFromUi();
            if (!_port.Ready)
            {
                ShowMessage("O bbport ainda não está pronto.\n\nAbra a aba Avançado, escolha a pasta do bbport (a que tem Bloodborne.exe) e clique em Verificar.");
                return;
            }

            if (_roleHost.Checked)
            {
                string problem = Validate(_ownAddress.Text);
                if (problem.Length > 0)
                {
                    ShowMessage("Este PC não tem um endereço de VPN agora (" + problem + ").\n\nLigue o Radmin VPN e clique em Recarregar na aba Avançado.");
                    return;
                }
                RunBusy("Hospedando o co-op...", DoHost);
            }
            else
            {
                RunBusy("Entrando no co-op...", DoGuest);
            }
        }

        private string DoHost()
        {
            StringBuilder say = new StringBuilder();
            string ip = _ownAddress.Text.Trim();

            // firewall (only as far as the player allows)
            List<string> missing = Firewall.Missing();
            if (missing.Count > 0 && _autoFirewall.Checked)
            {
                if (AskOnUi("As regras de firewall do BBRadmin ainda não estão instaladas (" + string.Join(", ", missing.ToArray()) +
                            ").\n\nSem elas o amigo pode não conseguir entrar. Instalar agora? (pede administrador)"))
                {
                    say.AppendLine(InstallFirewallElevated());
                }
            }
            if (Firewall.Missing().Count > 0)
            {
                say.AppendLine("Sem todas as regras de firewall: se o amigo não entrar, instale-as na aba Avançado.");
            }

            if (!_server.Running())
            {
                string error;
                say.AppendLine(_server.Start(ip, out error));
                if (error.Length > 0) return say.AppendLine("Não vou abrir o jogo sem servidor.").ToString();
                say.AppendLine(_server.WaitUntilReady(ip, 30));
                if (!_server.BackendAnswers()) return say.AppendLine("Não vou abrir o jogo: o backend 20443 não respondeu.").ToString();
            }
            else
            {
                say.AppendLine("O servidor já estava em execução.");
            }

            say.AppendLine("Endereço para o amigo: " + ip + "  (Radmin VPN)");
            return StartGame(ip, say);
        }

        private string DoGuest()
        {
            StringBuilder say = new StringBuilder();
            string ip = _hostIpBox.Text.Trim();
            string problem = Validate(ip);
            if (problem.Length > 0) return "Informe o IP do host (26.x.x.x), aquele que o amigo te passou.";

            say.AppendLine("Servidor do amigo: " + Health.Describe(ip, 1500));
            if (!Health.Reachable(ip, 1500))
            {
                say.AppendLine("Aviso: o servidor do amigo não respondeu. Confira se o Radmin VPN está ligado nos dois e se o host já hospedou.");
            }
            return StartGame(ip, say);
        }

        private string StartGame(string ip, StringBuilder say)
        {
            // The launcher's account is read fresh here: the player may have just signed in on the bbport
            // launcher with this window open. The rules and the controls are the ones this window shows.
            BbportLauncher.Invalidate();

            string error;
            say.AppendLine(Overrides.Write(_port, ip, out error));
            if (error.Length > 0)
            {
                return say.AppendLine("Parei: sem este arquivo o jogo procuraria o servidor da Sony.").ToString();
            }

            if (_config.PlayWith.Length > 0)
            {
                string inputError;
                say.AppendLine(InputConfig.Apply(_port, _config.PlayWith, out inputError));
                RefreshInputHint();
            }

            List<Process> running = Bbport.RunningGame();
            if (running.Count > 0)
            {
                say.AppendLine("Fechando o jogo que já estava aberto...");
                Bbport.CloseGame(running);
            }

            try
            {
                // The rules are the leader's: a guest sends none (the in-game team screen shows the
                // leader's own), so a guest's own Co-op tab choices cannot change someone else's game.
                HostRules rules = _roleHost.Checked ? _config.Rules : new HostRules();
                PlayPlan plan = GameEnvironment.Build(_port, ip, _config.Account, _config.Password, rules);
                _game = GameEnvironment.Start(_port, plan);
                say.AppendLine(_roleHost.Checked
                    ? "Regras do host: " + rules.Describe() + "."
                    : "Regras do co-op: as do líder (você entrou no time dele).");
                say.AppendLine("Jogo aberto (pid " + _game.Id.ToString(CultureInfo.InvariantCulture) + ").");
                say.AppendLine("No jogo: abra o menu, entre no time e convide o amigo.");
            }
            catch (Exception exception)
            {
                say.AppendLine("Não abri o jogo: " + exception.Message);
            }
            return say.ToString();
        }

        private void OnStartServerOnly(object sender, EventArgs e)
        {
            SyncConfigFromUi();
            if (!_port.Ready) return;
            string ip = _ownAddress.Text.Trim();
            if (Validate(ip).Length > 0)
            {
                ShowMessage("Este PC não tem um endereço de VPN agora. Ligue o Radmin VPN.");
                return;
            }
            RunBusy("Iniciando o servidor...", delegate
            {
                string error;
                string line = _server.Start(ip, out error);
                if (error.Length > 0) return line;
                return line + "\n" + _server.WaitUntilReady(ip, 30);
            });
        }

        private void OnStopServer(object sender, EventArgs e)
        {
            RunBusy("Parando o servidor...", delegate { return _server.Stop(); });
        }

        private void OnTestServer(object sender, EventArgs e)
        {
            string ip = _hostIpBox.Text.Trim();
            if (Validate(ip).Length > 0)
            {
                ShowMessage("Informe o IP do host antes de testar.");
                return;
            }
            RunBusy("Testando o servidor...", delegate { return "Servidor em " + ip + ": " + Health.Describe(ip, 2000); });
        }

        private void OnCopyAddress(object sender, EventArgs e)
        {
            string ip = _ownAddress.Text.Trim();
            if (ip.Length == 0) return;
            try
            {
                Clipboard.SetText(ip);
                Append("Endereço copiado: " + ip);
            }
            catch (Exception error)
            {
                Append("Não copiei (" + error.Message + ").");
            }
        }

        private void OnToggleLog(object sender, EventArgs e)
        {
            _logBox.Visible = !_logBox.Visible;
            _logToggle.Text = _logBox.Visible ? "Esconder log" : "Mostrar log";
            if (_logBox.Visible) Append(Log.Tail(60) + _server.Recent(40));
        }

        // ---------------------------------------------------------------- advanced

        private void OnBrowseFolder(object sender, EventArgs e)
        {
            FolderBrowserDialog dialog = new FolderBrowserDialog();
            dialog.Description = "Escolha a pasta do bbport (a que tem Bloodborne.exe)";
            dialog.ShowNewFolderButton = false;
            if (_port.Root.Length > 0) dialog.SelectedPath = _port.Root;
            if (dialog.ShowDialog(this) != DialogResult.OK) return;

            BbportInfo chosen = Bbport.Inspect(dialog.SelectedPath);
            if (!chosen.Ready)
            {
                ShowMessage("Nessa pasta ainda falta:\n\n" + string.Join("\n", chosen.Missing.ToArray()) +
                            "\n\nEscolha a pasta onde está o Bloodborne.exe do bbport.");
                return;
            }
            _config.BbportDir = chosen.Root;
            _config.Save();
            RefreshDetection();
            RefreshState();
            Append("Pasta do bbport: " + chosen.Root + " (" + chosen.Describe() + ")");
        }

        private void OnCheckFolder(object sender, EventArgs e)
        {
            RefreshDetection();
            RefreshState();
            StringBuilder report = new StringBuilder();
            report.AppendLine(_port.Describe());
            foreach (string missing in _port.Missing) report.AppendLine("FALTA: " + missing);
            foreach (string note in _port.Notes) report.AppendLine("aviso: " + note);
            Append(report.ToString());
            ShowMessage(report.ToString());
        }

        private void OnNewPassword(object sender, EventArgs e)
        {
            _passwordBox.Text = AppConfig.NewPassword();
            _showPassword.Checked = true;
            SyncConfigFromUi();
            Append("Senha nova gerada (os dois jogadores podem ter senhas diferentes).");
        }

        /// <summary>Takes the account the player already signs in with in the bbport launcher.</summary>
        private void OnUseLauncherAccount(object sender, EventArgs e)
        {
            BbportLauncher.Invalidate();
            if (!_config.TakeFromBbportLauncher())
            {
                ShowMessage("Não achei uma conta no launcher do bbport.\n\nAbra o launcher do bbport, entre em " +
                            "\"Seamless co-op\" e faça o login por lá uma vez; depois volte e clique aqui.");
                return;
            }
            _config.Save();
            _accountBox.Text = _config.Account;
            _passwordBox.Text = _config.Password;
            Append("Conta do launcher do bbport aplicada: " + _config.Account + ".");
            ShowMessage("Conta do launcher do bbport aplicada: " + _config.Account);
        }

        /// <summary>Keeps the custom boxes and the vote in step with the rules they depend on.</summary>
        private void OnRulesChanged(object sender, EventArgs e)
        {
            if (_ruleHint == null) return;
            if (sender is RadioButton && !((RadioButton)sender).Checked) return;   // ignore the one switching off
            bool custom = SelectedEnemies() == "custom";
            _ruleCustomHp.Enabled = custom;
            _ruleCustomDamage.Enabled = custom;
            _ruleCustomPoise.Enabled = custom;
            _ruleVote.Enabled = _ruleTravel.Checked;
            if (!_ruleTravel.Checked) _ruleVote.Checked = false;
            _ruleHint.Text = "No time, os outros veem: " + RulesFromUi().Describe() + ".";
        }

        /// <summary>Takes the host rules chosen on the bbport launcher's Seamless co-op page.</summary>
        private void OnUseLauncherRules(object sender, EventArgs e)
        {
            if (!_config.TakeRulesFromBbportLauncher())
            {
                ShowMessage("Não achei as regras no launcher do bbport.\n\nAbra o launcher do bbport, entre em " +
                            "\"Seamless co-op\" e escolha as regras do host por lá; depois volte e clique aqui.");
                return;
            }
            LoadRulesIntoUi();
            SyncConfigFromUi();
            Append("Regras do launcher do bbport aplicadas: " + _config.Rules.Describe() + ".");
            ShowMessage("Regras do launcher do bbport: " + _config.Rules.Describe());
        }

        /// <summary>The player changed what they play with: keep it and make input.ini match at once.</summary>
        private void OnPlayWithChanged(object sender, EventArgs e)
        {
            if (_uiLoading) return;
            if (sender is RadioButton && !((RadioButton)sender).Checked) return;
            SyncConfigFromUi();
            if (!_port.Ready)
            {
                RefreshInputHint();
                return;
            }
            string error;
            string line = InputConfig.Apply(_port, _config.PlayWith, out error);
            Append(line);
            RefreshInputHint();
            RefreshState();
        }

        private void OnApplyControls(object sender, EventArgs e)
        {
            SyncConfigFromUi();
            if (!_port.Ready)
            {
                ShowMessage("O bbport ainda não está pronto. Escolha a pasta dele na aba Avançado.");
                return;
            }
            string error;
            string line = InputConfig.Apply(_port, _config.PlayWith, out error);
            Append(line);
            RefreshInputHint();
        }

        private void OnRestoreControls(object sender, EventArgs e)
        {
            if (!_port.Ready) return;
            string error;
            string line = InputConfig.Restore(_port, out error);
            Append(line);
            ShowMessage(line);
            RefreshInputHint();
        }

        /// <summary>
        /// Opens the bbport launcher (Bloodborne.exe with no argument), whose Controls page has a field
        /// per key and per controller button. The very same button the Avançado tab has, with a line
        /// saying what to do there.
        /// </summary>
        private void OnOpenLauncherControls(object sender, EventArgs e)
        {
            OpenOfficialLauncher("Launcher do bbport aberto. Escolha a página \"Controles\" para trocar " +
                                 "teclas e botões.");
        }

        private static int IndexOf(string[] values, string value)
        {
            for (int i = 0; i < values.Length; i++)
            {
                if (string.Equals(values[i], value, StringComparison.OrdinalIgnoreCase)) return i;
            }
            return 0;
        }

        private static string ValueOf(string[] values, int index, string fallback)
        {
            return index >= 0 && index < values.Length ? values[index] : fallback;
        }

        private void OnFirewallButton(object sender, EventArgs e)
        {
            RunBusy("Instalando as regras de firewall (UAC)...", InstallFirewallElevated);
        }

        private string InstallFirewallElevated()
        {
            ProcessStartInfo start = new ProcessStartInfo(Application.ExecutablePath, "--install-firewall");
            start.UseShellExecute = true;
            start.Verb = "runas";
            start.WindowStyle = ProcessWindowStyle.Hidden;
            try
            {
                using (Process process = Process.Start(start)) process.WaitForExit();
            }
            catch (Exception error)
            {
                return "Não elevei o BBRadmin (" + error.Message + ").\n" +
                       "Você pode colar os comandos do lateral num console de administrador:\n" + Firewall.ManualCommands();
            }

            string report = "";
            try
            {
                if (File.Exists(Program.FirewallReportPath)) report = File.ReadAllText(Program.FirewallReportPath);
            }
            catch (Exception) { }

            List<string> still = Firewall.Missing();
            if (still.Count == 0) return "Firewall pronto: as três regras estão instaladas.\n" + report;
            return "Ainda faltam regras: " + string.Join(", ", still.ToArray()) + "\n" + report;
        }

        private void OnRestoreOverrides(object sender, EventArgs e)
        {
            if (!_port.Ready) return;
            if (MessageBox.Show(this, "Voltar o " + CoopSchema.OverridesFile + " ao que estava antes?\n\n" +
                                      "(o bbport volta a usar o servidor dele)", "BBRadmin",
                                MessageBoxButtons.YesNo, MessageBoxIcon.Question) != DialogResult.Yes) return;
            string error;
            string line = Overrides.Restore(_port, out error);
            Append(line);
            ShowMessage(line);
        }

        private void OnOpenOfficialLauncher(object sender, EventArgs e)
        {
            OpenOfficialLauncher("Launcher oficial aberto (para gráficos, FPS e mods do bbport).");
        }

        private void OpenOfficialLauncher(string said)
        {
            if (!_port.Ready)
            {
                ShowMessage("O bbport ainda não está pronto. Escolha a pasta dele na aba Avançado.");
                return;
            }
            try
            {
                ProcessStartInfo start = new ProcessStartInfo(Path.Combine(_port.Root, CoopSchema.GameExe));
                start.WorkingDirectory = _port.Root;
                start.UseShellExecute = true;
                Process.Start(start);
                Append(said);
            }
            catch (Exception error)
            {
                ShowMessage("Não abri o launcher do bbport: " + error.Message);
            }
        }

        private void OnOpenLogs(object sender, EventArgs e)
        {
            OpenFolder(Path.Combine(_port.Root, CoopSchema.CoopUserDir, "log"));
        }

        private void OnOpenPortFolder(object sender, EventArgs e)
        {
            OpenFolder(_port.Root);
        }

        private void OpenFolder(string path)
        {
            try
            {
                if (path.Length == 0) return;
                if (!Directory.Exists(path)) Directory.CreateDirectory(path);
                Process.Start("explorer.exe", "\"" + path + "\"");
            }
            catch (Exception error)
            {
                Append("Não abri a pasta (" + error.Message + ").");
            }
        }

        private void OnDryRun(object sender, EventArgs e)
        {
            SyncConfigFromUi();
            try
            {
                DryRun.Run(_config);
                Append("Relatório do dry-run:\n" + File.ReadAllText(DryRun.FilePath));
                Process.Start(DryRun.FilePath);
            }
            catch (Exception error)
            {
                ShowMessage("Não rodei o dry-run: " + error.Message);
            }
        }

        private void OnClosing(object sender, FormClosingEventArgs e)
        {
            _tick.Stop();
            bool host = _roleHost.Checked;
            if (host && _server.StartedByUs && !_keepServer.Checked)
            {
                Append("Fechando o BBRadmin: parando o servidor que eu iniciei.");
                _server.Stop();
            }
            Log.Line("BBRadmin fechado.");
        }

        // ---------------------------------------------------------------- plumbing

        private void RunBusy(string what, Func<string> work)
        {
            if (_busy) return;
            _busy = true;
            Append(what);
            Log.Line(what);
            RefreshState();

            BackgroundWorker worker = new BackgroundWorker();
            worker.DoWork += delegate(object ignored, DoWorkEventArgs done)
            {
                try
                {
                    done.Result = work();
                }
                catch (Exception error)
                {
                    Log.Line("Erro em \"" + what + "\": " + error);
                    done.Result = "Erro: " + error.Message;
                }
            };
            worker.RunWorkerCompleted += delegate(object ignored, RunWorkerCompletedEventArgs done)
            {
                _busy = false;
                if (done.Error != null) Append("Erro: " + done.Error.Message);
                else Append(Convert.ToString(done.Result));
                RefreshState();
            };
            worker.RunWorkerAsync();
        }

        private void Append(string text)
        {
            if (string.IsNullOrEmpty(text)) return;
            if (InvokeRequired)
            {
                try { BeginInvoke((MethodInvoker)delegate { Append(text); }); }
                catch (Exception) { }
                return;
            }
            string stamp = DateTime.Now.ToString("HH:mm:ss", CultureInfo.InvariantCulture);
            string[] lines = text.Replace("\r\n", "\n").TrimEnd('\n').Split('\n');
            StringBuilder block = new StringBuilder();
            foreach (string line in lines) block.Append(stamp).Append("  ").Append(line).Append("\r\n");

            _logBox.AppendText(block.ToString());
            if (_logBox.Lines.Length > 800)
            {
                _logBox.Text = string.Join("\r\n", Tail(_logBox.Lines, 500));
                _logBox.SelectionStart = _logBox.TextLength;
                _logBox.ScrollToCaret();
            }
        }

        private static string[] Tail(string[] lines, int count)
        {
            if (lines.Length <= count) return lines;
            string[] result = new string[count];
            Array.Copy(lines, lines.Length - count, result, 0, count);
            return result;
        }

        /// <summary>A question, asked on the window's own thread from a background step.</summary>
        private bool AskOnUi(string text)
        {
            bool answer = false;
            try
            {
                Invoke((MethodInvoker)delegate
                {
                    answer = MessageBox.Show(this, text, "BBRadmin", MessageBoxButtons.YesNo, MessageBoxIcon.Question) == DialogResult.Yes;
                });
            }
            catch (Exception)
            {
                answer = false;
            }
            return answer;
        }

        private void ShowMessage(string text)
        {
            MessageBox.Show(this, text, "BBRadmin", MessageBoxButtons.OK, MessageBoxIcon.Information);
        }
    }
}
