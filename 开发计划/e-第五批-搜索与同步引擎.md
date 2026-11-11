# e. 第五批 —— 搜索与同步引擎

> 目标：实现全文搜索与生产级同步引擎（增量 + IDLE + 通知），支撑离线可用。
> 前置：第四批（邮件与文件夹已完整）。

## 一、目标与交付物

1. SQLite FTS5 全文搜索（主题/发件人/正文）。
2. 筛选器：未读/星标/附件/日期/文件夹/账户。
3. 生产级同步引擎：每账户独立 task、定时 + IDLE、游标增量、错误隔离。
4. 新邮件系统通知与未读角标。

## 二、任务清单

### 1. 搜索
- [ ] `messages_fts` 虚拟表与触发器同步（主题、发件人、纯文本正文）
- [ ] `search(query, filters)` command：FTS5 MATCH + 组合筛选
- [ ] 搜索结果高亮与虚拟列表
- [ ] 搜索建议（发件人/主题自动补全，P2）

### 2. 同步引擎
- [ ] `sync_engine`：每账户一个 tokio task
- [ ] 双通道：定时轮询（默认 5 分钟，可配置）+ IDLE 实时推送
- [ ] 游标增量：UIDVALIDITY 变更时全量重拉；UIDNEXT 推进
- [ ] 同步事件 emit：`sync:new-email` / `sync:folders` / `sync:error`
- [ ] 单账户错误隔离与自动重试（指数退避）
- [ ] 同步期间 UI 状态（账户级「同步中」指示）

### 3. 通知与后台
- [ ] 新邮件系统通知（tauri-plugin-notification）：标题/发件人/摘要，点击跳转阅读
- [ ] 任务栏/托盘未读角标（Badge）
- [ ] 系统托盘常驻与最小化到托盘（可配置）
- [ ] 通知免打扰时段（P2）

## 三、验收标准

- [ ] 10 万封邮件内全文搜索 < 1 秒返回，关键词高亮正确
- [ ] 新邮件到达（IDLE）5 秒内系统通知，点击可定位
- [ ] 断网恢复后增量同步无重复、无丢失
- [ ] 单个账户失败不影响其他账户，自动重试生效
- [ ] 离线状态下已缓存邮件可读可搜索
- [ ] 未读角标数值与列表一致

## 四、涉及模块

- `src-tauri/src/services/sync_engine.rs`、`search_service.rs`、`notify_service.rs`
- `src-tauri/src/commands/search.rs`
- `src/features/search/`、`src/features/notifications/`

## 五、风险与注意

- FTS5 中文分词需确认 tokenizer（默认 unicode61 对中文按字切分，可接受；如需更好体验再评估 ICU/分词器）。
- IDLE 连接需要心跳保活与断线重连，避免「假在线」。
- 通知点击跳转需配合前端事件总线，确保窗口聚焦后定位到邮件。
