//! BBRadmin graphical interface (eframe / egui), preserving all four tabs (Jogar, Co-op,
//! Controles, Avançado), collapsible log panel, background workers, UAC firewall helper, and
//! automatic WindowPlacer management for fullscreen / borderless fullscreen.

use std::ffi::OsStr;
use std::fs;
use std::net::Ipv4Addr;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use chrono::Local;
use eframe::egui;
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::System::Threading::{WaitForSingleObject, INFINITE};
use windows_sys::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;

use crate::core::app_config::{self, AppConfig};
use crate::core::bbport::{self, BbportInfo};
use crate::core::bbport_launcher;
use crate::core::coop_schema;
use crate::core::display::{self, DisplayMode, ResolvedDisplay, WindowPlacer};
use crate::core::dry_run;
use crate::core::firewall;
use crate::core::game_environment;
use crate::core::health;
use crate::core::host_rules::HostRules;
use crate::core::input_config;
use crate::core::log;
use crate::core::overrides;
use crate::core::radmin::{self, NetworkChoice};
use crate::core::server_controller::ServerController;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

const ENEMIES_VALUES: &[&str] = &["off", "auto", "bit", "on", "much", "custom"];
const ENEMIES_LABELS: &[&str] = &[
    "Normal — como o jogo: vida ×1 · dano ×1 · equilíbrio ×1   (padrão)",
    "Automático — sobe com cada caçador no mesmo mapa (até \"Mais fortes\" com o time cheio)",
    "Um pouco mais fortes — vida ×1,5 · dano ×1,25 · equilíbrio ×1,5",
    "Mais fortes — vida ×3 · dano ×1,5 · equilíbrio ×3",
    "Muito mais fortes — vida ×5 · dano ×2 · equilíbrio ×5",
    "Personalizado — os meus números (1 a 10; 1 = o do jogo):",
];

const TEMPO_VALUES: &[&str] = &["", "aggressive", "relentless"];
const TEMPO_LABELS: &[&str] = &[
    "Normal — como o jogo (padrão)",
    "Agressivo — metade das pausas, giros ×1,3",
    "Implacável — um quarto das pausas, giros ×1,6",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Play,
    Coop,
    Controls,
    Advanced,
}

enum WorkerMsg {
    Done(String),
    GameStarted {
        child: Child,
        display: ResolvedDisplay,
    },
    RefreshInputHint,
}

#[derive(Debug, Clone)]
struct ServerSnapshot {
    running: bool,
    state: &'static str,
}

pub fn run(config: AppConfig) -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("BBRadmin · co-op em Radmin VPN")
            .with_inner_size([660.0, 700.0])
            .with_min_inner_size([610.0, 520.0]),
        ..Default::default()
    };

    eframe::run_native(
        "BBRadmin",
        options,
        Box::new(move |cc| {
            configure_visuals(&cc.egui_ctx);
            Ok(Box::new(BbradminApp::new(config, cc.egui_ctx.clone())))
        }),
    )
}

fn configure_visuals(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::light();
    visuals.window_fill = egui::Color32::from_rgb(243, 243, 243);
    visuals.panel_fill = egui::Color32::from_rgb(243, 243, 243);
    visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(248, 248, 248);
    ctx.set_visuals(visuals);
}

struct BbradminApp {
    config: AppConfig,
    tab: Tab,
    networks: Vec<NetworkChoice>,
    own_address: String,
    port: BbportInfo,
    server: Arc<ServerController>,
    server_snapshot: Arc<Mutex<ServerSnapshot>>,
    poller_stop: Arc<AtomicBool>,

    game: Option<Child>,
    placer: Option<WindowPlacer>,

    busy: bool,
    tx: Sender<WorkerMsg>,
    rx: Receiver<WorkerMsg>,
    ctx: egui::Context,

    // UI state
    show_password: bool,
    auto_firewall: bool,
    show_log: bool,
    log_lines: Vec<String>,
    launcher_account_hint: String,
    launcher_display_hint: String,
    input_hint: String,
}

impl BbradminApp {
    fn new(config: AppConfig, ctx: egui::Context) -> Self {
        let (tx, rx) = mpsc::channel();
        let server = Arc::new(ServerController::new());
        let server_snapshot = Arc::new(Mutex::new(ServerSnapshot {
            running: false,
            state: "parado",
        }));
        let poller_stop = Arc::new(AtomicBool::new(false));

        // Background poller for server status so the UI never blocks on TCP timeouts.
        {
            let srv = Arc::clone(&server);
            let snap = Arc::clone(&server_snapshot);
            let stop = Arc::clone(&poller_stop);
            let repaint_ctx = ctx.clone();
            thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    let running = srv.running();
                    let state = srv.state();
                    if let Ok(mut guard) = snap.lock() {
                        guard.running = running;
                        guard.state = state;
                    }
                    repaint_ctx.request_repaint();
                    for _ in 0..15 {
                        if stop.load(Ordering::Relaxed) {
                            break;
                        }
                        thread::sleep(Duration::from_millis(100));
                    }
                }
            });
        }

        let mut app = Self {
            config,
            tab: Tab::Play,
            networks: Vec::new(),
            own_address: String::new(),
            port: BbportInfo::default(),
            server,
            server_snapshot,
            poller_stop,
            game: None,
            placer: None,
            busy: false,
            tx,
            rx,
            ctx,
            show_password: false,
            auto_firewall: true,
            show_log: false,
            log_lines: Vec::new(),
            launcher_account_hint: String::new(),
            launcher_display_hint: String::new(),
            input_hint: String::new(),
        };
        app.refresh_detection();
        log::line("BBRadmin (Rust) aberto.");
        app
    }

    fn is_host(&self) -> bool {
        self.config.role != app_config::ROLE_GUEST
    }

    fn append_log(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let stamp = Local::now().format("%H:%M:%S");
        for line in text.replace("\r\n", "\n").trim_end_matches('\n').lines() {
            self.log_lines.push(format!("{stamp}  {line}"));
        }
        if self.log_lines.len() > 800 {
            let drain = self.log_lines.len() - 500;
            self.log_lines.drain(0..drain);
        }
    }

    fn refresh_detection(&mut self) {
        self.networks = radmin::detect();
        self.own_address = radmin::pick(&self.networks)
            .map(|c| c.ip.clone())
            .unwrap_or_default();

        let mut folder = self.config.bbport_dir.clone();
        if folder.is_empty() {
            folder = bbport::guess_folder();
        }
        self.port = bbport::inspect(&folder);
        if self.config.bbport_dir != self.port.root && !self.port.root.is_empty() {
            self.config.bbport_dir = self.port.root.clone();
            self.config.save();
        }

        let launcher = bbport_launcher::load();
        self.launcher_account_hint = if !launcher.supermedo_npid.is_empty() {
            format!(
                "Conta do seu launcher do bbport: {} — a mesma do Seamless co-op.",
                launcher.supermedo_npid
            )
        } else {
            "Sem conta no launcher do bbport: o servidor cria uma sozinho no primeiro entrar."
                .to_string()
        };
        self.launcher_display_hint = format!(
            "No launcher do bbport (settings.json): {}",
            display::describe_launcher_mode(&launcher)
        );

        if self.config.play_with.is_empty() {
            let current = input_config::current(&self.port);
            if !current.is_empty() {
                self.config.play_with = current;
                self.config.save();
            }
        }
        self.refresh_input_hint();
    }

    fn refresh_input_hint(&mut self) {
        let file = if self.port.ready() {
            input_config::path_in(&self.port).to_string_lossy().to_string()
        } else {
            String::new()
        };
        self.input_hint = if file.is_empty() {
            format!("Agora: {}.", input_config::describe(&self.port))
        } else {
            format!(
                "Agora: {}.\nArquivo: {file}",
                input_config::describe(&self.port)
            )
        };
    }

    fn poll_background(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                WorkerMsg::Done(text) => {
                    self.busy = false;
                    self.append_log(&text);
                }
                WorkerMsg::GameStarted { child, display } => {
                    // Stop any previous WindowPlacer and start a fresh one for this game launch.
                    self.placer = Some(WindowPlacer::start(display));
                    self.game = Some(child);
                }
                WorkerMsg::RefreshInputHint => {
                    self.refresh_input_hint();
                }
            }
        }

        // Check if the launched game process has exited.
        let mut exited = false;
        if let Some(child) = self.game.as_mut() {
            match child.try_wait() {
                Ok(Some(_)) | Err(_) => exited = true,
                Ok(None) => {}
            }
        }
        if exited {
            self.game = None;
            self.placer = None; // drops & stops the WindowPlacer thread
            self.append_log("O jogo fechou.");
            log::line("Jogo fechou.");
            if self.is_host() && self.server.started_by_us() && self.config.stop_server_with_game {
                self.append_log("Parando o servidor que eu iniciei (opção marcada).");
                let srv = Arc::clone(&self.server);
                self.spawn_worker("Parando o servidor...", move |_| srv.stop());
            }
        }
    }

    fn spawn_worker<F>(&mut self, what: &str, f: F)
    where
        F: FnOnce(Sender<WorkerMsg>) -> String + Send + 'static,
    {
        if self.busy {
            return;
        }
        self.busy = true;
        self.append_log(what);
        log::line(what);
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        thread::spawn(move || {
            let result = f(tx.clone());
            let _ = tx.send(WorkerMsg::Done(result));
            ctx.request_repaint();
        });
    }

    fn on_play(&mut self) {
        self.config.save();
        if !self.port.ready() {
            show_info("O bbport ainda não está pronto.\n\nAbra a aba Avançado, escolha a pasta do bbport (a que tem Bloodborne.exe) e clique em Verificar.");
            return;
        }

        if self.is_host() {
            let ip = self.own_address.trim().to_string();
            if let Err(problem) = validate_ip(&ip) {
                show_info(&format!(
                    "Este PC não tem um endereço de VPN agora ({problem}).\n\nLigue o Radmin VPN e clique em Recarregar na aba Avançado."
                ));
                return;
            }
            let port = self.port.clone();
            let config = self.config.clone();
            let server = Arc::clone(&self.server);
            let auto_fw = self.auto_firewall;
            self.spawn_worker("Hospedando o co-op...", move |tx| {
                do_host(port, config, server, ip, auto_fw, tx)
            });
        } else {
            let ip = self.config.remote_ip.trim().to_string();
            let port = self.port.clone();
            let config = self.config.clone();
            self.spawn_worker("Entrando no co-op...", move |tx| {
                do_guest(port, config, ip, tx)
            });
        }
    }

    fn on_start_server_only(&mut self) {
        self.config.save();
        if !self.port.ready() {
            return;
        }
        let ip = self.own_address.trim().to_string();
        if validate_ip(&ip).is_err() {
            show_info("Este PC não tem um endereço de VPN agora. Ligue o Radmin VPN.");
            return;
        }
        let server = Arc::clone(&self.server);
        self.spawn_worker("Iniciando o servidor...", move |_| match server.start(&ip) {
            Ok(line) => format!("{line}\n{}", server.wait_until_ready(&ip, 30)),
            Err(err) => err,
        });
    }

    fn on_stop_server(&mut self) {
        let server = Arc::clone(&self.server);
        self.spawn_worker("Parando o servidor...", move |_| server.stop());
    }

    fn on_test_server(&mut self) {
        let ip = self.config.remote_ip.trim().to_string();
        if validate_ip(&ip).is_err() {
            show_info("Informe o IP do host antes de testar.");
            return;
        }
        self.spawn_worker("Testando o servidor...", move |_| {
            format!("Servidor em {ip}: {}", health::describe(&ip, 2000))
        });
    }

    fn on_browse_folder(&mut self) {
        let mut dialog = rfd::FileDialog::new()
            .set_title("Escolha a pasta do bbport (a que tem Bloodborne.exe)");
        if !self.port.root.is_empty() && Path::new(&self.port.root).is_dir() {
            dialog = dialog.set_directory(&self.port.root);
        }
        let Some(picked) = dialog.pick_folder() else {
            return;
        };
        let chosen = bbport::inspect(&picked.to_string_lossy());
        if !chosen.ready() {
            show_info(&format!(
                "Nessa pasta ainda falta:\n\n{}\n\nEscolha a pasta onde está o Bloodborne.exe do bbport.",
                chosen.missing.join("\n")
            ));
            return;
        }
        self.config.bbport_dir = chosen.root.clone();
        self.config.save();
        self.refresh_detection();
        self.append_log(&format!(
            "Pasta do bbport: {} ({})",
            chosen.root,
            chosen.describe()
        ));
    }

    fn open_official_launcher(&mut self, said: &str) {
        if !self.port.ready() {
            show_info("O bbport ainda não está pronto. Escolha a pasta dele na aba Avançado.");
            return;
        }
        let exe = Path::new(&self.port.root).join(coop_schema::GAME_EXE);
        match Command::new(&exe).current_dir(&self.port.root).spawn() {
            Ok(_) => self.append_log(said),
            Err(err) => show_info(&format!("Não abri o launcher do bbport: {err}")),
        }
    }

    fn open_folder(&mut self, path: &Path) {
        if path.as_os_str().is_empty() {
            return;
        }
        let _ = fs::create_dir_all(path);
        if let Err(err) = Command::new("explorer.exe").arg(path).spawn() {
            self.append_log(&format!("Não abri a pasta ({err})."));
        }
    }
}

impl eframe::App for BbradminApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_background();

        // Keep polling while game is running or busy so exit detection is timely.
        if self.busy || self.game.is_some() {
            ctx.request_repaint_after(Duration::from_millis(500));
        }

        let snap = self
            .server_snapshot
            .lock()
            .map(|g| g.clone())
            .unwrap_or(ServerSnapshot {
                running: false,
                state: "parado",
            });

        // ---- Header ----
        egui::TopBottomPanel::top("header")
            .frame(
                egui::Frame::NONE
                    .fill(egui::Color32::WHITE)
                    .inner_margin(egui::Margin::symmetric(14, 10)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("BBRadmin")
                                .size(19.0)
                                .strong()
                                .color(egui::Color32::from_rgb(24, 24, 24)),
                        );
                        ui.label(
                            egui::RichText::new("co-op contínuo para Bloodborne, por Radmin VPN")
                                .size(12.0)
                                .color(egui::Color32::from_rgb(105, 105, 105)),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(self.port.describe())
                                .size(11.5)
                                .color(egui::Color32::from_rgb(105, 105, 105)),
                        );
                    });
                });
            });

        // ---- Collapsible Log Panel ----
        if self.show_log {
            egui::TopBottomPanel::bottom("log_panel")
                .resizable(true)
                .default_height(190.0)
                .min_height(100.0)
                .frame(
                    egui::Frame::NONE
                        .fill(egui::Color32::from_rgb(250, 250, 250))
                        .stroke(egui::Stroke::new(
                            1.0_f32,
                            egui::Color32::from_rgb(215, 215, 215),
                        ))
                        .inner_margin(egui::Margin::same(8)),
                )
                .show(ctx, |ui| {
                    egui::ScrollArea::both()
                        .auto_shrink([false, false])
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            let content = self.log_lines.join("\n");
                            ui.add(
                                egui::TextEdit::multiline(&mut content.as_str())
                                    .font(egui::TextStyle::Monospace)
                                    .desired_width(f32::INFINITY)
                                    .frame(false),
                            );
                        });
                });
        }

        // ---- Bottom Status Bar ----
        egui::TopBottomPanel::bottom("bottom_bar")
            .frame(
                egui::Frame::NONE
                    .fill(egui::Color32::from_rgb(236, 236, 236))
                    .stroke(egui::Stroke::new(
                        1.0_f32,
                        egui::Color32::from_rgb(215, 215, 215),
                    ))
                    .inner_margin(egui::Margin::symmetric(14, 7)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let host = self.is_host();
                    let mut state = format!(
                        "Servidor: {}",
                        if host { snap.state } else { "no PC do amigo" }
                    );
                    if host
                        && self.port.ready()
                        && !radmin::is_vpn(radmin::kind_of_ip_only(&self.own_address))
                        && !self.networks.is_empty()
                    {
                        state.push_str("  ·  ligue o Radmin VPN");
                    }
                    if self.busy {
                        state = format!("Trabalhando...  ·  {state}");
                    }
                    ui.label(egui::RichText::new(state).size(12.0));

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let btn_text = if self.show_log {
                            "Esconder log"
                        } else {
                            "Mostrar log"
                        };
                        if ui.button(btn_text).clicked() {
                            self.show_log = !self.show_log;
                            if self.show_log {
                                let combined =
                                    format!("{}{}", log::tail(60), self.server.recent(40));
                                self.append_log(&combined);
                            }
                        }
                    });
                });
            });

        // ---- Tabs + Main Content ----
        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(egui::Color32::from_rgb(243, 243, 243))
                    .inner_margin(egui::Margin::symmetric(12, 8)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.tab, Tab::Play, "  Jogar  ");
                    ui.selectable_value(&mut self.tab, Tab::Coop, "  Co-op  ");
                    ui.selectable_value(&mut self.tab, Tab::Controls, "  Controles  ");
                    ui.selectable_value(&mut self.tab, Tab::Advanced, "  Avançado  ");
                });
                ui.separator();

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| match self.tab {
                        Tab::Play => self.render_play_tab(ui, &snap),
                        Tab::Coop => self.render_coop_tab(ui),
                        Tab::Controls => self.render_controls_tab(ui),
                        Tab::Advanced => self.render_advanced_tab(ui),
                    });
            });
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.poller_stop.store(true, Ordering::Relaxed);
        self.placer = None;
        if self.is_host() && self.server.started_by_us() && !self.config.keep_server {
            log::line("Fechando o BBRadmin: parando o servidor que eu iniciei.");
            self.server.stop();
        }
        log::line("BBRadmin fechado.");
    }
}

impl BbradminApp {
    fn render_play_tab(&mut self, ui: &mut egui::Ui, snap: &ServerSnapshot) {
        let mut host = self.is_host();
        group_box(ui, "Quem hospeda?", |ui| {
            if ui
                .radio(host, "Eu hospedo (o servidor roda neste PC)")
                .clicked()
            {
                self.config.role = app_config::ROLE_HOST.to_string();
                self.config.save();
                host = true;
            }
            if ui
                .radio(!host, "Eu entro (o servidor está no PC do amigo)")
                .clicked()
            {
                self.config.role = app_config::ROLE_GUEST.to_string();
                self.config.save();
                host = false;
            }
        });

        ui.add_space(6.0);
        group_box(ui, "Endereço", |ui| {
            if host {
                ui.label("Este PC (dê este endereço ao amigo):");
                ui.horizontal(|ui| {
                    let mut addr_display = self.own_address.clone();
                    ui.add(
                        egui::TextEdit::singleline(&mut addr_display)
                            .desired_width(300.0)
                            .interactive(false),
                    );
                    if ui.button("Copiar").clicked() && !self.own_address.is_empty() {
                        ui.ctx().copy_text(self.own_address.clone());
                        let ip = self.own_address.clone();
                        self.append_log(&format!("Endereço copiado: {ip}"));
                    }
                });
            } else {
                ui.label("IP do host (o amigo te passa, 26.x.x.x):");
                ui.horizontal(|ui| {
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut self.config.remote_ip)
                            .desired_width(300.0)
                            .hint_text("26.x.x.x"),
                    );
                    if resp.lost_focus() {
                        self.config.save();
                    }
                    let valid = validate_ip(self.config.remote_ip.trim()).is_ok();
                    if ui
                        .add_enabled(!self.busy && valid, egui::Button::new("Testar"))
                        .clicked()
                    {
                        self.on_test_server();
                    }
                });
            }
        });

        ui.add_space(6.0);
        group_box(ui, "Começar", |ui| {
            ui.horizontal(|ui| {
                let play_btn = egui::Button::new(
                    egui::RichText::new("Jogar co-op").size(16.0).strong(),
                )
                .min_size(egui::vec2(210.0, 52.0));
                if ui
                    .add_enabled(!self.busy && self.port.ready(), play_btn)
                    .clicked()
                {
                    self.on_play();
                }

                ui.vertical(|ui| {
                    if ui
                        .add_enabled(
                            !self.busy && host && self.port.ready() && !snap.running,
                            egui::Button::new("Só iniciar o servidor")
                                .min_size(egui::vec2(155.0, 24.0)),
                        )
                        .clicked()
                    {
                        self.on_start_server_only();
                    }
                    if ui
                        .add_enabled(
                            !self.busy && snap.running,
                            egui::Button::new("Parar o servidor")
                                .min_size(egui::vec2(155.0, 24.0)),
                        )
                        .clicked()
                    {
                        self.on_stop_server();
                    }
                });

                let hint = if host {
                    "Você hospeda: o servidor sobe e o jogo abre. Depois passe seu endereço ao amigo."
                } else {
                    "Você entra: o jogo abre apontando para o servidor do amigo."
                };
                ui.label(
                    egui::RichText::new(hint)
                        .size(11.5)
                        .color(egui::Color32::from_rgb(100, 100, 100)),
                );
            });
        });

        ui.add_space(6.0);
        group_box(ui, "Como funciona", |ui| {
            ui.label(
                "1.  Os dois ligam o Radmin VPN e entram na mesma rede (aparece um IP 26.x.x.x).\n\
                 2.  Um clica em \"Jogar co-op\" com \"Eu hospedo\" e passa o endereço ao outro (botão Copiar).\n\
                 3.  O outro escolhe \"Eu entro\", cola o endereço em \"IP do host\" e clica em \"Jogar co-op\".\n\
                 4.  No jogo, o que hospeda abre o menu, cria o time e convida; o amigo aceita o convite.\n\
                 5.  Para sair, feche o jogo. O servidor continua rodando para o próximo encontro.\n\n\
                 As regras do co-op ficam na aba \"Co-op\"; os controles, na aba \"Controles\".",
            );
        });
    }

    fn render_coop_tab(&mut self, ui: &mut egui::Ui) {
        ui.label(
            egui::RichText::new(
                "As regras do líder do time: valem para o time inteiro e aparecem na tela do time \
                 dentro do jogo. Só Joining / Invading / Death mode se mudam ali. Mudanças contam \
                 do próximo start (marque antes de \"Jogar co-op\").",
            )
            .size(12.0)
            .color(egui::Color32::from_rgb(95, 95, 95)),
        );
        ui.add_space(6.0);

        let mut changed = false;

        group_box(ui, "Inimigos mais fortes", |ui| {
            for (val, label) in ENEMIES_VALUES.iter().zip(ENEMIES_LABELS.iter()) {
                let selected = self.config.rules.enemies.eq_ignore_ascii_case(val);
                if ui.radio(selected, *label).clicked() {
                    self.config.rules.enemies = (*val).to_string();
                    changed = true;
                }
            }
            let is_custom = self.config.rules.enemies.eq_ignore_ascii_case("custom");
            ui.add_enabled_ui(is_custom, |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(18.0);
                    ui.label("Vida");
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut self.config.rules.custom_hp)
                                .desired_width(48.0),
                        )
                        .changed()
                    {
                        changed = true;
                    }
                    ui.label("Dano");
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut self.config.rules.custom_damage)
                                .desired_width(48.0),
                        )
                        .changed()
                    {
                        changed = true;
                    }
                    ui.label("Equilíbrio");
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut self.config.rules.custom_poise)
                                .desired_width(48.0),
                        )
                        .changed()
                    {
                        changed = true;
                    }
                });
            });
        });

        ui.add_space(6.0);
        group_box(ui, "Ritmo do chefe", |ui| {
            ui.label(
                "Nos dois de baixo o chefe descansa menos e vira mais rápido (os golpes e o dano iguais).",
            );
            let current_idx = TEMPO_VALUES
                .iter()
                .position(|v| self.config.rules.tempo.eq_ignore_ascii_case(v))
                .unwrap_or(0);
            egui::ComboBox::from_id_salt("tempo_combo")
                .width(340.0)
                .selected_text(TEMPO_LABELS[current_idx])
                .show_ui(ui, |ui| {
                    for (idx, label) in TEMPO_LABELS.iter().enumerate() {
                        if ui.selectable_label(idx == current_idx, *label).clicked() {
                            self.config.rules.tempo = TEMPO_VALUES[idx].to_string();
                            changed = true;
                        }
                    }
                });
        });

        ui.add_space(6.0);
        group_box(ui, "Outras regras", |ui| {
            if ui
                .checkbox(
                    &mut self.config.rules.rot,
                    "Podridão (rot) — cada morte de verdade dá uma penalidade (até 5)",
                )
                .changed()
            {
                changed = true;
            }
            note_label(
                ui,
                "Uma lâmpada, um nível, um chefe, seu assassino ou sua mancha de sangue tiram.",
            );

            if ui
                .checkbox(
                    &mut self.config.rules.travel,
                    "Viajar juntos — uma lâmpada ou lápide pergunta se leva o time inteiro",
                )
                .changed()
            {
                if !self.config.rules.travel {
                    self.config.rules.vote = false;
                }
                changed = true;
            }
            note_label(ui, "Desligado: cada um viaja sozinho.");

            ui.horizontal(|ui| {
                ui.add_space(16.0);
                if ui
                    .add_enabled(
                        self.config.rules.travel,
                        egui::Checkbox::new(
                            &mut self.config.rules.vote,
                            "Com votação — o time tem 20 s; um \"não\" cancela a viagem (precisa de \"Viajar juntos\")",
                        ),
                    )
                    .changed()
                {
                    changed = true;
                }
            });

            if ui
                .checkbox(
                    &mut self.config.rules.revive,
                    "Reviver os caídos — os companheiros levantam um caçador caído",
                )
                .changed()
            {
                changed = true;
            }
            note_label(
                ui,
                "Desligado: morre de verdade, como no jogo normal.  (Padrão: ligado.)",
            );
        });

        if changed {
            self.config.save();
        }

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!(
                    "No time, os outros veem: {}.",
                    self.config.rules.describe()
                ))
                .size(12.0)
                .color(egui::Color32::from_rgb(95, 95, 95)),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Pegar do bbport").clicked() {
                    if !self.config.take_rules_from_bbport_launcher() {
                        show_info(
                            "Não achei as regras no launcher do bbport.\n\nAbra o launcher do bbport, entre em \"Seamless co-op\" e escolha as regras do host por lá; depois volte e clique aqui.",
                        );
                    } else {
                        self.config.save();
                        let desc = self.config.rules.describe();
                        self.append_log(&format!(
                            "Regras do launcher do bbport aplicadas: {desc}."
                        ));
                        show_info(&format!("Regras do launcher do bbport: {desc}"));
                    }
                }
            });
        });
    }

    fn render_controls_tab(&mut self, ui: &mut egui::Ui) {
        ui.label(
            egui::RichText::new(
                "O jogo aceita teclado/mouse e controle ao mesmo tempo: as teclas e os botões do \
                 controle respondem sempre. A escolha abaixo decide só o mouse.",
            )
            .size(12.0)
            .color(egui::Color32::from_rgb(95, 95, 95)),
        );
        ui.add_space(6.0);

        let is_keyboard = self.config.play_with != input_config::CONTROLLER;
        let mut apply_now = false;

        group_box(ui, "Com o que você joga", |ui| {
            if ui
                .radio(
                    is_keyboard,
                    "Teclado e mouse — o mouse gira a câmera (layout Dark Souls III) e fica capturado no jogo",
                )
                .clicked()
                && !is_keyboard
            {
                self.config.play_with = input_config::KEYBOARD.to_string();
                apply_now = true;
            }
            if ui
                .radio(
                    !is_keyboard,
                    "Controle (gamepad) — o mouse não mexe a câmera; o ponteiro fica livre",
                )
                .clicked()
                && is_keyboard
            {
                self.config.play_with = input_config::CONTROLLER.to_string();
                apply_now = true;
            }
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(
                    "Durante o jogo: F7 liga/desliga o mouse e F8 recarrega o arquivo. \
                     As teclas continuam valendo nas duas opções.",
                )
                .size(11.5)
                .color(egui::Color32::from_rgb(100, 100, 100)),
            );
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(&self.input_hint)
                    .size(11.5)
                    .color(egui::Color32::from_rgb(90, 90, 90)),
            );
        });

        if apply_now {
            self.config.save();
            if self.port.ready() {
                let msg = match input_config::apply(&self.port, &self.config.play_with) {
                    Ok(m) | Err(m) => m,
                };
                self.append_log(&msg);
            }
            self.refresh_input_hint();
        }

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui.button("Aplicar agora").clicked() {
                if self.config.play_with.is_empty() {
                    self.config.play_with = input_config::KEYBOARD.to_string();
                }
                self.config.save();
                if !self.port.ready() {
                    show_info(
                        "O bbport ainda não está pronto. Escolha a pasta dele na aba Avançado.",
                    );
                } else {
                    let msg = match input_config::apply(&self.port, &self.config.play_with) {
                        Ok(m) | Err(m) => m,
                    };
                    self.append_log(&msg);
                    self.refresh_input_hint();
                }
            }
            if ui.button("Restaurar input.ini").clicked() && self.port.ready() {
                let msg = match input_config::restore(&self.port) {
                    Ok(m) | Err(m) => m,
                };
                self.append_log(&msg);
                show_info(&msg);
                self.refresh_input_hint();
            }
            if ui.button("Abrir launcher do bbport").clicked() {
                self.open_official_launcher(
                    "Launcher do bbport aberto. Escolha a página \"Controles\" para trocar teclas e botões.",
                );
            }
        });

        ui.add_space(6.0);
        group_box(ui, "Trocar tecla por tecla, ou botão do controle", |ui| {
            ui.label(
                "O BBRadmin só alterna teclado+mouse / controle (acima). Para escolher cada tecla e cada \
                 botão, use a página Controles do launcher do bbport (botão acima): lá dá para clicar num \
                 campo e apertar a tecla ou o botão do controle.\n\n\
                 O arquivo é o input.ini da pasta do bbport. O BBRadmin guarda uma cópia dele \
                 (input.ini.bbradmin-bak) antes de mexer, e \"Restaurar input.ini\" devolve a sua.",
            );
        });
    }

    fn render_advanced_tab(&mut self, ui: &mut egui::Ui) {
        group_box(ui, "Pasta do bbport", |ui| {
            ui.label("A que tem o Bloodborne.exe:");
            ui.horizontal(|ui| {
                let mut root_disp = self.port.root.clone();
                ui.add(
                    egui::TextEdit::singleline(&mut root_disp)
                        .desired_width(420.0)
                        .interactive(false),
                );
                if ui.button("Procurar...").clicked() {
                    self.on_browse_folder();
                }
                if ui.button("Verificar").clicked() {
                    self.refresh_detection();
                    let mut rep = format!("{}\n", self.port.describe());
                    for m in &self.port.missing {
                        rep.push_str(&format!("FALTA: {m}\n"));
                    }
                    for n in &self.port.notes {
                        rep.push_str(&format!("aviso: {n}\n"));
                    }
                    self.append_log(&rep);
                    show_info(&rep);
                }
            });
            let info_text = if !self.port.missing.is_empty() {
                format!("Não deu para usar: {}", self.port.missing.join(", "))
            } else if !self.port.notes.is_empty() {
                format!("{} · observe: {}", self.port.root, self.port.notes.join(", "))
            } else if !self.port.root.is_empty() {
                self.port.describe()
            } else {
                "Escolha a pasta onde você descompactou o bbport (a que tem Bloodborne.exe)."
                    .to_string()
            };
            ui.label(
                egui::RichText::new(info_text)
                    .size(11.5)
                    .color(egui::Color32::from_rgb(95, 95, 95)),
            );
        });

        ui.add_space(6.0);
        group_box(ui, "Modo de tela (tela cheia / sem bordas)", |ui| {
            ui.horizontal(|ui| {
                ui.label("Ao iniciar o jogo:");
                let current = self.config.display_mode;
                egui::ComboBox::from_id_salt("display_mode_combo")
                    .width(380.0)
                    .selected_text(current.label())
                    .show_ui(ui, |ui| {
                        for &mode in DisplayMode::ALL {
                            if ui.selectable_label(mode == current, mode.label()).clicked() {
                                self.config.display_mode = mode;
                                self.config.save();
                            }
                        }
                    });
            });
            ui.label(
                egui::RichText::new(&self.launcher_display_hint)
                    .size(11.5)
                    .color(egui::Color32::from_rgb(95, 95, 95)),
            );
        });

        ui.add_space(6.0);
        group_box(ui, "Conta do co-op", |ui| {
            ui.label("Id do jogador (cada um dos dois usa um diferente):");
            if ui
                .add(egui::TextEdit::singleline(&mut self.config.account).desired_width(240.0))
                .lost_focus()
            {
                self.config.account = self.config.account.trim().to_string();
                self.config.save();
            }

            ui.label("Senha:");
            ui.horizontal(|ui| {
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut self.config.password)
                            .password(!self.show_password)
                            .desired_width(240.0),
                    )
                    .lost_focus()
                {
                    self.config.save();
                }
                ui.checkbox(&mut self.show_password, "Mostrar");
                if ui.button("Gerar nova").clicked() {
                    self.config.password = AppConfig::new_password();
                    self.show_password = true;
                    self.config.save();
                    self.append_log(
                        "Senha nova gerada (os dois jogadores podem ter senhas diferentes).",
                    );
                }
                if ui.button("Usar do bbport").clicked() {
                    if !self.config.take_from_bbport_launcher() {
                        show_info(
                            "Não achei uma conta no launcher do bbport.\n\nAbra o launcher do bbport, entre em \"Seamless co-op\" e faça o login por lá uma vez; depois volte e clique aqui.",
                        );
                    } else {
                        self.config.save();
                        let acc = self.config.account.clone();
                        self.append_log(&format!(
                            "Conta do launcher do bbport aplicada: {acc}."
                        ));
                        show_info(&format!("Conta do launcher do bbport aplicada: {acc}"));
                    }
                }
            });
            ui.label(
                egui::RichText::new(&self.launcher_account_hint)
                    .size(11.5)
                    .color(egui::Color32::from_rgb(95, 95, 95)),
            );
        });

        ui.add_space(6.0);
        group_box(ui, "Opções", |ui| {
            if ui
                .checkbox(
                    &mut self.config.keep_server,
                    "Deixar o servidor rodando quando eu fechar o BBRadmin",
                )
                .changed()
            {
                self.config.save();
            }
            if ui
                .checkbox(
                    &mut self.config.stop_server_with_game,
                    "Parar o servidor que eu iniciei quando o jogo fechar",
                )
                .changed()
            {
                self.config.save();
            }
            ui.checkbox(
                &mut self.auto_firewall,
                "Pedir as regras de firewall antes de hospedar",
            );
        });

        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(!self.busy, egui::Button::new("Instalar regras de firewall"))
                .clicked()
            {
                self.spawn_worker("Instalando as regras de firewall (UAC)...", |_| {
                    install_firewall_elevated()
                });
            }
            if ui.button("Restaurar overrides").clicked() && self.port.ready() {
                let confirm = ask_yes_no(&format!(
                    "Voltar o {} ao que estava antes?\n\n(o bbport volta a usar o servidor dele)",
                    coop_schema::OVERRIDES_FILE
                ));
                if confirm {
                    let msg = match overrides::restore(&self.port) {
                        Ok(m) | Err(m) => m,
                    };
                    self.append_log(&msg);
                    show_info(&msg);
                }
            }
            if ui.button("Abrir launcher oficial").clicked() {
                self.open_official_launcher(
                    "Launcher oficial aberto (para gráficos, FPS e mods do bbport).",
                );
            }
            if ui.button("Recarregar").clicked() {
                self.refresh_detection();
            }
        });

        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            if ui.button("Abrir pasta de logs").clicked() {
                let log_dir = Path::new(&self.port.root)
                    .join(coop_schema::COOP_USER_DIR)
                    .join("log");
                self.open_folder(&log_dir);
            }
            if ui.button("Abrir pasta do bbport").clicked() {
                let root = Path::new(&self.port.root).to_path_buf();
                self.open_folder(&root);
            }
            if ui.button("Ver o que um start faria").clicked() {
                self.config.save();
                dry_run::run(&self.config, false);
                let path = dry_run::file_path();
                if let Ok(content) = fs::read_to_string(&path) {
                    self.append_log(&format!("Relatório do dry-run:\n{content}"));
                }
                let _ = Command::new("cmd.exe")
                    .args(["/c", "start", "", &path.to_string_lossy()])
                    .creation_flags(CREATE_NO_WINDOW)
                    .spawn();
            }
        });
    }
}

fn group_box(ui: &mut egui::Ui, title: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::group(ui.style())
        .fill(egui::Color32::from_rgb(249, 249, 249))
        .corner_radius(4.0)
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                egui::RichText::new(title)
                    .strong()
                    .size(12.5)
                    .color(egui::Color32::from_rgb(35, 35, 35)),
            );
            ui.add_space(3.0);
            add_contents(ui);
        });
}

fn note_label(ui: &mut egui::Ui, text: &str) {
    ui.horizontal(|ui| {
        ui.add_space(20.0);
        ui.label(
            egui::RichText::new(text)
                .size(11.5)
                .color(egui::Color32::from_rgb(105, 105, 105)),
        );
    });
}

fn validate_ip(ip: &str) -> Result<(), &'static str> {
    if ip.is_empty() {
        return Err("sem endereço");
    }
    ip.parse::<Ipv4Addr>()
        .map(|_| ())
        .map_err(|_| "não é um endereço IPv4")
}

fn do_host(
    port: BbportInfo,
    config: AppConfig,
    server: Arc<ServerController>,
    ip: String,
    auto_firewall: bool,
    tx: Sender<WorkerMsg>,
) -> String {
    let mut say = String::new();

    let missing = firewall::missing();
    if !missing.is_empty() && auto_firewall {
        let prompt = format!(
            "As regras de firewall do BBRadmin ainda não estão instaladas ({}).\n\nSem elas o amigo pode não conseguir entrar. Instalar agora? (pede administrador)",
            missing.join(", ")
        );
        if ask_yes_no(&prompt) {
            say.push_str(&install_firewall_elevated());
            say.push('\n');
        }
    }
    if !firewall::missing().is_empty() {
        say.push_str(
            "Sem todas as regras de firewall: se o amigo não entrar, instale-as na aba Avançado.\n",
        );
    }

    if !server.running() {
        match server.start(&ip) {
            Ok(line) => say.push_str(&format!("{line}\n")),
            Err(err) => {
                say.push_str(&format!("{err}\nNão vou abrir o jogo sem servidor.\n"));
                return say;
            }
        }
        say.push_str(&format!("{}\n", server.wait_until_ready(&ip, 30)));
        if !server.backend_answers() {
            say.push_str("Não vou abrir o jogo: o backend 20443 não respondeu.\n");
            return say;
        }
    } else {
        say.push_str("O servidor já estava em execução.\n");
    }

    say.push_str(&format!("Endereço para o amigo: {ip}  (Radmin VPN)\n"));
    start_game(&port, &config, true, &ip, &mut say, tx);
    say
}

fn do_guest(port: BbportInfo, config: AppConfig, ip: String, tx: Sender<WorkerMsg>) -> String {
    if validate_ip(&ip).is_err() {
        return "Informe o IP do host (26.x.x.x), aquele que o amigo te passou.".to_string();
    }
    let mut say = format!("Servidor do amigo: {}\n", health::describe(&ip, 1500));
    if !health::reachable(&ip, 1500) {
        say.push_str("Aviso: o servidor do amigo não respondeu. Confira se o Radmin VPN está ligado nos dois e se o host já hospedou.\n");
    }
    start_game(&port, &config, false, &ip, &mut say, tx);
    say
}

fn start_game(
    port: &BbportInfo,
    config: &AppConfig,
    is_host: bool,
    ip: &str,
    say: &mut String,
    tx: Sender<WorkerMsg>,
) {
    match overrides::write(port, ip) {
        Ok(line) => say.push_str(&format!("{line}\n")),
        Err(err) => {
            say.push_str(&format!(
                "{err}\nParei: sem este arquivo o jogo procuraria o servidor da Sony.\n"
            ));
            return;
        }
    }

    if !config.play_with.is_empty() {
        let line = match input_config::apply(port, &config.play_with) {
            Ok(m) | Err(m) => m,
        };
        say.push_str(&format!("{line}\n"));
        let _ = tx.send(WorkerMsg::RefreshInputHint);
    }

    let running = bbport::running_game_pids();
    if !running.is_empty() {
        say.push_str("Fechando o jogo que já estava aberto...\n");
        bbport::close_game(&running);
    }

    let rules = if is_host {
        config.rules.clone()
    } else {
        HostRules::default()
    };
    let plan = game_environment::build(
        port,
        ip,
        &config.account,
        &config.password,
        &rules,
        config.display_mode,
    );

    match game_environment::start(port, &plan) {
        Ok(child) => {
            let pid = child.id();
            say.push_str(&format!("Modo de tela: {}.\n", plan.display.describe()));
            if is_host {
                say.push_str(&format!("Regras do host: {}.\n", rules.describe()));
            } else {
                say.push_str("Regras do co-op: as do líder (você entrou no time dele).\n");
            }
            say.push_str(&format!("Jogo aberto (pid {pid}).\n"));
            say.push_str("No jogo: abra o menu, entre no time e convide o amigo.\n");
            let _ = tx.send(WorkerMsg::GameStarted {
                child,
                display: plan.display,
            });
        }
        Err(err) => {
            say.push_str(&format!("Não abri o jogo: {err}\n"));
        }
    }
}

fn install_firewall_elevated() -> String {
    let Ok(exe) = std::env::current_exe() else {
        return format!(
            "Não localizei o executável do BBRadmin.\nVocê pode colar os comandos num console de administrador:\n{}",
            firewall::manual_commands()
        );
    };

    let verb: Vec<u16> = OsStr::new("runas").encode_wide().chain(Some(0)).collect();
    let file: Vec<u16> = exe.as_os_str().encode_wide().chain(Some(0)).collect();
    let params: Vec<u16> = OsStr::new("--install-firewall")
        .encode_wide()
        .chain(Some(0))
        .collect();

    let mut sei: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    sei.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
    sei.fMask = SEE_MASK_NOCLOSEPROCESS;
    sei.lpVerb = verb.as_ptr();
    sei.lpFile = file.as_ptr();
    sei.lpParameters = params.as_ptr();
    sei.nShow = SW_HIDE;

    let ok = unsafe { ShellExecuteExW(&mut sei) };
    if ok == 0 {
        return format!(
            "Não elevei o BBRadmin (UAC cancelado ou bloqueado).\nVocê pode colar os comandos num console de administrador:\n{}",
            firewall::manual_commands()
        );
    }

    if !sei.hProcess.is_null() {
        unsafe {
            WaitForSingleObject(sei.hProcess, INFINITE);
            CloseHandle(sei.hProcess);
        }
    }

    let report = fs::read_to_string(app_config::firewall_report_path()).unwrap_or_default();
    let still = firewall::missing();
    if still.is_empty() {
        format!("Firewall pronto: as três regras estão instaladas.\n{report}")
    } else {
        format!("Ainda faltam regras: {}\n{report}", still.join(", "))
    }
}

fn show_info(text: &str) {
    rfd::MessageDialog::new()
        .set_title("BBRadmin")
        .set_description(text)
        .set_level(rfd::MessageLevel::Info)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
}

fn ask_yes_no(text: &str) -> bool {
    rfd::MessageDialog::new()
        .set_title("BBRadmin")
        .set_description(text)
        .set_level(rfd::MessageLevel::Info)
        .set_buttons(rfd::MessageButtons::YesNo)
        .show()
        == rfd::MessageDialogResult::Yes
}
