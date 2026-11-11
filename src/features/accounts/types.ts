/** 与 Rust `models.rs` 对齐的账户相关类型 */

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
  /** 已保存账户：传入以从系统凭据管理器读取凭据（凭据不经过前端） */
  account_id?: number | null;
  email: string;
  imap_host: string;
  imap_port: number;
  imap_encryption: string;
  smtp_host: string;
  smtp_port: number;
  smtp_encryption: string;
  auth_type: string;
  /** 保存前的即时测试传入；已保存账户留空 */
  secret?: string | null;
}

export interface TestConnectionResult {
  ok: boolean;
  imap_ok: boolean;
  smtp_ok: boolean;
  imap_error: string | null;
  smtp_error: string | null;
}
