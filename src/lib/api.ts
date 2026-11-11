import { invoke } from "@tauri-apps/api/core";
import type {
  Account,
  AccountInput,
  ProviderPreset,
  TestConnectionInput,
  TestConnectionResult,
} from "@/features/accounts/types";

export const accountsApi = {
  list: () => invoke<Account[]>("list_accounts"),

  add: (input: AccountInput) => invoke<Account>("add_account", { input }),

  update: (id: number, input: AccountInput) =>
    invoke<Account | null>("update_account", { id, input }),

  remove: (id: number) => invoke<void>("delete_account", { id }),

  presets: () => invoke<ProviderPreset[]>("provider_presets"),

  testConnection: (input: TestConnectionInput) =>
    invoke<TestConnectionResult>("test_connection", { input }),
};
