# LiveEmail

轻量、快速、注重隐私的 Windows 桌面邮件客户端。基于 Tauri 2 + Rust + React 构建，聚焦 IMAP/SMTP 收发，支持多账户统一收件箱、全文搜索、离线缓存与系统通知。

## 功能

- **账户**：通用 IMAP/SMTP、QQ、163/126、Gmail OAuth2、Outlook OAuth2；连接测试、预设自动填充。
- **接收与阅读**：增量同步（UIDVALIDITY + UID 游标）、文件夹同步、IDLE 新邮件监听、HTML 净化渲染、内联图片、附件预览/保存。
- **撰写与发送**：富文本编辑、附件、签名、回复/转发、草稿自动保存、发送队列与重试。
- **邮件操作**：已读/未读、星标、移动、删除、归档；批量操作与服务器状态同步。
- **搜索**：SQLite FTS5 全文搜索，中文自动回退 `LIKE`；按账户、文件夹、未读、星标筛选。
- **体验**：明暗主题、快捷键、虚拟滚动列表、系统通知、托盘未读角标、存储管理。
- **安全**：凭据存于 Windows 凭据管理器、强制 TLS、DOMPurify 净化、CSP 限制、OAuth2 PKCE。
- **更新**：`tauri-plugin-updater` 自动更新（minisign 签名校验）。

## 技术栈

Tauri 2.12 · Rust（async-imap、mail-send、rusqlite）· React 19 · TypeScript 6 · Vite 8 · Tailwind 4 · Zustand · TanStack Query/Virtual · Tiptap 3 · SQLite（WAL + FTS5）。

## 开发

前置：Node/Bun、Rust 稳定版、Windows WebView2。

```bash
bun install
bun run tauri dev        # 启动桌面应用（前端热更新）
bun run dev              # 仅启动前端（浏览器调试）
```

## 测试与构建

```bash
bun run test                                     # Vitest（25 个用例）
cargo test --manifest-path src-tauri/Cargo.toml --lib   # Rust（19 个用例）
bun run build                                    # 类型检查 + 前端产物
bunx tauri build --bundles nsis                  # Windows x64 安装包
```

安装包输出：`src-tauri/target/release/bundle/nsis/LiveEmail_<version>_x64-setup.exe`。

## 目录

```
src/                     React 前端（features / components / store / lib）
src-tauri/src/           Rust 后端（commands / services / db / models）
src-tauri/migrations 逻辑 在 db/mod.rs（PRAGMA user_version 版本化迁移）
docs/                    用户手册、发布说明、检查清单
开发计划/                分批开发计划
需求文档.md              产品需求与验收标准
```

## 文档

- [用户手册](./docs/用户手册.md)
- [发布与自动更新](./docs/发布与自动更新.md)
- [发布检查清单](./docs/发布检查清单.md)
- [变更日志](./CHANGELOG.md)
- [需求文档](./需求文档.md)
- [技术选型 v2.0](./邮箱客户端-技术选型文档-v2.0.md)

## 数据与隐私

邮件、附件索引与设置保存在本机 `%APPDATA%\com.liveemail.app`，凭据保存在 Windows 凭据管理器；应用不含遥测与广告，仅在用户主动操作时连接邮箱服务器与更新源。
