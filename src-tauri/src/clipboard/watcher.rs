use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::sync::{oneshot, Mutex};

/// 剪贴板监听配置
#[derive(Debug, Clone)]
pub struct WatcherConfig {
    /// 轮询间隔（毫秒），默认 500
    pub poll_interval_ms: u64,
    /// 最大文本长度，超过则忽略，默认 5000
    pub max_text_length: usize,
    /// 最小文本长度，低于则忽略，默认 1
    pub min_text_length: usize,
    /// 防抖时间（毫秒），在此时间内重复变化只触发最后一次，默认 300
    pub debounce_ms: u64,
}

impl Default for WatcherConfig {
    fn default() -> Self {
        Self {
            poll_interval_ms: 500,
            max_text_length: 5000,
            min_text_length: 1,
            debounce_ms: 300,
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardChangedEvent {
    pub text: String,
    pub source: String,
}

struct WatcherInner {
    running: AtomicBool,
    stop_tx: Mutex<Option<oneshot::Sender<()>>>,
    last_text: Mutex<String>,
    config: WatcherConfig,
}

#[derive(Clone)]
pub struct ClipboardWatcher {
    inner: Arc<WatcherInner>,
}

impl ClipboardWatcher {
    pub fn new() -> Self {
        Self::with_config(WatcherConfig::default())
    }

    pub fn with_config(config: WatcherConfig) -> Self {
        Self {
            inner: Arc::new(WatcherInner {
                running: AtomicBool::new(false),
                stop_tx: Mutex::new(None),
                last_text: Mutex::new(String::new()),
                config,
            }),
        }
    }

    pub async fn start(&self, app: AppHandle) {
        if self.inner.running.swap(true, Ordering::SeqCst) {
            println!("[ClipboardWatcher] 已经在运行，跳过重复启动");
            return;
        }

        println!(
            "[ClipboardWatcher] 启动剪贴板轮询 (间隔={}ms, 防抖={}ms, 长度限制={}-{})",
            self.inner.config.poll_interval_ms,
            self.inner.config.debounce_ms,
            self.inner.config.min_text_length,
            self.inner.config.max_text_length,
        );

        let (tx, mut rx) = oneshot::channel::<()>();
        {
            let mut stop_tx = self.inner.stop_tx.lock().await;
            *stop_tx = Some(tx);
        }

        let inner = self.inner.clone();

        tokio::spawn(async move {
            let poll_interval =
                tokio::time::Duration::from_millis(inner.config.poll_interval_ms);
            let debounce_duration =
                tokio::time::Duration::from_millis(inner.config.debounce_ms);

            // 防抖状态：pending_text 保存待触发的文本
            let mut pending_text: Option<String> = None;
            let mut debounce_deadline: Option<tokio::time::Instant> = None;

            loop {
                tokio::select! {
                    _ = &mut rx => {
                        println!("[ClipboardWatcher] 收到停止信号");
                        break;
                    }
                    _ = tokio::time::sleep(poll_interval) => {
                        // 读取剪贴板
                        let text = match read_clipboard_text() {
                            Some(t) => t,
                            None => {
                                // 无内容，检查防抖是否到期
                                if let Some(deadline) = debounce_deadline {
                                    if tokio::time::Instant::now() >= deadline {
                                        if let Some(text) = pending_text.take() {
                                            emit_clipboard_changed(&app, &text);
                                        }
                                        debounce_deadline = None;
                                    }
                                }
                                continue;
                            }
                        };

                        // 过滤逻辑
                        if !should_translate(&text, &inner.config) {
                            // 检查防抖是否到期（即使当前文本被过滤）
                            if let Some(deadline) = debounce_deadline {
                                if tokio::time::Instant::now() >= deadline {
                                    if let Some(text) = pending_text.take() {
                                        emit_clipboard_changed(&app, &text);
                                    }
                                    debounce_deadline = None;
                                }
                            }
                            continue;
                        }

                        // 与上次相同则跳过
                        let mut last = inner.last_text.lock().await;
                        if *last == text {
                            drop(last);
                            // 检查防抖是否到期
                            if let Some(deadline) = debounce_deadline {
                                if tokio::time::Instant::now() >= deadline {
                                    if let Some(text) = pending_text.take() {
                                        emit_clipboard_changed(&app, &text);
                                    }
                                    debounce_deadline = None;
                                }
                            }
                            continue;
                        }
                        *last = text.clone();
                        drop(last);

                        // 设置防抖：保存文本，重置计时器
                        pending_text = Some(text);
                        debounce_deadline = Some(tokio::time::Instant::now() + debounce_duration);
                    }
                }
            }

            // 退出前触发待处理的事件
            if let Some(text) = pending_text {
                emit_clipboard_changed(&app, &text);
            }

            inner.running.store(false, Ordering::SeqCst);
        });
    }

    pub async fn stop(&self) {
        let mut stop_tx = self.inner.stop_tx.lock().await;
        if let Some(tx) = stop_tx.take() {
            let _ = tx.send(());
        }
        self.inner.running.store(false, Ordering::SeqCst);
    }

    pub fn is_running(&self) -> bool {
        self.inner.running.load(Ordering::SeqCst)
    }
}

/// 判断文本是否应该触发翻译
fn should_translate(text: &str, config: &WatcherConfig) -> bool {
    let trimmed = text.trim();

    // 长度检查
    if trimmed.len() < config.min_text_length || trimmed.len() > config.max_text_length {
        return false;
    }

    // 纯空白（二次检查，trimmed 后）
    if trimmed.is_empty() {
        return false;
    }

    // 纯标点符号过滤
    if is_only_punctuation(trimmed) {
        return false;
    }

    // 纯数字过滤（可选：数字也可能需要翻译，如 "一二三"）
    // 这里只过滤阿拉伯数字
    if trimmed.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }

    true
}

/// 判断是否全是标点符号
fn is_only_punctuation(text: &str) -> bool {
    text.chars().all(|c| {
        // 标点符号、空白、控制字符
        c.is_ascii_punctuation()
            || c.is_whitespace()
            || unicode_is_punctuation(c)
    })
}

/// Unicode 标点检测（CJK 标点等）
fn unicode_is_punctuation(c: char) -> bool {
    matches!(c,
        '\u{3000}'..='\u{303F}' |  // CJK 符号和标点
        '\u{FF01}'..='\u{FF0F}' |  // 全角 ASCII 标点
        '\u{FF1A}'..='\u{FF20}' |  // 全角 ASCII 标点
        '\u{FF3B}'..='\u{FF40}' |  // 全角 ASCII 标点
        '\u{FF5B}'..='\u{FF65}'    // 全角 ASCII 标点
    )
}

/// 发送剪贴板变化事件
fn emit_clipboard_changed(app: &AppHandle, text: &str) {
    println!("[ClipboardWatcher] 检测到剪贴板变化: {} 字符", text.len());
    let event = ClipboardChangedEvent {
        text: text.to_string(),
        source: "clipboard-watch".to_string(),
    };
    let _ = app.emit("clipboard-changed", &event);
}

/// 读取剪贴板文本
fn read_clipboard_text() -> Option<String> {
    let mut clipboard = arboard::Clipboard::new().ok()?;
    clipboard.get_text().ok()
}
