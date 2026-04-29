# Realtime Translator

Windows 桌面实时翻译工具，基于 [Tauri 2](https://v2.tauri.app/) + React + TypeScript 构建。

## 功能

### 实时音频翻译
- 捕获系统音频（WASAPI loopback），实时语音识别后自动翻译
- 支持 DashScope Paraformer-realtime-v2 ASR 引擎
- 支持 8 声道（7.1 环绕声）音频设备，自动降混为单声道
- **逐句实时显示**：ASR 中间结果实时预览当前句，最终结果逐句累积显示原文+译文
- **整合翻译**：停止后可将所有逐句原文合并，调用翻译引擎做一次完整语境翻译
- 实时模式与手动翻译模式可随时切换

### 文本翻译
- 划词翻译：选中文本后通过全局快捷键触发
- 剪贴板监听：自动翻译复制的内容
- 悬浮气泡窗口显示翻译结果，始终置顶
- 侧边栏：手动输入翻译 + 语言切换 + 文件拖入翻译

### 翻译引擎
| 引擎 | 说明 |
|------|------|
| 腾讯云翻译 | 机器翻译 TMT |
| OpenAI | GPT-4o-mini |
| DeepL | Free API |

### OCR 识别
- 截图区域 OCR 识别文字
- 支持 Google Vision / 百度 OCR 引擎

### 辅助功能
- UIA（UI Automation）事件监听，自动抓取前台应用选中文本
- 全局快捷键（可自定义）
- 翻译历史记录
- 深色主题 UI

## 安装

从 [Releases](https://github.com/chenglinwang079-lab/realtime-translator/releases) 下载 `realtime-translator_x.x.x_x64-setup.exe` 安装包。

## 开发

### 环境要求

- [Node.js](https://nodejs.org/) >= 18
- [pnpm](https://pnpm.io/)
- [Rust](https://www.rust-lang.org/tools/install) + MSVC (Windows)
- Windows 10/11

### 启动

```bash
# 安装依赖
pnpm install

# 启动开发服务器
npx tauri dev
```

### 构建安装包

```bash
npx tauri build
```

安装包输出到 `src-tauri/target/release/bundle/nsis/`。

### API Key 配置

启动后在设置面板中配置各引擎的 API Key，或通过环境变量：

```bash
# 翻译引擎
set TENCENT_SECRET_ID=xxx
set TENCENT_SECRET_KEY=xxx
set OPENAI_API_KEY=xxx
set DEEPL_AUTH_KEY=xxx

# ASR
set DASHSCOPE_API_KEY=xxx

# OCR
set GOOGLE_VISION_API_KEY=xxx
set BAIDU_OCR_API_KEY=xxx
```

## 技术栈

| 层 | 技术 |
|----|------|
| 前端 | React 19, TypeScript, Vite, Zustand |
| 后端 | Rust, Tauri 2, Tokio |
| 音频 | Windows WASAPI Loopback |
| ASR | DashScope Paraformer (WebSocket) |
| 打包 | NSIS (Windows Installer) |

## 项目结构

```
src/                          # 前端
  components/
    bubble/                   # 悬浮气泡 UI
    settings/                 # 设置面板
    sidebar/                  # 侧边栏
    region-selector/          # 截图区域选择
  hooks/                      # React Hooks
  stores/                     # Zustand 状态管理
  lib/                        # 工具函数

src-tauri/                    # 后端 (Rust)
  src/
    audio/                    # WASAPI 音频捕获
    speech_translation/       # 实时 ASR (DashScope)
    translation/              # 翻译引擎
    ocr/                      # OCR 引擎
    accessibility/            # UIA 事件监听
    commands/                 # Tauri 命令
    db/                       # SQLite 数据库
```

## License

MIT
