use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Instant;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::audio::SystemAudioCapture;
use crate::speech_translation::{SpeechTranslationEngine, LiveTranslationPayload, dashscope::{DashScopeAsrEngine, DashScopeAsrConfig}};
use crate::translation::engine_manager::EngineManager;
use super::translation::EngineManagerState;

/// 音频处理常量
const CHUNK_DURATION_MS: u32 = 800; // 每 800ms 切片（足够 ASR 识别完整句子）
const POLL_INTERVAL_MS: u64 = 50; // 轮询间隔

/// 实时音频翻译状态
pub struct LiveAudioState {
    is_active: Arc<AtomicBool>,
    task_handle: Mutex<Option<JoinHandle<()>>>,
    chunk_id: Arc<AtomicU32>,
    started_at: Mutex<Option<Instant>>,
    engine: Mutex<Option<Arc<dyn SpeechTranslationEngine>>>,
}

impl LiveAudioState {
    pub fn new() -> Self {
        Self {
            is_active: Arc::new(AtomicBool::new(false)),
            task_handle: Mutex::new(None),
            chunk_id: Arc::new(AtomicU32::new(0)),
            started_at: Mutex::new(None),
            engine: Mutex::new(None),
        }
    }

    pub fn is_active(&self) -> bool {
        self.is_active.load(Ordering::Acquire)
    }

    pub async fn start(
        &self,
        app: AppHandle,
        capture: Arc<Mutex<SystemAudioCapture>>,
        engine: Arc<dyn SpeechTranslationEngine>,
        translation_mgr: Option<Arc<Mutex<EngineManager>>>,
    ) -> Result<(), String> {
        // 使用 Acquire 内存顺序，确保之前的写入对其他线程可见
        if self.is_active.swap(true, Ordering::AcqRel) {
            return Err("已经在运行中".to_string());
        }

        // 重置 chunk 计数
        self.chunk_id.store(0, Ordering::Release);

        // 记录开始时间
        *self.started_at.lock().await = Some(Instant::now());

        // 启动捕获
        match capture.lock().await.start() {
            Ok(()) => {}
            Err(e) => {
                // 启动失败，回滚 is_active 状态
                self.is_active.store(false, Ordering::Release);
                return Err(format!("启动音频捕获失败: {}", e));
            }
        }

        // 设置结果通道
        let (result_tx, mut result_rx) = tokio::sync::mpsc::channel::<LiveTranslationPayload>(100);
        engine.set_result_channel(result_tx);

        // 保存引擎引用
        *self.engine.lock().await = Some(engine.clone());

        // 启动翻译会话
        if let Err(e) = engine.start_session().await {
            // 启动失败，回滚
            self.is_active.store(false, Ordering::Release);
            capture.lock().await.stop().ok();
            return Err(format!("启动翻译会话失败: {}", e));
        }

        // 发送状态变化事件
        let _ = app.emit(
            "live-translation-state-changed",
            LiveTranslationState {
                is_active: true,
                duration_ms: 0,
            },
        );

        // 启动后台任务
        let app_clone = app.clone();
        let capture_clone = capture.clone();
        let engine_clone = engine.clone();
        let chunk_id = self.chunk_id.clone();
        let is_active = self.is_active.clone();

        // 启动结果接收任务（仅对最终结果触发翻译）
        let app_for_results = app.clone();
        let mgr = translation_mgr;
        tokio::spawn(async move {
            while let Some(mut result) = result_rx.recv().await {
                // 只对最终结果（is_final = true）触发翻译
                // 中间结果直接透传，前端仅更新 transcript
                if result.is_final && !result.transcript_text.is_empty() {
                    if let Some(ref mgr) = mgr {
                        match mgr.lock().await.translate(&result.transcript_text, None).await {
                            Ok(tr) => {
                                result.translated_text = tr.translated_text;
                                result.target_language = Some(tr.target_lang);
                                result.source_language = Some(tr.source_lang);
                            }
                            Err(e) => {
                                log::warn!("[LiveAudio] 翻译失败: {}", e);
                                let _ = app_for_results.emit("live-translation-error", LiveTranslationError {
                                    error: format!("翻译失败: {}", e),
                                    recoverable: true,
                                });
                            }
                        }
                    }
                }
                if let Err(e) = app_for_results.emit("live-translation-result", result) {
                    log::error!("[LiveAudio] 发送翻译结果事件失败: {}", e);
                }
            }
        });

        let handle = match tokio::spawn(async move {
            Self::process_loop(app_clone, capture_clone, engine_clone, chunk_id, is_active).await;
        }) {
            handle => handle,
        };

        // 如果任务句柄为空（理论上不会发生），回滚
        if handle.is_finished() {
            self.is_active.store(false, Ordering::Release);
            capture.lock().await.stop().ok();
            engine.stop_session().await.ok();
            return Err("启动后台任务失败".to_string());
        }

        *self.task_handle.lock().await = Some(handle);

        log::info!("[LiveAudio] 开始实时音频翻译");
        Ok(())
    }

    pub async fn stop(&self, app: AppHandle, capture: Arc<Mutex<SystemAudioCapture>>) -> Result<(), String> {
        // 标记停止，使用 Release 确保对其他线程可见
        self.is_active.store(false, Ordering::Release);

        // abort 后台任务
        if let Some(handle) = self.task_handle.lock().await.take() {
            handle.abort();
        }

        // 获取引擎并停止会话
        let engine = self.engine.lock().await.take();
        if let Some(engine) = engine {
            if let Err(e) = engine.stop_session().await {
                log::warn!("[LiveAudio] 停止翻译会话失败: {}", e);
            }
        }

        // 显式停止 capture，清理资源
        capture
            .lock()
            .await
            .stop()
            .map_err(|e| format!("停止音频捕获失败: {}", e))?;

        // 重置 chunk 计数
        self.chunk_id.store(0, Ordering::Release);

        // 清空开始时间
        *self.started_at.lock().await = None;

        // 通知前端状态已变更（因为 abort 会阻止 process_loop 发送此事件）
        let _ = app.emit(
            "live-translation-state-changed",
            LiveTranslationState {
                is_active: false,
                duration_ms: 0,
            },
        );

        log::info!("[LiveAudio] 停止实时音频翻译");
        Ok(())
    }

    /// 音频处理循环
    async fn process_loop(
        app: AppHandle,
        capture: Arc<Mutex<SystemAudioCapture>>,
        engine: Arc<dyn SpeechTranslationEngine>,
        chunk_id: Arc<AtomicU32>,
        is_active: Arc<AtomicBool>,
    ) {
        while is_active.load(Ordering::Acquire) {
            // 等待音频数据
            tokio::time::sleep(tokio::time::Duration::from_millis(POLL_INTERVAL_MS)).await;

            // 读取音频数据
            let audio_data = {
                let capture = capture.lock().await;
                capture.read_chunk(CHUNK_DURATION_MS)
            };

            let Some(audio) = audio_data else {
                continue;
            };

            if audio.is_empty() {
                continue;
            }

            // 从 capture 获取采样率
            let sample_rate = {
                let capture = capture.lock().await;
                capture.sample_rate()
            };

            // 发送音频到翻译引擎
            if let Err(e) = engine.send_audio_chunk(&audio, sample_rate).await {
                log::warn!("[LiveAudio] 发送音频失败: {}", e);

                // 发送错误事件
                let payload = LiveTranslationError {
                    error: e.to_string(),
                    recoverable: true,
                };

                let _ = app.emit("live-translation-error", payload);
            }
        }

        // 发送停止状态事件
        let _ = app.emit(
            "live-translation-state-changed",
            LiveTranslationState {
                is_active: false,
                duration_ms: 0,
            },
        );
    }
}

/// 实时翻译错误事件
#[derive(Serialize, Clone)]
pub struct LiveTranslationError {
    pub error: String,
    pub recoverable: bool,
}

/// 实时翻译状态变化事件
#[derive(Serialize, Clone)]
pub struct LiveTranslationState {
    pub is_active: bool,
    pub duration_ms: u64,
}

/// 开始实时音频翻译
#[tauri::command]
pub async fn start_live_audio_translation(
    app: AppHandle,
    state: State<'_, Arc<LiveAudioState>>,
    capture: State<'_, Arc<Mutex<SystemAudioCapture>>>,
    db: State<'_, crate::db::Database>,
    engine_mgr: State<'_, EngineManagerState>,
) -> Result<(), String> {
    // 构建 DashScope ASR 引擎
    let engine: Arc<dyn SpeechTranslationEngine> = match db.get_engine_config("dashscope-asr").await {
        Ok(Some(config)) => {
            if let Some(api_key) = config.api_key {
                if !api_key.is_empty() {
                    // 从 extra_json 读取语言配置
                    let extra: serde_json::Value = config.extra_json
                        .and_then(|s| serde_json::from_str(&s).ok())
                        .unwrap_or_default();

                    let language_hints: Vec<String> = extra["language_hints"]
                        .as_array()
                        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                        .unwrap_or_else(|| vec!["en".to_string()]);

                    let engine_config = DashScopeAsrConfig { api_key, language_hints };
                    Arc::new(DashScopeAsrEngine::new(engine_config, capture.inner().clone()))
                } else {
                    return Err("DashScope API Key 为空，请在设置中配置".to_string());
                }
            } else {
                return Err("DashScope API Key 未配置，请在设置中配置".to_string());
            }
        }
        Ok(None) => {
            // 数据库中没有配置，尝试环境变量
            log::warn!("[LiveAudio] 数据库中未找到 DashScope 配置，尝试环境变量");
            let api_key = std::env::var("DASHSCOPE_API_KEY")
                .map_err(|_| "环境变量 DASHSCOPE_API_KEY 未设置".to_string())?;

            let engine_config = DashScopeAsrConfig {
                api_key,
                language_hints: vec!["en".to_string()],
            };
            Arc::new(DashScopeAsrEngine::new(engine_config, capture.inner().clone()))
        }
        Err(e) => {
            log::error!("[LiveAudio] 读取数据库配置失败: {}", e);
            return Err(format!("读取配置失败: {}", e));
        }
    };

    state.start(app, capture.inner().clone(), engine, Some(engine_mgr.inner().0.clone())).await
}

/// 停止实时音频翻译
#[tauri::command]
pub async fn stop_live_audio_translation(
    app: AppHandle,
    state: State<'_, Arc<LiveAudioState>>,
    capture: State<'_, Arc<Mutex<SystemAudioCapture>>>,
) -> Result<(), String> {
    state.stop(app, capture.inner().clone()).await
}

/// 获取实时翻译状态
#[tauri::command]
pub async fn get_live_audio_state(
    state: State<'_, Arc<LiveAudioState>>,
) -> Result<bool, String> {
    Ok(state.is_active())
}
