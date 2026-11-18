# 变更日志

本项目遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

## [Unreleased]

### 计划中
- Microsoft 365 / Exchange Graph API 接入
- 会话（线程）视图
- 通知免打扰时间段
- 数据导入导出（不含凭据）

## [0.1.0] - 2026-11-18

首个可交付版本，覆盖需求文档 v1 的 P0/P1 范围。

### 新增
- 账户体系
  - IMAP/SMTP 手动配置与 Gmail / Outlook.com / QQ / 163 / 126 服务商预设
  - 密码 / 授权码认证，以及 Gmail、Outlook.com 的 OAuth2 浏览器授权（PKCE + loopback 回调）
  - OAuth2 令牌自动刷新；访问令牌、刷新令牌、密码均保存在 Windows 凭据管理器
  - 保存前连接测试（IMAP + SMTP），错误分类提示
  - 多账户、默认账户、账户级同步与未读数
- 邮件接收与阅读
  - IMAP SSL / STARTTLS、UIDVALIDITY + UID 增量同步、断点续传、幂等入库
  - IDLE 新邮件推送 + 定时同步兜底（可配置 1–30 分钟）
  - 统一收件箱与账户/文件夹视图，虚拟列表支持万级邮件
  - HTML 邮件经 DOMPurify 净化，默认拦截远程图片；cid 内联图片转 data URI
  - 附件元数据入库、按需打开 / 另存为
- 撰写与发送
  - 新邮件 / 回复 / 全部回复 / 转发，Tiptap 富文本与纯文本切换
  - 附件添加、25MB 大小校验、签名自动插入
  - 草稿自动保存（1.2s 防抖）、草稿列表恢复
  - SMTP SSL / STARTTLS / OAuth2 发送；成功后归档到「已发送」
  - 发送失败进入发送队列，可重试；附件路径持久化
- 文件夹与邮件操作
  - 文件夹树、创建 / 删除自定义文件夹
  - 已读/未读、星标、归档、垃圾、移动、删除；批量多选操作
  - 服务端 STORE / COPY / UID EXPUNGE 同步，删除到已删除后可彻底删除
- 搜索与筛选
  - SQLite FTS5 全文搜索（主题 / 发件人 / 正文），中文子串自动回退 LIKE
  - 未读 / 星标 / 含附件筛选、账户与文件夹过滤、分页
- 通知与体验
  - 系统通知、托盘图标与菜单、未读角标 / 窗口标题
  - 明暗主题、字体大小、关闭到托盘
  - 快捷键：C / R / F / / / E / # / U / S
  - 存储统计与缓存清理
- 发布与安全
  - Windows x64 NSIS 安装包（当前用户安装，简体中文/英文）
  - tauri-plugin-updater 脚手架与签名公钥
  - CSP、Tauri Capability 最小权限、凭据不落明文数据库

### 修复
- 修复 v2 数据库迁移因 `references` 保留字导致失败的问题，并改为逐列幂等迁移。
- 修复发件人显示名被整体当作邮箱地址写入 MIME 头的问题。
- 修复 OAuth2 账户在同步 / 发送 / 连接测试时未刷新令牌的问题。

### 测试
- Rust 单元测试 19 项：MIME 解析、附件文件名消毒、cid 内联、SMTP 地址校验与 MIME 构造、FTS/筛选/分页、同步游标、文件夹计数、PKCE 与 OAuth 回调解析。
- 前端 Vitest 25 项：日期/字节/HTML 工具函数、Zustand 状态、Toast、Button/Badge 组件。
