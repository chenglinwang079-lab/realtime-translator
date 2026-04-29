use anyhow::{Context, Result};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[cfg(target_os = "windows")]
use windows::Win32::Media::Audio::*;
#[cfg(target_os = "windows")]
use windows::Win32::System::Com::*;
#[cfg(target_os = "windows")]
use windows::Win32::Media::KernelStreaming::WAVE_FORMAT_EXTENSIBLE;
#[cfg(target_os = "windows")]
use windows::core::*;

use std::sync::atomic::{AtomicU32, Ordering as AtomicOrdering};

/// 系统音频捕获配置
#[derive(Clone)]
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

/// 音频处理常量
const MAX_BUFFER_DURATION_SECS: u32 = 5;
const DEFAULT_CHUNK_DURATION_MS: u32 = 3000;

/// 系统音频捕获状态
#[cfg(target_os = "windows")]
pub struct SystemAudioCapture {
    config: Arc<Mutex<AudioCaptureConfig>>,
    is_running: Arc<Mutex<bool>>,
    buffer: Arc<Mutex<Vec<f32>>>,
    /// 实际设备采样率（由 capture 线程更新）
    actual_sample_rate: Arc<AtomicU32>,
    /// 实际设备声道数（由 capture 线程更新）
    actual_channels: Arc<AtomicU32>,
}

#[cfg(target_os = "windows")]
impl SystemAudioCapture {
    /// 创建新的系统音频捕获实例
    pub fn new(config: AudioCaptureConfig) -> Result<Self> {
        let sample_rate = config.sample_rate;
        let channels = config.channels as u32;
        Ok(Self {
            config: Arc::new(Mutex::new(config)),
            is_running: Arc::new(Mutex::new(false)),
            buffer: Arc::new(Mutex::new(Vec::new())),
            actual_sample_rate: Arc::new(AtomicU32::new(sample_rate)),
            actual_channels: Arc::new(AtomicU32::new(channels)),
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
        let config = self.config.clone();
        let actual_sample_rate = self.actual_sample_rate.clone();
        let actual_channels = self.actual_channels.clone();

        // 启动后台线程进行音频捕获
        std::thread::spawn(move || {
            if let Err(e) = Self::capture_loop(&is_running, &buffer, &config, &actual_sample_rate, &actual_channels) {
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
        let sample_rate = self.actual_sample_rate.load(AtomicOrdering::Relaxed);
        let channels = self.actual_channels.load(AtomicOrdering::Relaxed);
        let samples_needed = (sample_rate * duration_ms / 1000) as usize
            * channels as usize;

        if buffer.len() < samples_needed {
            return None;
        }

        Some(buffer.drain(..samples_needed).collect())
    }

    /// 检查是否正在运行
    pub fn is_running(&self) -> bool {
        *self.is_running.lock().unwrap()
    }

    /// 获取当前采样率
    pub fn sample_rate(&self) -> u32 {
        self.actual_sample_rate.load(AtomicOrdering::Relaxed)
    }

    /// 获取当前声道数
    pub fn channels(&self) -> u16 {
        self.actual_channels.load(AtomicOrdering::Relaxed) as u16
    }

    /// 获取配置（兼容旧接口）
    pub fn config(&self) -> AudioCaptureConfig {
        self.config.lock().unwrap().clone()
    }

    /// 音频捕获循环（Windows WASAPI）
    fn capture_loop(
        is_running: &Arc<Mutex<bool>>,
        buffer: &Arc<Mutex<Vec<f32>>>,
        config: &Arc<Mutex<AudioCaptureConfig>>,
        actual_sample_rate: &Arc<AtomicU32>,
        actual_channels: &Arc<AtomicU32>,
    ) -> Result<()> {
        log::info!("[AudioCapture] 启动 WASAPI loopback 捕获");

        // Step 1: 初始化 COM (STA 模式，WASAPI 要求)
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
            .ok()
            .context("COM 初始化失败")?;

        // COM 清理守卫
        let _com_guard = scopeguard::guard((), |_| {
            unsafe { CoUninitialize() };
        });

        // Step 2: 获取设备枚举器
        let enumerator: IMMDeviceEnumerator = unsafe {
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
        }.context("创建 IMMDeviceEnumerator 失败")?;

        // Step 3: 获取默认音频输出设备（eRender = 播放设备）
        let device = unsafe {
            enumerator.GetDefaultAudioEndpoint(eRender, eConsole)
        }.context("获取默认音频输出设备失败")?;

        // Step 4: 激活 IAudioClient
        let client: IAudioClient = unsafe {
            device.Activate::<IAudioClient>(CLSCTX_ALL, None)
        }.context("激活 IAudioClient 失败")?;

        // Step 5: 获取混音格式
        let wave_format = unsafe { client.GetMixFormat() }
            .context("GetMixFormat 失败")?;

        // 验证格式（WASAPI 共享模式通常是 f32）
        let format = unsafe { *wave_format };
        let samples_per_sec = format.nSamplesPerSec;
        let channels = format.nChannels;
        let bits_per_sample = format.wBitsPerSample;
        log::info!(
            "[AudioCapture] 设备格式: {}Hz, {}声道, {}位",
            samples_per_sec,
            channels,
            bits_per_sample
        );

        // 更新配置为实际设备参数
        let actual_config = AudioCaptureConfig {
            sample_rate: samples_per_sec,
            channels,
            bits_per_sample,
            buffer_duration_ms: 100,
        };
        // 写回共享配置
        *config.lock().unwrap() = actual_config.clone();
        // 更新原子值（供 read_chunk 和外部使用）
        actual_sample_rate.store(samples_per_sec, AtomicOrdering::Relaxed);
        actual_channels.store(channels as u32, AtomicOrdering::Relaxed);

        // 在函数退出前释放 wave_format
        let _format_guard = scopeguard::guard(wave_format, |ptr| {
            unsafe { CoTaskMemFree(Some(ptr as *const _)) };
        });

        // Step 6: 获取设备周期
        let mut default_period: i64 = 0;
        let mut min_period: i64 = 0;
        unsafe { client.GetDevicePeriod(Some(&mut default_period), Some(&mut min_period)) }
            .context("GetDevicePeriod 失败")?;

        // Step 7: 初始化共享模式 + loopback
        unsafe {
            client.Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_LOOPBACK,
                default_period,    // buffer duration (100ns units)
                0,                 // periodicity (0 = let engine decide)
                wave_format,
                None,              // session GUID
            )
        }.context("IAudioClient::Initialize 失败")?;

        // Step 8: 获取捕获客户端
        let capture_client: IAudioCaptureClient = unsafe {
            client.GetService::<IAudioCaptureClient>()
        }.context("获取 IAudioCaptureClient 失败")?;

        // Step 9: 启动流
        unsafe { client.Start() }
            .context("IAudioClient::Start 失败")?;

        log::info!("[AudioCapture] WASAPI loopback 已启动");

        // 计算最大缓冲区大小（使用实际声道数）
        let max_samples = (actual_config.sample_rate * MAX_BUFFER_DURATION_SECS
                           * actual_config.channels as u32) as usize;

        // 10ms 轮询间隔
        let sleep_duration = Duration::from_millis(10);

        // Step 10: 捕获循环
        while *is_running.lock().unwrap() {
            let packet_size = match unsafe { capture_client.GetNextPacketSize() } {
                Ok(size) => size,
                Err(e) => {
                    log::warn!("[AudioCapture] GetNextPacketSize 错误: {}", e);
                    std::thread::sleep(sleep_duration);
                    continue;
                }
            };

            if packet_size > 0 {
                let mut data_ptr: *mut u8 = std::ptr::null_mut();
                let mut num_frames: u32 = 0;
                let mut flags: u32 = 0;

                unsafe {
                    capture_client.GetBuffer(
                        &mut data_ptr,
                        &mut num_frames,
                        &mut flags,
                        None, // device position
                        None, // qpc position
                    )
                }.context("GetBuffer 失败")?;

                let sample_count = num_frames as usize * actual_config.channels as usize;

                // 检查是否静音
                if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 {
                    // 静音帧，填充 0
                    let mut buf = buffer.lock().unwrap();
                    buf.extend(std::iter::repeat(0.0).take(sample_count));
                } else {
                    // WASAPI 交付 IEEE f32，直接重解释
                    let samples = unsafe {
                        std::slice::from_raw_parts(
                            data_ptr as *const f32,
                            sample_count,
                        )
                    };

                    // WASAPI 共享模式的 loopback 数据有时会出现超过 [-1.0, 1.0] 的值
                    // 归一化到 [-1.0, 1.0] 范围，避免下游 PCM16 削波
                    let max_abs = samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
                    let clamped: Vec<f32> = if max_abs > 1.0 {
                        samples.iter().map(|s| (s / max_abs).max(-1.0).min(1.0)).collect()
                    } else {
                        samples.to_vec()
                    };

                    // 记录 RMS 振幅（每 100 帧打印一次）
                    if packet_size % 100 == 0 {
                        let rms: f32 = (clamped.iter().map(|s| s * s).sum::<f32>()
                            / sample_count as f32)
                            .sqrt();
                        log::debug!("[AudioCapture] RMS 振幅: {:.6}, max_abs: {:.4}", rms, max_abs);
                    }

                    // 复制到共享缓冲区
                    let mut buf = buffer.lock().unwrap();
                    buf.extend_from_slice(&clamped);
                }

                // 限制缓冲区大小
                {
                    let mut buf = buffer.lock().unwrap();
                    if buf.len() > max_samples {
                        let drain = buf.len() - max_samples;
                        buf.drain(..drain);
                    }
                }

                unsafe { capture_client.ReleaseBuffer(num_frames) }
                    .context("ReleaseBuffer 失败")?;
            } else {
                std::thread::sleep(sleep_duration);
            }
        }

        // 清理：停止流
        unsafe { client.Stop() }.ok();
        log::info!("[AudioCapture] WASAPI loopback 已停止");

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
