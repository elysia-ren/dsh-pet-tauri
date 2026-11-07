#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! dsh-pet 独立桌宠（Tauri 2）。
//!
//! 结构：
//!   - 窗口/穿透/拖拽/托盘：main.rs（壳）
//!   - 设置与 DSH 导入：settings.rs（本地适配器）
//!   - LLM / 余额 / 记忆：llm.rs / balance.rs / memory.rs / secrets.rs
//!
//! 与 DSH 插件的关系：行为、文案、档位口径对齐；但不依赖 DSH 的任何服务，
//! API Key 用 Windows DPAPI 保护后存在 `%APPDATA%/dsh-pet-tauri/settings.json`。

mod balance;
mod llm;
mod memory;
mod secrets;
mod settings;

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, State, WebviewWindow, WindowEvent};
use tauri_plugin_notification::NotificationExt;

use balance::BalanceOut;
use llm::Msg;
use settings::{Settings, SettingsView};

const CANVAS_W: f64 = 640.0;
const CANVAS_H: f64 = 360.0;
/// 光标轮询间隔（ms）
const POLL_MS: u64 = 40;
const MEMORY_BUCKET: &str = "main";

const WHISPER_ANIMS: [&str; 3] = [
    "碎碎念-擦桌碎碎念",
    "碎碎念-发呆碎碎念",
    "碎碎念-对屏碎碎念",
];
const THINKING_ANIM: &str = "工作状态-思考冒泡";

fn log(msg: &str) {
    let line = format!("[dsh-pet] {msg}\n");
    print!("{line}");
    let _ = std::io::stdout().flush();
    let path = std::env::temp_dir().join("dsh-pet-tauri.log");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = f.write_all(line.as_bytes());
    }
}

fn random_pick(list: &[&str]) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    list[(nanos as usize) % list.len()].to_string()
}

#[derive(Clone, Copy, Deserialize)]
struct HitBox {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

struct PetState {
    settings: Mutex<Settings>,
    memory: Mutex<serde_json::Value>,
    api_key: Mutex<Option<String>>,
    hitbox: Mutex<Option<HitBox>>,
    /// 正在拖拽（左键按住）
    dragging: AtomicBool,
    /// 菜单/对话面板打开，需要整窗可交互
    ui_busy: AtomicBool,
    last_balance: Mutex<Option<Instant>>,
    last_whisper: Mutex<Option<Instant>>,
}

impl PetState {
    fn key(&self) -> Option<String> {
        // 开发/自动化测试逃生口：不落盘，优先级最高
        if let Ok(key) = std::env::var("DSH_PET_API_KEY") {
            if !key.trim().is_empty() {
                return Some(key.trim().to_string());
            }
        }
        if let Some(cached) = self.api_key.lock().unwrap().clone() {
            return Some(cached);
        }
        let enc = self.settings.lock().unwrap().api_key_enc.clone();
        if enc.is_empty() {
            return None;
        }
        let key = secrets::decrypt_key(&enc)?;
        *self.api_key.lock().unwrap() = Some(key.clone());
        Some(key)
    }

    fn settings(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }
}

#[derive(Clone, Serialize)]
struct PetInfo {
    id: String,
    name: String,
    size: f64,
}

#[derive(Clone, Serialize)]
struct PetConfig {
    pet: PetInfo,
    animations: serde_json::Value,
    physics: serde_json::Value,
    animation_weights: serde_json::Value,
    thinking: String,
    boundary_mode: String,
}

fn pet_config(state: &PetState) -> PetConfig {
    let s = state.settings();
    PetConfig {
        pet: PetInfo {
            id: "main".into(),
            name: s.pet_name,
            size: s.pet_size,
        },
        animations: s.animations,
        physics: s.physics,
        animation_weights: s.animation_weights,
        thinking: THINKING_ANIM.into(),
        boundary_mode: s.boundary_mode,
    }
}

#[derive(Clone, Serialize)]
struct SayPayload {
    text: String,
    kind: String,
    image: Option<String>,
}

#[derive(Clone, Serialize)]
struct PlayPayload {
    name: String,
}

fn emit_say(app: &AppHandle, text: &str, kind: &str, image: Option<&str>) {
    let _ = app.emit(
        "pet://say",
        SayPayload {
            text: text.into(),
            kind: kind.into(),
            image: image.map(|s| s.to_string()),
        },
    );
}

fn random_meme(memes: &std::collections::BTreeMap<String, String>) -> Option<String> {
    let mut names: Vec<String> = memes.keys().cloned().collect();
    names.extend(custom_asset_names("memes", "png"));
    if names.is_empty() {
        return None;
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as usize)
        .unwrap_or(0);
    Some(names[nanos % names.len()].clone())
}

#[derive(Clone, Serialize)]
struct AssetEntry {
    name: String,
    path: String,
}

#[derive(Serialize)]
struct CustomAssets {
    animations: Vec<AssetEntry>,
    memes: Vec<AssetEntry>,
}

fn scan_assets(dir: std::path::PathBuf, ext: &str) -> Vec<AssetEntry> {
    let _ = std::fs::create_dir_all(&dir);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let matches_ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case(ext))
            .unwrap_or(false);
        if !matches_ext {
            continue;
        }
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            out.push(AssetEntry {
                name: stem.to_string(),
                path: path.display().to_string(),
            });
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn custom_asset_names(kind: &str, ext: &str) -> Vec<String> {
    scan_assets(settings::app_dir().join(kind), ext)
        .into_iter()
        .map(|a| a.name)
        .collect()
}

/// 读自定义素材并转成 data URL（避免依赖 asset 协议的作用域配置）。
#[tauri::command]
fn read_custom_asset(path: String) -> Result<String, String> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let root = std::fs::canonicalize(settings::app_dir()).map_err(|e| e.to_string())?;
    let p = std::fs::canonicalize(&path).map_err(|e| format!("文件不存在：{e}"))?;
    if !p.starts_with(&root) {
        return Err("路径越界".into());
    }
    let bytes = std::fs::read(&p).map_err(|e| format!("读取失败：{e}"))?;
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let mime = match ext.as_str() {
        "png" => "image/png",
        "webm" => "video/webm",
        _ => "application/octet-stream",
    };
    Ok(format!("data:{mime};base64,{}", STANDARD.encode(bytes)))
}

/// 用户放在 `%APPDATA%/dsh-pet-tauri/animations/*.webm` 与 `memes/*.png` 的自定义素材。
#[tauri::command]
fn get_custom_assets() -> CustomAssets {
    CustomAssets {
        animations: scan_assets(settings::app_dir().join("animations"), "webm"),
        memes: scan_assets(settings::app_dir().join("memes"), "png"),
    }
}

fn meme_catalog(memes: &std::collections::BTreeMap<String, String>) -> String {
    memes
        .iter()
        .map(|(name, desc)| format!("{name}：{desc}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// 从回复里抽 `[图:名称]`：命中池子才采纳，并把它从正文里去掉。
fn extract_image(
    reply: &str,
    memes: &std::collections::BTreeMap<String, String>,
) -> (String, Option<String>) {
    let Some(start) = reply.find("[图:") else {
        return (reply.trim().to_string(), None);
    };
    let after = start + "[图:".len();
    let Some(end_rel) = reply[after..].find(']') else {
        return (reply.trim().to_string(), None);
    };
    let end = after + end_rel;
    let name = reply[after..end].trim();
    if !memes.contains_key(name) {
        return (reply.trim().to_string(), None);
    }
    let mut cleaned = String::with_capacity(reply.len());
    cleaned.push_str(reply[..start].trim_end());
    cleaned.push_str(reply[end + 1..].trim_start());
    (cleaned.trim().to_string(), Some(name.to_string()))
}

fn notify(app: &AppHandle, title: &str, body: &str) {
    let _ = app
        .notification()
        .builder()
        .title(title)
        .body(body)
        .show();
}

fn emit_play(app: &AppHandle, name: &str) {
    let _ = app.emit(
        "pet://play",
        PlayPayload {
            name: name.into(),
        },
    );
}

fn emit_config(app: &AppHandle, state: &Arc<PetState>) {
    let _ = app.emit("pet://config", pet_config(state));
}

// ---------------------------------------------------------------------------
// 业务动作（命令与定时器共用）
// ---------------------------------------------------------------------------

async fn do_balance(app: &AppHandle, state: &Arc<PetState>) -> Result<BalanceOut, String> {
    let s = state.settings();
    let key = state
        .key()
        .ok_or_else(|| "未设置 API Key（右键 → 设置 → AI 模型）".to_string())?;
    let out = balance::fetch(&s.base_url, &key, s.balance_full).await?;
    emit_play(app, &out.anim);
    emit_say(app, &out.text, "balance", None);
    *state.last_balance.lock().unwrap() = Some(Instant::now());
    Ok(out)
}

async fn do_whisper(app: &AppHandle, state: &Arc<PetState>) -> Result<String, String> {
    let s = state.settings();
    let key = state.key().unwrap_or_default();
    let system = format!("{}\n你的名字是“{}”。", s.whisper_prompt, s.pet_name);
    let messages = vec![
        Msg::new("system", &system),
        Msg::new("user", "现在随便说一句（20 字以内），不要解释。"),
    ];
    let text = llm::complete(&s.base_url, &key, &s.model, &messages, s.temperature).await?;
    let image = if s.whisper_image_enabled {
        random_meme(&s.memes)
    } else {
        None
    };
    emit_play(app, &random_pick(&WHISPER_ANIMS));
    emit_say(app, &text, "whisper", image.as_deref());
    *state.last_whisper.lock().unwrap() = Some(Instant::now());
    Ok(text)
}

async fn do_chat(app: &AppHandle, state: &Arc<PetState>, text: &str) -> Result<String, String> {
    let s = state.settings();
    let key = state.key().unwrap_or_default();
    let system = format!("{}\n你的名字是“{}”。", s.whisper_prompt, s.pet_name);

    let history = {
        let memory = state.memory.lock().unwrap();
        memory::recent(&memory, MEMORY_BUCKET, s.chat_memory_rounds)
    };

    let mut messages = vec![Msg::new("system", &system)];
    for (role, content) in &history {
        messages.push(Msg::new(role, content));
    }
    let mut user_text = text.to_string();
    if s.chat_image_enabled && !s.memes.is_empty() {
        user_text.push_str(&format!(
            "\n\n[配图] 回复结尾可选附一张表情包给用户看，从下列清单里挑最贴合当前语境的：\n{}\n挑中就在回复最后另起一行写 [图:名称]（名称原样照抄）；没有合适的就完全不要写这个标记。",
            meme_catalog(&s.memes)
        ));
    }
    messages.push(Msg::new("user", &user_text));

    let raw = llm::complete(&s.base_url, &key, &s.model, &messages, s.temperature).await?;
    let (reply, image) = if s.chat_image_enabled {
        extract_image(&raw, &s.memes)
    } else {
        (raw.trim().to_string(), None)
    };

    {
        let mut memory = state.memory.lock().unwrap();
        memory::append(&mut memory, MEMORY_BUCKET, "user", text);
        memory::append(&mut memory, MEMORY_BUCKET, "assistant", &reply);
        memory::save(&memory);
    }

    // 窗口没聚焦时弹系统通知（用户正在看聊天面板就不打扰）
    if s.notifications_enabled {
        let focused = app
            .get_webview_window("pet")
            .and_then(|w| w.is_focused().ok())
            .unwrap_or(false);
        if !focused {
            let body: String = reply.chars().take(120).collect();
            notify(app, &format!("{} 回复了你", s.pet_name), &body);
        }
    }

    emit_say(app, &reply, "chat", image.as_deref());
    Ok(reply)
}

// ---------------------------------------------------------------------------
// 命令
// ---------------------------------------------------------------------------

#[tauri::command]
fn get_config(state: State<'_, Arc<PetState>>) -> PetConfig {
    pet_config(&state)
}

#[tauri::command]
fn get_settings(state: State<'_, Arc<PetState>>) -> SettingsView {
    let mut s = state.settings();
    s.autostart = settings::autostart_enabled();
    SettingsView::from(&s, state.key().is_some())
}

#[tauri::command]
fn save_settings(
    app: AppHandle,
    state: State<'_, Arc<PetState>>,
    mut incoming: Settings,
) -> Result<SettingsView, String> {
    let autostart_changed;
    {
        let mut s = state.settings.lock().unwrap();
        // API Key / memes / 拖拽位置只由各自专用命令写入，UI 保存不能覆盖
        incoming.api_key_enc = s.api_key_enc.clone();
        if incoming.memes.is_empty() {
            incoming.memes = s.memes.clone();
        }
        // 动画配置只由导入命令写入；物理参数允许设置页编辑（空对象 = 保留旧值）
        incoming.animations = s.animations.clone();
        if incoming
            .physics
            .as_object()
            .map(|o| o.is_empty())
            .unwrap_or(true)
        {
            incoming.physics = s.physics.clone();
        }
        incoming.animation_weights = s.animation_weights.clone();
        if incoming.pos_x.is_none() || incoming.pos_y.is_none() {
            incoming.pos_x = s.pos_x;
            incoming.pos_y = s.pos_y;
        }
        autostart_changed = incoming.autostart != s.autostart;
        *s = incoming;
    }
    if autostart_changed {
        let enabled = state.settings.lock().unwrap().autostart;
        settings::set_autostart(enabled)?;
    }
    {
        let s = state.settings.lock().unwrap();
        settings::save_settings(&s);
    }
    let s = state.settings();
    apply_pet_geometry(&app, &s)?;
    emit_config(&app, &state);
    log("settings saved");
    Ok(SettingsView::from(&s, state.key().is_some()))
}

#[tauri::command]
fn set_api_key(state: State<'_, Arc<PetState>>, key: String) -> Result<(), String> {
    let encrypted =
        secrets::encrypt_key(&key).ok_or_else(|| "API Key 加密失败（DPAPI）".to_string())?;
    {
        let mut s = state.settings.lock().unwrap();
        s.api_key_enc = encrypted;
        settings::save_settings(&s);
    }
    *state.api_key.lock().unwrap() = if key.trim().is_empty() {
        None
    } else {
        Some(key.trim().to_string())
    };
    log("api key updated");
    Ok(())
}

#[tauri::command]
fn import_dsh_key(state: State<'_, Arc<PetState>>) -> Result<(), String> {
    let key = settings::dsh_api_key()
        .ok_or_else(|| "DSH 凭证里没有 DEEPSEEK_API_KEY（$DSH_HOME/.credentials.yaml）".to_string())?;
    let encrypted =
        secrets::encrypt_key(&key).ok_or_else(|| "API Key 加密失败（DPAPI）".to_string())?;
    {
        let mut s = state.settings.lock().unwrap();
        s.api_key_enc = encrypted;
        settings::save_settings(&s);
    }
    *state.api_key.lock().unwrap() = Some(key);
    log("api key imported from DSH credentials");
    Ok(())
}

#[tauri::command]
fn clear_api_key(state: State<'_, Arc<PetState>>) -> Result<(), String> {
    {
        let mut s = state.settings.lock().unwrap();
        s.api_key_enc.clear();
        settings::save_settings(&s);
    }
    *state.api_key.lock().unwrap() = None;
    log("api key cleared");
    Ok(())
}

#[tauri::command]
async fn test_llm(state: State<'_, Arc<PetState>>) -> Result<String, String> {
    let s = state.settings();
    let key = state.key().unwrap_or_default();
    let messages = vec![Msg::new("user", "只回复两个字：pong")];
    let reply = llm::complete(&s.base_url, &key, &s.model, &messages, 0.0).await?;
    Ok(reply)
}

#[tauri::command]
async fn refresh_balance(
    app: AppHandle,
    state: State<'_, Arc<PetState>>,
) -> Result<BalanceOut, String> {
    do_balance(&app, &state).await
}

#[tauri::command]
async fn whisper_now(app: AppHandle, state: State<'_, Arc<PetState>>) -> Result<String, String> {
    do_whisper(&app, &state).await
}

#[tauri::command]
async fn chat_send(
    app: AppHandle,
    state: State<'_, Arc<PetState>>,
    text: String,
) -> Result<String, String> {
    if text.trim().is_empty() {
        return Err("消息为空".into());
    }
    do_chat(&app, &state, text.trim()).await
}

#[tauri::command]
fn get_memory(state: State<'_, Arc<PetState>>) -> serde_json::Value {
    let memory = state.memory.lock().unwrap();
    let list = memory::messages(&memory, MEMORY_BUCKET);
    serde_json::json!({
        "count": memory::count(&memory, MEMORY_BUCKET),
        "messages": list,
    })
}

#[tauri::command]
fn clear_memory(state: State<'_, Arc<PetState>>) -> Result<(), String> {
    let mut memory = state.memory.lock().unwrap();
    memory::clear(&mut memory, MEMORY_BUCKET);
    memory::save(&memory);
    log("memory cleared");
    Ok(())
}

#[tauri::command]
fn import_dsh_config(
    app: AppHandle,
    state: State<'_, Arc<PetState>>,
) -> Result<SettingsView, String> {
    let mut s = state.settings();
    if !settings::import_dsh(&mut s) {
        return Err("没有找到 DSH 配置（$DSH_HOME/dsh-pet/main-config.jsonc）".into());
    }
    settings::save_settings(&s);
    *state.settings.lock().unwrap() = s.clone();
    apply_pet_geometry(&app, &s)?;
    emit_config(&app, &state);
    log("imported DSH config");
    Ok(SettingsView::from(&s, state.key().is_some()))
}

#[tauri::command]
fn open_settings(app: AppHandle) -> Result<(), String> {
    open_settings_window(&app)
}

#[tauri::command]
fn close_settings(app: AppHandle) {
    if let Some(w) = app.get_webview_window("settings") {
        let _ = w.hide();
    }
}

#[tauri::command]
fn toggle_always_on_top(app: AppHandle, state: State<'_, Arc<PetState>>) -> Result<bool, String> {
    let mut s = state.settings.lock().unwrap();
    s.always_on_top = !s.always_on_top;
    let value = s.always_on_top;
    settings::save_settings(&s);
    drop(s);
    if let Some(w) = app.get_webview_window("pet") {
        let _ = w.set_always_on_top(value);
    }
    Ok(value)
}

#[tauri::command]
fn quit(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
fn report_hitbox(state: State<'_, Arc<PetState>>, x: f64, y: f64, w: f64, h: f64) {
    *state.hitbox.lock().unwrap() = Some(HitBox { x, y, w, h });
}

/// 拖拽结束后把当前窗口位置记下来（逻辑像素），下次启动回到这里。
#[tauri::command]
fn save_position(window: WebviewWindow, state: State<'_, Arc<PetState>>) -> Result<(), String> {
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let pos = window.outer_position().map_err(|e| e.to_string())?;
    let mut s = state.settings.lock().unwrap();
    s.pos_x = Some(pos.x as f64 / scale);
    s.pos_y = Some(pos.y as f64 / scale);
    settings::save_settings(&s);
    Ok(())
}

#[tauri::command]
fn reset_position(
    app: AppHandle,
    state: State<'_, Arc<PetState>>,
) -> Result<(), String> {
    {
        let mut s = state.settings.lock().unwrap();
        s.pos_x = None;
        s.pos_y = None;
        settings::save_settings(&s);
    }
    let s = state.settings();
    apply_pet_geometry(&app, &s)
}

#[tauri::command]
fn open_app_dir() -> Result<(), String> {
    std::process::Command::new("explorer")
        .arg(settings::app_dir())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("打开文件夹失败：{e}"))
}

#[tauri::command]
fn export_memory(state: State<'_, Arc<PetState>>) -> Result<String, String> {
    let memory = state.memory.lock().unwrap();
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let path = settings::app_dir().join(format!("memory-export-{millis}.json"));
    let text = serde_json::to_string_pretty(&*memory).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("写导出文件失败：{e}"))?;
    Ok(path.display().to_string())
}

#[tauri::command]
fn set_drag(state: State<'_, Arc<PetState>>, active: bool) {
    state.dragging.store(active, Ordering::SeqCst);
}

#[tauri::command]
fn set_ui_busy(state: State<'_, Arc<PetState>>, active: bool) {
    state.ui_busy.store(active, Ordering::SeqCst);
}

#[tauri::command]
fn focus_pet(window: WebviewWindow) {
    let _ = window.set_focus();
}

#[tauri::command]
fn move_by(window: WebviewWindow, dx: f64, dy: f64) -> Result<(), String> {
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let pos = window.outer_position().map_err(|e| e.to_string())?;
    let nx = pos.x as f64 + dx * scale;
    let ny = pos.y as f64 + dy * scale;
    window
        .set_position(PhysicalPosition::new(nx.round() as i32, ny.round() as i32))
        .map_err(|e| e.to_string())
}

#[derive(Serialize)]
struct RectF {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

#[derive(Serialize)]
struct WindowState {
    x: f64,
    y: f64,
    scale: f64,
    width: f64,
    height: f64,
    /// 完整显示器矩形（含任务栏）
    monitor: RectF,
    /// 工作区（排除任务栏）
    work: RectF,
}

/// 给渲染层的窗口/屏幕几何（逻辑像素）：自主走路和抛掷物理都要用它算边界。
#[tauri::command]
fn get_window_state(window: WebviewWindow) -> Result<WindowState, String> {
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let pos = window.outer_position().map_err(|e| e.to_string())?;
    let size = window.outer_size().map_err(|e| e.to_string())?;
    let monitor = window
        .current_monitor()
        .map_err(|e| e.to_string())?
        .or(window.primary_monitor().map_err(|e| e.to_string())?);
    let (monitor_rect, work_rect, width, height) = if let Some(m) = monitor {
        let ms = m.scale_factor();
        let area = m.work_area();
        (
            RectF {
                x: m.position().x as f64 / ms,
                y: m.position().y as f64 / ms,
                w: m.size().width as f64 / ms,
                h: m.size().height as f64 / ms,
            },
            RectF {
                x: area.position.x as f64 / ms,
                y: area.position.y as f64 / ms,
                w: area.size.width as f64 / ms,
                h: area.size.height as f64 / ms,
            },
            size.width as f64 / scale,
            size.height as f64 / scale,
        )
    } else {
        (
            RectF { x: 0.0, y: 0.0, w: 1920.0, h: 1080.0 },
            RectF { x: 0.0, y: 0.0, w: 1920.0, h: 1080.0 },
            size.width as f64 / scale,
            size.height as f64 / scale,
        )
    };
    Ok(WindowState {
        x: pos.x as f64 / scale,
        y: pos.y as f64 / scale,
        scale,
        width,
        height,
        monitor: monitor_rect,
        work: work_rect,
    })
}

#[tauri::command]
fn set_window_pos(window: WebviewWindow, x: f64, y: f64) -> Result<(), String> {
    window
        .set_position(tauri::LogicalPosition::new(x, y))
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn ready(window: WebviewWindow) -> Result<(), String> {
    window.show().map_err(|e| e.to_string())?;
    log("window shown");
    Ok(())
}

#[tauri::command]
fn js_log(msg: String) {
    log(&format!("js: {msg}"));
}

// ---------------------------------------------------------------------------
// 窗口 / 托盘 / 轮询
// ---------------------------------------------------------------------------

fn apply_pet_geometry(app: &AppHandle, s: &Settings) -> Result<(), String> {
    let Some(win) = app.get_webview_window("pet") else {
        return Ok(());
    };
    let size = s.pet_size;
    let stage_h = size * CANVAS_H / CANVAS_W;
    // 与 Electron 版一致：窗口 = 宠物 + 四周各半只宠物的余量（给气泡/菜单/聊天框留位置）
    let win_w = size * 2.0;
    let win_h = stage_h + size;
    win.set_size(tauri::LogicalSize::new(win_w, win_h))
        .map_err(|e| e.to_string())?;

    if let Ok(Some(monitor)) = win.primary_monitor() {
        let sf = monitor.scale_factor();
        let mw = monitor.size().width as f64 / sf;
        let mh = monitor.size().height as f64 / sf;
        let mx = monitor.position().x as f64 / sf;
        let my = monitor.position().y as f64 / sf;
        let (x, y) = if let (Some(px), Some(py)) = (s.pos_x, s.pos_y) {
            // 用户拖过的位置：限制在屏幕内再用
            (
                px.clamp(mx, (mx + mw - win_w).max(mx)),
                py.clamp(my, (my + mh - win_h).max(my)),
            )
        } else {
            let mut x = mx + mw - win_w - s.margin_x;
            let mut y = my + s.margin_y;
            if s.corner.contains("left") {
                x = mx + s.margin_x;
            }
            if s.corner.contains("bottom") {
                y = my + mh - win_h - s.margin_y;
            }
            (x.max(mx), y.max(my))
        };
        win.set_position(tauri::LogicalPosition::new(x, y))
            .map_err(|e| e.to_string())?;
    }
    win.set_always_on_top(s.always_on_top)
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn open_settings_window(app: &AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("settings") {
        let _ = w.show();
        let _ = w.set_focus();
        return Ok(());
    }
    // 兜底：理论上配置里已经建好 settings 窗口，这里只处理被销毁的极端情况
    tauri::WebviewWindowBuilder::new(
        app,
        "settings",
        tauri::WebviewUrl::App("settings.html".into()),
    )
    .title("dsh-pet 设置")
    .inner_size(800.0, 720.0)
    .center()
    .build()
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let settings_item = MenuItem::with_id(app, "settings", "设置…", true, None::<&str>)?;
    let toggle = MenuItem::with_id(app, "toggle", "显示 / 隐藏", true, None::<&str>)?;
    let exit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&settings_item, &toggle, &exit])?;
    let mut builder = TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "settings" => {
                let _ = open_settings_window(app);
            }
            "toggle" => {
                if let Some(w) = app.get_webview_window("pet") {
                    if w.is_visible().unwrap_or(false) {
                        let _ = w.hide();
                    } else {
                        let _ = w.show();
                    }
                }
            }
            "quit" => app.exit(0),
            _ => {}
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

fn start_pointer_poll(window: WebviewWindow, state: Arc<PetState>) {
    std::thread::spawn(move || {
        let mut last_left_down = Instant::now();
        let mut last_ignore = true;
        loop {
            std::thread::sleep(Duration::from_millis(POLL_MS));
            if !window.is_visible().unwrap_or(false) {
                continue;
            }
            let left_down = left_button_down();
            if left_down {
                last_left_down = Instant::now();
            }
            let Some((cx, cy)) = cursor_pos() else { continue };
            let Ok(pos) = window.outer_position() else { continue };
            let Ok(scale) = window.scale_factor() else { continue };

            let lx = (cx - pos.x as f64) / scale;
            let ly = (cy - pos.y as f64) / scale;
            let mut dragging = state.dragging.load(Ordering::SeqCst);
            let ui_busy = state.ui_busy.load(Ordering::SeqCst);

            if dragging && !left_down && last_left_down.elapsed() > Duration::from_millis(220) {
                state.dragging.store(false, Ordering::SeqCst);
                dragging = false;
                let _ = window.emit("pet://drag-cancel", ());
            }
            let busy = dragging || ui_busy;

            let hit = *state.hitbox.lock().unwrap();
            let on_body = hit
                .map(|h| lx >= h.x && lx <= h.x + h.w && ly >= h.y && ly <= h.y + h.h)
                .unwrap_or(false);
            let ignore = !(busy || on_body);

            if ignore != last_ignore {
                let _ = window.set_ignore_cursor_events(ignore);
                last_ignore = ignore;
                log(&format!(
                    "ignore={ignore} busy={busy} on_body={on_body} local=({lx:.0},{ly:.0})"
                ));
            }
        }
    });
}

fn start_topmost_watchdog(window: WebviewWindow, state: Arc<PetState>) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(2));
        if !window.is_visible().unwrap_or(false) {
            continue;
        }
        if state.settings().always_on_top {
            // Windows 上别的置顶窗口 / 全屏应用可能把我们从置顶带挤下来；定期重设一次
            let _ = window.set_always_on_top(true);
        }
    });
}

fn spawn_timers(app: AppHandle, state: Arc<PetState>) {
    // 余额
    let a1 = app.clone();
    let s1 = state.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(20));
        let cfg = s1.settings();
        if !cfg.balance_enabled || s1.key().is_none() {
            continue;
        }
        let due = s1
            .last_balance
            .lock()
            .unwrap()
            .map(|t| t.elapsed().as_secs() >= cfg.balance_interval_sec)
            .unwrap_or(false);
        if !due {
            continue;
        }
        *s1.last_balance.lock().unwrap() = Some(Instant::now());
        if let Err(e) = tauri::async_runtime::block_on(do_balance(&a1, &s1)) {
            log(&format!("auto balance failed: {e}"));
        }
    });

    // 碎碎念
    let a2 = app.clone();
    let s2 = state.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(15));
        let cfg = s2.settings();
        if !cfg.whisper_enabled {
            continue;
        }
        let due = s2
            .last_whisper
            .lock()
            .unwrap()
            .map(|t| t.elapsed().as_secs() >= cfg.whisper_interval_sec)
            .unwrap_or(false);
        if !due {
            continue;
        }
        *s2.last_whisper.lock().unwrap() = Some(Instant::now());
        if let Err(e) = tauri::async_runtime::block_on(do_whisper(&a2, &s2)) {
            log(&format!("auto whisper failed: {e}"));
        }
    });
}

#[cfg(windows)]
#[repr(C)]
struct Point {
    x: i32,
    y: i32,
}
#[cfg(windows)]
#[link(name = "user32")]
extern "system" {
    fn GetCursorPos(p: *mut Point) -> i32;
    fn GetAsyncKeyState(v_key: i32) -> i16;
}
#[cfg(windows)]
fn cursor_pos() -> Option<(f64, f64)> {
    let mut p = Point { x: 0, y: 0 };
    if unsafe { GetCursorPos(&mut p) } != 0 {
        Some((p.x as f64, p.y as f64))
    } else {
        None
    }
}
#[cfg(windows)]
fn left_button_down() -> bool {
    (unsafe { GetAsyncKeyState(0x01) } as u16 & 0x8000) != 0
}
#[cfg(not(windows))]
fn cursor_pos() -> Option<(f64, f64)> {
    None
}
#[cfg(not(windows))]
fn left_button_down() -> bool {
    false
}

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn CreateMutexW(
        lp_mutex_attributes: *const std::ffi::c_void,
        b_initial_owner: i32,
        lp_name: *const u16,
    ) -> *mut std::ffi::c_void;
    fn GetLastError() -> u32;
}

/// 单实例：同用户下只允许一个桌宠进程（第二个直接退出）。
fn ensure_single_instance() {
    #[cfg(windows)]
    unsafe {
        let name: Vec<u16> = "Local\\dsh-pet-tauri-single\0".encode_utf16().collect();
        let handle = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
        if !handle.is_null() && GetLastError() == 183 {
            // ERROR_ALREADY_EXISTS
            std::process::exit(0);
        }
        // 句柄故意不 CloseHandle：进程存活期间互斥体要一直在
        let _ = handle;
    }
}

fn main() {
    ensure_single_instance();
    let loaded = settings::load_settings();
    let state = Arc::new(PetState {
        settings: Mutex::new(loaded),
        memory: Mutex::new(memory::load()),
        api_key: Mutex::new(None),
        hitbox: Mutex::new(None),
        dragging: AtomicBool::new(false),
        ui_busy: AtomicBool::new(false),
        last_balance: Mutex::new(Some(Instant::now())),
        last_whisper: Mutex::new(Some(Instant::now())),
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![
            get_config,
            get_settings,
            save_settings,
            set_api_key,
            import_dsh_key,
            clear_api_key,
            test_llm,
            refresh_balance,
            whisper_now,
            chat_send,
            get_memory,
            clear_memory,
            import_dsh_config,
            open_settings,
            close_settings,
            toggle_always_on_top,
            quit,
            report_hitbox,
            set_drag,
            set_ui_busy,
            focus_pet,
            move_by,
            get_window_state,
            set_window_pos,
            save_position,
            reset_position,
            export_memory,
            open_app_dir,
            get_custom_assets,
            read_custom_asset,
            ready,
            js_log
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            let cfg = state.settings();
            log(&format!(
                "startup: base_url={} model={} pet={} size={}",
                cfg.base_url, cfg.model, cfg.pet_name, cfg.pet_size
            ));
            if let Some(window) = app.get_webview_window("pet") {
                apply_pet_geometry(&handle, &cfg)?;
                start_pointer_poll(window.clone(), state.clone());
                start_topmost_watchdog(window.clone(), state.clone());
            }
            build_tray(&handle)?;
            spawn_timers(handle, state.clone());
            let _ = state.key(); // 预热 DPAPI 解密
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "pet" || window.label() == "settings" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
