# b. 第二批 —— 邮件接收与阅读

> 目标：打通 IMAP 接收 → MIME 解析 → 本地缓存 → 列表与阅读渲染的完整链路。
> 前置：第一批（账户体系与连接能力）。

## 一、目标与交付物

1. async-imap 集成，完成文件夹与邮件拉取。
2. mail-parser 解析 MIME（headers / 正文 / 附件元数据）。
3. 邮件落库 SQLite（messages / message_bodies / attachments）。
4. 邮件列表（虚拟列表）+ 邮件阅读器（HTML 净化渲染）。
5. 附件下载与另存为。

## 二、任务清单

### 1. IMAP 服务
- [ ] `imap_service`：连接（SSL/STARTTLS）、登录、文件夹列表（LIST）
- [ ] 首次全量同步：按 folder 拉取 `UID + FLAGS + INTERNALDATE + ENVELOPE + BODY` 
- [ ] 增量同步：以 `sync_state`（UIDVALIDITY/UIDNEXT）为游标拉取新增
- [ ] 幂等 upsert：`ON CONFLICT(folder_id, uid)` 不产生重复
- [ ] 连接池/超时与错误分类（认证/网络/协议）

### 2. 解析与落库
- [ ] `parse_service`：mail-parser 解析 headers、text/plain、text/html、附件（含内嵌图 content_id）
- [ ] 正文分表存储；附件元数据入库、二进制写入 `$APPDATA/LiveEmail/attachments`
- [ ] 邮件 flags（未读/星标）映射落库
- [ ] 大附件策略：默认只存元数据，正文图片按需下载

### 3. Tauri commands
- [ ] `sync_account(account_id)`：触发同步，返回/推送进度事件
- [ ] `fetch_messages(folder_id, page, cursor)`：分页查询（供虚拟列表）
- [ ] `fetch_message(message_id)`：完整邮件（正文 + 附件元数据）
- [ ] `download_attachment(attachment_id)`：附件下载到临时/指定目录
- [ ] 同步事件：`sync:progress` / `sync:done` / `sync:error` 通过 emit 推送

### 4. 前端 UI
- [ ] 三栏布局：账户栏（第一批已有）｜邮件列表｜阅读器
- [ ] 邮件列表项：发件人、主题、摘要、时间、未读/星标/附件标识
- [ ] @tanstack/react-virtual 虚拟列表，React Query 管理分页缓存
- [ ] 阅读器：HTML 经 DOMPurify 净化渲染；远程图片默认阻止，提供「显示图片」信任操作
- [ ] 纯文本邮件渲染；头部信息折叠展示
- [ ] 附件栏：内联图片预览、下载、另存为（dialog）
- [ ] 统一收件箱视图（跨账户聚合，按时间倒序）+ 按账户/文件夹切换

## 三、验收标准

- [ ] 一个含 ≥1 万封邮件的文件夹可完整同步且无重复
- [ ] 列表万封滚动流畅（虚拟列表生效，不一次性渲染全部）
- [ ] HTML 邮件（含脚本/远程图片/恶意标签）被安全净化渲染，无脚本执行
- [ ] 附件可下载、可另存为，内嵌图片正常显示
- [ ] 断网后已缓存邮件仍可阅读
- [ ] 同步进度与错误在前端可见，单账户失败不影响其他账户

## 四、涉及模块

- `src-tauri/src/services/imap_service.rs`、`parse_service.rs`、`sync_service.rs`
- `src-tauri/src/commands/mail.rs`
- `src/features/mail/`（列表、阅读器、附件）

## 五、风险与注意

- 大文件夹首次全量同步耗时长，需进度反馈与可中断/续传。
- HTML 邮件渲染是安全重点：DOMPurify + CSP + 远程内容默认阻止缺一不可。
- 附件下载与缓存需磁盘空间检查，避免撑爆磁盘。
