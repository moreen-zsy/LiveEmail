/** 与 Rust `models.rs` 对齐的前端类型 */

export interface Account {
  id: number;
  email: string;
  display_name: string;
  provider: string;
  imap_host: string;
  imap_port: number;
  imap_encryption: string;
  smtp_host: string;
  smtp_port: number;
  smtp_encryption: string;
  auth_type: string;
  created_at: number;
  is_default: boolean;
  last_sync_at: number | null;
  last_error: string | null;
  oauth_client_id: string | null;
  oauth_tenant: string | null;
  oauth_client_secret: string | null;
}

export interface AccountInput {
  email: string;
  display_name: string;
  provider: string;
  imap_host: string;
  imap_port: number;
  imap_encryption: string;
  smtp_host: string;
  smtp_port: number;
  smtp_encryption: string;
  auth_type: string;
  /** 密码 / 授权码；为空表示不修改已有凭据 */
  secret: string | null;
  oauth_client_id: string | null;
  oauth_tenant: string | null;
  oauth_client_secret: string | null;
}

export interface ProviderPreset {
  id: string;
  name: string;
  imap_host: string;
  imap_port: number;
  imap_encryption: string;
  smtp_host: string;
  smtp_port: number;
  smtp_encryption: string;
  auth_type: string;
  note: string | null;
}

export interface TestConnectionInput {
  account_id?: number | null;
  email: string;
  imap_host: string;
  imap_port: number;
  imap_encryption: string;
  smtp_host: string;
  smtp_port: number;
  smtp_encryption: string;
  auth_type: string;
  secret?: string | null;
}

export interface OAuthResult {
  ok: boolean;
  email: string;
  message: string;
}

export interface TestConnectionResult {
  ok: boolean;
  imap_ok: boolean;
  smtp_ok: boolean;
  imap_error: string | null;
  smtp_error: string | null;
}

export interface Folder {
  id: number;
  account_id: number;
  name: string;
  path: string;
  delimiter: string;
  kind: string;
  unread_count: number;
  total_count: number;
  uid_validity: number | null;
  uid_next: number | null;
}

export interface MessageSummary {
  id: number;
  account_id: number;
  folder_id: number;
  folder_name: string;
  account_email: string;
  uid: number;
  subject: string;
  from_name: string;
  from_addr: string;
  to_addrs: string;
  preview: string;
  date: number | null;
  is_read: boolean;
  is_flagged: boolean;
  has_attachment: boolean;
  size: number;
}

export interface AttachmentMeta {
  id: number;
  filename: string;
  mime_type: string;
  size: number;
  content_id: string | null;
  local_path: string | null;
}

export interface MessageDetail extends MessageSummary {
  message_id: string | null;
  cc_addrs: string;
  bcc_addrs: string;
  in_reply_to: string | null;
  references: string | null;
  text_body: string | null;
  html_body: string | null;
  attachments: AttachmentMeta[];
}

export interface MessageQuery {
  account_id?: number | null;
  folder_id?: number | null;
  unified?: boolean;
  search?: string | null;
  unread_only?: boolean;
  flagged_only?: boolean;
  has_attachment?: boolean;
  date_from?: number | null;
  date_to?: number | null;
  limit?: number;
  offset?: number;
}

export interface MessagePage {
  items: MessageSummary[];
  total: number;
}

export interface FlagInput {
  message_ids: number[];
  read?: boolean | null;
  flagged?: boolean | null;
}

export interface MoveInput {
  message_ids: number[];
  target_folder_id?: number | null;
  action: "delete" | "archive" | "junk" | "move";
}

export interface ActionResult {
  updated: number;
  message: string;
}

export interface Draft {
  id: number;
  account_id: number | null;
  subject: string;
  to_addrs: string;
  cc_addrs: string;
  bcc_addrs: string;
  body_html: string;
  body_text: string;
  updated_at: number;
  attachments: string[];
}

export interface DraftInput {
  id?: number | null;
  account_id?: number | null;
  subject: string;
  to_addrs: string;
  cc_addrs: string;
  bcc_addrs: string;
  body_html: string;
  body_text: string;
  attachments: string[];
}

export interface OutboxItem {
  id: number;
  account_id: number;
  subject: string;
  to_addrs: string;
  status: string;
  error: string | null;
  created_at: number;
  updated_at: number;
}

export interface ComposeInput {
  account_id: number;
  to_addrs: string;
  cc_addrs: string;
  bcc_addrs: string;
  subject: string;
  body_html: string;
  body_text: string;
  attachments: string[];
  in_reply_to: string | null;
  references: string | null;
  draft_id: number | null;
}

export interface SendResult {
  ok: boolean;
  message: string;
  archived: boolean;
  queued: boolean;
  outbox_id: number | null;
}

export interface StorageStats {
  db_bytes: number;
  attachments_bytes: number;
  attachment_count: number;
  message_count: number;
  account_count: number;
}

export interface AppSettings {
  theme: string;
  font_size: string;
  sync_interval_secs: number;
  signature: string;
  signature_per_account: boolean;
  close_to_tray: boolean;
  notifications_enabled: boolean;
  remote_images_trusted: boolean;
}

export interface SyncOutcome {
  new_messages: number;
  folders: number;
}

export interface SyncProgress {
  account_id: number;
  folder: string;
  index: number;
  total: number;
  new_messages: number;
}
