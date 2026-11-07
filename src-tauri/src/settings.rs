//! 设置、路径、DSH 配置导入。
//!
//! 独立桌宠不依赖 DSH：设置写在 `%APPDATA%/dsh-pet-tauri/settings.json`；
//! 首次运行时尝试从 `$DSH_HOME/dsh-pet/main-config.jsonc` 导入一份初始值（只读，不写回）。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;

pub const APP_DIR_NAME: &str = "dsh-pet-tauri";

/// 内置的完整动画表/物理参数（从 DSH 默认配置抽出来的 JSON）
fn default_runtime() -> Value {
    serde_json::from_str::<Value>(include_str!("default-runtime.json"))
        .unwrap_or_else(|_| serde_json::json!({}))
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// OpenAI 兼容端点，例如 https://api.deepseek.com 或 https://api.openai.com/v1
    pub base_url: String,
    pub model: String,
    pub temperature: f64,
    pub pet_name: String,
    pub pet_size: f64,
    pub corner: String,
    pub margin_x: f64,
    pub margin_y: f64,
    pub always_on_top: bool,
    pub boundary_mode: String,
    pub whisper_prompt: String,
    pub chat_memory_rounds: u32,
    pub whisper_enabled: bool,
    pub whisper_interval_sec: u64,
    pub balance_enabled: bool,
    pub balance_interval_sec: u64,
    /// 余额档位基准（DeepSeek 官方口径 ¥20）
    pub balance_full: f64,
    /// 碎碎念随机配图 / 对话让模型选配图
    pub whisper_image_enabled: bool,
    pub chat_image_enabled: bool,
    /// 表情包：键 = 图片名（不含扩展名），值 = 给模型看的描述
    pub memes: BTreeMap<String, String>,
    /// 完整动画配置（idle/turn/drag/clicks/moves/categories/events）——直接沿用 DSH 的结构
    pub animations: Value,
    /// 拖拽抛掷物理参数（gravity/restitution/groundFriction/throwPower…）
    pub physics: Value,
    /// 随机链权重（idle/turn/move）
    pub animation_weights: Value,
    /// 拖动后的自定义位置（逻辑像素；None = 按 corner/margin 停靠）
    pub pos_x: Option<f64>,
    pub pos_y: Option<f64>,
    /// 开机自启（注册表 HKCU Run）
    pub autostart: bool,
    /// 窗口未聚焦时回消息弹系统通知
    pub notifications_enabled: bool,
    /// DPAPI 保护后的 API Key（hex），空 = 未设置
    pub api_key_enc: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            base_url: "https://api.deepseek.com".into(),
            model: "deepseek-flash".into(),
            temperature: 1.0,
            pet_name: "蓝毛小女仆".into(),
            pet_size: 462.0,
            corner: "top-right".into(),
            margin_x: 24.0,
            margin_y: 100.0,
            always_on_top: true,
            boundary_mode: "screen".into(),
            whisper_prompt:
                "你是主人桌面上的Q版蓝发小女仆，会时不时碎碎念一句。说话要自然随意、短短一句（20字以内），温柔乖巧带点俏皮，说人话不啰嗦，不要解释你自己，不要提你是AI。"
                    .into(),
            chat_memory_rounds: 5,
            whisper_enabled: true,
            whisper_interval_sec: 300,
            balance_enabled: true,
            balance_interval_sec: 1800,
            balance_full: 20.0,
            whisper_image_enabled: true,
            chat_image_enabled: true,
            memes: BTreeMap::new(),
            animations: default_runtime().get("animations").cloned().unwrap_or_default(),
            physics: default_runtime().get("physics").cloned().unwrap_or_default(),
            animation_weights: default_runtime()
                .get("animationWeights")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({"idle": 10, "turn": 5, "move": 5})),
            pos_x: None,
            pos_y: None,
            autostart: false,
            notifications_enabled: true,
            api_key_enc: String::new(),
        }
    }
}

pub fn app_dir() -> PathBuf {
    let base = std::env::var("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir());
    let dir = base.join(APP_DIR_NAME);
    let _ = std::fs::create_dir_all(&dir);
    dir
}

pub fn settings_path() -> PathBuf {
    app_dir().join("settings.json")
}

pub fn memory_path() -> PathBuf {
    app_dir().join("memory.json")
}

pub fn dsh_home() -> PathBuf {
    if let Ok(h) = std::env::var("DSH_HOME") {
        if !h.trim().is_empty() {
            return PathBuf::from(h);
        }
    }
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    PathBuf::from(home).join(".dsh")
}

pub fn load_settings() -> Settings {
    let path = settings_path();
    if let Ok(text) = std::fs::read_to_string(&path) {
        // 兼容 PowerShell 5.1 `Set-Content -Encoding UTF8` 写出的 BOM
        let text = text.trim_start_matches('\u{feff}');
        if let Ok(mut s) = serde_json::from_str::<Settings>(text) {
            let mut changed = false;
            // 注册表是自启的真实来源，避免设置文件与系统状态漂移
            let real_autostart = autostart_enabled();
            if s.autostart != real_autostart {
                s.autostart = real_autostart;
                changed = true;
            }
            // 老设置没有 memes / animations：从 DSH 配置补一份
            if s.memes.is_empty() {
                let memes = dsh_memes();
                if !memes.is_empty() {
                    s.memes = memes;
                    changed = true;
                }
            }
            if s.animations.as_object().map(|o| o.is_empty()).unwrap_or(true) {
                let imported = import_dsh_animations();
                s.animations = imported.0;
                s.physics = imported.1;
                s.animation_weights = imported.2;
                changed = true;
            }
            // 插件升级（0.4.5）新增 sleep / baifan 动画池：旧设置里没有就整段换成新版内置动画表，
            // 物理参数保持用户自己的设置不动。
            if s.animations.get("sleep").is_none() || s.animations.get("baifan").is_none() {
                let defaults = default_runtime();
                if let Some(anims) = defaults.get("animations") {
                    s.animations = anims.clone();
                    if let Some(w) = defaults.get("animationWeights") {
                        s.animation_weights = w.clone();
                    }
                    changed = true;
                }
            }
            if changed {
                save_settings(&s);
            }
            return s;
        }
    }
    // 首次运行：用 DSH 的配置做一轮导入，再立刻落盘
    let mut settings = Settings::default();
    import_dsh(&mut settings);
    settings.autostart = autostart_enabled();
    save_settings(&settings);
    settings
}

pub fn save_settings(settings: &Settings) {
    if let Ok(text) = serde_json::to_string_pretty(settings) {
        let _ = std::fs::write(settings_path(), text);
    }
}

/// 从 DSH 的 `.credentials.yaml` 里读 `refs.DEEPSEEK_API_KEY`（只读）。
/// 文件是 YAML，但这里只需要一行，避免为它引入 YAML 依赖。
pub fn dsh_api_key() -> Option<String> {
    let path = dsh_home().join(".credentials.yaml");
    let text = std::fs::read_to_string(path).ok()?;
    for line in text.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("DEEPSEEK_API_KEY:") else {
            continue;
        };
        let value = rest
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .trim()
            .to_string();
        if value.starts_with("sk-") {
            return Some(value);
        }
    }
    None
}

/// 从 DSH 的 `main-config.jsonc` 读 `memes` 映射（键 = 图片名，值 = 描述）。
pub fn dsh_memes() -> BTreeMap<String, String> {
    let path = dsh_home().join("dsh-pet").join("main-config.jsonc");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return BTreeMap::new();
    };
    let Ok(doc) = json5::from_str::<serde_json::Value>(&text) else {
        return BTreeMap::new();
    };
    doc.get("memes")
        .and_then(|v| v.as_object())
        .map(|obj| {
            obj.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

/// 从 DSH 配置读完整的动画/物理/权重；缺省回落内置默认。
pub fn import_dsh_animations() -> (Value, Value, Value) {
    let fallback = default_runtime();
    let path = dsh_home().join("dsh-pet").join("main-config.jsonc");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return (
            fallback.get("animations").cloned().unwrap_or_default(),
            fallback.get("physics").cloned().unwrap_or_default(),
            fallback.get("animationWeights").cloned().unwrap_or_default(),
        );
    };
    let Ok(doc) = json5::from_str::<Value>(&text) else {
        return (
            fallback.get("animations").cloned().unwrap_or_default(),
            fallback.get("physics").cloned().unwrap_or_default(),
            fallback.get("animationWeights").cloned().unwrap_or_default(),
        );
    };
    let pick = |key: &str, fb: &str| {
        doc.get(key)
            .cloned()
            .or_else(|| fallback.get(fb).cloned())
            .unwrap_or_default()
    };
    (
        pick("animations", "animations"),
        pick("physics", "physics"),
        pick("animationWeights", "animationWeights"),
    )
}

/// 从 DSH 的 `main-config.jsonc` 导入尺寸/位置/人设/间隔。返回是否找到配置文件。
pub fn import_dsh(settings: &mut Settings) -> bool {
    let path = dsh_home().join("dsh-pet").join("main-config.jsonc");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return false;
    };
    let Ok(doc) = json5::from_str::<serde_json::Value>(&text) else {
        return false;
    };

    if let Some(p) = doc
        .get("pets")
        .and_then(|x| x.as_array())
        .and_then(|a| a.first())
    {
        if let Some(v) = p.get("name").and_then(|v| v.as_str()) {
            if !v.trim().is_empty() {
                settings.pet_name = v.into();
            }
        }
        if let Some(v) = p.get("size").and_then(|v| v.as_f64()) {
            if v > 40.0 {
                settings.pet_size = v;
            }
        }
        if let Some(pos) = p.get("position") {
            if let Some(v) = pos.get("corner").and_then(|v| v.as_str()) {
                settings.corner = v.into();
            }
            if let Some(v) = pos.get("marginX").and_then(|v| v.as_f64()) {
                settings.margin_x = v;
            }
            if let Some(v) = pos.get("marginY").and_then(|v| v.as_f64()) {
                settings.margin_y = v;
            }
        }
    }
    if let Some(v) = doc.get("whisperPrompt").and_then(|v| v.as_str()) {
        if !v.trim().is_empty() {
            settings.whisper_prompt = v.into();
        }
    }
    if let Some(v) = doc.get("chatMemoryRounds").and_then(|v| v.as_u64()) {
        settings.chat_memory_rounds = v.min(50) as u32;
    }
    if let Some(v) = doc.get("whisperImageEnabled").and_then(|v| v.as_bool()) {
        settings.whisper_image_enabled = v;
    }
    if let Some(v) = doc.get("chatImageEnabled").and_then(|v| v.as_bool()) {
        settings.chat_image_enabled = v;
    }
    let memes = dsh_memes();
    if !memes.is_empty() {
        settings.memes = memes;
    }
    let (animations, physics, weights) = import_dsh_animations();
    settings.animations = animations;
    settings.physics = physics;
    settings.animation_weights = weights;
    if let Some(ev) = doc.get("eventsRefreshSec") {
        if let Some(v) = ev.get("balance").and_then(|v| v.as_u64()) {
            settings.balance_interval_sec = v.max(60);
        }
        if let Some(v) = ev.get("whisper").and_then(|v| v.as_u64()) {
            settings.whisper_interval_sec = v.max(30);
        }
    }
    // DSH 里的模型别名与官方直连模型的对应（当前官方聊天模型是 deepseek-flash）
    settings.base_url = "https://api.deepseek.com".into();
    settings.model = "deepseek-flash".into();
    true
}

/// 开机自启：写/删 `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`。
pub fn set_autostart(enabled: bool) -> Result<(), String> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const VALUE: &str = "dsh-pet-tauri";

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu
        .create_subkey(RUN_KEY)
        .map_err(|e| format!("打开注册表 Run 键失败：{e}"))?;
    if enabled {
        let exe = std::env::current_exe().map_err(|e| format!("取 exe 路径失败：{e}"))?;
        let command = format!("\"{}\"", exe.display());
        key.set_value(VALUE, &command)
            .map_err(|e| format!("写自启注册表失败：{e}"))?;
    } else {
        let _ = key.delete_value(VALUE);
    }
    Ok(())
}

pub fn autostart_enabled() -> bool {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const VALUE: &str = "dsh-pet-tauri";

    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(RUN_KEY)
        .and_then(|key| key.get_value::<String, _>(VALUE))
        .is_ok()
}

/// 给前端看的设置视图：不返回密文，只返回「有没有设置 key」。
#[derive(Serialize)]
pub struct SettingsView {
    pub base_url: String,
    pub model: String,
    pub temperature: f64,
    pub pet_name: String,
    pub pet_size: f64,
    pub corner: String,
    pub margin_x: f64,
    pub margin_y: f64,
    pub always_on_top: bool,
    pub boundary_mode: String,
    pub whisper_prompt: String,
    pub chat_memory_rounds: u32,
    pub whisper_enabled: bool,
    pub whisper_interval_sec: u64,
    pub balance_enabled: bool,
    pub balance_interval_sec: u64,
    pub balance_full: f64,
    pub whisper_image_enabled: bool,
    pub chat_image_enabled: bool,
    pub meme_count: usize,
    pub pos_set: bool,
    pub autostart: bool,
    pub notifications_enabled: bool,
    /// 抛掷/弹性参数（settings 页可编辑）
    pub physics: Value,
    pub api_key_set: bool,
    pub settings_path: String,
    pub memory_path: String,
    pub version: String,
}

impl SettingsView {
    pub fn from(settings: &Settings, key_set: bool) -> Self {
        Self {
            base_url: settings.base_url.clone(),
            model: settings.model.clone(),
            temperature: settings.temperature,
            pet_name: settings.pet_name.clone(),
            pet_size: settings.pet_size,
            corner: settings.corner.clone(),
            margin_x: settings.margin_x,
            margin_y: settings.margin_y,
            always_on_top: settings.always_on_top,
            boundary_mode: settings.boundary_mode.clone(),
            whisper_prompt: settings.whisper_prompt.clone(),
            chat_memory_rounds: settings.chat_memory_rounds,
            whisper_enabled: settings.whisper_enabled,
            whisper_interval_sec: settings.whisper_interval_sec,
            balance_enabled: settings.balance_enabled,
            balance_interval_sec: settings.balance_interval_sec,
            balance_full: settings.balance_full,
            whisper_image_enabled: settings.whisper_image_enabled,
            chat_image_enabled: settings.chat_image_enabled,
            meme_count: settings.memes.len(),
            pos_set: settings.pos_x.is_some() && settings.pos_y.is_some(),
            autostart: settings.autostart,
            notifications_enabled: settings.notifications_enabled,
            physics: settings.physics.clone(),
            api_key_set: key_set,
            settings_path: settings_path().display().to_string(),
            memory_path: memory_path().display().to_string(),
            version: env!("CARGO_PKG_VERSION").into(),
        }
    }
}
