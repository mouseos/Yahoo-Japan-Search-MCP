//! One-click registration GUI: writes the MCP server entry into whichever
//! agent configs are present on this machine.

#![windows_subsystem = "windows"]

use eframe::egui;
use std::path::PathBuf;

const MCP_NAME: &str = "Yahoo-Japan-Search-MCP";

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([620.0, 640.0])
            .with_min_inner_size([500.0, 400.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Yahoo! JAPAN Search MCP Setup",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}

// ── Font setup ──

fn setup_system_fonts(ctx: &egui::Context) {
    use font_kit::family_name::FamilyName;
    use font_kit::properties::Properties;
    use font_kit::source::SystemSource;

    let source = SystemSource::new();
    let props = Properties::new();

    let load = |handle: font_kit::handle::Handle| -> Option<(Vec<u8>, u32)> {
        match handle {
            font_kit::handle::Handle::Path { path, font_index } => {
                std::fs::read(&path).ok().map(|d| (d, font_index))
            }
            font_kit::handle::Handle::Memory { bytes, font_index } => {
                Some(((*bytes).clone(), font_index))
            }
        }
    };

    let add = |ctx: &egui::Context, name: &str, data: Vec<u8>, index: u32, highest: bool| {
        let mut fd = egui::FontData::from_owned(data);
        fd.index = index;
        let prio = if highest {
            egui::epaint::text::FontPriority::Highest
        } else {
            egui::epaint::text::FontPriority::Lowest
        };
        ctx.add_font(egui::epaint::text::FontInsert::new(
            name,
            fd,
            vec![
                egui::epaint::text::InsertFontFamily {
                    family: egui::FontFamily::Proportional,
                    priority: prio,
                },
                egui::epaint::text::InsertFontFamily {
                    family: egui::FontFamily::Monospace,
                    priority: egui::epaint::text::FontPriority::Lowest,
                },
            ],
        ));
    };

    if let Some((data, idx)) = source
        .select_best_match(&[FamilyName::SansSerif], &props)
        .ok()
        .and_then(&load)
    {
        add(ctx, "sans", data, idx, true);
    }

    // The UI is bilingual, so a CJK fallback is required for the Japanese strings.
    let cjk_names = [
        "Yu Gothic UI",
        "Meiryo UI",
        "Hiragino Sans",
        "Noto Sans CJK JP",
        "Noto Sans JP",
        "Microsoft YaHei UI",
        "Malgun Gothic",
        "WenQuanYi Micro Hei",
        "Droid Sans Fallback",
    ];
    for name in cjk_names {
        if let Some((data, idx)) = source
            .select_best_match(&[FamilyName::Title(name.to_string())], &props)
            .ok()
            .and_then(&load)
        {
            add(ctx, "cjk", data, idx, false);
            break;
        }
    }
}

// ── Paths & detection ──

fn server_exe_path() -> PathBuf {
    let mut path = std::env::current_exe().unwrap_or_default();
    path.pop();
    path.push(if cfg!(windows) {
        "yahoo-search-mcp.exe"
    } else {
        "yahoo-search-mcp"
    });
    path
}

fn home_dir() -> Option<PathBuf> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
        .map(PathBuf::from)
}

fn claude_desktop_config_path() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var("APPDATA").ok().map(|a| {
            PathBuf::from(a)
                .join("Claude")
                .join("claude_desktop_config.json")
        })
    }
    #[cfg(target_os = "macos")]
    {
        home_dir().map(|h| h.join("Library/Application Support/Claude/claude_desktop_config.json"))
    }
    #[cfg(target_os = "linux")]
    {
        home_dir().map(|h| h.join(".config/Claude/claude_desktop_config.json"))
    }
}

fn claude_code_config_path() -> Option<PathBuf> {
    home_dir().map(|h| h.join(".claude.json"))
}

fn codex_config_path() -> Option<PathBuf> {
    home_dir().map(|h| h.join(".codex").join("config.toml"))
}

fn antigravity_config_path() -> Option<PathBuf> {
    home_dir().map(|h| h.join(".gemini").join("config").join("mcp_config.json"))
}

fn command_exists(name: &str) -> bool {
    let check = if cfg!(windows) { "where" } else { "which" };
    run_cmd(check, &[name])
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn run_cmd(cmd: &str, args: &[&str]) -> Option<std::process::Output> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new(cmd)
            .args(args)
            .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
            .output()
            .ok()
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new(cmd).args(args).output().ok()
    }
}

fn is_registered_in_json(path: &std::path::Path) -> bool {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v.get("mcpServers")?.get(MCP_NAME).cloned())
        .is_some()
}

fn remove_toml_section(content: &str, name: &str) -> String {
    let header_a = format!("[mcp_servers.{name}]");
    let header_b = format!("[mcp_servers.\"{name}\"]");
    let mut result = String::new();
    let mut skip = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == header_a || trimmed == header_b {
            skip = true;
            continue;
        }
        if skip && trimmed.starts_with('[') {
            skip = false;
        }
        if !skip {
            result.push_str(line);
            result.push('\n');
        }
    }
    result
}

fn is_registered_in_toml(path: &std::path::Path) -> bool {
    std::fs::read_to_string(path)
        .map(|s| {
            s.contains(&format!("[mcp_servers.{MCP_NAME}]"))
                || s.contains(&format!("[mcp_servers.\"{MCP_NAME}\"]"))
        })
        .unwrap_or(false)
}

fn mcp_server_entry(exe: &std::path::Path) -> serde_json::Value {
    serde_json::json!({ "command": exe.to_string_lossy(), "args": [] })
}

fn mcp_config_json(exe: &std::path::Path) -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "mcpServers": { MCP_NAME: mcp_server_entry(exe) }
    }))
    .unwrap_or_default()
}

// ── i18n ──

fn detect_lang() -> &'static str {
    #[cfg(windows)]
    {
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;
        extern "system" {
            fn GetUserDefaultLocaleName(p: *mut u16, n: i32) -> i32;
        }
        let mut buf = [0u16; 85];
        let len = unsafe { GetUserDefaultLocaleName(buf.as_mut_ptr(), 85) };
        if len > 0 {
            let s = OsString::from_wide(&buf[..len as usize])
                .to_string_lossy()
                .to_lowercase();
            if s.starts_with("ja") {
                return "ja";
            }
        }
    }
    #[cfg(not(windows))]
    {
        if let Ok(lang) = std::env::var("LANG") {
            if lang.to_lowercase().starts_with("ja") {
                return "ja";
            }
        }
    }
    "en"
}

fn t(lang: &str, key: &str) -> &'static str {
    match (lang, key) {
        ("ja", "title") => "Yahoo!検索 MCP セットアップ",
        ("ja", "desc") => "Yahoo! JAPAN 検索のMCPサーバー（APIキー・設定不要）",
        ("ja", "providers") => "対応: ウェブ / 画像 / 動画 / ニュース / リアルタイム / 知恵袋 / サジェスト",
        ("ja", "server_path") => "サーバーパス:",
        ("ja", "sec_claude") => "Claude (Desktop / Code)",
        ("ja", "sec_antigravity") => "Google Antigravity",
        ("ja", "sec_codex") => "Codex",
        ("ja", "sec_other") => "その他のMCPクライアント",
        ("ja", "sec_prompt") => "AIエージェントに貼り付け",
        ("ja", "other_desc") => "以下のJSONをMCP設定に追加:",
        ("ja", "prompt_desc") => "以下をAIエージェントに送信すると自動設定されます:",
        ("ja", "btn_install") => "インストール",
        ("ja", "btn_uninstall") => "削除",
        ("ja", "btn_copy_json") => "JSONをコピー",
        ("ja", "btn_copy_prompt") => "プロンプトをコピー",
        ("ja", "registered") => "登録済み",
        ("ja", "not_registered") => "未登録",
        ("ja", "not_installed") => "未インストール",
        ("ja", "install_ok") => "インストール完了！再起動してください。",
        ("ja", "uninstall_ok") => "削除完了！再起動してください。",
        ("ja", "already_installed") => "既にインストール済みです",
        ("ja", "err_read") => "設定ファイル読み込みエラー",
        ("ja", "err_write") => "設定ファイル書き込みエラー",
        ("ja", "err_cmd") => "コマンド実行エラー",
        ("ja", "err_not_found") => "コマンドが見つかりません",
        ("ja", "copied") => "コピーしました",
        ("ja", "agent_prompt") => "以下のMCPサーバーをMCP設定に追加してください。下のJSONをこの環境の適切なMCP設定ファイルに書き込んでください。\n\n{config}\n\n設定ファイルを書き込んだ後、新しいMCPサーバーを有効にするためにこのエージェント/アプリの再起動が必要であることを伝えてください。サーバーを直接起動しないでください — MCPクライアントが自動的に起動します。",

        (_, "title") => "Yahoo! JAPAN Search MCP Setup",
        (_, "desc") => "Yahoo! JAPAN search MCP server (no API key, no configuration)",
        (_, "providers") => "Verticals: Web / Image / Video / News / Realtime / Chiebukuro / Suggest",
        (_, "server_path") => "Server path:",
        (_, "sec_claude") => "Claude (Desktop / Code)",
        (_, "sec_antigravity") => "Google Antigravity",
        (_, "sec_codex") => "Codex",
        (_, "sec_other") => "Other MCP Clients",
        (_, "sec_prompt") => "Paste to AI Agent",
        (_, "other_desc") => "Add this JSON to your MCP settings:",
        (_, "prompt_desc") => "Send the following to your AI agent for auto-config:",
        (_, "btn_install") => "Install",
        (_, "btn_uninstall") => "Uninstall",
        (_, "btn_copy_json") => "Copy JSON",
        (_, "btn_copy_prompt") => "Copy Prompt",
        (_, "registered") => "Registered",
        (_, "not_registered") => "Not registered",
        (_, "not_installed") => "Not installed",
        (_, "install_ok") => "Installed! Restart to activate.",
        (_, "uninstall_ok") => "Removed! Restart to apply.",
        (_, "already_installed") => "Already installed",
        (_, "err_read") => "Error reading config",
        (_, "err_write") => "Error writing config",
        (_, "err_cmd") => "Command failed",
        (_, "err_not_found") => "Command not found",
        (_, "copied") => "Copied",
        (_, "agent_prompt") => "Add the following MCP server to my MCP configuration. Write the JSON below into the appropriate MCP config file for this environment.\n\n{config}\n\nAfter writing the config, tell me to restart this agent/app so the new MCP server takes effect. Do not start the server yourself - the MCP client launches it automatically.",
        _ => "",
    }
}

// ── Client state ──

#[derive(Clone)]
struct ClientState {
    available: bool,
    registered: bool,
    message: String,
}

impl ClientState {
    fn new(available: bool, registered: bool) -> Self {
        Self {
            available,
            registered,
            message: String::new(),
        }
    }
    fn unavailable() -> Self {
        Self {
            available: false,
            registered: false,
            message: String::new(),
        }
    }
}

// ── JSON config helpers ──

fn install_to_json(
    config_path: Option<PathBuf>,
    server_path: &std::path::Path,
    lang: &str,
) -> (bool, String) {
    let Some(config_path) = config_path else {
        return (false, t(lang, "err_write").to_string());
    };

    if let Some(parent) = config_path.parent() {
        if !parent.exists() {
            let _ = std::fs::create_dir_all(parent);
        }
    }

    let mut config: serde_json::Value = if config_path.exists() {
        match std::fs::read_to_string(&config_path) {
            Ok(c) => serde_json::from_str(&c).unwrap_or(serde_json::json!({})),
            Err(e) => return (false, format!("{}: {e}", t(lang, "err_read"))),
        }
    } else {
        serde_json::json!({})
    };

    if !config.is_object() {
        config = serde_json::json!({});
    }

    let servers = config
        .as_object_mut()
        .unwrap()
        .entry("mcpServers")
        .or_insert(serde_json::json!({}));

    let new_entry = mcp_server_entry(server_path);
    if servers.get(MCP_NAME) == Some(&new_entry) {
        return (true, t(lang, "already_installed").to_string());
    }

    servers
        .as_object_mut()
        .unwrap()
        .insert(MCP_NAME.to_string(), new_entry);

    match std::fs::write(&config_path, serde_json::to_string_pretty(&config).unwrap()) {
        Ok(_) => (true, t(lang, "install_ok").to_string()),
        Err(e) => (false, format!("{}: {e}", t(lang, "err_write"))),
    }
}

fn uninstall_from_json(config_path: Option<PathBuf>, lang: &str) -> (bool, String) {
    let Some(config_path) = config_path else {
        return (false, t(lang, "err_write").to_string());
    };
    if !config_path.exists() {
        return (false, t(lang, "err_read").to_string());
    }

    let Ok(content) = std::fs::read_to_string(&config_path) else {
        return (false, t(lang, "err_read").to_string());
    };
    let Ok(mut config) = serde_json::from_str::<serde_json::Value>(&content) else {
        return (false, t(lang, "err_read").to_string());
    };

    if let Some(servers) = config.get_mut("mcpServers").and_then(|s| s.as_object_mut()) {
        servers.remove(MCP_NAME);
    }

    match std::fs::write(&config_path, serde_json::to_string_pretty(&config).unwrap()) {
        Ok(_) => (true, t(lang, "uninstall_ok").to_string()),
        Err(e) => (false, format!("{}: {e}", t(lang, "err_write"))),
    }
}

// ── App ──

struct App {
    server_path: PathBuf,
    config_json: String,
    lang: &'static str,
    claude_desktop: ClientState,
    claude_code: ClientState,
    antigravity: ClientState,
    codex: ClientState,
    copied_toast_time: f64,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        setup_system_fonts(&cc.egui_ctx);
        let server_path = server_exe_path();
        let config_json = mcp_config_json(&server_path);
        let lang = detect_lang();

        let claude_desktop = match claude_desktop_config_path() {
            Some(path) => ClientState::new(true, is_registered_in_json(&path)),
            None => ClientState::unavailable(),
        };

        let claude_code = ClientState::new(
            true,
            claude_code_config_path()
                .map(|p| is_registered_in_json(&p))
                .unwrap_or(false),
        );

        let antigravity = match antigravity_config_path() {
            Some(path) => ClientState::new(true, is_registered_in_json(&path)),
            None => ClientState::unavailable(),
        };

        let codex = ClientState::new(
            true,
            codex_config_path()
                .map(|p| is_registered_in_toml(&p))
                .unwrap_or(false),
        );

        Self {
            server_path,
            config_json,
            lang,
            claude_desktop,
            claude_code,
            antigravity,
            codex,
            copied_toast_time: 0.0,
        }
    }

    fn install_claude_desktop(&mut self) {
        let (ok, msg) = install_to_json(claude_desktop_config_path(), &self.server_path, self.lang);
        self.claude_desktop.registered = ok;
        self.claude_desktop.message = msg;
    }

    fn uninstall_claude_desktop(&mut self) {
        let (ok, msg) = uninstall_from_json(claude_desktop_config_path(), self.lang);
        self.claude_desktop.registered = !ok;
        self.claude_desktop.message = msg;
    }

    fn install_antigravity(&mut self) {
        let (ok, msg) = install_to_json(antigravity_config_path(), &self.server_path, self.lang);
        self.antigravity.registered = ok;
        self.antigravity.message = msg;
    }

    fn uninstall_antigravity(&mut self) {
        let (ok, msg) = uninstall_from_json(antigravity_config_path(), self.lang);
        self.antigravity.registered = !ok;
        self.antigravity.message = msg;
    }

    // ── CLI-based install (Claude Code / Codex), with a config-file fallback ──

    fn cli_install(&self, cmd: &str) -> Result<String, String> {
        let exe_str = self.server_path.to_string_lossy().to_string();
        let output = run_cmd(
            cmd,
            &["mcp", "add", "--scope", "user", MCP_NAME, "--", &exe_str],
        )
        .ok_or_else(|| format!("{}: {cmd}", t(self.lang, "err_not_found")))?;
        if output.status.success() {
            Ok(t(self.lang, "install_ok").to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            if stderr.contains("already") {
                Ok(t(self.lang, "already_installed").to_string())
            } else {
                Err(format!("{}: {}", t(self.lang, "err_cmd"), stderr.trim()))
            }
        }
    }

    fn cli_uninstall(&self, cmd: &str) -> Result<String, String> {
        let output = run_cmd(cmd, &["mcp", "remove", "--scope", "user", MCP_NAME])
            .ok_or_else(|| format!("{}: {cmd}", t(self.lang, "err_not_found")))?;
        if output.status.success() {
            Ok(t(self.lang, "uninstall_ok").to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Err(format!("{}: {}", t(self.lang, "err_cmd"), stderr.trim()))
        }
    }

    fn install_claude_code(&mut self) {
        if self.claude_code.registered {
            self.claude_code.message = t(self.lang, "already_installed").to_string();
            return;
        }
        if command_exists("claude") {
            if let Ok(msg) = self.cli_install("claude") {
                self.claude_code.registered = true;
                self.claude_code.message = msg;
                return;
            }
        }
        let (ok, msg) = install_to_json(claude_code_config_path(), &self.server_path, self.lang);
        self.claude_code.registered = ok;
        self.claude_code.message = msg;
    }

    fn uninstall_claude_code(&mut self) {
        if command_exists("claude") {
            if let Ok(msg) = self.cli_uninstall("claude") {
                self.claude_code.registered = false;
                self.claude_code.message = msg;
                return;
            }
        }
        let (ok, msg) = uninstall_from_json(claude_code_config_path(), self.lang);
        self.claude_code.registered = !ok;
        self.claude_code.message = msg;
    }

    fn install_codex(&mut self) {
        if self.codex.registered {
            self.codex.message = t(self.lang, "already_installed").to_string();
            return;
        }
        if command_exists("codex") {
            if let Ok(msg) = self.cli_install("codex") {
                self.codex.registered = true;
                self.codex.message = msg;
                return;
            }
        }
        self.install_codex_toml();
    }

    fn uninstall_codex(&mut self) {
        if command_exists("codex") {
            if let Ok(msg) = self.cli_uninstall("codex") {
                self.codex.registered = false;
                self.codex.message = msg;
                return;
            }
        }
        self.uninstall_codex_toml();
    }

    fn install_codex_toml(&mut self) {
        let Some(config_path) = codex_config_path() else {
            return;
        };

        if let Some(parent) = config_path.parent() {
            if !parent.exists() {
                let _ = std::fs::create_dir_all(parent);
            }
        }

        let mut content = if config_path.exists() {
            match std::fs::read_to_string(&config_path) {
                Ok(c) => c,
                Err(e) => {
                    self.codex.message = format!("{}: {e}", t(self.lang, "err_read"));
                    return;
                }
            }
        } else {
            String::new()
        };

        let exe_escaped = self.server_path.to_string_lossy().replace('\\', "\\\\");

        if is_registered_in_toml(&config_path) {
            if content.contains(&exe_escaped) {
                self.codex.registered = true;
                self.codex.message = t(self.lang, "already_installed").to_string();
                return;
            }
            // Drop the stale entry before re-adding it with the current path.
            content = remove_toml_section(&content, MCP_NAME);
        }

        content.push_str(&format!(
            "\n[mcp_servers.\"{MCP_NAME}\"]\ncommand = \"{exe_escaped}\"\nargs = []\n"
        ));

        match std::fs::write(&config_path, &content) {
            Ok(_) => {
                self.codex.registered = true;
                self.codex.message = t(self.lang, "install_ok").to_string();
            }
            Err(e) => self.codex.message = format!("{}: {e}", t(self.lang, "err_write")),
        }
    }

    fn uninstall_codex_toml(&mut self) {
        let Some(config_path) = codex_config_path() else {
            return;
        };
        if !config_path.exists() {
            return;
        }
        let Ok(content) = std::fs::read_to_string(&config_path) else {
            return;
        };
        let result = remove_toml_section(&content, MCP_NAME);

        match std::fs::write(&config_path, result.trim_end()) {
            Ok(_) => {
                self.codex.registered = false;
                self.codex.message = t(self.lang, "uninstall_ok").to_string();
            }
            Err(e) => self.codex.message = format!("{}: {e}", t(self.lang, "err_write")),
        }
    }

    fn show_toast(&mut self, ctx: &egui::Context, text: &str) {
        ctx.copy_text(text.to_string());
        self.copied_toast_time = ctx.input(|i| i.time);
    }
}

// ── UI helpers ──

fn status_label(lang: &str, state: &ClientState) -> (String, egui::Color32) {
    if !state.available {
        (
            t(lang, "not_installed").to_string(),
            egui::Color32::from_rgb(120, 120, 120),
        )
    } else if state.registered {
        (
            format!("\u{2705} {}", t(lang, "registered")),
            egui::Color32::from_rgb(50, 180, 50),
        )
    } else {
        (
            t(lang, "not_registered").to_string(),
            egui::Color32::from_rgb(180, 180, 50),
        )
    }
}

fn show_client_row(
    ui: &mut egui::Ui,
    label: &str,
    lang: &str,
    state: &ClientState,
) -> Option<bool> {
    let mut action = None;
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add_space(8.0);
        let (status_text, color) = status_label(lang, state);
        ui.colored_label(color, status_text);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if state.available {
                if state.registered {
                    if ui.button(t(lang, "btn_uninstall")).clicked() {
                        action = Some(false);
                    }
                } else if ui.button(t(lang, "btn_install")).clicked() {
                    action = Some(true);
                }
            }
        });
    });
    if !state.message.is_empty() {
        let color = if state.message.contains("Error") || state.message.contains("エラー") {
            egui::Color32::from_rgb(220, 50, 50)
        } else {
            egui::Color32::from_rgb(80, 180, 80)
        };
        ui.colored_label(color, &state.message);
    }
    action
}

// ── Main UI ──

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let is_toast_visible =
            ctx.input(|i| i.time) - self.copied_toast_time < 2.0 && self.copied_toast_time > 0.0;

        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.add_space(10.0);
            ui.heading(t(self.lang, "title"));
            ui.label(t(self.lang, "desc"));
            ui.label(t(self.lang, "providers"));
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(t(self.lang, "server_path"));
                ui.monospace(self.server_path.to_string_lossy().to_string());
            });
            ui.add_space(8.0);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add_space(8.0);

                ui.heading(t(self.lang, "sec_claude"));
                ui.add_space(4.0);
                if let Some(install) =
                    show_client_row(ui, "Claude Desktop", self.lang, &self.claude_desktop)
                {
                    if install {
                        self.install_claude_desktop();
                    } else {
                        self.uninstall_claude_desktop();
                    }
                }
                ui.add_space(4.0);
                if let Some(install) =
                    show_client_row(ui, "Claude Code", self.lang, &self.claude_code)
                {
                    if install {
                        self.install_claude_code();
                    } else {
                        self.uninstall_claude_code();
                    }
                }

                ui.add_space(16.0);
                ui.separator();
                ui.add_space(8.0);

                ui.heading(t(self.lang, "sec_antigravity"));
                ui.add_space(4.0);
                if let Some(install) =
                    show_client_row(ui, "Antigravity", self.lang, &self.antigravity)
                {
                    if install {
                        self.install_antigravity();
                    } else {
                        self.uninstall_antigravity();
                    }
                }

                ui.add_space(16.0);
                ui.separator();
                ui.add_space(8.0);

                ui.heading(t(self.lang, "sec_codex"));
                ui.add_space(4.0);
                if let Some(install) = show_client_row(ui, "Codex CLI", self.lang, &self.codex) {
                    if install {
                        self.install_codex();
                    } else {
                        self.uninstall_codex();
                    }
                }

                ui.add_space(16.0);
                ui.separator();
                ui.add_space(8.0);

                ui.heading(t(self.lang, "sec_other"));
                ui.label(t(self.lang, "other_desc"));
                ui.add_space(4.0);
                let mut config = self.config_json.clone();
                ui.add(
                    egui::TextEdit::multiline(&mut config)
                        .font(egui::TextStyle::Monospace)
                        .desired_rows(7)
                        .desired_width(f32::INFINITY)
                        .interactive(false),
                );
                ui.add_space(4.0);
                if ui.button(t(self.lang, "btn_copy_json")).clicked() {
                    let cj = self.config_json.clone();
                    self.show_toast(ctx, &cj);
                }

                ui.add_space(16.0);
                ui.separator();
                ui.add_space(8.0);

                ui.heading(t(self.lang, "sec_prompt"));
                ui.label(t(self.lang, "prompt_desc"));
                ui.add_space(4.0);
                let prompt_text =
                    t(self.lang, "agent_prompt").replace("{config}", &self.config_json);
                let mut prompt_display = prompt_text.clone();
                ui.add(
                    egui::TextEdit::multiline(&mut prompt_display)
                        .font(egui::TextStyle::Monospace)
                        .desired_rows(6)
                        .desired_width(f32::INFINITY)
                        .interactive(false),
                );
                ui.add_space(4.0);
                if ui.button(t(self.lang, "btn_copy_prompt")).clicked() {
                    self.show_toast(ctx, &prompt_text);
                }

                ui.add_space(16.0);
            });
        });

        if is_toast_visible {
            egui::Area::new(egui::Id::new("toast"))
                .anchor(egui::Align2::CENTER_BOTTOM, [0.0, -20.0])
                .show(ctx, |ui| {
                    egui::Frame::popup(ui.style())
                        .fill(egui::Color32::from_rgb(40, 40, 40))
                        .corner_radius(egui::CornerRadius::same(8))
                        .show(ui, |ui| {
                            ui.colored_label(
                                egui::Color32::from_rgb(100, 220, 100),
                                format!("\u{2705} {}", t(self.lang, "copied")),
                            );
                        });
                });
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remove_toml_section_drops_only_our_block() {
        let content = format!(
            "[other]\nx = 1\n\n[mcp_servers.\"{MCP_NAME}\"]\ncommand = \"a\"\nargs = []\n\n[tail]\ny = 2\n"
        );
        let out = remove_toml_section(&content, MCP_NAME);
        assert!(!out.contains(MCP_NAME));
        assert!(out.contains("[other]") && out.contains("[tail]") && out.contains("y = 2"));
    }

    #[test]
    fn mcp_config_json_contains_server_name_and_path() {
        let json = mcp_config_json(std::path::Path::new("/opt/yahoo-search-mcp"));
        assert!(json.contains(MCP_NAME));
        assert!(json.contains("yahoo-search-mcp"));
    }
}
