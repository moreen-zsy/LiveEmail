# Tauri 2.0 完整知识文档

> 本文档系统整理 Tauri 2.0 的核心概念、架构原理、开发实践和生态工具，帮助开发者快速掌握这一现代跨平台桌面/移动应用开发框架。

---

## 目录

1. [框架概述](#一框架概述)
2. [架构与进程模型](#二架构与进程模型)
3. [底层原理：TAO 与 WRY](#三底层原理tao-与-wry)
4. [项目初始化与配置](#四项目初始化与配置)
5. [命令系统（Commands）](#五命令系统commands)
6. [插件系统（Plugin System）](#六插件系统plugin-system)
7. [安全模型与权限系统](#七安全模型与权限系统)
8. [IPC 通信机制](#八ipc-通信机制)
9. [移动端支持（iOS/Android）](#九移动端支持iosandroid)
10. [与 Electron 对比](#十与-electron-对比)
11. [最佳实践](#十一最佳实践)
12. [常用命令速查](#十二常用命令速查)

---

## 一、框架概述

### 1.1 什么是 Tauri

Tauri 是一个用于构建**跨平台桌面和移动应用**的框架，使用 Web 前端技术（HTML/CSS/JS）构建用户界面，Rust 提供原生后端能力。

**核心理念**：
- **轻量**：不捆绑浏览器引擎，应用体积仅 2-20 MB
- **安全**：Rust 内存安全 + 严格的 IPC 权限控制
- **高性能**：原生 Rust 后端 + 系统 WebView 渲染
- **跨平台**：一套代码覆盖 Windows、macOS、Linux、iOS、Android

### 1.2 技术栈

| 层级 | 技术 | 说明 |
|-----|------|------|
| 前端 | HTML/CSS/JS + 任意框架 | React、Vue、Svelte、Solid 等 |
| 后端 | Rust | 系统 API 调用、业务逻辑 |
| 渲染 | 系统 WebView | Windows: WebView2 / macOS: WKWebView / Linux: WebKitGTK |
| 构建 | Cargo + Vite/Webpack | Rust 编译 + 前端打包 |

### 1.3 版本演进

| 版本 | 发布时间 | 核心特性 |
|-----|---------|---------|
| Tauri 1.0 | 2022.06 | 稳定版，支持桌面三平台 |
| Tauri 2.0 | 2024.10 | **移动端支持**（iOS/Android）、插件化 API、Capability 权限系统 |
| Tauri 2.5 | 2025.x | IPC 加密增强、性能优化 |

---

## 二、架构与进程模型

### 2.1 三明治架构

```
┌─────────────────────────────────────┐
│         前端应用层 (HTML/CSS/JS)        │
│      React / Vue / Svelte / Solid      │
├─────────────────────────────────────┤
│      Tauri Core (Rust)               │
│   ├── IPC 通信系统                     │
│   ├── 命令路由                         │
│   ├── 插件系统                         │
│   └── 安全拦截（ACL）                  │
├─────────────────────────────────────┤
│      WRY (WebView 渲染抽象层)          │
├─────────────────────────────────────┤
│      TAO (窗口管理器)                  │
├─────────────────────────────────────┤
│      操作系统原生 WebView              │
│  WebView2 / WKWebView / WebKitGTK    │
├─────────────────────────────────────┤
│      操作系统层                        │
│  Windows / macOS / Linux / iOS / Android│
└─────────────────────────────────────┘
```

### 2.2 双进程架构

Tauri 采用**双进程隔离**设计，这是其安全模型的基础：

| 进程 | 技术栈 | 权限级别 | 职责 |
|-----|--------|---------|------|
| **Core Process（核心进程）** | Rust | 🔴 完整系统权限 | 应用入口、窗口管理、系统 API 调用、安全网关 |
| **WebView Process** | 系统 WebView | 🟡 受限沙箱 | UI 渲染、用户交互、前端逻辑执行 |

**关键设计原则**：
1. **前端无系统权限**：WebView 中的 JS 代码无法直接访问文件系统、网络、硬件等
2. **IPC 是唯一通道**：所有系统能力请求必须通过 IPC 由 Rust 代理
3. **Core 是安全网关**：集中拦截、校验、审计所有请求

### 2.3 多窗口架构

```
┌─────────────────────────────────────────┐
│           Core Process (Rust)            │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐  │
│  │ Window  │ │ Window  │ │ Window  │  │
│  │ Manager │ │ Manager │ │ Manager │  │
│  │ (main)  │ │ (settings)│ │ (about) │  │
│  └────┬────┘ └────┬────┘ └────┬────┘  │
│       └─────────────┴─────────────┘    │
│              共享全局状态                  │
└─────────────────────────────────────────┘
         │              │              │
    ┌────┴────┐    ┌────┴────┐    ┌────┴────┐
    │ WebView │    │ WebView │    │ WebView │
    │ (main)  │    │(settings)│    │ (about) │
    └─────────┘    └─────────┘    └─────────┘
```

每个窗口拥有独立的 WebView 进程，但共享同一个 Rust Core 进程，便于全局状态同步。

---

## 三、底层原理：TAO 与 WRY

### 3.1 TAO：跨平台窗口管理器

**TAO**（Tauri Application Operating System）是 Tauri 的底层窗口管理库，基于知名的 Rust 窗口库 `winit` 分支开发。

| 特性 | 说明 |
|------|------|
| **来源** | `winit` 的分支 |
| **职责** | 窗口创建、事件循环、系统托盘、菜单栏 |
| **支持平台** | Windows、macOS、Linux、iOS、Android |
| **核心功能** | 窗口生命周期管理、事件循环驱动、输入设备事件分发 |

```rust
// TAO 的核心职责
TAO 负责：
├── 窗口生命周期管理（创建/显示/隐藏/销毁）
├── 事件循环（Event Loop）驱动
├── 系统级交互（窗口装饰、阴影、置顶等）
├── 输入设备事件分发（键盘、鼠标、触摸）
└── 与操作系统窗口管理器通信
```

### 3.2 WRY：WebView 渲染抽象层

**WRY**（WebView Rendering Yard）是 Tauri 最核心的创新——**不做浏览器，只做胶水层**，复用系统已有的 WebView 引擎。

#### 平台适配策略

| 操作系统 | 底层引擎 | 技术基础 |
|---------|---------|---------|
| **Windows** | Microsoft Edge WebView2 | 基于 Chromium 内核 |
| **macOS** | WebKit.WKWebView | Safari 同款引擎 |
| **iOS** | WKWebView | WebKit 移动端 |
| **Linux** | WebKitGTK | GTK 集成的 WebKit |
| **Android** | Android System WebView | 基于 Chromium |

#### WRY 的渲染流程

```
应用启动流程：
┌─────────────┐     ┌─────────────┐     ┌─────────────────┐
│ Tauri 初始化 │ ──▶ │ WRY 初始化   │ ──▶ │ 检测平台加载对应 │
│             │     │             │     │ WebView 实现     │
└─────────────┘     └─────────────┘     └─────────────────┘
                                                │
                    ┌───────────────────────────┼───────────┐
                    ▼                           ▼           ▼
              ┌─────────┐                ┌─────────┐  ┌─────────┐
              │ Windows │                │  macOS  │  │  Linux  │
              │WebView2 │                │WKWebView│  │WebKitGTK│
              └────┬────┘                └────┬────┘  └────┬────┘
                   │                          │            │
                   ▼                          ▼            ▼
              ┌─────────┐                ┌─────────┐  ┌─────────┐
              │加载前端 │                │加载前端 │  │加载前端 │
              │静态资源 │                │静态资源 │  │静态资源 │
              └────┬────┘                └────┬────┘  └────┬────┘
                   │                          │            │
                   └──────────────────────────┴────────────┘
                                              │
                                              ▼
                                       ┌─────────────┐
                                       │ 渲染 HTML/  │
                                       │ CSS/JS 内容  │
                                       └─────────────┘
```

#### WRY 的代码结构

```
wry/
├── src/lib.rs           # 公共 API 层（WebViewBuilder 等）
├── src/webview2/        # Windows 实现（Edge WebView2）
│   ├── mod.rs
│   └── web_context.rs
├── src/wkwebview/       # macOS/iOS 实现（WebKit）
│   └── mod.rs
├── src/webkitgtk/       # Linux 实现（WebKitGTK）
│   └── mod.rs
├── src/android/         # Android 实现
└── src/web_context.rs   # 共享资源管理
```

### 3.3 各平台 WebView 渲染机制详解

#### Windows - WebView2

```
组件交互：
Tauri App → tauri-runtime-wry → WRY → WebView2Loader.dll 
                                              │
                                              ▼
                                       ICoreWebView2Environment
                                              │
                                              ▼
                                       ICoreWebView2Controller
                                              │
                                              ▼
                                       Edge Chromium 内核
                                              │
                                              ▼
                                       实际渲染 HTML/CSS/JS
```

**关键 COM 接口**：
- `ICoreWebView2Environment`：管理 WebView2 运行环境
- `ICoreWebView2`：核心 WebView 控制接口
- `ICoreWebView2Controller`：窗口位置和可视化控制

#### macOS - WKWebView

```
渲染流程：
WRY → WKWebView (WebKit 框架)
         │
         ├── WKWebViewConfiguration (配置)
         ├── WKUserContentController (JS 通信)
         └── 实际渲染由 WebKit 内核处理
              └── 与 Safari 共享渲染引擎
```

#### Linux - WebKitGTK

```
渲染流程：
WRY → WebKitGTK (libwebkit2gtk-4.0)
         │
         ├── WebKitWebView (GTK 组件)
         ├── WebKitWebContext (共享配置)
         └── 渲染由 WebKit 引擎完成
              └── 与 GNOME Web/Epiphany 同源
```

### 3.4 TAO 与 WRY 的关系

```
┌─────────────────────────────────────┐
│         你的前端代码                   │
│    (React/Vue/Svelte + Vite)        │
├─────────────────────────────────────┤
│      @tauri-apps/api                │
├─────────────────────────────────────┤
│      IPC 通信层（加密）               │
├─────────────────────────────────────┤
│    核心进程（Rust）                   │
│  ├─ 窗口管理（TAO）                  │
│  ├─ WebView 渲染（WRY）              │
│  ├─ 系统交互（菜单/托盘/通知）         │
│  ├─ 插件系统                         │
│  ├─ 全局状态管理                      │
│  └─ 安全拦截（ACL）                   │
├─────────────────────────────────────┤
│      系统 WebView 层                  │
│  WebView2 / WKWebView / WebKitGTK   │
├─────────────────────────────────────┤
│      操作系统层                       │
└─────────────────────────────────────┘
```

**TAO 和 WRY 的协作**：
1. TAO 创建原生窗口（HWND/NSWindow/GtkWindow）
2. WRY 在该窗口内"嵌入" WebView 组件
3. 两者通过 `tauri-runtime-wry` 协调配合

---

## 四、项目初始化与配置

### 4.1 创建新项目

```bash
# 使用官方脚手架
npm create tauri-app@latest

# 选项说明
# - 项目名称
# - 前端框架（Vanilla / Vue / React / Svelte / Solid / Angular / Preact）
# - 前端构建工具（Vite / Webpack / Cargo）
# - 包管理器（npm / yarn / pnpm / cargo）
```

### 4.2 项目结构

```
my-tauri-app/
├── src/                      # 前端源码
│   ├── main.tsx             # 前端入口
│   └── App.tsx              # 根组件
├── src-tauri/               # Rust 后端
│   ├── src/
│   │   └── lib.rs           # Rust 入口
│   ├── Cargo.toml           # Rust 依赖
│   ├── tauri.conf.json      # Tauri 配置（2.0 格式）
│   └── capabilities/        # 权限配置目录
│       └── default.json     # 默认权限
├── package.json             # 前端依赖
├── vite.config.ts           # Vite 配置
└── index.html               # HTML 模板
```

### 4.3 Tauri 2.0 配置详解

#### 配置文件格式变化（1.0 → 2.0）

| 1.x 版本 | 2.0 版本 |
|---------|---------|
| `tauri.conf.json` 包含 `tauri` 键 | `tauri` 键改名为 `app` |
| `package > productName/version` | 移到顶层 |
| `tauri > bundle` | 移到顶层 `bundle` |
| `tauri > allowlist` | **移除**，改为 Capabilities 系统 |
| `build > withGlobalTauri` | 移到 `app > withGlobalTauri` |
| `build > distDir` | 改为 `frontendDist` |
| `build > devPath` | 改为 `devUrl` |

#### 完整配置示例

```json
{
  "$schema": "../node_modules/@tauri-apps/cli/config.schema.json",
  "productName": "My App",
  "version": "1.0.0",
  "identifier": "com.example.myapp",
  "mainBinaryName": "my-app",
  
  "build": {
    "beforeDevCommand": "npm run dev",
    "beforeBuildCommand": "npm run build",
    "devUrl": "http://localhost:3000",
    "frontendDist": "../dist"
  },
  
  "app": {
    "withGlobalTauri": false,
    "windows": [
      {
        "label": "main",
        "title": "My App",
        "width": 800,
        "height": 600,
        "minWidth": 400,
        "minHeight": 300,
        "center": true,
        "resizable": true,
        "fullscreen": false,
        "alwaysOnTop": false,
        "visible": true,
        "decorations": true,
        "transparent": false
      }
    ],
    "security": {
      "csp": "default-src 'self'; connect-src 'self' ipc: http://ipc.localhost; img-src 'self' data: https://*.example.com"
    }
  },
  
  "bundle": {
    "active": true,
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ],
    "targets": ["deb", "appimage", "nsis", "dmg", "app"],
    "category": "DeveloperTool",
    "shortDescription": "A Tauri App",
    "longDescription": "A longer description of your app",
    "publisher": "Your Name",
    "copyright": "© 2024 Your Name"
  },
  
  "plugins": {
    "updater": {
      "pubkey": "your-public-key",
      "endpoints": [
        "https://updates.example.com/{{target}}/{{current_version}}"
      ]
    }
  }
}
```

#### 窗口配置选项

| 选项 | 类型 | 默认值 | 说明 |
|-----|------|-------|------|
| `label` | string | - | 窗口唯一标识 |
| `title` | string | - | 窗口标题 |
| `width` | number | 800 | 窗口宽度 |
| `height` | number | 600 | 窗口高度 |
| `minWidth` | number | - | 最小宽度 |
| `minHeight` | number | - | 最小高度 |
| `maxWidth` | number | - | 最大宽度 |
| `maxHeight` | number | - | 最大高度 |
| `center` | boolean | false | 是否居中显示 |
| `resizable` | boolean | true | 是否可调整大小 |
| `fullscreen` | boolean | false | 是否全屏 |
| `alwaysOnTop` | boolean | false | 是否置顶 |
| `visible` | boolean | true | 初始是否可见 |
| `decorations` | boolean | true | 是否显示边框和标题栏 |
| `transparent` | boolean | false | 是否透明背景 |
| `url` | string | - | 窗口加载的 URL |

#### 多平台配置支持

Tauri 2.0 支持平台特定配置文件，自动合并：

| 平台 | JSON 格式 | TOML 格式 |
|-----|----------|----------|
| Linux | `tauri.linux.conf.json` | `Tauri.linux.toml` |
| Windows | `tauri.windows.conf.json` | `Tauri.windows.toml` |
| macOS | `tauri.macos.conf.json` | `Tauri.macos.toml` |
| Android | `tauri.android.conf.json` | `Tauri.android.toml` |
| iOS | `tauri.ios.conf.json` | `Tauri.ios.toml` |

---

## 五、命令系统（Commands）

### 5.1 基础命令定义

Commands 是前端调用 Rust 函数的机制，基于 JSON-RPC 协议。

```rust
// src-tauri/src/lib.rs
use tauri::command;

// 基础命令
#[command]
fn greet(name: String) -> String {
    format!("Hello, {}!", name)
}

// 异步命令
#[command]
async fn fetch_data(url: String) -> Result<String, String> {
    reqwest::get(&url)
        .await
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())
}

// 注册命令
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![greet, fetch_data])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### 5.2 前端调用

```javascript
import { invoke } from '@tauri-apps/api/core';

// 同步调用
const response = await invoke('greet', { name: 'Tauri' });
console.log(response); // "Hello, Tauri!"

// 错误处理
try {
  const data = await invoke('fetch_data', { url: 'https://api.example.com' });
} catch (error) {
  console.error('Failed:', error);
}
```

### 5.3 命令参数类型

| Rust 类型 | JS 类型 | 说明 |
|----------|---------|------|
| `String` | `string` | 字符串 |
| `i32`/`i64` | `number` | 整数 |
| `f32`/`f64` | `number` | 浮点数 |
| `bool` | `boolean` | 布尔值 |
| `Vec<T>` | `T[]` | 数组 |
| `HashMap<K, V>` | `Object` | 对象 |
| `Option<T>` | `T \| null` | 可选值 |
| `Result<T, E>` | `T` 或抛出异常 | 错误处理 |
| 自定义结构体 | `Object` | 需实现 `Serialize`/`Deserialize` |

### 5.4 状态管理

```rust
use tauri::{State, Window, AppHandle};
use std::sync::Mutex;

// 定义应用状态
struct AppState {
    counter: Mutex<i32>,
    db_connection: Database,
}

// 使用状态
#[command]
fn increment(state: State<AppState>) -> i32 {
    let mut counter = state.counter.lock().unwrap();
    *counter += 1;
    *counter
}

// 使用窗口句柄
#[command]
fn notify_window(window: Window, message: String) {
    window.emit("notification", message).unwrap();
}

// 使用应用句柄
#[command]
fn open_settings(app: AppHandle) {
    let settings_window = tauri::WebviewWindowBuilder::new(
        &app,
        "settings",
        tauri::WebviewUrl::App("settings.html".into())
    )
    .title("Settings")
    .inner_size(600.0, 400.0)
    .build()
    .unwrap();
}

// 注册状态
pub fn run() {
    tauri::Builder::default()
        .manage(AppState {
            counter: Mutex::new(0),
            db_connection: Database::new(),
        })
        .invoke_handler(tauri::generate_handler![increment, notify_window, open_settings])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### 5.5 错误处理最佳实践

```rust
use serde::Serialize;
use thiserror::Error;

// 定义错误类型
#[derive(Error, Debug)]
pub enum AppError {
    #[error("数据库错误: {0}")]
    Database(String),
    #[error("网络错误: {0}")]
    Network(String),
    #[error("验证失败: {0}")]
    Validation(String),
    #[error("未授权")]
    Unauthorized,
}

// 实现 Serialize 以便传递给前端
impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.to_string().as_str())
    }
}

// 在命令中使用
#[command]
async fn get_user(id: String) -> Result<User, AppError> {
    if id.is_empty() {
        return Err(AppError::Validation("ID 不能为空".to_string()));
    }
    
    db::find_user(&id)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?
        .ok_or(AppError::Validation("用户不存在".to_string()))
}
```

---

## 六、插件系统（Plugin System）

### 6.1 核心变化：API 全面插件化

Tauri 2.0 将 1.x 的内置模块拆分为独立插件，实现按需引入：

| 功能 | 1.x 内置模块 | 2.0 插件 |
|-----|-----------|---------|
| 文件系统 | `@tauri-apps/api/fs` | `@tauri-apps/plugin-fs` |
| HTTP 请求 | `@tauri-apps/api/http` | `@tauri-apps/plugin-http` |
| Shell 命令 | `@tauri-apps/api/shell` | `@tauri-apps/plugin-shell` |
| 通知 | `@tauri-apps/api/notification` | `@tauri-apps/plugin-notification` |
| 对话框 | `@tauri-apps/api/dialog` | `@tauri-apps/plugin-dialog` |
| 剪贴板 | `@tauri-apps/api/clipboard` | `@tauri-apps/plugin-clipboard-manager` |
| 全局快捷键 | `@tauri-apps/api/globalShortcut` | `@tauri-apps/plugin-global-shortcut` |
| 更新器 | 内置 | `@tauri-apps/plugin-updater` |
| 进程 | `@tauri-apps/api/process` | `@tauri-apps/plugin-process` |
| OS 信息 | `@tauri-apps/api/os` | `@tauri-apps/plugin-os` |

### 6.2 插件安装流程

以文件系统插件为例：

**步骤 1：安装 Rust 依赖**
```toml
# Cargo.toml
[dependencies]
tauri-plugin-fs = "2"
```

**步骤 2：注册插件**
```rust
// src-tauri/src/lib.rs
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

**步骤 3：安装 JS 依赖**
```bash
npm install @tauri-apps/plugin-fs
```

**步骤 4：前端使用**
```javascript
import { readTextFile, writeTextFile, BaseDirectory } from '@tauri-apps/plugin-fs';

// 读取文件
const content = await readTextFile('config.txt', { 
    baseDir: BaseDirectory.AppData 
});

// 写入文件
await writeTextFile('output.txt', 'Hello World', { 
    baseDir: BaseDirectory.AppData 
});

// 其他常用 API
import { 
    readDir,      // 读取目录
    mkdir,        // 创建目录
    remove,       // 删除文件/目录
    rename,       // 重命名
    exists,       // 检查存在
    copyFile      // 复制文件
} from '@tauri-apps/plugin-fs';
```

### 6.3 常用插件详解

#### HTTP 插件

```javascript
import { fetch } from '@tauri-apps/plugin-http';

// 发送请求（绕过 CORS 限制）
const response = await fetch('https://api.example.com/data', {
    method: 'POST',
    headers: {
        'Content-Type': 'application/json',
        'Authorization': 'Bearer token'
    },
    body: JSON.stringify({ key: 'value' })
});

const data = await response.json();
```

#### Shell 插件

```javascript
import { Command } from '@tauri-apps/plugin-shell';

// 执行命令
const result = await Command.create('git', ['status']).execute();
console.log(result.stdout);

// 带事件监听
const command = Command.create('node', ['script.js']);
command.on('close', data => {
    console.log(`命令退出，code: ${data.code}, signal: ${data.signal}`);
});
command.on('error', error => {
    console.error('错误:', error);
});
command.stdout.on('data', line => {
    console.log('输出:', line);
});
command.stderr.on('data', line => {
    console.error('错误输出:', line);
});
await command.spawn();
```

#### 对话框插件

```javascript
import { open, save, message, confirm, ask } from '@tauri-apps/plugin-dialog';

// 打开文件
const selected = await open({
    multiple: false,
    directory: false,
    filters: [{
        name: 'Images',
        extensions: ['png', 'jpg', 'jpeg', 'gif']
    }]
});

// 保存文件
const filePath = await save({
    filters: [{
        name: 'Text',
        extensions: ['txt']
    }]
});

// 消息框
await message('操作成功完成', { title: '提示', type: 'info' });

// 确认框
const yes = await confirm('确定要删除吗？', '警告');

// 询问框
const response = await ask('是否保存更改？', '确认');
```

#### 通知插件

```javascript
import { isPermissionGranted, requestPermission, sendNotification } from '@tauri-apps/plugin-notification';

// 请求权限
let permissionGranted = await isPermissionGranted();
if (!permissionGranted) {
    const permission = await requestPermission();
    permissionGranted = permission === 'granted';
}

// 发送通知
if (permissionGranted) {
    sendNotification({
        title: '新消息',
        body: '您有一条未读消息',
        icon: 'icons/128x128.png'
    });
}
```

#### 剪贴板插件

```javascript
import { readText, writeText, readImage, writeImage } from '@tauri-apps/plugin-clipboard-manager';

// 文本操作
await writeText('Hello from Tauri!');
const text = await readText();

// 图片操作
const image = await readImage(); // 返回 Image 对象
await writeImage(image);
```

### 6.4 自定义插件开发

```rust
// 插件结构
use tauri::{
    plugin::{Builder, TauriPlugin},
    Runtime,
};

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("my-plugin")
        .invoke_handler(tauri::generate_handler![my_command])
        .setup(|app| {
            // 插件初始化逻辑
            println!("Plugin initialized!");
            
            // 可以在这里创建后台任务
            let app_handle = app.clone();
            std::thread::spawn(move || {
                // 后台工作
            });
            
            Ok(())
        })
        .on_event(|app, event| {
            // 监听应用事件
            match event {
                tauri::RunEvent::Exit => {
                    println!("App exiting, cleanup...");
                }
                _ => {}
            }
        })
        .build()
}

#[tauri::command]
fn my_command(arg: String) -> Result<String, String> {
    Ok(format!("Processed: {}", arg))
}
```

---

## 七、安全模型与权限系统

### 7.1 安全设计原则

Tauri 的安全模型基于以下核心原则：

1. **默认拒绝**：所有系统访问默认关闭，需显式授权
2. **最小权限**：每个窗口只授予必要的权限
3. **IPC 隔离**：前端无法直接访问系统资源
4. **输入验证**：所有前端输入在 Rust 侧验证
5. **CSP 保护**：内容安全策略防止 XSS

### 7.2 Capability 权限系统

Tauri 2.0 用 **Capabilities** 替代了 1.x 的 allowlist，实现更精细的权限控制。

#### 目录结构

```
src-tauri/
├── capabilities/
│   ├── default.json          # 主窗口权限
│   └── settings.json         # 设置窗口权限
```

#### 基础配置示例

```json
// capabilities/default.json
{
    "identifier": "default",
    "description": "主窗口的默认权限",
    "windows": ["main"],
    "permissions": [
        "core:default",
        "fs:allow-read-text-file",
        "fs:allow-write-text-file",
        "dialog:allow-open",
        "dialog:allow-save",
        "http:default",
        "notification:default",
        "shell:allow-open"
    ]
}
```

```json
// capabilities/settings.json
{
    "identifier": "settings-window",
    "description": "设置窗口——只读权限",
    "windows": ["settings"],
    "permissions": [
        "core:default",
        "fs:allow-read-text-file",
        "store:allow-get"
    ]
}
```

#### 带 Scope 的精细控制

```json
{
    "identifier": "default",
    "windows": ["main"],
    "permissions": [
        {
            "identifier": "fs:allow-read-text-file",
            "allow": [
                { "path": "$APPDATA/**" },
                { "path": "$RESOURCE/**" }
            ],
            "deny": [
                { "path": "$APPDATA/secrets/**" }
            ]
        },
        {
            "identifier": "fs:allow-write-text-file",
            "allow": [
                { "path": "$APPDATA/**" }
            ]
        },
        {
            "identifier": "http:default",
            "allow": [
                { "url": "https://api.example.com/**" }
            ],
            "deny": [
                { "url": "https://api.example.com/admin/**" }
            ]
        }
    ]
}
```

### 7.3 路径变量

| 变量 | Windows | macOS | Linux |
|-----|---------|-------|-------|
| `$APPDATA` | `%APPDATA%\` | `~/Library/Application Support/` | `~/.config/` |
| `$APPLOCAL` | `%LOCALAPPDATA%\` | `~/Library/Application Support/` | `~/.local/share/` |
| `$DESKTOP` | `Desktop` | `Desktop` | `Desktop` |
| `$DOCUMENT` | `Documents` | `Documents` | `Documents` |
| `$RESOURCE` | 应用资源目录 | 应用资源目录 | 应用资源目录 |
| `$TEMP` | 系统临时目录 | 系统临时目录 | 系统临时目录 |
| `$HOME` | 用户主目录 | 用户主目录 | 用户主目录 |

### 7.4 Content Security Policy (CSP)

```json
{
    "app": {
        "security": {
            "csp": {
                "default-src": "'self'",
                "img-src": "'self' https://images.example.com data:",
                "script-src": "'self'",
                "style-src": "'self' 'unsafe-inline'",
                "connect-src": "'self' https://api.example.com ipc: http://ipc.localhost",
                "font-src": "'self'",
                "object-src": "'none'",
                "frame-ancestors": "'none'"
            }
        }
    }
}
```

### 7.5 安全最佳实践

#### 输入验证

```rust
#[command]
async fn read_user_file(path: String) -> Result<String, String> {
    // 防止路径穿越攻击
    let canonical = std::fs::canonicalize(&path)
        .map_err(|e| format!("无效路径: {}", e))?;
    
    let app_data = dirs::data_dir()
        .ok_or("找不到应用数据目录")?
        .join("my-app");
    
    // 确保路径在允许范围内
    if !canonical.starts_with(&app_data) {
        return Err("访问被拒绝：路径超出允许范围".to_string());
    }
    
    std::fs::read_to_string(canonical)
        .map_err(|e| format!("读取失败: {}", e))
}
```

#### 安全清单

| 措施 | 实现方式 |
|-----|---------|
| 最小权限 | 每个窗口只授予必要权限 |
| Scope 限制 | 文件系统/HTTP 操作加路径/URL 范围 |
| CSP 配置 | 避免 `'unsafe-eval'`，谨慎使用 `'unsafe-inline'` |
| 输入验证 | Rust 命令中验证所有前端参数 |
| 依赖审计 | 定期运行 `cargo audit` 和 `npm audit` |
| 禁用 DevTools | 正式版本关闭开发者工具 |
| 自动更新 | 使用 updater 插件及时修复安全漏洞 |

---

## 八、IPC 通信机制

### 8.1 通信模式

| 模式 | 方向 | 用途 | API |
|-----|------|------|-----|
| **Commands** | 前端 → 后端 | RPC 调用，请求-响应 | `invoke()` |
| **Events** | 双向 | 推送通知、状态变更 | `listen()` / `emit()` |

### 8.2 Commands 详解

```javascript
import { invoke } from '@tauri-apps/api/core';

// 基础调用
const result = await invoke('command_name', { 
    arg1: 'value1', 
    arg2: 42 
});

// 带类型定义（TypeScript）
interface User {
    id: string;
    name: string;
    email: string;
}

const user = await invoke<User>('get_user', { id: '123' });
```

### 8.3 Events 详解

```javascript
import { listen, emit, once } from '@tauri-apps/api/event';

// 监听事件
const unlisten = await listen('update-available', (event) => {
    console.log('新版本可用:', event.payload);
});

// 只监听一次
await once('app-ready', () => {
    console.log('应用准备就绪');
});

// 发送事件（前端 → 前端）
await emit('user-login', { userId: '123', name: 'John' });

// 取消监听
unlisten();
```

### 8.4 窗口间通信

```javascript
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';

// 获取其他窗口并发送事件
const settingsWindow = WebviewWindow.getByLabel('settings');
if (settingsWindow) {
    await settingsWindow.emit('theme-changed', { theme: 'dark' });
}

// 向所有窗口广播
import { emitTo } from '@tauri-apps/api/event';
await emitTo('main', 'notification', { message: 'Hello main!' });
```

### 8.5 Rust 侧事件

```rust
use tauri::{Manager, Emitter};

// 发送事件到所有窗口
app.emit_all("global-event", payload)?;

// 发送事件到指定窗口
let window = app.get_webview_window("main").unwrap();
window.emit("window-event", payload)?;

// 监听前端事件
window.listen("frontend-event", |event| {
    println!("收到前端消息: {:?}", event.payload());
});
```

### 8.6 IPC 安全机制

| 安全特性 | 说明 |
|---------|------|
| **Capabilities** | 定义哪些窗口能调用哪些命令/权限 |
| **Permissions** | 白名单机制，最小授权原则 |
| **加密通道** | Tauri 2.5+ 默认使用 AES-GCM 加密 |
| **Isolation 模式** | iframe 沙盒隔离，消息加密传输 |

---

## 九、移动端支持（iOS/Android）

### 9.1 Tauri 2.0 移动平台突破

Tauri 2.0 实现了从桌面端到移动端的跨越，成为**五平台统一**开发框架：

| 平台类型 | 支持平台 |
|---------|---------|
| 桌面端 | Windows, macOS, Linux |
| 移动端 | **iOS**, **Android** |

### 9.2 移动端技术实现

| 平台 | 渲染引擎 | 技术细节 |
|-----|---------|---------|
| **iOS** | **WKWebView** | 使用与 Safari 相同的 WebKit 引擎，支持 Nitro JavaScript 引擎 |
| **Android** | **Android System WebView** | 基于 Chromium，支持最新 Web 标准 |

**关键优势**：
- 应用体积极小（最小可低于 600KB）
- 安全补丁通过系统自动更新
- 无需捆绑浏览器引擎

### 9.3 移动端项目初始化

```bash
# 创建支持移动端的项目
npm create tauri-app@latest
# 选择 "Vanilla" / "React" / "Vue" 等
# 确保勾选移动端支持

# iOS 项目初始化
tauri ios init

# Android 项目初始化  
tauri android init

# 开发调试
tauri ios dev      # iOS 模拟器/真机
tauri android dev  # Android 模拟器/真机

# 构建
tauri ios build    # 构建 iOS 应用
tauri android build # 构建 Android 应用
```

自动生成：
- **iOS**: Xcode 项目（`src-tauri/gen/apple` 目录）
- **Android**: Gradle 项目（`src-tauri/gen/android` 目录）

### 9.4 移动端项目结构

```
src-tauri/
├── gen/
│   ├── apple/              # iOS 项目
│   │   ├── src/
│   │   ├── xcode/
│   │   └── Info.plist
│   └── android/            # Android 项目
│       ├── src/
│       ├── build.gradle
│       └── AndroidManifest.xml
├── src/
│   └── lib.rs
├── capabilities/
│   └── mobile.json         # 移动端权限
└── tauri.conf.json
```

### 9.5 移动端特定 API

```javascript
// 检查平台
import { platform } from '@tauri-apps/plugin-os';

const currentPlatform = platform(); // 'ios' | 'android' | 'windows' | 'macos' | 'linux'

// 移动端特定插件
import { 
    impactFeedback,      // 触觉反馈
    notification,        // 本地通知
    statusBar,          // 状态栏控制
    navigationBar       // 导航栏控制
} from '@tauri-apps/plugin-haptics';

// 触觉反馈
await impactFeedback({ style: 'medium' });
```

### 9.6 移动端插件生态

| 插件 | 功能 | 平台 |
|-----|------|------|
| `tauri-plugin-mobile-push` | 远程推送通知（APNs/FCM） | iOS 13+, Android 7+ |
| `tauri-plugin-tts` | 文本转语音 | 桌面 + 移动 |
| `tauri-plugin-subscriptions` | 应用内购买/订阅 | iOS + Android |
| `tauri-plugin-barcode-scanner` | 二维码/条码扫描 | iOS + Android |
| `tauri-plugin-nfc` | NFC 读写 | iOS + Android |
| `tauri-plugin-biometric` | 生物识别认证 | iOS + Android |

---

## 十、与 Electron 对比

### 10.1 架构对比

| 特性 | Tauri | Electron |
|-----|-------|----------|
| **主进程** | Rust 原生二进制 | Node.js 进程 |
| **渲染进程** | 系统 WebView | 完整 Chromium |
| **应用体积** | ~2-20 MB | ~100-300 MB |
| **内存占用（6窗口）** | ~172 MB | ~409 MB |
| **首次构建** | 较慢（Rust 编译） | 较快 |
| **跨平台一致性** | 需适配不同 WebView | 高度一致（自带 Chromium） |
| **后端性能** | 原生性能，适合计算密集型 | 需额外进程处理高性能任务 |
| **移动端支持** | ✅ iOS + Android | ❌ 不支持 |
| **安全模型** | 默认拒绝，Capability 权限 | 默认开放，需手动限制 |

### 10.2 安全对比

| 特性 | Electron | Tauri 2.0 |
|-----|----------|-----------|
| 默认行为 | 需要主动限制（nodeIntegration） | 默认拒绝，需主动授权 |
| 进程隔离 | 可选（preload script） | **强制**（IPC 唯一通道） |
| 原生 API 访问 | Node.js 直接访问 | 必须通过 Plugin + 权限 |
| CSP 支持 | 需手动配置 | 框架内建支持 |
| 攻击面 | 较大（Chromium + Node.js） | 较小（系统 WebView + Rust） |
| 权限粒度 | 粗粒度 | **窗口级 + 命令级 + Scope 级** |

### 10.3 适用场景

| 场景 | 推荐框架 |
|-----|---------|
| 追求极致体积和性能 | **Tauri** |
| 需要移动端支持 | **Tauri** |
| 需要完整 Chromium 特性 | Electron |
| 团队熟悉 Node.js 生态 | Electron |
| 需要深度浏览器定制 | Electron |
| 追求内存安全和高安全性 | **Tauri** |
| 快速原型开发 | 两者皆可 |

---

## 十一、最佳实践

### 11.1 项目结构建议

```
my-tauri-app/
├── src/                          # 前端源码
│   ├── components/              # 通用组件
│   ├── pages/                   # 页面组件
│   ├── hooks/                   # 自定义 Hooks
│   ├── stores/                  # 状态管理
│   ├── services/                # API 服务
│   │   └── tauri-commands.ts    # Tauri 命令封装
│   └── utils/                   # 工具函数
├── src-tauri/                   # Rust 后端
│   ├── src/
│   │   ├── main.rs              # 入口
│   │   ├── lib.rs               # 库入口
│   │   ├── commands/            # 命令模块
│   │   │   ├── mod.rs
│   │   │   ├── user.rs
│   │   │   └── file.rs
│   │   ├── models/              # 数据模型
│   │   ├── services/            # 业务服务
│   │   └── utils/               # 工具函数
│   ├── capabilities/            # 权限配置
│   └── migrations/              # 数据库迁移
├── shared/                      # 前后端共享类型
│   └── types.ts
└── tests/                       # 测试
```

### 11.2 开发建议

1. **业务逻辑下沉**：尽可能将敏感操作放在 Rust Core
2. **权限最小化**：为不同窗口配置不同的 Capabilities
3. **状态集中管理**：利用 Core 管理全局状态，多窗口同步
4. **注意 WebView 差异**：做好跨平台兼容性测试
5. **IPC 优化**：避免频繁调用，批量处理请求
6. **错误处理**：Rust 侧统一错误类型，前端友好展示
7. **类型安全**：前后端共享 TypeScript/Rust 类型定义

### 11.3 性能优化

```rust
// 1. 使用异步命令避免阻塞
#[command]
async fn heavy_computation() -> Result<String, String> {
    tokio::task::spawn_blocking(|| {
        // 耗时操作
    }).await.map_err(|e| e.to_string())
}

// 2. 批量 IPC 调用
#[command]
fn batch_operations(items: Vec<Item>) -> Result<Vec<Result>, String> {
    items.into_iter().map(process_item).collect()
}

// 3. 使用流式数据
#[command]
async fn stream_data(window: Window) -> Result<(), String> {
    for i in 0..100 {
        window.emit("progress", ProgressEvent { current: i, total: 100 }).unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Ok(())
}
```

### 11.4 调试技巧

```bash
# 开启详细日志
RUST_LOG=trace npm run tauri dev

# 特定模块日志
RUST_LOG=tauri::webview=debug npm run tauri dev

# 前端调试
# 1. 打开 DevTools: Ctrl+Shift+I (Windows/Linux) 或 Cmd+Option+I (macOS)
# 2. 在 tauri.conf.json 中确保 devtools 启用

# Rust 调试
# 使用 VSCode + CodeLLDB 插件
# 或命令行: cargo run --bin app
```

---

## 十二、常用命令速查

### 12.1 CLI 命令

| 命令 | 说明 |
|-----|------|
| `npm create tauri-app@latest` | 创建新项目 |
| `npm run tauri dev` | 开发模式 |
| `npm run tauri build` | 构建生产版本 |
| `tauri info` | 显示环境信息 |
| `tauri add <plugin>` | 添加插件 |

### 12.2 移动端命令

| 命令 | 说明 |
|-----|------|
| `tauri ios init` | 初始化 iOS 项目 |
| `tauri android init` | 初始化 Android 项目 |
| `tauri ios dev` | iOS 开发模式 |
| `tauri android dev` | Android 开发模式 |
| `tauri ios build` | 构建 iOS 应用 |
| `tauri android build` | 构建 Android 应用 |

### 12.3 构建目标

| 平台 | 目标 |
|-----|------|
| Windows | `nsis`, `msi`, `app` |
| macOS | `app`, `dmg` |
| Linux | `deb`, `appimage`, `rpm` |
| iOS | `app` |
| Android | `apk`, `aab` |

---

## 参考资源

- [Tauri 官方文档](https://v2.tauri.app/)
- [Tauri GitHub](https://github.com/tauri-apps/tauri)
- [Tauri 插件生态](https://v2.tauri.app/plugin/)
- [WRY 项目](https://github.com/tauri-apps/wry)
- [TAO 项目](https://github.com/tauri-apps/tao)
- [Rust 官方文档](https://www.rust-lang.org/learn)

---

> 本文档基于 Tauri 2.0+ 版本整理，部分 API 可能随版本更新而变化，请以[官方文档](https://v2.tauri.app/)为准。
