use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};
use url::Url;

const DEFAULT_PROFILE_ID: &str = "local";
const DEFAULT_PROFILE_NAME: &str = "本地 Ourea";
const DEFAULT_PROFILE_URL: &str = "http://127.0.0.1:8008";
const LAUNCHER_WINDOW_LABEL: &str = "main";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OureaProfile {
    id: String,
    name: String,
    url: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WindowPlacement {
    width: Option<f64>,
    height: Option<f64>,
    x: Option<f64>,
    y: Option<f64>,
    maximized: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DesktopConfig {
    #[serde(default)]
    profiles: Vec<OureaProfile>,
    active_profile_id: Option<String>,
    #[serde(default)]
    theme_mode: String,
    #[serde(default)]
    window_states: HashMap<String, WindowPlacement>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyDesktopConfig {
    ourea_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProfileInput {
    id: Option<String>,
    name: String,
    url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppSnapshot {
    profiles: Vec<OureaProfile>,
    active_profile_id: Option<String>,
    theme_mode: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConnectionCheck {
    ok: bool,
    message: String,
    health_url: String,
}

struct DesktopState {
    config: Mutex<DesktopConfig>,
}

#[tauri::command]
fn get_state(state: tauri::State<'_, DesktopState>) -> AppSnapshot {
    snapshot(&state)
}

#[tauri::command]
fn upsert_profile(
    app: AppHandle,
    state: tauri::State<'_, DesktopState>,
    input: ProfileInput,
) -> Result<AppSnapshot, String> {
    let normalized_url = normalize_ourea_url(&input.url)?;
    let name = normalize_profile_name(&input.name, &normalized_url);
    let id = input.id.unwrap_or_else(generate_profile_id);

    {
        let mut config = state
            .config
            .lock()
            .map_err(|_| "failed to lock desktop config".to_string())?;

        if let Some(profile) = config.profiles.iter_mut().find(|profile| profile.id == id) {
            profile.name = name;
            profile.url = normalized_url;
        } else {
            config.profiles.push(OureaProfile {
                id: id.clone(),
                name,
                url: normalized_url,
            });
        }

        if config.active_profile_id.is_none() {
            config.active_profile_id = Some(id);
        }

        save_config(&app, &config)?;
    }

    Ok(snapshot(&state))
}

#[tauri::command]
fn delete_profile(
    app: AppHandle,
    state: tauri::State<'_, DesktopState>,
    profile_id: String,
) -> Result<AppSnapshot, String> {
    {
        let mut config = state
            .config
            .lock()
            .map_err(|_| "failed to lock desktop config".to_string())?;
        let active_before = config.active_profile_id.as_deref() == Some(profile_id.as_str());

        config.profiles.retain(|profile| profile.id != profile_id);
        if active_before {
            config.active_profile_id = config.profiles.first().map(|profile| profile.id.clone());
        }

        save_config(&app, &config)?;
    }

    Ok(snapshot(&state))
}

#[tauri::command]
fn test_profile(
    state: tauri::State<'_, DesktopState>,
    profile_id: String,
) -> Result<ConnectionCheck, String> {
    let profile = {
        let config = state
            .config
            .lock()
            .map_err(|_| "failed to lock desktop config".to_string())?;
        find_profile(&config, &profile_id)
            .ok_or_else(|| "未找到这个 Ourea 地址配置".to_string())?
            .clone()
    };

    Ok(check_ourea_health(&profile.url))
}

#[tauri::command]
fn test_ourea_url(url: String) -> Result<ConnectionCheck, String> {
    let normalized_url = normalize_ourea_url(&url)?;
    Ok(check_ourea_health(&normalized_url))
}

#[tauri::command]
fn set_theme(
    app: AppHandle,
    state: tauri::State<'_, DesktopState>,
    theme_mode: String,
) -> Result<AppSnapshot, String> {
    if !matches!(theme_mode.as_str(), "system" | "light" | "dark") {
        return Err("未知主题模式".to_string());
    }

    {
        let mut config = state
            .config
            .lock()
            .map_err(|_| "failed to lock desktop config".to_string())?;
        config.theme_mode = theme_mode;
        save_config(&app, &config)?;
    }

    Ok(snapshot(&state))
}

#[tauri::command]
fn activate_profile(
    app: AppHandle,
    state: tauri::State<'_, DesktopState>,
    profile_id: String,
) -> Result<AppSnapshot, String> {
    let profile = {
        let mut config = state
            .config
            .lock()
            .map_err(|_| "failed to lock desktop config".to_string())?;
        let profile = find_profile(&config, &profile_id)
            .ok_or_else(|| "未找到这个 Ourea 地址配置".to_string())?
            .clone();
        config.active_profile_id = Some(profile_id);
        save_config(&app, &config)?;
        profile
    };

    let check = check_ourea_health(&profile.url);
    if check.ok {
        Ok(snapshot(&state))
    } else {
        Err(check.message)
    }
}

#[tauri::command]
fn window_action(app: AppHandle, label: String, action: String) -> Result<(), String> {
    let target_label = match label.as_str() {
        LAUNCHER_WINDOW_LABEL => LAUNCHER_WINDOW_LABEL,
        _ => return Err("未知窗口".to_string()),
    };
    let target_window = app
        .get_webview_window(target_label)
        .ok_or_else(|| "未找到当前窗口".to_string())?;

    match action.as_str() {
        "minimize" => target_window.minimize().map_err(|error| error.to_string()),
        "maximize" => if target_window.is_maximized().unwrap_or(false) {
            target_window.unmaximize()
        } else {
            target_window.maximize()
        }
        .map_err(|error| error.to_string()),
        "close" => target_window.close().map_err(|error| error.to_string()),
        _ => Err("未知窗口操作".to_string()),
    }
}

fn snapshot(state: &tauri::State<'_, DesktopState>) -> AppSnapshot {
    let config = state
        .config
        .lock()
        .map(|config| config.clone())
        .unwrap_or_default();

    AppSnapshot {
        profiles: config.profiles,
        active_profile_id: config.active_profile_id,
        theme_mode: if config.theme_mode.is_empty() {
            "system".to_string()
        } else {
            config.theme_mode
        },
    }
}

fn find_profile<'a>(config: &'a DesktopConfig, profile_id: &str) -> Option<&'a OureaProfile> {
    config
        .profiles
        .iter()
        .find(|profile| profile.id == profile_id)
}

fn default_profile() -> OureaProfile {
    OureaProfile {
        id: DEFAULT_PROFILE_ID.to_string(),
        name: DEFAULT_PROFILE_NAME.to_string(),
        url: DEFAULT_PROFILE_URL.to_string(),
    }
}

fn normalize_profile_name(input: &str, url: &str) -> String {
    let name = input.trim();
    if !name.is_empty() {
        return name.to_string();
    }

    Url::parse(url)
        .ok()
        .and_then(|url| url.host_str().map(ToString::to_string))
        .unwrap_or_else(|| "Ourea 工作台".to_string())
}

fn generate_profile_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("profile-{millis}")
}

fn normalize_ourea_url(input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("请输入 Ourea 服务地址".to_string());
    }

    let candidate = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };

    let mut parsed = Url::parse(&candidate).map_err(|_| "请输入有效的 URL".to_string())?;
    match parsed.scheme() {
        "http" | "https" => {}
        _ => return Err("只支持 http 或 https 地址".to_string()),
    }

    if parsed.host_str().is_none() {
        return Err("URL 缺少主机名".to_string());
    }

    parsed.set_fragment(None);
    Ok(parsed.to_string().trim_end_matches('/').to_string())
}

fn check_ourea_health(ourea_url: &str) -> ConnectionCheck {
    let health_url = match Url::parse(ourea_url).and_then(|url| url.join("/api/health")) {
        Ok(url) => url,
        Err(error) => {
            return ConnectionCheck {
                ok: false,
                message: format!("健康检查地址无效：{error}"),
                health_url: ourea_url.to_string(),
            };
        }
    };

    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            return ConnectionCheck {
                ok: false,
                message: format!("健康检查客户端初始化失败：{error}"),
                health_url: health_url.to_string(),
            };
        }
    };

    let response = match client.get(health_url.clone()).send() {
        Ok(response) => response,
        Err(error) => {
            return ConnectionCheck {
                ok: false,
                message: format!("无法连接到 Ourea：{error}"),
                health_url: health_url.to_string(),
            };
        }
    };

    let status = response.status();
    if !status.is_success() {
        return ConnectionCheck {
            ok: false,
            message: format!("健康检查失败：{} 返回 HTTP {}", health_url, status.as_u16()),
            health_url: health_url.to_string(),
        };
    }

    let body = match response.text() {
        Ok(body) => body,
        Err(error) => {
            return ConnectionCheck {
                ok: false,
                message: format!("健康检查响应读取失败：{error}"),
                health_url: health_url.to_string(),
            };
        }
    };

    if body.trim() == "OK" {
        ConnectionCheck {
            ok: true,
            message: "连接正常：/api/health 返回 OK".to_string(),
            health_url: health_url.to_string(),
        }
    } else {
        let preview: String = body.trim().chars().take(80).collect();
        ConnectionCheck {
            ok: false,
            message: format!("健康检查响应异常：期望 OK，实际返回 `{preview}`"),
            health_url: health_url.to_string(),
        }
    }
}

fn attach_window_state_listener(app: AppHandle, window: WebviewWindow) {
    let tracked_window = window.clone();
    window.on_window_event(move |event| match event {
        WindowEvent::Resized(_) | WindowEvent::Moved(_) | WindowEvent::CloseRequested { .. } => {
            persist_window_state(&app, &tracked_window);
        }
        _ => {}
    });
}

fn desktop_bridge_script() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        return r#"Object.defineProperty(window, '__OUREA_DESKTOP__', { value: { kind: 'ourea-desktop', platform: 'macos', titleBar: 'custom', navbarDrag: false, trafficLights: false }, configurable: false, enumerable: false });"#;
    }

    #[cfg(target_os = "windows")]
    {
        return r#"Object.defineProperty(window, '__OUREA_DESKTOP__', { value: { kind: 'ourea-desktop', platform: 'windows', titleBar: 'custom', navbarDrag: false, trafficLights: false }, configurable: false, enumerable: false });"#;
    }

    #[cfg(target_os = "linux")]
    {
        return r#"Object.defineProperty(window, '__OUREA_DESKTOP__', { value: { kind: 'ourea-desktop', platform: 'linux', titleBar: 'custom', navbarDrag: false, trafficLights: false }, configurable: false, enumerable: false });"#;
    }

    #[allow(unreachable_code)]
    r#"Object.defineProperty(window, '__OUREA_DESKTOP__', { value: { kind: 'ourea-desktop', platform: 'unknown', titleBar: 'custom', navbarDrag: false, trafficLights: false }, configurable: false, enumerable: false });"#
}

fn persist_window_state(app: &AppHandle, window: &WebviewWindow) {
    let Ok(scale_factor) = window.scale_factor() else {
        return;
    };
    let Ok(size) = window.inner_size() else {
        return;
    };
    let Ok(position) = window.outer_position() else {
        return;
    };
    let maximized = window.is_maximized().unwrap_or(false);

    let placement = WindowPlacement {
        width: Some(size.width as f64 / scale_factor),
        height: Some(size.height as f64 / scale_factor),
        x: Some(position.x as f64 / scale_factor),
        y: Some(position.y as f64 / scale_factor),
        maximized,
    };

    let label = window.label().to_string();
    let state = app.state::<DesktopState>();
    let Ok(mut config) = state.config.lock() else {
        return;
    };
    config.window_states.insert(label, placement);
    let _ = save_config(app, &config);
}

fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|error| format!("failed to resolve config dir: {error}"))?;
    Ok(dir.join("config.json"))
}

fn load_config(app: &AppHandle) -> DesktopConfig {
    let Ok(path) = config_path(app) else {
        return first_run_config();
    };

    let Ok(content) = fs::read_to_string(path) else {
        return first_run_config();
    };

    let mut config = serde_json::from_str::<DesktopConfig>(&content)
        .or_else(|_| migrate_legacy_config(&content))
        .unwrap_or_else(|_| first_run_config());

    if config.profiles.is_empty() {
        config = first_run_config();
    } else if config.active_profile_id.as_ref().is_none_or(|active_id| {
        !config
            .profiles
            .iter()
            .any(|profile| &profile.id == active_id)
    }) {
        config.active_profile_id = config.profiles.first().map(|profile| profile.id.clone());
    }

    config
}

fn migrate_legacy_config(content: &str) -> Result<DesktopConfig, serde_json::Error> {
    let legacy = serde_json::from_str::<LegacyDesktopConfig>(content)?;
    let Some(url) = legacy.ourea_url else {
        return Ok(first_run_config());
    };

    Ok(DesktopConfig {
        profiles: vec![OureaProfile {
            id: DEFAULT_PROFILE_ID.to_string(),
            name: DEFAULT_PROFILE_NAME.to_string(),
            url,
        }],
        active_profile_id: Some(DEFAULT_PROFILE_ID.to_string()),
        theme_mode: "system".to_string(),
        window_states: HashMap::new(),
    })
}

fn first_run_config() -> DesktopConfig {
    DesktopConfig {
        profiles: vec![default_profile()],
        active_profile_id: Some(DEFAULT_PROFILE_ID.to_string()),
        theme_mode: "system".to_string(),
        window_states: HashMap::new(),
    }
}

fn save_config(app: &AppHandle, config: &DesktopConfig) -> Result<(), String> {
    let path = config_path(app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create config dir: {error}"))?;
    }

    let content = serde_json::to_string_pretty(config)
        .map_err(|error| format!("failed to serialize config: {error}"))?;
    fs::write(path, content).map_err(|error| format!("failed to write config: {error}"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let config = load_config(app.handle());
            app.manage(DesktopState {
                config: Mutex::new(config),
            });

            let launcher = WebviewWindowBuilder::new(
                app,
                LAUNCHER_WINDOW_LABEL,
                WebviewUrl::App("index.html".into()),
            )
            .title("Ourea Desktop")
            .inner_size(1280.0, 800.0)
            .min_inner_size(960.0, 640.0)
            .center()
            .resizable(true)
            .decorations(false)
            .initialization_script_for_all_frames(desktop_bridge_script())
            .build()?;
            attach_window_state_listener(app.handle().clone(), launcher);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            upsert_profile,
            delete_profile,
            test_profile,
            test_ourea_url,
            set_theme,
            activate_profile,
            window_action
        ])
        .run(tauri::generate_context!())
        .expect("error while running Ourea Desktop");
}
