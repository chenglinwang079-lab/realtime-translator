use anyhow::{Context, Result};
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

use super::engine::{LiveTranslationPayload, SpeechTranslationEngine};

/// 阿里云实时语音识别配置
pub struct AliyunStreamingAsrConfig {
    /// Access Key ID
    pub access_key_id: String,
    /// Access Key Secret
    pub access_key_secret: String,
    /// App Key
    pub app_key: String,
    /// 服务地址
    pub endpoint: String,
}

/// Token 响应
#[derive(Deserialize)]
struct TokenResponse {
    #[serde(rename = "RequestId", default)]
    request_id: Option<String>,
    #[serde(rename = "Token")]
    token: TokenInfo,
}

#[derive(Deserialize)]
struct TokenInfo {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "ExpireTime")]
    expire_time: u64,
}

/// WebSocket sender 类型
type WsSender = futures_util::stream::SplitSink<
    WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>,
    Message,
>;

/// 阿里云实时语音识别引擎
pub struct AliyunStreamingAsrEngine {
    config: AliyunStreamingAsrConfig,
    /// 结果发送通道 (使用 std::sync::Mutex 以支持同步写入)
    result_tx: Arc<std::sync::Mutex<Option<mpsc::Sender<LiveTranslationPayload>>>>,
    /// WebSocket 连接状态
    is_connected: Arc<std::sync::atomic::AtomicBool>,
    /// WebSocket sender
    ws_sender: Arc<tokio::sync::Mutex<Option<WsSender>>>,
    /// 当前会话的 task_id
    task_id: Arc<tokio::sync::Mutex<Option<String>>>,
}

impl AliyunStreamingAsrEngine {
    /// 创建新的阿里云实时语音识别引擎
    pub fn new(config: AliyunStreamingAsrConfig) -> Self {
        Self {
            config,
            result_tx: Arc::new(std::sync::Mutex::new(None)),
            is_connected: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            ws_sender: Arc::new(tokio::sync::Mutex::new(None)),
            task_id: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }

    /// 获取阿里云 NLS Token
    async fn get_nls_token(&self) -> Result<String> {
        use hmac::{Hmac, Mac};
        use sha1::Sha1;
        type HmacSha1 = Hmac<Sha1>;

        let url = "https://nls-meta.cn-shanghai.aliyuncs.com/";

        // 构建请求参数
        let mut params: Vec<(&str, String)> = vec![
            ("AccessKeyId", self.config.access_key_id.clone()),
            ("Action", "CreateToken".to_string()),
            ("Format", "JSON".to_string()),
            ("RegionId", "cn-shanghai".to_string()),
            ("SignatureMethod", "HMAC-SHA1".to_string()),
            ("SignatureNonce", uuid::Uuid::new_v4().to_string()),
            ("SignatureVersion", "1.0".to_string()),
            ("Timestamp", chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()),
            ("Version", "2019-02-28".to_string()),
        ];

        // 按参数名排序
        params.sort_by_key(|&(k, _)| k.to_string());

        // 构建规范化查询字符串
        let canonical_query = params
            .iter()
            .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&");

        // 构建待签名字符串
        let string_to_sign = format!("POST&{}&{}", urlencoding::encode("/"), urlencoding::encode(&canonical_query));

        // 使用 HMAC-SHA1 签名
        let signing_key = format!("{}&", self.config.access_key_secret);
        let mut mac = HmacSha1::new_from_slice(signing_key.as_bytes())
            .map_err(|e| anyhow::anyhow!("HMAC key error: {}", e))?;
        mac.update(string_to_sign.as_bytes());
        let signature = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, mac.finalize().into_bytes());

        // 构建完整请求体
        let full_query = format!("{}&Signature={}", canonical_query, urlencoding::encode(&signature));

        // 发送请求
        let client = reqwest::Client::new();
        let response = client
            .post(url)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(full_query)
            .send()
            .await
            .context("获取 token 请求失败")?;

        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        log::debug!("[AliyunStreamingAsr] Token 响应 ({}): {}", status, body);

        if !status.is_success() {
            return Err(anyhow::anyhow!("获取 token 失败 ({}): {}", status, body));
        }

        let token_response: TokenResponse = serde_json::from_str(&body)
            .context("解析 token 响应失败")?;

        log::info!("[AliyunStreamingAsr] 获取 token 成功, 过期时间: {}", token_response.token.expire_time);

        Ok(token_response.token.id)
    }

    /// 获取 WebSocket URL
    async fn get_ws_url(&self) -> Result<String> {
        log::debug!("[AliyunStreamingAsr] get_ws_url: 开始获取 token");
        let token = self.get_nls_token().await?;
        log::debug!("[AliyunStreamingAsr] get_ws_url: token={}", &token[..8]);

        // 实时语音识别 WebSocket endpoint（无 /translate 路径）
        let url = format!(
            "wss://{}/ws/v1?token={}",
            self.config.endpoint, token
        );
        log::debug!("[AliyunStreamingAsr] get_ws_url: url={}", url);
        Ok(url)
    }

    /// 发送开始帧，返回 task_id
    async fn send_start_frame(
        ws_sender: &mut WsSender,
        config: &AliyunStreamingAsrConfig,
    ) -> Result<String> {
        let task_id = uuid::Uuid::new_v4().as_simple().to_string();
        let start_frame = serde_json::json!({
            "header": {
                "message_id": uuid::Uuid::new_v4().as_simple().to_string(),
                "task_id": task_id,
                "action": "start",
                "namespace": "SpeechTranscriber",
                "name": "StartTranscription",
                "appkey": config.app_key
            },
            "payload": {
                "format": "pcm",
                "sample_rate": 16000,
                "language": "en-US",
                "enable_intermediate_result": true,
                "enable_punctuation_prediction": true,
                "enable_inverse_text_normalization": true
            }
        });

        ws_sender
            .send(Message::Text(start_frame.to_string().into()))
            .await
            .context("发送开始帧失败")?;

        log::info!("[AliyunStreamingAsr] 已发送开始帧, task_id={}", task_id);
        Ok(task_id)
    }

    /// 发送停止帧
    async fn send_stop_frame(
        ws_sender: &mut WsSender,
        task_id: &str,
        config: &AliyunStreamingAsrConfig,
    ) -> Result<()> {
        let stop_frame = serde_json::json!({
            "header": {
                "message_id": uuid::Uuid::new_v4().as_simple().to_string(),
                "task_id": task_id,
                "action": "stop",
                "namespace": "SpeechTranscriber",
                "name": "StopTranscription",
                "appkey": config.app_key
            },
            "payload": {}
        });

        ws_sender
            .send(Message::Text(stop_frame.to_string().into()))
            .await
            .context("发送停止帧失败")?;

        log::info!("[AliyunStreamingAsr] 已发送停止帧");
        Ok(())
    }

    /// 处理服务端消息
    fn handle_server_message(
        message: &str,
        result_tx: Option<&mpsc::Sender<LiveTranslationPayload>>,
        chunk_id: &mut u32,
    ) -> Result<()> {
        let msg: serde_json::Value = serde_json::from_str(message)
            .context("解析服务端消息失败")?;

        // 检查消息类型
        let header = msg.get("header").context("消息缺少 header 字段")?;
        let name = header.get("name").and_then(|n| n.as_str()).unwrap_or("");

        match name {
            "TranscriptionStarted" => {
                log::info!("[AliyunStreamingAsr] 会话已开始");
            }
            "SentenceBegin" => {
                log::debug!("[AliyunStreamingAsr] 句子开始");
            }
            "TranscriptionResultChanged" => {
                let payload = msg.get("payload").context("消息缺少 payload 字段")?;

                // 提取识别文本
                let transcript_text = payload
                    .get("result")
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string();

                log::debug!(
                    "[AliyunStreamingAsr] 中间结果: {}",
                    if transcript_text.len() > 20 {
                        &transcript_text[..20]
                    } else {
                        &transcript_text
                    }
                );

                // 中间结果：is_final = false，translated_text 留空，不触发翻译
                let result = LiveTranslationPayload {
                    transcript_text,
                    translated_text: String::new(),
                    source_language: None,
                    target_language: None,
                    is_final: false,
                    chunk_id: *chunk_id,
                    timestamp_ms: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64,
                    duration_ms: None,
                };

                *chunk_id += 1;

                // 发送结果
                if let Some(tx) = result_tx {
                    let tx = tx.clone();
                    tokio::spawn(async move {
                        if let Err(e) = tx.send(result).await {
                            log::error!("[AliyunStreamingAsr] 发送结果失败: {}", e);
                        }
                    });
                }
            }
            "SentenceEnd" => {
                let payload = msg.get("payload").context("消息缺少 payload 字段")?;

                // 提取最终识别文本（完整句子）
                let transcript_text = payload
                    .get("result")
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string();

                log::info!(
                    "[AliyunStreamingAsr] 最终结果: {}",
                    if transcript_text.len() > 20 {
                        &transcript_text[..20]
                    } else {
                        &transcript_text
                    }
                );

                // 最终结果：is_final = true，translated_text 留空（由外层翻译引擎填充）
                let result = LiveTranslationPayload {
                    transcript_text,
                    translated_text: String::new(),
                    source_language: None,
                    target_language: None,
                    is_final: true,
                    chunk_id: *chunk_id,
                    timestamp_ms: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64,
                    duration_ms: None,
                };

                *chunk_id += 1;

                // 发送结果
                if let Some(tx) = result_tx {
                    let tx = tx.clone();
                    tokio::spawn(async move {
                        if let Err(e) = tx.send(result).await {
                            log::error!("[AliyunStreamingAsr] 发送结果失败: {}", e);
                        }
                    });
                }
            }
            "TranscriptionCompleted" => {
                log::info!("[AliyunStreamingAsr] 转写已完成");
            }
            "TaskFailed" => {
                let header = msg.get("header").context("消息缺少 header 字段")?;
                let status = header.get("status").and_then(|s| s.as_u64()).unwrap_or(0);
                let error_msg = header
                    .get("status_text")
                    .and_then(|s| s.as_str())
                    .unwrap_or("未知错误");
                log::error!("[AliyunStreamingAsr] 任务失败 (status={}): {}", status, error_msg);
                log::error!("[AliyunStreamingAsr] 完整错误消息: {}", msg);
                return Err(anyhow::anyhow!("识别任务失败 ({}): {}", status, error_msg));
            }
            _ => {
                log::debug!("[AliyunStreamingAsr] 收到未知消息类型: {}", name);
            }
        }

        Ok(())
    }

    /// 音频格式转换：f32 PCM 重采样（带抗混叠低通滤波 + sinc 插值）
    fn resample_linear(pcm: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
        if from_rate == to_rate {
            return pcm.to_vec();
        }

        let ratio = from_rate as f64 / to_rate as f64;
        let new_len = (pcm.len() as f64 / ratio).ceil() as usize;

        // 抗混叠低通滤波：sinc 窗口滤波器
        // 截止频率 = min(Nyquist_from, Nyquist_to) = to_rate/2
        let cutoff = 0.9 / ratio; // 归一化截止频率（相对于 from_rate 的 Nyquist）
        let tap_count = 64; // 滤波器阶数
        let mut filtered = vec![0.0f32; pcm.len()];

        for i in 0..pcm.len() {
            let mut sum = 0.0f32;
            let mut weight_sum = 0.0f32;
            for tap in 0..tap_count {
                let offset = tap as i32 - (tap_count as i32 / 2);
                let idx = i as i32 + offset;
                if idx >= 0 && (idx as usize) < pcm.len() {
                    let x = offset as f64;
                    // sinc 低通
                    let sinc = if x.abs() < 1e-6 {
                        1.0
                    } else {
                        (std::f64::consts::PI * cutoff * x).sin() / (std::f64::consts::PI * x)
                    };
                    // Hann 窗口
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

        // sinc 插值重采样
        let mut result = Vec::with_capacity(new_len);
        for i in 0..new_len {
            let src_pos = i as f64 * ratio;
            let center = src_pos as usize;
            let frac = (src_pos - center as f64) as f64;

            // 8 点 sinc 插值
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

    /// 音频格式转换：f32 转 PCM16
    fn convert_f32_to_pcm16(pcm: &[f32]) -> Vec<u8> {
        let mut result = Vec::with_capacity(pcm.len() * 2);
        for &sample in pcm {
            // 将 f32 [-1.0, 1.0] 转换为 i16 [-32768, 32767]
            let sample_i16 = (sample * 32767.0).round() as i16;
            result.extend_from_slice(&sample_i16.to_le_bytes());
        }
        result
    }
}

#[async_trait]
impl SpeechTranslationEngine for AliyunStreamingAsrEngine {
    fn engine_id(&self) -> &str {
        "aliyun-streaming-asr"
    }

    fn is_available(&self) -> bool {
        !self.config.access_key_id.is_empty()
            && !self.config.access_key_secret.is_empty()
            && !self.config.app_key.is_empty()
    }

    fn set_result_channel(&self, tx: mpsc::Sender<LiveTranslationPayload>) {
        *self.result_tx.lock().unwrap() = Some(tx);
    }

    async fn start_session(&self) -> Result<()> {
        if !self.is_available() {
            return Err(anyhow::anyhow!("阿里云语音识别引擎未配置"));
        }

        log::info!("[AliyunStreamingAsr] 开始会话");

        // 建立 WebSocket 连接
        let url = self.get_ws_url().await.map_err(|e| {
            log::error!("[AliyunStreamingAsr] get_ws_url 失败: {:?}", e);
            e
        })?;
        log::info!("[AliyunStreamingAsr] 连接到: {}", url);

        let (ws_stream, _) = connect_async(&url)
            .await
            .context("WebSocket 连接失败")?;

        log::info!("[AliyunStreamingAsr] WebSocket 连接成功");

        let (ws_sender, mut ws_receiver) = ws_stream.split();

        // 保存 ws_sender
        *self.ws_sender.lock().await = Some(ws_sender);

        // 发送开始帧
        {
            let mut sender_guard = self.ws_sender.lock().await;
            if let Some(ref mut sender) = *sender_guard {
                let tid = Self::send_start_frame(sender, &self.config).await?;
                *self.task_id.lock().await = Some(tid);
            }
        }

        // 更新连接状态
        self.is_connected.store(true, std::sync::atomic::Ordering::SeqCst);

        // 启动消息接收任务
        let result_tx = self.result_tx.lock().unwrap().clone();
        let is_connected = self.is_connected.clone();
        let mut chunk_id = 0;

        tokio::spawn(async move {
            log::info!("[AliyunStreamingAsr] 消息接收任务已启动");
            while let Some(msg) = ws_receiver.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        log::debug!("[AliyunStreamingAsr] 收到服务端消息: {}", if text.len() > 200 { &text[..200] } else { &text });
                        if let Err(e) = Self::handle_server_message(
                            &text,
                            result_tx.as_ref(),
                            &mut chunk_id,
                        ) {
                            log::error!("[AliyunStreamingAsr] 处理消息失败: {}", e);
                        }
                    }
                    Ok(Message::Ping(d)) => {
                        log::debug!("[AliyunStreamingAsr] 收到 Ping: {:?}", d);
                    }
                    Ok(Message::Pong(d)) => {
                        log::debug!("[AliyunStreamingAsr] 收到 Pong: {:?}", d);
                    }
                    Ok(Message::Binary(d)) => {
                        log::debug!("[AliyunStreamingAsr] 收到二进制消息: {} 字节", d.len());
                    }
                    Ok(Message::Close(_)) => {
                        log::info!("[AliyunStreamingAsr] WebSocket 已关闭");
                        break;
                    }
                    Ok(_) => {
                        log::debug!("[AliyunStreamingAsr] 收到其他类型消息");
                    }
                    Err(e) => {
                        log::error!("[AliyunStreamingAsr] WebSocket 错误: {}", e);
                        break;
                    }
                }
            }

            // 连接已断开
            is_connected.store(false, std::sync::atomic::Ordering::SeqCst);
            log::info!("[AliyunStreamingAsr] 消息接收任务已结束");
        });

        log::info!("[AliyunStreamingAsr] 会话已启动");
        Ok(())
    }

    async fn send_audio_chunk(&self, pcm: &[f32], sample_rate: u32) -> Result<()> {
        if !self.is_connected.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(anyhow::anyhow!("WebSocket 未连接"));
        }

        // 立体声转单声道（取左右声道平均值）
        let mono = if pcm.len() >= 2 {
            pcm.chunks(2)
                .map(|frame| (frame[0] + frame.get(1).copied().unwrap_or(frame[0])) * 0.5)
                .collect::<Vec<f32>>()
        } else {
            pcm.to_vec()
        };

        // 重采样到 16kHz
        let resampled = Self::resample_linear(&mono, sample_rate, 16000);

        // 转换为 PCM16
        let pcm16 = Self::convert_f32_to_pcm16(&resampled);

        // 记录 chunk 元信息和音频电平
        let max_amp = mono.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        let rms = (mono.iter().map(|s| s * s).sum::<f32>() / mono.len() as f32).sqrt();
        log::debug!(
            "[AliyunStreamingAsr] 发送音频: {}Hz -> 16kHz, {} 样本(立体声) -> {} 样本(单声道) -> {} 字节, max={:.4}, rms={:.4}",
            sample_rate,
            pcm.len(),
            mono.len(),
            pcm16.len(),
            max_amp,
            rms
        );

        // 分帧发送：阿里云要求每帧约 200ms（16kHz/16bit/mono = 6400 字节/帧）
        const FRAME_BYTES: usize = 6400; // 200ms at 16kHz 16-bit mono

        let mut sender_guard = self.ws_sender.lock().await;
        if let Some(ref mut sender) = *sender_guard {
            for chunk in pcm16.chunks(FRAME_BYTES) {
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

        log::info!("[AliyunStreamingAsr] 停止会话");

        // 获取存储的 task_id
        let task_id = self.task_id.lock().await.clone().unwrap_or_default();

        // 发送停止帧
        let mut sender_guard = self.ws_sender.lock().await;
        if let Some(ref mut sender) = *sender_guard {
            Self::send_stop_frame(sender, &task_id, &self.config).await?;
        }

        // 清理 sender 和 task_id
        *sender_guard = None;
        *self.task_id.lock().await = None;

        // 更新连接状态
        self.is_connected.store(false, std::sync::atomic::Ordering::SeqCst);

        log::info!("[AliyunStreamingAsr] 会话已停止");
        Ok(())
    }
}
