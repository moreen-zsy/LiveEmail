# a. 第一批 —— 项目脚手架与账户体系

> 目标：完成可运行的项目骨架与账户数据模型，打通「添加账户 → 连接测试」闭环。
> 前置：无（起点）。交付后即进入第二批。

## 一、目标与交付物

1. 项目依赖对齐 [技术选型 v2.0](../邮箱客户端-技术选型文档-v2.0.md)（Vite 8 / TS 6 / Tailwind 4 / Tauri 2 插件）。
2. SQLite 数据库 Schema 落地（accounts / folders / messages 等表 + 迁移机制）。
3. 账户 CRUD：添加（手动 + 服务商预设）、编辑、删除、连接测试。
4. OAuth2（Gmail/Outlook.com）与授权码（QQ/163）两条认证路径可用。
5. 凭据安全存储（Windows 凭据管理器），密码/令牌不落明文。

## 二、任务清单

### 1. 脚手架升级
- [ ] 升级前端依赖至 v2 目标版本：`vite@^8.3`、`typescript@~6.0`、`@vitejs/plugin-react@^6.2`、`@tauri-apps/*@^2.12`
- [ ] 引入 Tailwind 4（`tailwindcss` + `@tailwindcss/vite`），用 CSS-first 配置替换旧 tailwind.config
- [ ] 初始化 shadcn/ui 到 Tailwind 4
- [ ] 安装 Tauri 2 插件：notification / store / sql / fs / dialog / shell / opener / updater
- [ ] 配置 `tauri.conf.json`（窗口 1200×800、minWidth/minHeight、CSP）与 `capabilities/default.json` 最小权限
- [ ] 清理模板示例代码，建立目录结构：`src/components`、`src/features`、`src/lib`、`src/store`；`src-tauri/src/{commands,services,db,models}`

### 2. Rust 基础与数据库
- [ ] Cargo.toml 引入 `rusqlite(bundled)`、`serde`、`thiserror`、`tracing`、`keyring`
- [ ] 建 `db/mod.rs`：初始化 SQLite（`$APPDATA/LiveEmail/liveemail.db`）、连接管理、迁移框架
- [ ] 编写 migration v1：`accounts`、`folders`、`messages`、`message_bodies`、`attachments`、`sync_state`、`drafts`、`outbox`、`messages_fts`
- [ ] 建 `models/`：Account、Folder、Message 等 serde 结构体，与前端 TS 类型对齐
- [ ] 日志：`tracing-subscriber` 初始化，输出到文件 + 控制台，日志脱敏宏

### 3. 账户 CRUD 与认证
- [ ] `accounts` 表 DAO：增删改查、按邮箱唯一约束
- [ ] Tauri commands：`list_accounts`、`add_account`、`update_account`、`delete_account`、`test_connection`
- [ ] 服务商预设：Gmail / Outlook.com / QQ / 163 / 自建 的 IMAP/SMTP 主机端口表
- [ ] 手动配置：主机/端口/加密方式（SSL/STARTTLS）/账号/密码（授权码）
- [ ] keyring 封装：`credential_store`（保存/读取/删除凭据，以 account_id 为 key）
- [ ] IMAP/SMTP 连接测试：登录成功/失败（认证/网络/证书）分类返回

### 4. OAuth2 流程
- [ ] Rust `oauth_service`（mail-auth）生成授权 URL
- [ ] 打开系统浏览器（tauri-plugin-opener）
- [ ] 本地 loopback 回调（127.0.0.1 随机端口）接收授权码
- [ ] 换取访问/刷新令牌并写入 keyring
- [ ] IMAP XOAUTH2 / SMTP XOAUTH2 登录验证

### 5. 账户 UI
- [ ] 账户设置页：账户列表、添加表单（预设 + 手动两个 tab）、编辑、删除（二次确认）
- [ ] 授权码指引（QQ/163 分步说明）
- [ ] 连接测试按钮与结果反馈（loading / 成功 / 错误分类）
- [ ] 左侧账户栏渲染（账户名、未读占位）

## 三、验收标准

- [ ] `npm run build` 与 `npm run tauri dev` 均可通过，应用窗口正常启动
- [ ] 可成功添加 Gmail（OAuth2）、QQ（授权码）、163（授权码）、自建 IMAP 四种账户并通过连接测试
- [ ] 错误场景可分类提示：密码错误 / 无法连接主机 / 证书错误 / 需开启 IMAP
- [ ] 密码/授权码/令牌不写入 SQLite 与日志，可在 Windows 凭据管理器中查到
- [ ] 删除账户二次确认，删除后凭据同步清理

## 四、涉及模块

- `src-tauri/src/db/`、`src-tauri/src/models/`、`src-tauri/src/services/credential_store.rs`、`src-tauri/src/services/oauth.rs`、`src-tauri/src/commands/accounts.rs`
- `src/features/accounts/`、`src/lib/`、`src/store/`

## 五、风险与注意

- Tailwind 4 配置方式与 3.x 完全不同，先通读迁移指南再动手。
- OAuth2 loopback 回调可能被系统代理/安全软件拦截，需提供「授权码手动粘贴」兜底。
- 连接测试需设置超时（如 15s），避免 UI 长时间卡死。
