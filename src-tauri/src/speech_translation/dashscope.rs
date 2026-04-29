use anyhow::{Context, Result};
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::io::{Write, Seek, SeekFrom};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex as TokioMutex};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

use super::engine::{LiveTranslationPayload, SpeechTranslationEngine};
use crate::audio::SystemAudioCapture;

/// DashScope 实时 ASR 配置
pub struct DashScopeAsrConfig {
    /// DashScope API Key
    pub api_key: String,
    /// 语言提示（如 ["en"]、["zh", "en"]）
    pub language_hints: Vec<String>,
}

impl Default for DashScopeAsrConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            language_hints: vec!["en".to_string()],
        }
    }
}

/// WebSocket sender 类型
type WsSender = futures_util::stream::SplitSink<
    WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>,
    Message,
>;

/// DashScope Paraformer 实时 ASR 引擎
pub struct DashScopeAsrEngine {
    config: DashScopeAsrConfig,
    capture: Arc<TokioMutex<SystemAudioCapture>>,
    result_tx: Arc<std::sync::Mutex<Option<mpsc::Sender<LiveTranslationPayload>>>>,
    is_connected: Arc<std::sync::atomic::AtomicBool>,
    task_started: Arc<std::sync::atomic::AtomicBool>,
    ws_sender: Arc<TokioMutex<Option<WsSender>>>,
    task_id: Arc<TokioMutex<Option<String>>>,
    wav_writer: Arc<std::sync::Mutex<Option<WavWriter>>>,
}

/// 简易 WAV 写入器（16kHz mono PCM16）
struct WavWriter {
    file: std::fs::File,
    data_size: u32,
}

impl WavWriter {
    fn create(path: &str, sample_rate: u32) -> std::io::Result<Self> {
        let mut file = std::fs::File::create(path)?;
        // WAV header placeholder（44 bytes），close 时回填大小
        let header = Self::make_header(sample_rate, 0);
        file.write_all(&header)?;
        Ok(Self { file, data_size: 0 })
    }

    fn write_pcm16(&mut self, data: &[u8]) -> std::io::Result<()> {
        self.file.write_all(data)?;
        self.data_size += data.len() as u32;
        Ok(())
    }

    fn finish(mut self) -> std::io::Result<()> {
        // 回填 RIFF 和 data chunk 的大小
        let riff_size = 36 + self.data_size;
        self.file.seek(std::io::SeekFrom::Start(4))?;
        self.file.write_all(&riff_size.to_le_bytes())?;
        self.file.seek(std::io::SeekFrom::Start(40))?;
        self.file.write_all(&self.data_size.to_le_bytes())?;
        self.file.flush()?;
        Ok(())
    }

    fn make_header(sample_rate: u32, data_size: u32) -> [u8; 44] {
        let byte_rate = sample_rate * 2; // mono 16-bit
        let mut h = [0u8; 44];
        h[0..4].copy_from_slice(b"RIFF");
        h[4..8].copy_from_slice(&(36u32 + data_size).to_le_bytes());
        h[8..12].copy_from_slice(b"WAVE");
        h[12..16].copy_from_slice(b"fmt ");
        h[16..20].copy_from_slice(&16u32.to_le_bytes()); // fmt chunk size
        h[20..22].copy_from_slice(&1u16.to_le_bytes());  // PCM
        h[22..24].copy_from_slice(&1u16.to_le_bytes());  // mono
        h[24..28].copy_from_slice(&sample_rate.to_le_bytes());
        h[28..32].copy_from_slice(&byte_rate.to_le_bytes());
        h[32..34].copy_from_slice(&2u16.to_le_bytes());  // block align
        h[34..36].copy_from_slice(&16u16.to_le_bytes()); // bits per sample
        h[36..40].copy_from_slice(b"data");
        h[40..44].copy_from_slice(&data_size.to_le_bytes());
        h
    }
}

// === DashScope WebSocket 协议消息结构 ===

#[derive(Serialize)]
struct RunTaskMessage {
    header: RunTaskHeader,
    payload: RunTaskPayload,
}

#[derive(Serialize)]
struct RunTaskHeader {
    action: &'static str,
    task_id: String,
    streaming: &'static str,
}

#[derive(Serialize)]
struct RunTaskPayload {
    task_group: &'static str,
    task: &'static str,
    function: &'static str,
    model: &'static str,
    parameters: RunTaskParameters,
    input: serde_json::Value,
}

#[derive(Serialize)]
struct RunTaskParameters {
    format: &'static str,
    sample_rate: u32,
    language_hints: Vec<String>,
    semantic_punctuation_enabled: bool,
    disfluency_removal_enabled: bool,
    punctuation_prediction_enabled: bool,
    inverse_text_normalization_enabled: bool,
    max_sentence_silence: u32,
}

#[derive(Serialize)]
struct FinishTaskMessage {
    header: FinishTaskHeader,
    payload: FinishTaskPayload,
}

#[derive(Serialize)]
struct FinishTaskHeader {
    action: &'static str,
    task_id: String,
    streaming: &'static str,
}

#[derive(Serialize)]
struct FinishTaskPayload {
    input: serde_json::Value,
}

// === 服务端响应结构 ===

#[derive(Deserialize, Debug)]
struct ServerMessage {
    header: ServerHeader,
    payload: Option<ServerPayload>,
}

#[derive(Deserialize, Debug)]
struct ServerHeader {
    task_id: Option<String>,
    event: Option<String>,
    error_code: Option<String>,
    error_message: Option<String>,
}

#[derive(Deserialize, Debug)]
struct ServerPayload {
    output: Option<ServerOutput>,
    usage: Option<serde_json::Value>,
}

#[derive(Deserialize, Debug)]
struct ServerOutput {
    sentence: Option<Sentence>,
}

#[derive(Deserialize, Debug)]
struct Sentence {
    text: Option<String>,
    heartbeat: Option<bool>,
    sentence_end: Option<bool>,
    end_time: Option<f64>,
}

impl DashScopeAsrEngine {
    pub fn new(config: DashScopeAsrConfig, capture: Arc<TokioMutex<SystemAudioCapture>>) -> Self {
        Self {
            config,
            capture,
            result_tx: Arc::new(std::sync::Mutex::new(None)),
            is_connected: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            task_started: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            ws_sender: Arc::new(TokioMutex::new(None)),
            task_id: Arc::new(TokioMutex::new(None)),
            wav_writer: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// 发送 run-task 开始帧
    async fn send_run_task(ws_sender: &mut WsSender, config: &DashScopeAsrConfig) -> Result<String> {
        let task_id = uuid::Uuid::new_v4().as_simple().to_string();

        let msg = RunTaskMessage {
            header: RunTaskHeader {
                action: "run-task",
                task_id: task_id.clone(),
                streaming: "duplex",
            },
            payload: RunTaskPayload {
                task_group: "audio",
                task: "asr",
                function: "recognition",
                model: "paraformer-realtime-v2",
                parameters: RunTaskParameters {
                    format: "pcm",
                    sample_rate: 16000,
                    language_hints: config.language_hints.clone(),
                    semantic_punctuation_enabled: false,
                    disfluency_removal_enabled: false,
                    punctuation_prediction_enabled: true,
                    inverse_text_normalization_enabled: true,
                    max_sentence_silence: 800,
                },
                input: serde_json::json!({}),
            },
        };

        let json = serde_json::to_string(&msg).context("序列化 run-task 消息失败")?;
        ws_sender
            .send(Message::Text(json.into()))
            .await
            .context("发送 run-task 帧失败")?;

        log::info!("[DashScopeAsr] 已发送 run-task, task_id={}", task_id);
        Ok(task_id)
    }

    /// 发送 finish-task 停止帧
    async fn send_finish_task(ws_sender: &mut WsSender, task_id: &str) -> Result<()> {
        let msg = FinishTaskMessage {
            header: FinishTaskHeader {
                action: "finish-task",
                task_id: task_id.to_string(),
                streaming: "duplex",
            },
            payload: FinishTaskPayload {
                input: serde_json::json!({}),
            },
        };

        let json = serde_json::to_string(&msg).context("序列化 finish-task 消息失败")?;
        ws_sender
            .send(Message::Text(json.into()))
            .await
            .context("发送 finish-task 帧失败")?;

        log::info!("[DashScopeAsr] 已发送 finish-task, task_id={}", task_id);
        Ok(())
    }

    /// 处理服务端消息
    fn handle_server_message(
        message: &str,
        result_tx: Option<&mpsc::Sender<LiveTranslationPayload>>,
        chunk_id: &mut u32,
        task_started: &std::sync::atomic::AtomicBool,
    ) -> Result<()> {
        let msg: ServerMessage = serde_json::from_str(message)
            .context("解析 DashScope 服务端消息失败")?;

        let event = msg.header.event.as_deref().unwrap_or("");

        match event {
            "task-started" => {
                log::info!("[DashScopeAsr] 任务已开始");
                task_started.store(true, std::sync::atomic::Ordering::SeqCst);
            }
            "result-generated" => {
                let Some(payload) = &msg.payload else { return Ok(()) };
                let Some(output) = &payload.output else { return Ok(()) };
                let Some(sentence) = &output.sentence else { return Ok(()) };

                // 跳过心跳
                if sentence.heartbeat.unwrap_or(false) {
                    return Ok(());
                }

                let text = sentence.text.as_deref().unwrap_or("").to_string();
                if text.is_empty() {
                    return Ok(());
                }

                let is_final = sentence.sentence_end.unwrap_or(false)
                    || sentence.end_time.is_some();

                if is_final {
                    log::info!("[DashScopeAsr] 最终结果: {}", truncate_for_log(&text, 60));
                } else {
                    log::debug!("[DashScopeAsr] 中间结果: {}", truncate_for_log(&text, 40));
                }

                let result = LiveTranslationPayload {
                    transcript_text: text,
                    translated_text: String::new(),
                    source_language: None,
                    target_language: None,
                    is_final,
                    chunk_id: *chunk_id,
                    timestamp_ms: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64,
                    duration_ms: None,
                };

                *chunk_id += 1;

                if let Some(tx) = result_tx {
                    let tx = tx.clone();
                    tokio::spawn(async move {
                        if let Err(e) = tx.send(result).await {
                            log::error!("[DashScopeAsr] 发送结果失败: {}", e);
                        }
                    });
                }
            }
            "task-finished" => {
                log::info!("[DashScopeAsr] 任务已完成");
            }
            "task-failed" => {
                let code = msg.header.error_code.as_deref().unwrap_or("UNKNOWN");
                let err_msg = msg.header.error_message.as_deref().unwrap_or("未知错误");
                log::error!("[DashScopeAsr] 任务失败 ({}): {}", code, err_msg);
                return Err(anyhow::anyhow!("DashScope ASR 失败 ({}): {}", code, err_msg));
            }
            _ => {
                log::debug!("[DashScopeAsr] 未知事件: {}", event);
            }
        }

        Ok(())
    }
}

/// 抗混叠 sinc 重采样（从 aliyun.rs 移植）
fn resample_linear(pcm: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate {
        return pcm.to_vec();
    }

    let ratio = from_rate as f64 / to_rate as f64;
    let new_len = (pcm.len() as f64 / ratio).ceil() as usize;

    let cutoff = 0.9 / ratio;
    let tap_count = 64;
    let mut filtered = vec![0.0f32; pcm.len()];

    for i in 0..pcm.len() {
        let mut sum = 0.0f32;
        let mut weight_sum = 0.0f32;
        for tap in 0..tap_count {
            let offset = tap as i32 - (tap_count as i32 / 2);
            let idx = i as i32 + offset;
            if idx >= 0 && (idx as usize) < pcm.len() {
                let x = offset as f64;
                let sinc = if x.abs() < 1e-6 {
                    1.0
                } else {
                    (std::f64::consts::PI * cutoff * x).sin() / (std::f64::consts::PI * x)
                };
                let window = 0.5 * (1.0
                    + (2.0 * std::f64::consts::PI * tap as f64 / tap_count as f64).cos());
                let weight = (sinc * window) as f32;
                sum += pcm[idx as usize] * weight;
                weight_sum += weight;
            }
        }
        if weight_sum.abs() > 1e-6 {
            filtered[i] = sum / weight_sum;
        }
    }

    let mut result = Vec::with_capacity(new_len);
    for i in 0..new_len {
        let src_pos = i as f64 * ratio;
        let center = src_pos as usize;
        let frac = src_pos - center as f64;

        let mut sample = 0.0f64;
        for tap in -3..=4 {
            let idx = center as i32 + tap;
            if idx >= 0 && (idx as usize) < filtered.len() {
                let x = frac - tap as f64;
                let sinc = if x.abs() < 1e-6 {
                    1.0
                } else {
                    (std::f64::consts::PI * x).sin() / (std::f64::consts::PI * x)
                };
                sample += filtered[idx as usize] as f64 * sinc;
            }
        }
        result.push(sample as f32);
    }

    result
}

/// 截取字符串用于日志（UTF-8 安全）
fn truncate_for_log(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}

#[async_trait]
impl SpeechTranslationEngine for DashScopeAsrEngine {
    fn engine_id(&self) -> &str {
        "dashscope-asr"
    }

    fn is_available(&self) -> bool {
        !self.config.api_key.is_empty()
    }

    fn set_result_channel(&self, tx: mpsc::Sender<LiveTranslationPayload>) {
        *self.result_tx.lock().unwrap() = Some(tx);
    }

    async fn start_session(&self) -> Result<()> {
        if !self.is_available() {
            return Err(anyhow::anyhow!("DashScope API Key 未配置"));
        }

        log::info!("[DashScopeAsr] 开始会话 (model=paraformer-realtime-v2, lang={:?})", self.config.language_hints);

        // WebSocket 连接（通过 HTTP header 传递 API Key）
        let url = "wss://dashscope.aliyuncs.com/api-ws/v1/inference";
        let auth_header = format!("Bearer {}", self.config.api_key);

        // 构建带认证头的 WebSocket 请求
        let mut request = url.into_client_request()
            .context("构建 WebSocket 请求失败")?;
        request.headers_mut().insert(
            "Authorization",
            auth_header.parse().context("解析 Authorization header 失败")?,
        );

        let (ws_stream, _) = connect_async(request)
            .await
            .context("DashScope WebSocket 连接失败")?;

        log::info!("[DashScopeAsr] WebSocket 连接成功");

        let (ws_sender, mut ws_receiver) = ws_stream.split();

        // 保存 sender
        *self.ws_sender.lock().await = Some(ws_sender);

        // 创建 WAV 录音文件（用于调试音频质量）
        {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let path = dirs::desktop_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("C:\\Users\\mu"))
                .join(format!("dashscope_asr_debug_{}.wav", timestamp));
            let path_str = path.to_string_lossy().to_string();
            match WavWriter::create(&path_str, 16000) {
                Ok(writer) => {
                    log::info!("[DashScopeAsr] WAV 调试录音: {}", path_str);
                    *self.wav_writer.lock().unwrap() = Some(writer);
                }
                Err(e) => {
                    log::warn!("[DashScopeAsr] 创建 WAV 文件失败: {}", e);
                }
            }
        }

        // 发送 run-task
        {
            let mut sender_guard = self.ws_sender.lock().await;
            if let Some(ref mut sender) = *sender_guard {
                let tid = Self::send_run_task(sender, &self.config).await?;
                *self.task_id.lock().await = Some(tid);
            }
        }

        self.is_connected.store(true, std::sync::atomic::Ordering::SeqCst);

        // 启动消息接收任务
        let result_tx = self.result_tx.lock().unwrap().clone();
        let is_connected = self.is_connected.clone();
        let task_started_flag = self.task_started.clone();
        let mut chunk_id = 0u32;

        tokio::spawn(async move {
            log::info!("[DashScopeAsr] 消息接收任务已启动");
            while let Some(msg) = ws_receiver.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        log::debug!("[DashScopeAsr] 收到消息: {}", truncate_for_log(&text, 200));
                        if let Err(e) = Self::handle_server_message(&text, result_tx.as_ref(), &mut chunk_id, &task_started_flag) {
                            log::error!("[DashScopeAsr] 处理消息失败: {}", e);
                        }
                    }
                    Ok(Message::Binary(d)) => {
                        log::debug!("[DashScopeAsr] 收到二进制消息: {} 字节", d.len());
                    }
                    Ok(Message::Close(_)) => {
                        log::info!("[DashScopeAsr] WebSocket 已关闭");
                        break;
                    }
                    Ok(_) => {}
                    Err(e) => {
                        log::error!("[DashScopeAsr] WebSocket 错误: {}", e);
                        break;
                    }
                }
            }
            is_connected.store(false, std::sync::atomic::Ordering::SeqCst);
            log::info!("[DashScopeAsr] 消息接收任务已结束");
        });

        log::info!("[DashScopeAsr] 会话已启动");
        Ok(())
    }

    async fn send_audio_chunk(&self, pcm: &[f32], sample_rate: u32) -> Result<()> {
        if !self.is_connected.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(anyhow::anyhow!("WebSocket 未连接"));
        }

        // 等待 task-started 确认（最多 5 秒）
        let mut waited = 0u64;
        while !self.task_started.load(std::sync::atomic::Ordering::SeqCst) {
            if waited >= 5000 {
                return Err(anyhow::anyhow!("等待 task-started 超时"));
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            waited += 50;
        }

        // 多声道转单声道
        // WASAPI loopback 可能返回 8 声道 (7.1)：
        //   0=Front L, 1=Front R, 2=Center, 3=LFE,
        //   4=Surround L, 5=Surround R, 6=Side L, 7=Side R
        // 人声主要在 Center (2)，需要加权混合
        let num_channels = self.capture.lock().await.config().channels as usize;

        let mono: Vec<f32> = if num_channels >= 2 {
            pcm.chunks(num_channels)
                .map(|frame| {
                    if num_channels >= 3 {
                        // 7.1 / 5.1: 中置声道权重最高（人声）
                        let fl = frame[0]; // Front Left
                        let fr = frame[1]; // Front Right
                        let center = frame[2]; // Center (speech)
                        let lfe = if num_channels >= 4 { frame[3] } else { 0.0 }; // LFE
                        let sl = if num_channels >= 5 { frame[4] } else { 0.0 }; // Surround L
                        let sr = if num_channels >= 6 { frame[5] } else { 0.0 }; // Surround R
                        // 加权混合：中置 * 0.5 + 前左前右 * 0.25 + 其余衰减
                        (center * 0.5 + (fl + fr) * 0.25 + (sl + sr) * 0.1 + lfe * 0.05)
                    } else {
                        // 立体声：简单平均
                        (frame[0] + frame[1]) * 0.5
                    }
                })
                .collect()
        } else {
            pcm.to_vec()
        };

        // 重采样到 16kHz
        let resampled = resample_linear(&mono, sample_rate, 16000);

        // f32 转 PCM16 字节
        let pcm16: Vec<u8> = resampled.iter()
            .flat_map(|&sample| {
                let clamped = sample.max(-1.0).min(1.0);
                let i16_sample = (clamped * 32767.0).round() as i16;
                i16_sample.to_le_bytes()
            })
            .collect();

        // 音频电平诊断
        let max_amp = resampled.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        let rms = if resampled.is_empty() { 0.0 } else { (resampled.iter().map(|s| s * s).sum::<f32>() / resampled.len() as f32).sqrt() };
        log::debug!(
            "[DashScopeAsr] 发送音频: {}Hz→16kHz, {} 样本(立体声) → {} 样本(单声道) → {} 字节, max={:.4}, rms={:.4}",
            sample_rate, pcm.len(), resampled.len(), pcm16.len(), max_amp, rms
        );

        // 录制到 WAV 文件（调试用）
        if let Ok(mut writer_guard) = self.wav_writer.lock() {
            if let Some(ref mut writer) = *writer_guard {
                if let Err(e) = writer.write_pcm16(&pcm16) {
                    log::warn!("[DashScopeAsr] WAV 写入失败: {}", e);
                }
            }
        }

        // 分帧发送：每帧约 100ms（16kHz/16bit/mono = 3200 字节/帧）
        let frame_bytes = (16000 * 2 * 100) / 1000; // 100ms of PCM16 mono at 16kHz

        let mut sender_guard = self.ws_sender.lock().await;
        if let Some(ref mut sender) = *sender_guard {
            for chunk in pcm16.chunks(frame_bytes) {
                sender
                    .send(Message::Binary(chunk.to_vec().into()))
                    .await
                    .context("发送音频帧失败")?;
            }
        } else {
            return Err(anyhow::anyhow!("WebSocket sender 未初始化"));
        }

        Ok(())
    }

    async fn stop_session(&self) -> Result<()> {
        if !self.is_connected.load(std::sync::atomic::Ordering::SeqCst) {
            return Ok(());
        }

        log::info!("[DashScopeAsr] 停止会话");

        let task_id = self.task_id.lock().await.clone().unwrap_or_default();

        // 发送 finish-task
        let mut sender_guard = self.ws_sender.lock().await;
        if let Some(ref mut sender) = *sender_guard {
            Self::send_finish_task(sender, &task_id).await?;
        }

        *sender_guard = None;
        *self.task_id.lock().await = None;
        self.task_started.store(false, std::sync::atomic::Ordering::SeqCst);
        self.is_connected.store(false, std::sync::atomic::Ordering::SeqCst);

        // 完成 WAV 录音
        if let Ok(mut guard) = self.wav_writer.lock() {
            if let Some(writer) = guard.take() {
                let data_size = writer.data_size;
                if let Err(e) = writer.finish() {
                    log::warn!("[DashScopeAsr] WAV 完成失败: {}", e);
                } else {
                    log::info!("[DashScopeAsr] WAV 录音完成, {} 字节音频数据", data_size);
                }
            }
        }

        log::info!("[DashScopeAsr] 会话已停止");
        Ok(())
    }
}
