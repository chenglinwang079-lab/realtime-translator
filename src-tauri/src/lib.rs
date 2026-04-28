mod accessibility;
mod audio;
mod asr;
mod clipboard;
mod commands;
mod db;
mod logger;
mod ocr;
mod screenshot;
mod shortcuts;
mod translation;
mod tray;
mod window;

use std::sync::Arc;

use tauri::Manager;
use tokio::sync::Mutex;
use translation::engine_manager::EngineManager;
use ocr::engine_manager::OcrEngineManager;
use translation::memory_cache::MemoryCache;
use window::bubble::{
    get_cursor_position, hide_bubble_window, move_bubble_follow, move_bubble_to_cursor,
    set_window_state, set_window_size, show_bubble_window,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            get_cursor_position,
            move_bubble_to_cursor,
            move_bubble_follow,
            set_window_state,
            set_window_size,
            show_bubble_window,
            hide_bubble_window,
            commands::clipboard::start_clipboard_watch,
            commands::clipboard::stop_clipboard_watch,
            commands::clipboard::toggle_clipboard_watch,
            commands::clipboard::get_watch_state,
            commands::translation::translate,
            commands::translation::translate_with_engine,
            commands::translation::get_engines,
            commands::translation::set_default_engine,
            commands::translation::test_engine,
            commands::translation::clear_cache,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::settings::save_api_key,
            commands::settings::delete_api_key,
            commands::shortcuts::register_shortcut,
            commands::shortcuts::unregister_shortcut,
            commands::history::save_history,
            commands::history::get_history,
            commands::history::clear_history,
            commands::accessibility::get_selected_text,
            commands::accessibility::get_focused_app,
            commands::accessibility::start_uia_events,
            commands::accessibility::stop_uia_events,
            commands::accessibility::get_uia_events_state,
            commands::accessibility::start_uia_polling_fallback,
            commands::accessibility::stop_uia_polling_fallback,
            commands::accessibility::get_uia_polling_state,
            commands::screenshot::capture_screen,
            commands::screenshot::capture_screen_region,
            commands::selector::show_region_selector,
            commands::selector::submit_region_selection,
            commands::selector::cancel_region_selection,
            commands::ocr::ocr_recognize,
            commands::ocr::get_ocr_engines,
            commands::ocr::test_ocr_engine,
            commands::audio::start_live_audio_translation,
            commands::audio::stop_live_audio_translation,
            commands::audio::get_live_audio_state,
            commands::uninstall::uninstall_app,
        ])
        .setup(|app| {
            // 初始化日志（必须在所有模块初始化之前）
            logger::init();

            // 初始化数据库（先于引擎，因为引擎需要从 DB 读取 API key）
            let db = match tauri::async_runtime::block_on(db::Database::new()) {
                Ok(db) => {
                    log::info!("数据库已初始化");
                    db
                }
                Err(e) => {
                    log::error!("数据库初始化失败: {}", e);
                    panic!("数据库初始化失败: {}", e);
                }
            };

            // 初始化翻译引擎管理器
            let mut engine_manager = EngineManager::new();

            // 从 DB 读取已保存的 API key 初始化引擎
            let db_configs = tauri::async_runtime::block_on(db.get_all_engine_configs())
                .unwrap_or_default();

            for config in &db_configs {
                if let Some(ref key) = config.api_key {
                    if !key.is_empty() {
                        let extra = config.extra_json.as_deref();
                        match engine_manager.reload_engine(&config.engine_id, key, extra) {
                            Ok(()) => {
                                log::info!("引擎已从 DB 加载: {}", config.engine_id);
                            }
                            Err(e) => {
                                log::error!("DB 引擎加载失败 {}: {}", config.engine_id, e);
                            }
                        }
                    }
                }
            }

            // DB 中没有的引擎，尝试环境变量作为 fallback
            if engine_manager.get_engine("tencent-tmt").is_none() {
                match translation::tencent::TencentEngine::new() {
                    Ok(engine) => {
                        log::info!("腾讯云翻译引擎已注册（环境变量）");
                        engine_manager.register(Arc::new(engine));
                    }
                    Err(e) => {
                        log::debug!("腾讯云引擎跳过: {}", e);
                    }
                }
            }

            if engine_manager.get_engine("openai-gpt-4o-mini").is_none() {
                match translation::openai::OpenAiEngine::new() {
                    Ok(engine) => {
                        log::info!("OpenAI 引擎已注册（环境变量）");
                        engine_manager.register(Arc::new(engine));
                    }
                    Err(e) => {
                        log::debug!("OpenAI 引擎跳过: {}", e);
                    }
                }
            }

            if engine_manager.get_engine("deepl-free").is_none() {
                match translation::deepl::DeepLEngine::new() {
                    Ok(engine) => {
                        log::info!("DeepL 引擎已注册（环境变量）");
                        engine_manager.register(Arc::new(engine));
                    }
                    Err(e) => {
                        log::debug!("DeepL 引擎跳过: {}", e);
                    }
                }
            }

            let status = engine_manager.status();
            log::info!(
                "引擎管理器初始化完成: {} 个引擎, 默认: {}",
                status.engines.len(),
                status.default_engine
            );

            app.manage(commands::translation::EngineManagerState(Arc::new(
                Mutex::new(engine_manager),
            )));

            // 初始化 OCR 引擎管理器
            let mut ocr_manager = OcrEngineManager::new();

            // 从 DB 加载 OCR 引擎 API key
            for config in &db_configs {
                if let Some(ref key) = config.api_key {
                    if !key.is_empty()
                        && (config.engine_id.starts_with("google-vision")
                            || config.engine_id == "baidu-ocr")
                    {
                        match ocr_manager.reload_engine(&config.engine_id, key, config.extra_json.as_deref()) {
                            Ok(()) => {
                                log::info!("OCR 引擎已从 DB 加载: {}", config.engine_id);
                            }
                            Err(e) => {
                                log::error!("DB OCR 引擎加载失败 {}: {}", config.engine_id, e);
                            }
                        }
                    }
                }
            }

            // DB 没有时尝试环境变量
            if ocr_manager.get_engine("google-vision").is_none() {
                match ocr::google_vision::GoogleVisionEngine::new() {
                    Ok(engine) => {
                        log::info!("Google Vision OCR 已注册（环境变量）");
                        ocr_manager.register(Arc::new(engine));
                    }
                    Err(e) => {
                        log::debug!("Google Vision OCR 跳过: {}", e);
                    }
                }
            }

            if ocr_manager.get_engine("baidu-ocr").is_none() {
                match ocr::baidu_ocr::BaiduOcrEngine::new() {
                    Ok(engine) => {
                        log::info!("百度 OCR 已注册（环境变量）");
                        ocr_manager.register(Arc::new(engine));
                    }
                    Err(e) => {
                        log::debug!("百度 OCR 跳过: {}", e);
                    }
                }
            }

            let ocr_status = ocr_manager.status();
            log::info!(
                "OCR 引擎管理器初始化完成: {} 个引擎, 默认: {}",
                ocr_status.engines.len(),
                ocr_status.default_engine
            );

            app.manage(commands::ocr::OcrEngineManagerState(Arc::new(
                Mutex::new(ocr_manager),
            )));

            // 初始化 LRU 内存缓存（1000 条）
            app.manage(MemoryCache::new(1000));
            log::info!("LRU 内存缓存已初始化 (容量: 1000)");

            // 初始化当前快捷键状态
            app.manage(commands::shortcuts::CurrentShortcut(Arc::new(
                Mutex::new("Ctrl+Shift+T".to_string()),
            )));

            // 初始化剪贴板 watcher（不启动，由前端调用 start_clipboard_watch 启动）
            let watcher = clipboard::watcher::ClipboardWatcher::new();
            app.manage(watcher);
            log::info!("剪贴板 watcher 已初始化（等待前端启动）");

            // 初始化系统音频捕获
            let audio_capture = Arc::new(Mutex::new(
                audio::SystemAudioCapture::new(audio::capture::AudioCaptureConfig::default())
                    .expect("初始化系统音频捕获失败"),
            ));
            app.manage(audio_capture);
            log::info!("系统音频捕获已初始化");

            // 初始化实时音频翻译状态
            let live_audio_state = Arc::new(commands::audio::LiveAudioState::new());
            app.manage(live_audio_state);
            log::info!("实时音频翻译状态已初始化");

            app.manage(db);

            // 注册全局快捷键
            if let Err(e) = shortcuts::register_default_shortcuts(app.handle()) {
                log::error!("全局快捷键注册失败: {}", e);
            }

            // 启动前台窗口追踪（让「抓取选中文本」按钮在 Tauri 获焦后仍能工作）
            accessibility::start_focus_tracking();

            // 创建系统托盘
            if let Err(e) = tray::create_tray(app.handle()) {
                log::error!("系统托盘创建失败: {}", e);
            }

            if let Some(window) = app.get_webview_window("bubble") {
                if let Err(e) = window.show() {
                    log::error!("显示 bubble 窗口失败: {}", e);
                }
                if let Err(e) = window.set_always_on_top(true) {
                    log::error!("设置置顶失败: {}", e);
                }
                if let Err(e) = window.set_ignore_cursor_events(false) {
                    log::warn!("设置穿透鼠标事件失败: {}", e);
                }
                if let Err(e) = window.set_focus() {
                    log::warn!("设置焦点失败: {}", e);
                }
            } else {
                log::error!("找不到 bubble 窗口");
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
