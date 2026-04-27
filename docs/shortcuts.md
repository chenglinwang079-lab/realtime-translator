# RealtimeTranslator 快捷键汇总

> 最后更新: 2026-04-26

## 全局快捷键 (系统级)

这些快捷键在应用运行时全局生效，即使应用不在前台也能触发。

| 快捷键 | 功能 | 定义位置 |
|--------|------|----------|
| `Ctrl+Shift+T` | 读取剪贴板内容并触发翻译 | `src-tauri/src/shortcuts.rs` |

### 实现细节

```rust
// shortcuts.rs
let shortcut = "Ctrl+Shift+T";

app.global_shortcut().on_shortcut(
    shortcut,
    move |_app, _shortcut, event| {
        if event.state == ShortcutState::Pressed {
            // 1. 读取剪贴板
            // 2. 过滤空文本和超长文本 (>5000 字符)
            // 3. emit "clipboard-changed" 事件
        }
    },
)?;
```

## 应用内快捷键 (UI 级)

这些快捷键仅在应用窗口获得焦点时生效。

| 快捷键 | 功能 | 定义位置 |
|--------|------|----------|
| `Escape` | 关闭气泡窗口 | `src/components/bubble/FloatingBubble.tsx` |

### 实现细节

```typescript
// FloatingBubble.tsx
useEffect(() => {
  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Escape" && bubbleState !== "dismissed") {
      e.preventDefault();
      handleClose();
    }
  };
  window.addEventListener("keydown", handleKeyDown);
  return () => window.removeEventListener("keydown", handleKeyDown);
}, [bubbleState, handleClose]);
```

## IPC 命令接口

前端可通过以下 IPC 命令动态管理快捷键：

| 命令 | 说明 | 参数 |
|------|------|------|
| `register_shortcut` | 注册新的全局快捷键 | `shortcut: string` |
| `unregister_shortcut` | 注销指定快捷键 | `shortcut: string` |
| `shortcut-triggered` | 监听快捷键触发事件 | callback |

### TypeScript 接口

```typescript
// src/lib/tauri-bridge.ts
export async function registerShortcut(shortcut: string): Promise<void> {
  return invoke("register_shortcut", { shortcut });
}

export async function unregisterShortcut(shortcut: string): Promise<void> {
  return invoke("unregister_shortcut", { shortcut });
}

export function onShortcutTriggered(
  callback: () => void
): Promise<UnlistenFn> {
  return listen("shortcut-triggered", () => callback());
}
```

## 设置中的快捷键配置

`AppSettings` 接口包含 `shortcut` 字段，用于存储用户自定义的快捷键：

```typescript
export interface AppSettings {
  theme: "light" | "dark" | "system";
  defaultSourceLang: string;
  defaultTargetLang: string;
  defaultEngine: string;
  autoStart: boolean;
  enableHistory: boolean;
  shortcut: string;  // 默认: "Ctrl+Shift+T"
}
```

## 快捷键格式规范

Tauri 全局快捷键支持以下修饰键组合：

- **修饰键**: `Ctrl`, `Shift`, `Alt`, `Super` (Windows 键 / macOS Command)
- **字母键**: `A` - `Z`
- **数字键**: `0` - `9`
- **功能键**: `F1` - `F24`
- **特殊键**: `Space`, `Tab`, `Enter`, `Escape`, `Backspace`, `Delete`

### 示例格式

```
Ctrl+Shift+T
Alt+F1
Super+Shift+A
Ctrl+Alt+Delete
```

## 未来扩展

根据 MVP 计划，以下快捷键功能待实现：

- [ ] 用户自定义快捷键（设置面板）
- [ ] 快捷键冲突检测
- [ ] 快捷键禁用/启用开关
- [ ] 多快捷键支持（同时注册多个）

---

> 相关文件:
> - `src-tauri/src/shortcuts.rs` - 全局快捷键注册
> - `src/components/bubble/FloatingBubble.tsx` - UI 快捷键监听
> - `src/lib/tauri-bridge.ts` - IPC 快捷键接口
> - `src/stores/settingsStore.ts` - 快捷键设置存储
