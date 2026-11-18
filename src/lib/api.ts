import { invoke } from "@tauri-apps/api/core";
import { check as checkForUpdate } from "@tauri-apps/plugin-updater";
import type {
  Account,
  AccountInput,
  ActionResult,
  AppSettings,
  ComposeInput,
  Draft,
  DraftInput,
  FlagInput,
  Folder,
  MessageDetail,
  MessagePage,
  MessageQuery,
  MoveInput,
  OAuthResult,
  OutboxItem,
  ProviderPreset,
  SendResult,
  StorageStats,
  SyncOutcome,
  TestConnectionInput,
  TestConnectionResult,
} from "@/lib/types";

export const accountsApi = {
  list: () => invoke<Account[]>("list_accounts"),
  add: (input: AccountInput) => invoke<Account>("add_account", { input }),
  update: (id: number, input: AccountInput) =>
    invoke<Account | null>("update_account", { id, input }),
  remove: (id: number) => invoke<void>("delete_account", { id }),
  setDefault: (id: number) => invoke<void>("set_default_account", { id }),
  presets: () => invoke<ProviderPreset[]>("provider_presets"),
  testConnection: (input: TestConnectionInput) =>
    invoke<TestConnectionResult>("test_connection", { input }),

  oauthAuthorize: (accountId: number) =>
    invoke<OAuthResult>("oauth_authorize", { accountId }),
};

export const mailApi = {
  list: (query: MessageQuery) =>
    invoke<MessagePage>("list_messages", { query }),
  get: (id: number) => invoke<MessageDetail | null>("get_message", { id }),
  unreadCount: (accountId?: number | null) =>
    invoke<number>("unread_count", { accountId }),
  refreshBadge: () => invoke<number>("refresh_badge"),
  sync: (accountId: number) =>
    invoke<SyncOutcome>("sync_account", { accountId }),
  syncAll: () => invoke<number>("sync_all"),
  folders: (accountId?: number | null) =>
    invoke<Folder[]>("list_folders", { accountId }),
  createFolder: (accountId: number, name: string, parentPath?: string | null) =>
    invoke<Folder>("create_folder", { accountId, name, parentPath }),
  deleteFolder: (accountId: number, folderId: number) =>
    invoke<void>("delete_folder", { accountId, folderId }),
  setFlags: (input: FlagInput) =>
    invoke<ActionResult>("set_message_flags", { input }),
  move: (input: MoveInput) =>
    invoke<ActionResult>("move_messages", { input }),
  attachmentPath: (attachmentId: number) =>
    invoke<string | null>("attachment_path", { attachmentId }),
  saveAttachment: (attachmentId: number, destPath: string) =>
    invoke<string>("save_attachment", { attachmentId, destPath }),
};

export const composeApi = {
  send: (input: ComposeInput) => invoke<SendResult>("send_mail", { input }),
  retry: (id: number) => invoke<SendResult>("retry_outbox", { id }),
  outbox: () => invoke<OutboxItem[]>("list_outbox"),
  deleteOutbox: (id: number) => invoke<void>("delete_outbox", { id }),
  saveDraft: (input: DraftInput) => invoke<Draft>("save_draft", { input }),
  drafts: () => invoke<Draft[]>("list_drafts"),
  deleteDraft: (id: number) => invoke<void>("delete_draft", { id }),
};

export const updaterApi = {
  check: () => checkForUpdate(),
};

export const settingsApi = {
  get: () => invoke<AppSettings>("get_settings"),
  update: (value: AppSettings) => invoke<void>("update_settings", { value }),
  stats: () => invoke<StorageStats>("storage_stats"),
  clearCache: (scope: "attachments" | "messages" | "all") =>
    invoke<StorageStats>("clear_cache", { scope }),
};
