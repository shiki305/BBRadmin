//! BBRadmin graphical interface (eframe / egui), with:
//! - Four tabs (Jogar/Play, Co-op, Controles/Controls, Avançado/Advanced)
//! - Windows-synced Dark/Light theme (quick toggle in bottom bar)
//! - Full Português (BR) and English (US) localization (auto-detects Windows UI language)
//! - Visual server connection test badge (green checkmark / red X circle)
//! - Background server poller, WindowPlacer for fullscreen/borderless, and GitHub Releases auto-updater

use std::ffi::OsStr;
use std::fs;
use std::net::Ipv4Addr;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
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
use crate::core::display::{ResolvedDisplay, WindowPlacer};
use crate::core::dry_run;
use crate::core::firewall;
use crate::core::game_environment;
use crate::core::health;
use crate::core::host_rules::HostRules;
use crate::core::i18n::{t, Lang, LangMode};
use crate::core::input_config;
use crate::core::log;
use crate::core::overrides;
use crate::core::radmin::{self, NetworkChoice};
use crate::core::server_controller::{ServerController, ServerState};
use crate::core::updater;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

const ENEMIES_VALUES: &[&str] = &["off", "auto", "bit", "on", "much", "custom"];
const ENEMIES_LABELS_PT: &[&str] = &[
    "Normal — como o jogo: vida ×1 · dano ×1 · equilíbrio ×1   (padrão)",
    "Automático — sobe com cada caçador no mesmo mapa (até \"Mais fortes\" com o time cheio)",
    "Um pouco mais fortes — vida ×1,5 · dano ×1,25 · equilíbrio ×1,5",
    "Mais fortes — vida ×3 · dano ×1,5 · equilíbrio ×3",
    "Muito mais fortes — vida ×5 · dano ×2 · equilíbrio ×5",
    "Personalizado — os meus números (1 a 10; 1 = o do jogo):",
];
const ENEMIES_LABELS_EN: &[&str] = &[
    "Normal — vanilla game: HP ×1 · damage ×1 · poise ×1   (default)",
    "Automatic — scales with each hunter on the same map (up to \"Stronger\" with a full party)",
    "A bit stronger — HP ×1.5 · damage ×1.25 · poise ×1.5",
    "Stronger — HP ×3 · damage ×1.5 · poise ×3",
    "Much stronger — HP ×5 · damage ×2 · poise ×5",
    "Custom — your own multipliers (1 to 10; 1 = vanilla):",
];

const TEMPO_VALUES: &[&str] = &["", "aggressive", "relentless"];
const TEMPO_LABELS_PT: &[&str] = &[
    "Normal — como o jogo (padrão)",
    "Agressivo — metade das pausas, giros ×1,3",
    "Implacável — um quarto das pausas, giros ×1,6",
];
const TEMPO_LABELS_EN: &[&str] = &[
    "Normal — vanilla game (default)",
    "Aggressive — half the pauses, turn speed ×1.3",
    "Relentless — quarter the pauses, turn speed ×1.6",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Play,
    Coop,
    Controls,
    Advanced,
}

#[derive(Debug, Clone)]
enum TestResult {
    Testing,
    Ok(String),
    Fail(String),
}

enum WorkerMsg {
    Done(String),
    GameStarted {
        child: Child,
        display: ResolvedDisplay,
    },
    TestFinished {
        ok: bool,
        detail: String,
    },
    RefreshInputHint,
    UpdateStatus(Option<String>),
    UpdateLog(String),
    UpdateReadyToRestart {
        exe: PathBuf,
        version: String,
    },
}

#[derive(Debug, Clone, Copy)]
struct ServerSnapshot {
    running: bool,
    state: ServerState,
}

#[derive(Debug, Clone, Copy)]
struct Palette {
    window_bg: egui::Color32,
    header_bg: egui::Color32,
    card_bg: egui::Color32,
    bar_bg: egui::Color32,
    log_bg: egui::Color32,
    border: egui::Color32,
    title_text: egui::Color32,
    section_text: egui::Color32,
    muted_text: egui::Color32,
    play_btn_bg: egui::Color32,
    play_btn_text: egui::Color32,
}

impl Palette {
    fn of(dark: bool) -> Self {
        if dark {
            Self {
                window_bg: egui::Color32::from_rgb(20, 21, 25),
                header_bg: egui::Color32::from_rgb(27, 29, 35),
                card_bg: egui::Color32::from_rgb(29, 31, 38),
                bar_bg: egui::Color32::from_rgb(24, 26, 31),
                log_bg: egui::Color32::from_rgb(15, 16, 20),
                border: egui::Color32::from_rgb(52, 56, 66),
                title_text: egui::Color32::from_rgb(240, 242, 247),
                section_text: egui::Color32::from_rgb(225, 228, 236),
                muted_text: egui::Color32::from_rgb(158, 165, 180),
                play_btn_bg: egui::Color32::from_rgb(138, 32, 44),
                play_btn_text: egui::Color32::from_rgb(252, 248, 248),
            }
        } else {
            Self {
                window_bg: egui::Color32::from_rgb(243, 243, 243),
                header_bg: egui::Color32::WHITE,
                card_bg: egui::Color32::from_rgb(249, 249, 249),
                bar_bg: egui::Color32::from_rgb(236, 236, 236),
                log_bg: egui::Color32::from_rgb(250, 250, 250),
                border: egui::Color32::from_rgb(215, 215, 215),
                title_text: egui::Color32::from_rgb(24, 24, 24),
                section_text: egui::Color32::from_rgb(35, 35, 35),
                muted_text: egui::Color32::from_rgb(95, 95, 95),
                play_btn_bg: egui::Color32::from_rgb(148, 36, 48),
                play_btn_text: egui::Color32::WHITE,
            }
        }
    }
}

pub fn run(config: AppConfig) -> eframe::Result<()> {
    let initial_theme = config.theme;
    let initial_lang = config.lang.resolve();
    let title = t(
        initial_lang,
        "BBRadmin · co-op em Radmin VPN",
        "BBRadmin · Radmin VPN co-op",
    );
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(title)
            .with_inner_size([690.0, 690.0])
            .with_min_inner_size([640.0, 520.0]),
        ..Default::default()
    };

    eframe::run_native(
        "BBRadmin",
        options,
        Box::new(move |cc| {
            install_themes(&cc.egui_ctx);
            apply_theme(&cc.egui_ctx, initial_theme);
            Ok(Box::new(BbradminApp::new(config, cc.egui_ctx.clone())))
        }),
    )
}

fn build_visuals(dark: bool) -> egui::Visuals {
    let p = Palette::of(dark);
    let mut visuals = if dark {
        let mut v = egui::Visuals::dark();
        v.extreme_bg_color = egui::Color32::from_rgb(15, 16, 20);
        v.faint_bg_color = egui::Color32::from_rgb(36, 39, 48);
        v.widgets.noninteractive.bg_fill = p.card_bg;
        v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, p.border);
        v.widgets.inactive.bg_fill = egui::Color32::from_rgb(42, 45, 55);
        v.widgets.hovered.bg_fill = egui::Color32::from_rgb(56, 60, 74);
        v.widgets.active.bg_fill = egui::Color32::from_rgb(70, 75, 92);
        v.selection.bg_fill = egui::Color32::from_rgb(138, 32, 44);
        v.selection.stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(210, 90, 102));
        v
    } else {
        let mut v = egui::Visuals::light();
        v.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(248, 248, 248);
        v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, p.border);
        v
    };
    visuals.window_fill = p.window_bg;
    visuals.panel_fill = p.window_bg;
    visuals
}

fn install_themes(ctx: &egui::Context) {
    ctx.set_visuals_of(egui::Theme::Dark, build_visuals(true));
    ctx.set_visuals_of(egui::Theme::Light, build_visuals(false));
}

fn apply_theme(ctx: &egui::Context, mode: app_config::ThemeMode) {
    let pref = match mode {
        app_config::ThemeMode::System => egui::ThemePreference::System,
        app_config::ThemeMode::Dark => egui::ThemePreference::Dark,
        app_config::ThemeMode::Light => egui::ThemePreference::Light,
    };
    ctx.set_theme(pref);
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
    launcher_settings: bbport_launcher::LauncherSettings,
    test_result: Option<TestResult>,
    update_status: Option<String>,
    restarting_for_update: bool,
}

impl BbradminApp {
    fn new(config: AppConfig, ctx: egui::Context) -> Self {
        let (tx, rx) = mpsc::channel();
        let server = Arc::new(ServerController::new());
        let server_snapshot = Arc::new(Mutex::new(ServerSnapshot {
            running: false,
            state: ServerState::Stopped,
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
                    let state = srv.state_code();
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
            launcher_settings: bbport_launcher::LauncherSettings::default(),
            test_result: None,
            update_status: None,
            restarting_for_update: false,
        };
        app.refresh_detection();
        log::line(&format!(
            "BBRadmin v{} (Rust) aberto.",
            updater::CURRENT_VERSION
        ));
        if app.config.auto_update {
            app.start_auto_update_check();
        }
        app
    }

    fn lang(&self) -> Lang {
        self.config.lang.resolve()
    }

    fn start_auto_update_check(&self) {
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        let lang = self.lang();
        thread::spawn(move || match updater::check_latest() {
            Ok(Some(info)) => {
                let msg = format!(
                    "{} v{}...",
                    t(
                        lang,
                        "Nova versão encontrada; atualizando para",
                        "New version found; updating to"
                    ),
                    info.version
                );
                log::line(&msg);
                let _ = tx.send(WorkerMsg::UpdateStatus(Some(format!(
                    "{} v{}...",
                    t(lang, "Atualizando para", "Updating to"),
                    info.version
                ))));
                let _ = tx.send(WorkerMsg::UpdateLog(msg));
                ctx.request_repaint();

                match updater::download_and_apply(&info) {
                    Ok(exe) => {
                        let _ = tx.send(WorkerMsg::UpdateReadyToRestart {
                            exe,
                            version: info.version,
                        });
                    }
                    Err(err) => {
                        log::line(&format!("Atualização automática falhou: {err}"));
                        let _ = tx.send(WorkerMsg::UpdateStatus(None));
                        let _ = tx.send(WorkerMsg::UpdateLog(format!(
                            "{} v{}: {err} ({})",
                            t(
                                lang,
                                "Não foi possível atualizar automaticamente para",
                                "Could not auto-update to"
                            ),
                            info.version,
                            info.html_url
                        )));
                    }
                }
                ctx.request_repaint();
            }
            Ok(None) => {}
            Err(err) => {
                log::line(&format!("Verificação de atualização ignorada: {err}"));
            }
        });
    }

    fn on_check_updates_manual(&mut self) {
        let lang = self.lang();
        let what = t(
            lang,
            "Verificando atualizações no GitHub...",
            "Checking GitHub for updates...",
        );
        self.spawn_worker(what, move |tx| match updater::check_latest() {
            Ok(None) => {
                let msg = format!(
                    "{} (v{}).",
                    t(
                        lang,
                        "Você já está na versão mais recente",
                        "You are already on the latest version"
                    ),
                    updater::CURRENT_VERSION
                );
                show_info(&msg);
                msg
            }
            Ok(Some(info)) => {
                let prompt = match lang {
                    Lang::PtBr => format!(
                        "Nova versão v{} disponível (você está na v{}).\n\nDeseja baixar, atualizar e reiniciar o BBRadmin agora?",
                        info.version, updater::CURRENT_VERSION
                    ),
                    Lang::EnUs => format!(
                        "New version v{} is available (you are on v{}).\n\nDownload, update, and restart BBRadmin now?",
                        info.version, updater::CURRENT_VERSION
                    ),
                };
                if !ask_yes_no(&prompt) {
                    return format!(
                        "{} v{}.",
                        t(
                            lang,
                            "Atualização adiada pelo usuário:",
                            "Update postponed by user:"
                        ),
                        info.version
                    );
                }
                let _ = tx.send(WorkerMsg::UpdateStatus(Some(format!(
                    "{} v{}...",
                    t(lang, "Baixando", "Downloading"),
                    info.version
                ))));
                match updater::download_and_apply(&info) {
                    Ok(exe) => {
                        let ver = info.version.clone();
                        let _ = tx.send(WorkerMsg::UpdateReadyToRestart {
                            exe,
                            version: ver.clone(),
                        });
                        format!(
                            "{} v{}...",
                            t(lang, "Atualização instalada; reiniciando para", "Update installed; restarting into"),
                            ver
                        )
                    }
                    Err(err) => {
                        let _ = tx.send(WorkerMsg::UpdateStatus(None));
                        let msg = format!(
                            "{} v{}: {err}",
                            t(lang, "Falha ao atualizar para", "Failed to update to"),
                            info.version
                        );
                        show_info(&msg);
                        msg
                    }
                }
            }
            Err(err) => {
                let msg = format!(
                    "{}: {err}",
                    t(
                        lang,
                        "Não foi possível verificar atualizações",
                        "Could not check for updates"
                    )
                );
                show_info(&msg);
                msg
            }
        });
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

        self.launcher_settings = bbport_launcher::load();

        if self.config.play_with.is_empty() {
            let current = input_config::current(&self.port);
            if !current.is_empty() {
                self.config.play_with = current;
                self.config.save();
            }
        }
    }

    fn input_hint(&self, lang: Lang) -> String {
        let file = if self.port.ready() {
            input_config::path_in(&self.port).to_string_lossy().to_string()
        } else {
            String::new()
        };
        let desc = input_config::describe_for(&self.port, lang);
        if file.is_empty() {
            format!("{}: {desc}.", t(lang, "Agora", "Current"))
        } else {
            format!(
                "{}: {desc}.\n{}: {file}",
                t(lang, "Agora", "Current"),
                t(lang, "Arquivo", "File")
            )
        }
    }

    fn poll_background(&mut self) {
        let lang = self.lang();
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                WorkerMsg::Done(text) => {
                    self.busy = false;
                    self.append_log(&text);
                }
                WorkerMsg::GameStarted { child, display } => {
                    self.placer = Some(WindowPlacer::start(display));
                    self.game = Some(child);
                }
                WorkerMsg::TestFinished { ok, detail } => {
                    self.test_result = Some(if ok {
                        TestResult::Ok(detail)
                    } else {
                        TestResult::Fail(detail)
                    });
                }
                WorkerMsg::RefreshInputHint => {}
                WorkerMsg::UpdateStatus(status) => {
                    self.update_status = status;
                }
                WorkerMsg::UpdateLog(text) => {
                    self.append_log(&text);
                }
                WorkerMsg::UpdateReadyToRestart { exe, version } => {
                    self.update_status = None;
                    if self.game.is_none() && !self.server.started_by_us() {
                        match updater::relaunch(&exe) {
                            Ok(()) => {
                                self.restarting_for_update = true;
                                self.ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                            Err(err) => {
                                self.append_log(&format!(
                                    "{} v{version} ({err}).",
                                    t(
                                        lang,
                                        "Atualização instalada, mas não consegui reabrir automaticamente:",
                                        "Update installed, but could not relaunch automatically:"
                                    )
                                ));
                            }
                        }
                    } else {
                        self.append_log(&format!(
                            "{} v{version}.",
                            t(
                                lang,
                                "Atualização instalada — será aplicada ao reabrir o BBRadmin:",
                                "Update installed — will take effect next time you open BBRadmin:"
                            )
                        ));
                    }
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
            self.placer = None;
            self.append_log(t(lang, "O jogo fechou.", "Game closed."));
            log::line("Jogo fechou.");
            if self.is_host() && self.server.started_by_us() && self.config.auto_stop_server {
                self.append_log(t(
                    lang,
                    "Parando o servidor que eu iniciei (parada automática ativada).",
                    "Stopping the server I started (auto-stop enabled).",
                ));
                let srv = Arc::clone(&self.server);
                self.spawn_worker(
                    t(lang, "Parando o servidor...", "Stopping server..."),
                    move |_| srv.stop(),
                );
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
        let lang = self.lang();
        self.config.save();
        if !self.port.ready() {
            show_info(t(
                lang,
                "O bbport ainda não está pronto.\n\nAbra a aba Avançado, escolha a pasta do bbport (a que tem Bloodborne.exe) e clique em Verificar.",
                "bbport is not ready yet.\n\nOpen the Advanced tab, choose your bbport folder (the one with Bloodborne.exe), and click Verify.",
            ));
            return;
        }

        if self.is_host() {
            let ip = self.own_address.trim().to_string();
            if let Err(problem) = validate_ip(&ip, lang) {
                show_info(&format!(
                    "{} ({problem}).\n\n{}",
                    t(
                        lang,
                        "Este PC não tem um endereço de VPN agora",
                        "This PC does not have a VPN address right now"
                    ),
                    t(
                        lang,
                        "Ligue o Radmin VPN e clique em Recarregar na aba Avançado.",
                        "Turn on Radmin VPN and click Reload on the Advanced tab."
                    )
                ));
                return;
            }
            let port = self.port.clone();
            let config = self.config.clone();
            let server = Arc::clone(&self.server);
            let auto_fw = self.auto_firewall;
            self.spawn_worker(
                t(lang, "Hospedando o co-op...", "Hosting co-op..."),
                move |tx| do_host(port, config, server, ip, auto_fw, lang, tx),
            );
        } else {
            let ip = self.config.remote_ip.trim().to_string();
            let port = self.port.clone();
            let config = self.config.clone();
            self.spawn_worker(
                t(lang, "Entrando no co-op...", "Joining co-op..."),
                move |tx| do_guest(port, config, ip, lang, tx),
            );
        }
    }

    fn on_start_server_only(&mut self) {
        let lang = self.lang();
        self.config.save();
        if !self.port.ready() {
            return;
        }
        let ip = self.own_address.trim().to_string();
        if validate_ip(&ip, lang).is_err() {
            show_info(t(
                lang,
                "Este PC não tem um endereço de VPN agora. Ligue o Radmin VPN.",
                "This PC does not have a VPN address right now. Turn on Radmin VPN.",
            ));
            return;
        }
        let server = Arc::clone(&self.server);
        self.spawn_worker(
            t(lang, "Iniciando o servidor...", "Starting server..."),
            move |_| match server.start(&ip) {
                Ok(line) => format!("{line}\n{}", server.wait_until_ready(&ip, 30)),
                Err(err) => err,
            },
        );
    }

    fn on_stop_server(&mut self) {
        let lang = self.lang();
        let server = Arc::clone(&self.server);
        self.spawn_worker(
            t(lang, "Parando o servidor...", "Stopping server..."),
            move |_| server.stop(),
        );
    }

    fn on_test_server(&mut self) {
        let lang = self.lang();
        let ip = self.config.remote_ip.trim().to_string();
        if validate_ip(&ip, lang).is_err() {
            show_info(t(
                lang,
                "Informe o IP do host antes de testar.",
                "Enter the host IP before testing.",
            ));
            return;
        }
        self.test_result = Some(TestResult::Testing);
        self.spawn_worker(
            t(lang, "Testando o servidor...", "Testing server..."),
            move |tx| {
                let ok = health::reachable(&ip, 2000);
                let detail = health::describe_for(&ip, 2000, lang);
                let _ = tx.send(WorkerMsg::TestFinished {
                    ok,
                    detail: detail.clone(),
                });
                format!(
                    "{} {ip}: {detail}",
                    t(lang, "Servidor em", "Server at")
                )
            },
        );
    }

    fn on_browse_folder(&mut self) {
        let lang = self.lang();
        let mut dialog = rfd::FileDialog::new().set_title(t(
            lang,
            "Escolha a pasta do bbport (a que tem Bloodborne.exe)",
            "Choose the bbport folder (the one with Bloodborne.exe)",
        ));
        if !self.port.root.is_empty() && Path::new(&self.port.root).is_dir() {
            dialog = dialog.set_directory(&self.port.root);
        }
        let Some(picked) = dialog.pick_folder() else {
            return;
        };
        let chosen = bbport::inspect(&picked.to_string_lossy());
        if !chosen.ready() {
            show_info(&format!(
                "{}\n\n{}\n\n{}",
                t(
                    lang,
                    "Nessa pasta ainda falta:",
                    "This folder is still missing:"
                ),
                chosen.missing.join("\n"),
                t(
                    lang,
                    "Escolha a pasta onde está o Bloodborne.exe do bbport.",
                    "Choose the folder containing bbport's Bloodborne.exe."
                )
            ));
            return;
        }
        self.config.bbport_dir = chosen.root.clone();
        self.config.save();
        self.refresh_detection();
        self.append_log(&format!(
            "{}: {} ({})",
            t(lang, "Pasta do bbport", "bbport folder"),
            chosen.root,
            chosen.describe_for(lang)
        ));
    }

    fn open_official_launcher(&mut self, said: &str) {
        let lang = self.lang();
        if !self.port.ready() {
            show_info(t(
                lang,
                "O bbport ainda não está pronto. Escolha a pasta dele na aba Avançado.",
                "bbport is not ready yet. Choose its folder on the Advanced tab.",
            ));
            return;
        }
        let exe = Path::new(&self.port.root).join(coop_schema::GAME_EXE);
        match Command::new(&exe).current_dir(&self.port.root).spawn() {
            Ok(_) => self.append_log(said),
            Err(err) => show_info(&format!(
                "{}: {err}",
                t(
                    lang,
                    "Não abri o launcher do bbport",
                    "Could not open bbport launcher"
                )
            )),
        }
    }

    fn open_folder(&mut self, path: &Path) {
        let lang = self.lang();
        if path.as_os_str().is_empty() {
            return;
        }
        let _ = fs::create_dir_all(path);
        if let Err(err) = Command::new("explorer.exe").arg(path).spawn() {
            self.append_log(&format!(
                "{} ({err}).",
                t(lang, "Não abri a pasta", "Could not open folder")
            ));
        }
    }
}

impl eframe::App for BbradminApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_background();

        if self.busy || self.game.is_some() {
            ctx.request_repaint_after(Duration::from_millis(500));
        }

        let lang = self.lang();
        let snap = self
            .server_snapshot
            .lock()
            .map(|g| *g)
            .unwrap_or(ServerSnapshot {
                running: false,
                state: ServerState::Stopped,
            });

        let p = Palette::of(ctx.style().visuals.dark_mode);

        // ---- Header ----
        egui::TopBottomPanel::top("header")
            .frame(
                egui::Frame::NONE
                    .fill(p.header_bg)
                    .stroke(egui::Stroke::new(1.0_f32, p.border))
                    .inner_margin(egui::Margin::symmetric(14, 10)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("BBRadmin")
                                    .size(19.0)
                                    .strong()
                                    .color(p.title_text),
                            );
                            ui.label(
                                egui::RichText::new(format!("v{}", updater::CURRENT_VERSION))
                                    .size(11.5)
                                    .color(p.muted_text),
                            );
                        });
                        ui.label(
                            egui::RichText::new(t(
                                lang,
                                "co-op contínuo para Bloodborne, por Radmin VPN",
                                "seamless Bloodborne co-op over Radmin VPN",
                            ))
                            .size(12.0)
                            .color(p.muted_text),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(self.port.describe_for(lang))
                                .size(11.5)
                                .color(p.muted_text),
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
                        .fill(p.log_bg)
                        .stroke(egui::Stroke::new(1.0_f32, p.border))
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
                    .fill(p.bar_bg)
                    .stroke(egui::Stroke::new(1.0_f32, p.border))
                    .inner_margin(egui::Margin::symmetric(14, 7)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let host = self.is_host();
                    let mut state = format!(
                        "{}: {}",
                        t(lang, "Servidor", "Server"),
                        if host {
                            snap.state.label(lang)
                        } else {
                            t(lang, "no PC do amigo", "on friend's PC")
                        }
                    );
                    if host
                        && self.port.ready()
                        && !radmin::is_vpn(radmin::kind_of_ip_only(&self.own_address))
                        && !self.networks.is_empty()
                    {
                        state.push_str(t(
                            lang,
                            "  ·  ligue o Radmin VPN",
                            "  ·  turn on Radmin VPN",
                        ));
                    }
                    if self.busy {
                        state = format!(
                            "{}  ·  {state}",
                            t(lang, "Trabalhando...", "Working...")
                        );
                    }
                    if let Some(upd) = &self.update_status {
                        state = format!("{upd}  ·  {state}");
                    }
                    ui.label(egui::RichText::new(state).size(12.0));

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let btn_text = if self.show_log {
                            t(lang, "Esconder log", "Hide log")
                        } else {
                            t(lang, "Mostrar log", "Show log")
                        };
                        if ui.button(btn_text).clicked() {
                            self.show_log = !self.show_log;
                            if self.show_log {
                                let combined =
                                    format!("{}{}", log::tail(60), self.server.recent(40));
                                self.append_log(&combined);
                            }
                        }

                        if ui.button(self.config.theme.short_label_for(lang)).clicked() {
                            self.config.theme = self.config.theme.next();
                            apply_theme(ctx, self.config.theme);
                            self.config.save();
                        }

                        if ui
                            .button(t(lang, "Abrir launcher oficial", "Official launcher"))
                            .clicked()
                        {
                            self.open_official_launcher(t(
                                lang,
                                "Launcher oficial aberto (para gráficos, FPS e mods do bbport).",
                                "Official launcher opened (for bbport graphics, FPS, and mods).",
                            ));
                        }
                    });
                });
            });

        // ---- Tabs + Main Content ----
        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(p.window_bg)
                    .inner_margin(egui::Margin::symmetric(12, 8)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.tab, Tab::Play, t(lang, "  Jogar  ", "  Play  "));
                    ui.selectable_value(&mut self.tab, Tab::Coop, "  Co-op  ");
                    ui.selectable_value(
                        &mut self.tab,
                        Tab::Controls,
                        t(lang, "  Controles  ", "  Controls  "),
                    );
                    ui.selectable_value(
                        &mut self.tab,
                        Tab::Advanced,
                        t(lang, "  Avançado  ", "  Advanced  "),
                    );
                });
                ui.separator();

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| match self.tab {
                        Tab::Play => self.render_play_tab(ui, &snap, lang),
                        Tab::Coop => self.render_coop_tab(ui, lang),
                        Tab::Controls => self.render_controls_tab(ui, lang),
                        Tab::Advanced => self.render_advanced_tab(ui, lang),
                    });
            });
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.poller_stop.store(true, Ordering::Relaxed);
        self.placer = None;
        if !self.restarting_for_update
            && self.is_host()
            && self.server.started_by_us()
            && self.config.auto_stop_server
        {
            log::line("Fechando o BBRadmin: parando o servidor que eu iniciei.");
            self.server.stop();
        }
        log::line("BBRadmin fechado.");
    }
}

impl BbradminApp {
    fn render_play_tab(&mut self, ui: &mut egui::Ui, snap: &ServerSnapshot, lang: Lang) {
        let mut host = self.is_host();
        group_box(ui, t(lang, "Quem hospeda?", "Who is hosting?"), |ui| {
            if ui
                .radio(
                    host,
                    t(
                        lang,
                        "Eu hospedo (o servidor roda neste PC)",
                        "I'm hosting (the server runs on this PC)",
                    ),
                )
                .clicked()
            {
                self.config.role = app_config::ROLE_HOST.to_string();
                self.config.save();
                host = true;
            }
            if ui
                .radio(
                    !host,
                    t(
                        lang,
                        "Eu entro (o servidor está no PC do amigo)",
                        "I'm joining (the server is on my friend's PC)",
                    ),
                )
                .clicked()
            {
                self.config.role = app_config::ROLE_GUEST.to_string();
                self.config.save();
                host = false;
            }
        });

        ui.add_space(6.0);
        group_box(ui, t(lang, "Endereço", "Address"), |ui| {
            if host {
                ui.label(t(
                    lang,
                    "Este PC (dê este endereço ao amigo):",
                    "This PC (share this address with your friend):",
                ));
                ui.horizontal(|ui| {
                    let mut addr_display = self.own_address.clone();
                    ui.add(
                        egui::TextEdit::singleline(&mut addr_display)
                            .desired_width(300.0)
                            .interactive(false),
                    );
                    if ui.button(t(lang, "Copiar", "Copy")).clicked()
                        && !self.own_address.is_empty()
                    {
                        ui.ctx().copy_text(self.own_address.clone());
                        let ip = self.own_address.clone();
                        self.append_log(&format!(
                            "{}: {ip}",
                            t(lang, "Endereço copiado", "Address copied")
                        ));
                    }
                });
            } else {
                ui.label(t(
                    lang,
                    "IP do host (o amigo te passa, 26.x.x.x):",
                    "Host IP (from your friend, 26.x.x.x):",
                ));
                ui.horizontal(|ui| {
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut self.config.remote_ip)
                            .desired_width(300.0)
                            .hint_text("26.x.x.x"),
                    );
                    if resp.changed() {
                        self.test_result = None;
                    }
                    if resp.lost_focus() {
                        self.config.save();
                    }
                    let valid = validate_ip(self.config.remote_ip.trim(), lang).is_ok();
                    if ui
                        .add_enabled(
                            !self.busy && valid,
                            egui::Button::new(t(lang, "Testar", "Test")),
                        )
                        .clicked()
                    {
                        self.on_test_server();
                    }
                    match &self.test_result {
                        Some(TestResult::Testing) => {
                            ui.add_space(4.0);
                            ui.spinner();
                        }
                        Some(TestResult::Ok(detail)) => {
                            ui.add_space(4.0);
                            draw_status_badge(ui, true, detail);
                        }
                        Some(TestResult::Fail(detail)) => {
                            ui.add_space(4.0);
                            draw_status_badge(ui, false, detail);
                        }
                        None => {}
                    }
                });
            }
        });

        ui.add_space(6.0);
        group_box(ui, t(lang, "Começar", "Start"), |ui| {
            ui.horizontal(|ui| {
                let p = Palette::of(ui.visuals().dark_mode);
                let play_btn = egui::Button::new(
                    egui::RichText::new(t(lang, "Jogar co-op", "Play co-op"))
                        .size(16.0)
                        .strong()
                        .color(p.play_btn_text),
                )
                .fill(p.play_btn_bg)
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
                            egui::Button::new(t(
                                lang,
                                "Só iniciar o servidor",
                                "Start server only",
                            ))
                            .min_size(egui::vec2(165.0, 24.0)),
                        )
                        .clicked()
                    {
                        self.on_start_server_only();
                    }
                    if ui
                        .add_enabled(
                            !self.busy && snap.running,
                            egui::Button::new(t(lang, "Parar o servidor", "Stop server"))
                                .min_size(egui::vec2(165.0, 24.0)),
                        )
                        .clicked()
                    {
                        self.on_stop_server();
                    }
                });
            });
        });

        ui.add_space(6.0);
        group_box(ui, t(lang, "Como funciona", "How it works"), |ui| {
            ui.label(t(
                lang,
                "1.  Os dois ligam o Radmin VPN e entram na mesma rede (aparece um IP 26.x.x.x).\n\
                 2.  Um clica em \"Jogar co-op\" com \"Eu hospedo\" e passa o endereço ao outro (botão Copiar).\n\
                 3.  O outro escolhe \"Eu entro\", cola o endereço em \"IP do host\" e clica em \"Jogar co-op\".\n\
                 4.  No jogo, o que hospeda abre o menu, cria o time e convida; o amigo aceita o convite.\n\
                 5.  Para sair, feche o jogo.\n\n\
                 As regras do co-op ficam na aba \"Co-op\"; os controles, na aba \"Controles\".",
                "1.  Both players turn on Radmin VPN and join the same network (each gets a 26.x.x.x IP).\n\
                 2.  The host selects \"I'm hosting\", clicks \"Play co-op\", and sends their IP (Copy button).\n\
                 3.  The guest selects \"I'm joining\", pastes the IP into \"Host IP\", and clicks \"Play co-op\".\n\
                 4.  In-game, the host opens the co-op menu, creates a party, and invites; the friend accepts.\n\
                 5.  To finish, just close the game.\n\n\
                 Co-op host rules are on the \"Co-op\" tab; controls are on the \"Controls\" tab.",
            ));
        });
    }

    fn render_coop_tab(&mut self, ui: &mut egui::Ui, lang: Lang) {
        let p = Palette::of(ui.visuals().dark_mode);
        ui.label(
            egui::RichText::new(t(
                lang,
                "As regras do líder do time: valem para o time inteiro e aparecem na tela do time \
                 dentro do jogo. Só Joining / Invading / Death mode se mudam ali. Mudanças contam \
                 do próximo start (marque antes de \"Jogar co-op\").",
                "Party leader rules: apply to the whole party and appear on the in-game party screen. \
                 Only Joining / Invading / Death mode can be changed in-game. Changes take effect on \
                 the next launch (set them before clicking \"Play co-op\").",
            ))
            .size(12.0)
            .color(p.muted_text),
        );
        ui.add_space(6.0);

        let mut changed = false;
        let enemies_labels = match lang {
            Lang::PtBr => ENEMIES_LABELS_PT,
            Lang::EnUs => ENEMIES_LABELS_EN,
        };
        let tempo_labels = match lang {
            Lang::PtBr => TEMPO_LABELS_PT,
            Lang::EnUs => TEMPO_LABELS_EN,
        };

        group_box(
            ui,
            t(lang, "Inimigos mais fortes", "Stronger enemies"),
            |ui| {
                for (val, label) in ENEMIES_VALUES.iter().zip(enemies_labels.iter()) {
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
                        ui.label(t(lang, "Vida", "HP"));
                        if ui
                            .add(
                                egui::TextEdit::singleline(&mut self.config.rules.custom_hp)
                                    .desired_width(48.0),
                            )
                            .changed()
                        {
                            changed = true;
                        }
                        ui.label(t(lang, "Dano", "Damage"));
                        if ui
                            .add(
                                egui::TextEdit::singleline(&mut self.config.rules.custom_damage)
                                    .desired_width(48.0),
                            )
                            .changed()
                        {
                            changed = true;
                        }
                        ui.label(t(lang, "Equilíbrio", "Poise"));
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
            },
        );

        ui.add_space(6.0);
        group_box(ui, t(lang, "Ritmo do chefe", "Boss tempo"), |ui| {
            ui.label(t(
                lang,
                "Nos dois de baixo o chefe descansa menos e vira mais rápido (os golpes e o dano iguais).",
                "In the lower two options, bosses rest less and turn faster (moves and damage stay the same).",
            ));
            let current_idx = TEMPO_VALUES
                .iter()
                .position(|v| self.config.rules.tempo.eq_ignore_ascii_case(v))
                .unwrap_or(0);
            egui::ComboBox::from_id_salt("tempo_combo")
                .width(340.0)
                .selected_text(tempo_labels[current_idx])
                .show_ui(ui, |ui| {
                    for (idx, label) in tempo_labels.iter().enumerate() {
                        if ui.selectable_label(idx == current_idx, *label).clicked() {
                            self.config.rules.tempo = TEMPO_VALUES[idx].to_string();
                            changed = true;
                        }
                    }
                });
        });

        ui.add_space(6.0);
        group_box(ui, t(lang, "Outras regras", "Other rules"), |ui| {
            if ui
                .checkbox(
                    &mut self.config.rules.rot,
                    t(
                        lang,
                        "Podridão (rot) — cada morte de verdade dá uma penalidade (até 5)",
                        "Rot — each true death adds a penalty stack (up to 5)",
                    ),
                )
                .changed()
            {
                changed = true;
            }
            note_label(
                ui,
                t(
                    lang,
                    "Uma lâmpada, um nível, um chefe, seu assassino ou sua mancha de sangue tiram.",
                    "Cleared by a lamp, leveling up, defeating a boss, killing your killer, or recovering your bloodstain.",
                ),
            );

            if ui
                .checkbox(
                    &mut self.config.rules.travel,
                    t(
                        lang,
                        "Viajar juntos — uma lâmpada ou lápide pergunta se leva o time inteiro",
                        "Travel together — lamps and headstones ask to bring the whole party",
                    ),
                )
                .changed()
            {
                if !self.config.rules.travel {
                    self.config.rules.vote = false;
                }
                changed = true;
            }
            note_label(
                ui,
                t(
                    lang,
                    "Desligado: cada um viaja sozinho.",
                    "Off: each hunter travels individually.",
                ),
            );

            ui.horizontal(|ui| {
                ui.add_space(16.0);
                if ui
                    .add_enabled(
                        self.config.rules.travel,
                        egui::Checkbox::new(
                            &mut self.config.rules.vote,
                            t(
                                lang,
                                "Com votação — o time tem 20 s; um \"não\" cancela a viagem (precisa de \"Viajar juntos\")",
                                "With vote — party has 20 s; a single \"no\" cancels travel (requires \"Travel together\")",
                            ),
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
                    t(
                        lang,
                        "Reviver os caídos — os companheiros levantam um caçador caído",
                        "Revive fallen hunters — teammates can pick up a downed hunter",
                    ),
                )
                .changed()
            {
                changed = true;
            }
            note_label(
                ui,
                t(
                    lang,
                    "Desligado: morre de verdade, como no jogo normal.  (Padrão: ligado.)",
                    "Off: die immediately, like vanilla Bloodborne.  (Default: on.)",
                ),
            );
        });

        if changed {
            self.config.save();
        }

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!(
                    "{}: {}.",
                    t(lang, "No time, os outros veem", "Party sees"),
                    self.config.rules.describe_for(lang)
                ))
                .size(12.0)
                .color(p.muted_text),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button(t(lang, "Pegar do bbport", "Load from bbport"))
                    .clicked()
                {
                    if !self.config.take_rules_from_bbport_launcher() {
                        show_info(t(
                            lang,
                            "Não achei as regras no launcher do bbport.\n\nAbra o launcher do bbport, entre em \"Seamless co-op\" e escolha as regras do host por lá; depois volte e clique aqui.",
                            "Could not find host rules in the bbport launcher.\n\nOpen the bbport launcher, go to \"Seamless co-op\", pick your host rules there, then come back and click here.",
                        ));
                    } else {
                        self.config.save();
                        let desc = self.config.rules.describe_for(lang);
                        self.append_log(&format!(
                            "{}: {desc}.",
                            t(
                                lang,
                                "Regras do launcher do bbport aplicadas",
                                "Applied host rules from bbport launcher"
                            )
                        ));
                        show_info(&format!(
                            "{}: {desc}",
                            t(lang, "Regras do launcher do bbport", "bbport launcher rules")
                        ));
                    }
                }
            });
        });
    }

    fn render_controls_tab(&mut self, ui: &mut egui::Ui, lang: Lang) {
        let p = Palette::of(ui.visuals().dark_mode);
        ui.label(
            egui::RichText::new(t(
                lang,
                "O jogo aceita teclado/mouse e controle ao mesmo tempo: as teclas e os botões do \
                 controle respondem sempre. A escolha abaixo decide só o mouse.",
                "The game accepts keyboard/mouse and controller at the same time: keys and gamepad \
                 buttons always respond. The choice below only controls the mouse camera.",
            ))
            .size(12.0)
            .color(p.muted_text),
        );
        ui.add_space(6.0);

        let is_keyboard = self.config.play_with != input_config::CONTROLLER;
        let mut apply_now = false;

        group_box(
            ui,
            t(lang, "Com o que você joga", "What you play with"),
            |ui| {
                if ui
                    .radio(
                        is_keyboard,
                        t(
                            lang,
                            "Teclado e mouse — o mouse gira a câmera (layout Dark Souls III) e fica capturado no jogo",
                            "Keyboard and mouse — mouse turns the camera (Dark Souls III layout) and is captured in-game",
                        ),
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
                        t(
                            lang,
                            "Controle (gamepad) — o mouse não mexe a câmera; o ponteiro fica livre",
                            "Controller (gamepad) — mouse does not move the camera; cursor stays free",
                        ),
                    )
                    .clicked()
                    && is_keyboard
                {
                    self.config.play_with = input_config::CONTROLLER.to_string();
                    apply_now = true;
                }
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(t(
                        lang,
                        "Durante o jogo: F7 liga/desliga o mouse e F8 recarrega o arquivo. \
                         As teclas continuam valendo nas duas opções.",
                        "In-game: F7 toggles the mouse camera and F8 reloads input.ini. \
                         Keyboard keys work in both modes.",
                    ))
                    .size(11.5)
                    .color(p.muted_text),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(self.input_hint(lang))
                        .size(11.5)
                        .color(p.muted_text),
                );
            },
        );

        if apply_now {
            self.config.save();
            if self.port.ready() {
                let msg = match input_config::apply(&self.port, &self.config.play_with) {
                    Ok(m) | Err(m) => m,
                };
                self.append_log(&msg);
            }
        }

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui.button(t(lang, "Aplicar agora", "Apply now")).clicked() {
                if self.config.play_with.is_empty() {
                    self.config.play_with = input_config::KEYBOARD.to_string();
                }
                self.config.save();
                if !self.port.ready() {
                    show_info(t(
                        lang,
                        "O bbport ainda não está pronto. Escolha a pasta dele na aba Avançado.",
                        "bbport is not ready yet. Choose its folder on the Advanced tab.",
                    ));
                } else {
                    let msg = match input_config::apply(&self.port, &self.config.play_with) {
                        Ok(m) | Err(m) => m,
                    };
                    self.append_log(&msg);
                }
            }
            if ui
                .button(t(lang, "Restaurar input.ini", "Restore input.ini"))
                .clicked()
                && self.port.ready()
            {
                let msg = match input_config::restore(&self.port) {
                    Ok(m) | Err(m) => m,
                };
                self.append_log(&msg);
                show_info(&msg);
            }
            if ui
                .button(t(
                    lang,
                    "Abrir launcher do bbport",
                    "Open bbport launcher",
                ))
                .clicked()
            {
                self.open_official_launcher(t(
                    lang,
                    "Launcher do bbport aberto. Escolha a página \"Controles\" para trocar teclas e botões.",
                    "bbport launcher opened. Go to the \"Controls\" page to rebind keys and buttons.",
                ));
            }
        });

        ui.add_space(6.0);
        group_box(
            ui,
            t(
                lang,
                "Trocar tecla por tecla, ou botão do controle",
                "Rebinding individual keys or controller buttons",
            ),
            |ui| {
                ui.label(t(
                    lang,
                    "O BBRadmin só alterna teclado+mouse / controle (acima). Para escolher cada tecla e cada \
                     botão, use a página Controles do launcher do bbport (botão acima): lá dá para clicar num \
                     campo e apertar a tecla ou o botão do controle.\n\n\
                     O arquivo é o input.ini da pasta do bbport. O BBRadmin guarda uma cópia dele \
                     (input.ini.bbradmin-bak) antes de mexer, e \"Restaurar input.ini\" devolve a sua.",
                    "BBRadmin only toggles keyboard+mouse / controller above. To rebind individual keys \
                     or gamepad buttons, use the bbport launcher's Controls page (button above).\n\n\
                     The file is input.ini inside the bbport folder. BBRadmin keeps a backup \
                     (input.ini.bbradmin-bak) before editing, and \"Restore input.ini\" restores yours.",
                ));
            },
        );
    }

    fn render_advanced_tab(&mut self, ui: &mut egui::Ui, lang: Lang) {
        let p = Palette::of(ui.visuals().dark_mode);

        group_box(ui, t(lang, "Pasta do bbport", "bbport folder"), |ui| {
            ui.label(t(
                lang,
                "A que tem o Bloodborne.exe:",
                "The folder containing Bloodborne.exe:",
            ));
            ui.horizontal(|ui| {
                let mut root_disp = self.port.root.clone();
                ui.add(
                    egui::TextEdit::singleline(&mut root_disp)
                        .desired_width(420.0)
                        .interactive(false),
                );
                if ui.button(t(lang, "Procurar...", "Browse...")).clicked() {
                    self.on_browse_folder();
                }
                if ui.button(t(lang, "Verificar", "Verify")).clicked() {
                    self.refresh_detection();
                    let mut rep = format!("{}\n", self.port.describe_for(lang));
                    for m in &self.port.missing {
                        rep.push_str(&format!("{}: {m}\n", t(lang, "FALTA", "MISSING")));
                    }
                    for n in &self.port.notes {
                        rep.push_str(&format!("{}: {n}\n", t(lang, "aviso", "note")));
                    }
                    self.append_log(&rep);
                    show_info(&rep);
                }
            });
            let info_text = if !self.port.missing.is_empty() {
                format!(
                    "{}: {}",
                    t(lang, "Não deu para usar", "Cannot use this folder"),
                    self.port.missing.join(", ")
                )
            } else if !self.port.notes.is_empty() {
                format!(
                    "{} · {}: {}",
                    self.port.root,
                    t(lang, "observe", "note"),
                    self.port.notes.join(", ")
                )
            } else if !self.port.root.is_empty() {
                self.port.describe_for(lang)
            } else {
                t(
                    lang,
                    "Escolha a pasta onde você descompactou o bbport (a que tem Bloodborne.exe).",
                    "Choose the folder where you extracted bbport (containing Bloodborne.exe).",
                )
                .to_string()
            };
            ui.label(
                egui::RichText::new(info_text)
                    .size(11.5)
                    .color(p.muted_text),
            );
        });

        ui.add_space(6.0);
        group_box(ui, t(lang, "Conta do co-op", "Co-op account"), |ui| {
            ui.label(t(
                lang,
                "Id do jogador (cada um dos dois usa um diferente):",
                "Player ID (each player uses a different one):",
            ));
            if ui
                .add(egui::TextEdit::singleline(&mut self.config.account).desired_width(240.0))
                .lost_focus()
            {
                self.config.account = self.config.account.trim().to_string();
                self.config.save();
            }

            ui.label(t(lang, "Senha:", "Password:"));
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
                ui.checkbox(&mut self.show_password, t(lang, "Mostrar", "Show"));
                if ui.button(t(lang, "Gerar nova", "Generate")).clicked() {
                    self.config.password = AppConfig::new_password();
                    self.show_password = true;
                    self.config.save();
                    self.append_log(t(
                        lang,
                        "Senha nova gerada (os dois jogadores podem ter senhas diferentes).",
                        "New password generated (both players can have different passwords).",
                    ));
                }
                if ui
                    .button(t(lang, "Usar do bbport", "Use from bbport"))
                    .clicked()
                {
                    if !self.config.take_from_bbport_launcher() {
                        show_info(t(
                            lang,
                            "Não achei uma conta no launcher do bbport.\n\nAbra o launcher do bbport, entre em \"Seamless co-op\" e faça o login por lá uma vez; depois volte e clique aqui.",
                            "No account found in the bbport launcher.\n\nOpen the bbport launcher, go to \"Seamless co-op\" and sign in once; then come back and click here.",
                        ));
                    } else {
                        self.config.save();
                        let acc = self.config.account.clone();
                        self.append_log(&format!(
                            "{}: {acc}.",
                            t(
                                lang,
                                "Conta do launcher do bbport aplicada",
                                "Applied account from bbport launcher"
                            )
                        ));
                        show_info(&format!(
                            "{}: {acc}",
                            t(
                                lang,
                                "Conta do launcher do bbport aplicada",
                                "Applied bbport launcher account"
                            )
                        ));
                    }
                }
            });
            let account_hint = if !self.launcher_settings.supermedo_npid.is_empty() {
                format!(
                    "{}: {} — {}",
                    t(
                        lang,
                        "Conta do seu launcher do bbport",
                        "Your bbport launcher account"
                    ),
                    self.launcher_settings.supermedo_npid,
                    t(
                        lang,
                        "a mesma do Seamless co-op.",
                        "same as Seamless co-op."
                    )
                )
            } else {
                t(
                    lang,
                    "Sem conta no launcher do bbport: o servidor cria uma sozinho no primeiro entrar.",
                    "No account in bbport launcher: the server creates one automatically on first sign-in.",
                )
                .to_string()
            };
            ui.label(
                egui::RichText::new(account_hint)
                    .size(11.5)
                    .color(p.muted_text),
            );
        });

        ui.add_space(6.0);
        group_box(ui, t(lang, "Opções", "Options"), |ui| {
            ui.horizontal(|ui| {
                ui.label(t(lang, "Idioma:", "Language:"));
                let current_lang = self.config.lang;
                egui::ComboBox::from_id_salt("lang_combo")
                    .width(220.0)
                    .selected_text(current_lang.label(lang))
                    .show_ui(ui, |ui| {
                        for &mode in LangMode::ALL {
                            if ui
                                .selectable_label(mode == current_lang, mode.label(lang))
                                .clicked()
                            {
                                self.config.lang = mode;
                                self.config.save();
                                let new_lang = self.config.lang.resolve();
                                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Title(
                                    t(
                                        new_lang,
                                        "BBRadmin · co-op em Radmin VPN",
                                        "BBRadmin · Radmin VPN co-op",
                                    )
                                    .to_string(),
                                ));
                            }
                        }
                    });
            });
            ui.add_space(2.0);
            if ui
                .checkbox(
                    &mut self.config.auto_update,
                    t(
                        lang,
                        "Atualizar automaticamente ao abrir (via GitHub Releases)",
                        "Automatically update on startup (via GitHub Releases)",
                    ),
                )
                .changed()
            {
                self.config.save();
            }
            if ui
                .checkbox(
                    &mut self.config.auto_stop_server,
                    t(
                        lang,
                        "Parar o servidor automaticamente ao fechar o jogo ou o BBRadmin",
                        "Automatically stop the server when the game or BBRadmin closes",
                    ),
                )
                .changed()
            {
                self.config.save();
            }
            ui.checkbox(
                &mut self.auto_firewall,
                t(
                    lang,
                    "Pedir as regras de firewall antes de hospedar",
                    "Prompt to install firewall rules before hosting",
                ),
            );
        });

        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    !self.busy,
                    egui::Button::new(t(
                        lang,
                        "Instalar regras de firewall",
                        "Install firewall rules",
                    )),
                )
                .clicked()
            {
                self.spawn_worker(
                    t(
                        lang,
                        "Instalando as regras de firewall (UAC)...",
                        "Installing firewall rules (UAC)...",
                    ),
                    move |_| install_firewall_elevated(lang),
                );
            }
            if ui
                .button(t(lang, "Restaurar overrides", "Restore overrides"))
                .clicked()
                && self.port.ready()
            {
                let confirm = ask_yes_no(&format!(
                    "{} {}?\n\n{}",
                    t(lang, "Voltar o", "Restore"),
                    coop_schema::OVERRIDES_FILE,
                    t(
                        lang,
                        "(o bbport volta a usar o servidor dele)",
                        "(bbport will go back to using its own server)"
                    )
                ));
                if confirm {
                    let msg = match overrides::restore(&self.port) {
                        Ok(m) | Err(m) => m,
                    };
                    self.append_log(&msg);
                    show_info(&msg);
                }
            }
            if ui.button(t(lang, "Recarregar", "Reload")).clicked() {
                self.refresh_detection();
            }
            if ui
                .add_enabled(
                    !self.busy,
                    egui::Button::new(t(lang, "Verificar atualizações", "Check for updates")),
                )
                .clicked()
            {
                self.on_check_updates_manual();
            }
        });

        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            if ui
                .button(t(lang, "Abrir pasta de logs", "Open logs folder"))
                .clicked()
            {
                let log_dir = Path::new(&self.port.root)
                    .join(coop_schema::COOP_USER_DIR)
                    .join("log");
                self.open_folder(&log_dir);
            }
            if ui
                .button(t(lang, "Abrir pasta do bbport", "Open bbport folder"))
                .clicked()
            {
                let root = Path::new(&self.port.root).to_path_buf();
                self.open_folder(&root);
            }
            if ui
                .button(t(
                    lang,
                    "Ver o que um start faria",
                    "Preview launch (dry-run)",
                ))
                .clicked()
            {
                self.config.save();
                dry_run::run(&self.config, false);
                let path = dry_run::file_path();
                if let Ok(content) = fs::read_to_string(&path) {
                    self.append_log(&format!(
                        "{}:\n{content}",
                        t(lang, "Relatório do dry-run", "Dry-run report")
                    ));
                }
                let _ = Command::new("cmd.exe")
                    .args(["/c", "start", "", &path.to_string_lossy()])
                    .creation_flags(CREATE_NO_WINDOW)
                    .spawn();
            }
        });
    }
}

/// Draws a crisp vector circle badge (green with white checkmark, or red with white X) + hover tooltip.
fn draw_status_badge(ui: &mut egui::Ui, ok: bool, tooltip: &str) {
    let size = egui::vec2(20.0, 20.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
    let painter = ui.painter();
    let center = rect.center();
    let radius = 9.0_f32;
    let fill = if ok {
        egui::Color32::from_rgb(38, 166, 91)
    } else {
        egui::Color32::from_rgb(214, 58, 58)
    };
    painter.circle_filled(center, radius, fill);

    let stroke = egui::Stroke::new(2.0_f32, egui::Color32::WHITE);
    if ok {
        let p1 = egui::pos2(center.x - 4.2, center.y + 0.3);
        let p2 = egui::pos2(center.x - 1.2, center.y + 3.4);
        let p3 = egui::pos2(center.x + 4.5, center.y - 3.2);
        painter.line_segment([p1, p2], stroke);
        painter.line_segment([p2, p3], stroke);
    } else {
        let d = 3.6_f32;
        painter.line_segment(
            [
                egui::pos2(center.x - d, center.y - d),
                egui::pos2(center.x + d, center.y + d),
            ],
            stroke,
        );
        painter.line_segment(
            [
                egui::pos2(center.x + d, center.y - d),
                egui::pos2(center.x - d, center.y + d),
            ],
            stroke,
        );
    }

    response.on_hover_text(tooltip);
}

fn group_box(ui: &mut egui::Ui, title: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    let p = Palette::of(ui.visuals().dark_mode);
    egui::Frame::group(ui.style())
        .fill(p.card_bg)
        .stroke(egui::Stroke::new(1.0_f32, p.border))
        .corner_radius(5.0)
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                egui::RichText::new(title)
                    .strong()
                    .size(12.5)
                    .color(p.section_text),
            );
            ui.add_space(3.0);
            add_contents(ui);
        });
}

fn note_label(ui: &mut egui::Ui, text: &str) {
    let p = Palette::of(ui.visuals().dark_mode);
    ui.horizontal(|ui| {
        ui.add_space(20.0);
        ui.label(egui::RichText::new(text).size(11.5).color(p.muted_text));
    });
}

fn validate_ip(ip: &str, lang: Lang) -> Result<(), &'static str> {
    if ip.is_empty() {
        return Err(t(lang, "sem endereço", "no address"));
    }
    ip.parse::<Ipv4Addr>()
        .map(|_| ())
        .map_err(|_| t(lang, "não é um endereço IPv4", "not an IPv4 address"))
}

fn do_host(
    port: BbportInfo,
    config: AppConfig,
    server: Arc<ServerController>,
    ip: String,
    auto_firewall: bool,
    lang: Lang,
    tx: Sender<WorkerMsg>,
) -> String {
    let mut say = String::new();

    let missing = firewall::missing();
    if !missing.is_empty() && auto_firewall {
        let prompt = match lang {
            Lang::PtBr => format!(
                "As regras de firewall do BBRadmin ainda não estão instaladas ({}).\n\nSem elas o amigo pode não conseguir entrar. Instalar agora? (pede administrador)",
                missing.join(", ")
            ),
            Lang::EnUs => format!(
                "BBRadmin firewall rules are not installed yet ({}).\n\nWithout them, friends may not be able to join. Install now? (asks for administrator)",
                missing.join(", ")
            ),
        };
        if ask_yes_no(&prompt) {
            say.push_str(&install_firewall_elevated(lang));
            say.push('\n');
        }
    }
    if !firewall::missing().is_empty() {
        say.push_str(t(
            lang,
            "Sem todas as regras de firewall: se o amigo não entrar, instale-as na aba Avançado.\n",
            "Not all firewall rules are installed: if your friend cannot join, install them on the Advanced tab.\n",
        ));
    }

    if !server.running() {
        match server.start(&ip) {
            Ok(line) => say.push_str(&format!("{line}\n")),
            Err(err) => {
                say.push_str(&format!(
                    "{err}\n{}\n",
                    t(
                        lang,
                        "Não vou abrir o jogo sem servidor.",
                        "Not launching the game without the server."
                    )
                ));
                return say;
            }
        }
        say.push_str(&format!("{}\n", server.wait_until_ready(&ip, 30)));
        if !server.backend_answers() {
            say.push_str(t(
                lang,
                "Não vou abrir o jogo: o backend 20443 não respondeu.\n",
                "Not launching the game: backend 20443 did not respond.\n",
            ));
            return say;
        }
    } else {
        say.push_str(t(
            lang,
            "O servidor já estava em execução.\n",
            "Server was already running.\n",
        ));
    }

    say.push_str(&format!(
        "{}: {ip}  (Radmin VPN)\n",
        t(lang, "Endereço para o amigo", "Address for your friend")
    ));
    start_game(&port, &config, true, &ip, lang, &mut say, tx);
    say
}

fn do_guest(
    port: BbportInfo,
    config: AppConfig,
    ip: String,
    lang: Lang,
    tx: Sender<WorkerMsg>,
) -> String {
    if validate_ip(&ip, lang).is_err() {
        return t(
            lang,
            "Informe o IP do host (26.x.x.x), aquele que o amigo te passou.",
            "Enter the host IP (26.x.x.x) shared by your friend.",
        )
        .to_string();
    }
    let mut say = format!(
        "{}: {}\n",
        t(lang, "Servidor do amigo", "Friend's server"),
        health::describe_for(&ip, 1500, lang)
    );
    if !health::reachable(&ip, 1500) {
        say.push_str(t(
            lang,
            "Aviso: o servidor do amigo não respondeu. Confira se o Radmin VPN está ligado nos dois e se o host já hospedou.\n",
            "Warning: your friend's server did not respond. Check that Radmin VPN is on for both players and the host has started hosting.\n",
        ));
    }
    start_game(&port, &config, false, &ip, lang, &mut say, tx);
    say
}

fn start_game(
    port: &BbportInfo,
    config: &AppConfig,
    is_host: bool,
    ip: &str,
    lang: Lang,
    say: &mut String,
    tx: Sender<WorkerMsg>,
) {
    match overrides::write(port, ip) {
        Ok(line) => say.push_str(&format!("{line}\n")),
        Err(err) => {
            say.push_str(&format!(
                "{err}\n{}\n",
                t(
                    lang,
                    "Parei: sem este arquivo o jogo procuraria o servidor da Sony.",
                    "Stopped: without this file the game would try to reach Sony's server."
                )
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
        say.push_str(t(
            lang,
            "Fechando o jogo que já estava aberto...\n",
            "Closing already-running game...\n",
        ));
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
    );

    match game_environment::start(port, &plan) {
        Ok(child) => {
            let pid = child.id();
            say.push_str(&format!(
                "{}: {}.\n",
                t(lang, "Modo de tela", "Display mode"),
                plan.display.describe()
            ));
            if is_host {
                say.push_str(&format!(
                    "{}: {}.\n",
                    t(lang, "Regras do host", "Host rules"),
                    rules.describe_for(lang)
                ));
            } else {
                say.push_str(t(
                    lang,
                    "Regras do co-op: as do líder (você entrou no time dele).\n",
                    "Co-op rules: leader's rules (you joined their party).\n",
                ));
            }
            say.push_str(&format!(
                "{} (pid {pid}).\n",
                t(lang, "Jogo aberto", "Game launched")
            ));
            say.push_str(t(
                lang,
                "No jogo: abra o menu, entre no time e convide o amigo.\n",
                "In-game: open the menu, enter the party screen, and invite your friend.\n",
            ));
            let _ = tx.send(WorkerMsg::GameStarted {
                child,
                display: plan.display,
            });
        }
        Err(err) => {
            say.push_str(&format!(
                "{}: {err}\n",
                t(lang, "Não abri o jogo", "Could not launch the game")
            ));
        }
    }
}

fn install_firewall_elevated(lang: Lang) -> String {
    let Ok(exe) = std::env::current_exe() else {
        return format!(
            "{}\n{}\n{}",
            t(
                lang,
                "Não localizei o executável do BBRadmin.",
                "Could not locate the BBRadmin executable."
            ),
            t(
                lang,
                "Você pode colar os comandos num console de administrador:",
                "You can paste these commands into an administrator console:"
            ),
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
            "{}\n{}\n{}",
            t(
                lang,
                "Não elevei o BBRadmin (UAC cancelado ou bloqueado).",
                "Could not elevate BBRadmin (UAC cancelled or blocked)."
            ),
            t(
                lang,
                "Você pode colar os comandos num console de administrador:",
                "You can paste these commands into an administrator console:"
            ),
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
        format!(
            "{}\n{report}",
            t(
                lang,
                "Firewall pronto: as três regras estão instaladas.",
                "Firewall ready: all three rules are installed."
            )
        )
    } else {
        format!(
            "{}: {}\n{report}",
            t(lang, "Ainda faltam regras", "Still missing rules"),
            still.join(", ")
        )
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
