use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Instant;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::audio::SystemAudioCapture;
use crate::asr::AsrEngine;

/// 音频处理常量
const CHUNK_DURATION_MS: u32 = 3000; // 每 3 秒切片
const POLL_INTERVAL_MS: u64 = 100; // 轮询间隔

/// 实时音频翻译状态
pub struct LiveAudioState {
    is_active: Arc<AtomicBool>,
    task_handle: Mutex<Option<JoinHandle<()>>>,
    chunk_id: Arc<AtomicU32>,
    started_at: Mutex<Option<Instant>>,
}

impl LiveAudioState {
    pub fn new() -> Self {
        Self {
            is_active: Arc::new(AtomicBool::new(false)),
            task_handle: Mutex::new(None),
            chunk_id: Arc::new(AtomicU32::new(0)),
            started_at: Mutex::new(None),
        }
    }

    pub fn is_active(&self) -> bool {
        self.is_active.load(Ordering::Acquire)
    }

    pub async fn start(
        &self,
        app: AppHandle,
        capture: Arc<Mutex<SystemAudioCapture>>,
        asr: Arc<dyn AsrEngine>,
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
        let asr_clone = asr.clone();
        let chunk_id = self.chunk_id.clone();
        let is_active = self.is_active.clone();

        let handle = tokio::spawn(async move {
            Self::process_loop(app_clone, capture_clone, asr_clone, chunk_id, is_active).await;
        });

        *self.task_handle.lock().await = Some(handle);

        log::info!("[LiveAudio] 开始实时音频翻译");
        Ok(())
    }

    pub async fn stop(&self, capture: Arc<Mutex<SystemAudioCapture>>) -> Result<(), String> {
        // 标记停止，使用 Release 确保对其他线程可见
        self.is_active.store(false, Ordering::Release);

        // abort 后台任务
        if let Some(handle) = self.task_handle.lock().await.take() {
            handle.abort();
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

        log::info!("[LiveAudio] 停止实时音频翻译");
        Ok(())
    }

    /// 音频处理循环
    async fn process_loop(
        app: AppHandle,
        capture: Arc<Mutex<SystemAudioCapture>>,
        asr: Arc<dyn AsrEngine>,
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

            // 识别
            let current_chunk_id = chunk_id.fetch_add(1, Ordering::AcqRel);
            let timestamp_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;

            // 从 capture 配置中获取采样率
            let sample_rate = {
                let capture = capture.lock().await;
                capture.config().sample_rate
            };

            match asr.recognize(&audio, sample_rate).await {
                Ok(result) => {
                    // 发送识别结果事件
                    let payload = LiveTranscriptPayload {
                        text: result.text,
                        language: result.language,
                        confidence: result.confidence,
                        latency_ms: result.latency_ms,
                        is_final: true,
                        chunk_id: current_chunk_id,
                        timestamp_ms,
                        duration_ms: Some(CHUNK_DURATION_MS as u64),
                    };

                    if let Err(e) = app.emit("live-transcript", payload) {
                        log::error!("[LiveAudio] 发送事件失败: {}", e);
                    }
                }
                Err(e) => {
                    log::warn!("[LiveAudio] ASR 识别失败: {}", e);

                    // 发送错误事件
                    let payload = LiveTranslationError {
                        error: e.to_string(),
                        recoverable: true,
                    };

                    let _ = app.emit("live-translation-error", payload);
                }
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

/// 实时翻译转录事件 payload
#[derive(Serialize, Clone)]
pub struct LiveTranscriptPayload {
    pub text: String,
    pub language: Option<String>,
    pub confidence: f32,
    pub latency_ms: u64,
    pub is_final: bool,
    pub chunk_id: u32,
    pub timestamp_ms: u64,
    pub duration_ms: Option<u64>,
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
) -> Result<(), String> {
    // 优先从数据库读取 API Key
    let asr: Arc<dyn AsrEngine> = match db.get_engine_config("aliyun-asr").await {
        Ok(Some(config)) => {
            if let Some(api_key) = config.api_key {
                // 从 extra_json 中读取 app_key 和 access_key_secret
                let extra: serde_json::Value = config.extra_json
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .unwrap_or_default();

                let app_key = extra["app_key"].as_str().unwrap_or_default().to_string();
                let access_key_secret = extra["access_key_secret"].as_str().unwrap_or_default().to_string();

                if !app_key.is_empty() && !access_key_secret.is_empty() {
                    Arc::new(crate::asr::AliyunAsrEngine::from_config(
                        app_key,
                        api_key, // access_key_id
                        access_key_secret,
                    ))
                } else {
                    return Err("阿里云 ASR 配置不完整，请在设置中配置 app_key 和 access_key_secret".to_string());
                }
            } else {
                return Err("阿里云 ASR API Key 未配置，请在设置中配置".to_string());
            }
        }
        Ok(None) => {
            // 数据库中没有配置，尝试环境变量（仅开发环境）
            log::warn!("[LiveAudio] 数据库中未找到阿里云 ASR 配置，尝试环境变量");
            match crate::asr::AliyunAsrEngine::from_env() {
                Ok(engine) => Arc::new(engine),
                Err(e) => {
                    return Err(format!("ASR 引擎初始化失败: {}", e));
                }
            }
        }
        Err(e) => {
            log::error!("[LiveAudio] 读取数据库配置失败: {}", e);
            return Err(format!("读取 ASR 配置失败: {}", e));
        }
    };

    state.start(app, capture.inner().clone(), asr).await
}

/// 停止实时音频翻译
#[tauri::command]
pub async fn stop_live_audio_translation(
    state: State<'_, Arc<LiveAudioState>>,
    capture: State<'_, Arc<Mutex<SystemAudioCapture>>>,
) -> Result<(), String> {
    state.stop(capture.inner().clone()).await
}

/// 获取实时翻译状态
#[tauri::command]
pub async fn get_live_audio_state(
    state: State<'_, Arc<LiveAudioState>>,
) -> Result<bool, String> {
    Ok(state.is_active())
}
