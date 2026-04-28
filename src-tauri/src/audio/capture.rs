use anyhow::{Context, Result};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// 系统音频捕获配置
pub struct AudioCaptureConfig {
    /// 采样率（Hz）
    pub sample_rate: u32,
    /// 声道数
    pub channels: u16,
    /// 每个采样的位数
    pub bits_per_sample: u16,
    /// 缓冲区时长（毫秒）
    pub buffer_duration_ms: u32,
}

impl Default for AudioCaptureConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            channels: 2,
            bits_per_sample: 32,
            buffer_duration_ms: 100,
        }
    }
}

/// 系统音频捕获状态
#[cfg(target_os = "windows")]
pub struct SystemAudioCapture {
    config: AudioCaptureConfig,
    is_running: Arc<Mutex<bool>>,
    buffer: Arc<Mutex<Vec<f32>>>,
}

#[cfg(target_os = "windows")]
impl SystemAudioCapture {
    /// 创建新的系统音频捕获实例
    pub fn new(config: AudioCaptureConfig) -> Result<Self> {
        Ok(Self {
            config,
            is_running: Arc::new(Mutex::new(false)),
            buffer: Arc::new(Mutex::new(Vec::new())),
        })
    }

    /// 开始捕获系统音频
    pub fn start(&mut self) -> Result<()> {
        let mut is_running = self.is_running.lock().unwrap();
        if *is_running {
            return Ok(());
        }
        *is_running = true;

        let is_running = self.is_running.clone();
        let buffer = self.buffer.clone();
        let config = AudioCaptureConfig {
            sample_rate: self.config.sample_rate,
            channels: self.config.channels,
            bits_per_sample: self.config.bits_per_sample,
            buffer_duration_ms: self.config.buffer_duration_ms,
        };

        // 启动后台线程进行音频捕获
        std::thread::spawn(move || {
            if let Err(e) = Self::capture_loop(&is_running, &buffer, &config) {
                log::error!("[AudioCapture] 捕获循环错误: {}", e);
            }
        });

        log::info!("[AudioCapture] 开始捕获系统音频");
        Ok(())
    }

    /// 停止捕获系统音频
    pub fn stop(&mut self) -> Result<()> {
        let mut is_running = self.is_running.lock().unwrap();
        *is_running = false;

        // 清空缓冲区
        let mut buffer = self.buffer.lock().unwrap();
        buffer.clear();

        log::info!("[AudioCapture] 停止捕获系统音频");
        Ok(())
    }

    /// 读取缓冲区中的音频数据
    pub fn read_chunk(&self, duration_ms: u32) -> Option<Vec<f32>> {
        let mut buffer = self.buffer.lock().unwrap();
        let samples_needed = (self.config.sample_rate * duration_ms / 1000) as usize
            * self.config.channels as usize;

        if buffer.len() < samples_needed {
            return None;
        }

        Some(buffer.drain(..samples_needed).collect())
    }

    /// 检查是否正在运行
    pub fn is_running(&self) -> bool {
        *self.is_running.lock().unwrap()
    }

    /// 获取配置
    pub fn config(&self) -> &AudioCaptureConfig {
        &self.config
    }

    /// 音频捕获循环（Windows WASAPI）
    fn capture_loop(
        is_running: &Arc<Mutex<bool>>,
        buffer: &Arc<Mutex<Vec<f32>>>,
        config: &AudioCaptureConfig,
    ) -> Result<()> {
        // TODO: 实现 WASAPI loopback 捕获
        // 这里需要使用 Windows WASAPI API 来捕获系统音频
        //
        // 步骤：
        // 1. 初始化 COM
        // 2. 获取 IMMDeviceEnumerator
        // 3. 获取默认音频输出设备
        // 4. 激活 IAudioClient
        // 5. 设置共享模式
        // 6. 启动捕获
        // 7. 循环读取音频数据
        //
        // 目前使用占位实现，返回静音数据
        log::warn!("[AudioCapture] WASAPI loopback 尚未实现，返回静音数据");

        let samples_per_ms = config.sample_rate / 1000 * config.channels as u32;
        let chunk_size = samples_per_ms * config.buffer_duration_ms;

        while *is_running.lock().unwrap() {
            // 生成静音数据（占位）
            let silence: Vec<f32> = vec![0.0; chunk_size as usize];

            {
                let mut buf = buffer.lock().unwrap();
                buf.extend_from_slice(&silence);

                // 限制缓冲区大小（最多保存 5 秒音频）
                let max_samples = (config.sample_rate * 5 * config.channels as u32) as usize;
                if buf.len() > max_samples {
                    let drain_count = buf.len() - max_samples;
                    buf.drain(..drain_count);
                }
            }

            std::thread::sleep(Duration::from_millis(config.buffer_duration_ms as u64));
        }

        Ok(())
    }
}

/// 非 Windows 平台的占位实现
#[cfg(not(target_os = "windows"))]
pub struct SystemAudioCapture {
    config: AudioCaptureConfig,
    is_running: bool,
}

#[cfg(not(target_os = "windows"))]
impl SystemAudioCapture {
    pub fn new(config: AudioCaptureConfig) -> Result<Self> {
        Ok(Self {
            config,
            is_running: false,
        })
    }

    pub fn start(&mut self) -> Result<()> {
        log::warn!("[AudioCapture] 当前平台不支持系统音频捕获");
        self.is_running = true;
        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        self.is_running = false;
        Ok(())
    }

    pub fn read_chunk(&self, _duration_ms: u32) -> Option<Vec<f32>> {
        None
    }

    pub fn is_running(&self) -> bool {
        self.is_running
    }

    pub fn config(&self) -> &AudioCaptureConfig {
        &self.config
    }
}
