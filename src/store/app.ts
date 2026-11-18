import { create } from "zustand";
import type { MessageSummary } from "@/lib/types";

export type View = "mail" | "accounts" | "settings";

export type ComposeMode = "new" | "reply" | "replyAll" | "forward" | "draft";

export interface ComposeIntent {
  mode: ComposeMode;
  accountId: number | null;
  message?: MessageSummary | null;
  draftId?: number | null;
}

interface AppState {
  view: View;
  /** null 表示统一收件箱 */
  accountId: number | null;
  folderId: number | null;
  selectedId: number | null;
  search: string;
  unreadOnly: boolean;
  flaggedOnly: boolean;
  hasAttachment: boolean;
  selection: Set<number>;
  compose: ComposeIntent | null;
  showDrafts: boolean;

  setView: (view: View) => void;
  selectAccount: (accountId: number | null) => void;
  selectFolder: (accountId: number, folderId: number) => void;
  selectMessage: (id: number | null) => void;
  setSearch: (value: string) => void;
  setUnreadOnly: (value: boolean) => void;
  setFlaggedOnly: (value: boolean) => void;
  setHasAttachment: (value: boolean) => void;
  toggleSelection: (id: number) => void;
  setSelection: (ids: number[]) => void;
  clearSelection: () => void;
  openCompose: (intent: ComposeIntent) => void;
  closeCompose: () => void;
  setShowDrafts: (value: boolean) => void;
}

export const useAppStore = create<AppState>((set) => ({
  view: "mail",
  accountId: null,
  folderId: null,
  selectedId: null,
  search: "",
  unreadOnly: false,
  flaggedOnly: false,
  hasAttachment: false,
  selection: new Set<number>(),
  compose: null,
  showDrafts: false,

  setView: (view) => set({ view }),
  selectAccount: (accountId) =>
    set({
      accountId,
      folderId: null,
      selectedId: null,
      selection: new Set<number>(),
      view: "mail",
    }),
  selectFolder: (accountId, folderId) =>
    set({
      accountId,
      folderId,
      selectedId: null,
      selection: new Set<number>(),
      view: "mail",
    }),
  selectMessage: (selectedId) => set({ selectedId }),
  setSearch: (search) => set({ search }),
  setUnreadOnly: (unreadOnly) => set({ unreadOnly }),
  setFlaggedOnly: (flaggedOnly) => set({ flaggedOnly }),
  setHasAttachment: (hasAttachment) => set({ hasAttachment }),
  toggleSelection: (id) =>
    set((state) => {
      const next = new Set(state.selection);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return { selection: next };
    }),
  setSelection: (ids) => set({ selection: new Set(ids) }),
  clearSelection: () => set({ selection: new Set<number>() }),
  openCompose: (compose) => set({ compose }),
  closeCompose: () => set({ compose: null }),
  setShowDrafts: (showDrafts) => set({ showDrafts }),
}));

export interface Toast {
  id: number;
  kind: "info" | "success" | "error";
  text: string;
}

interface ToastState {
  toasts: Toast[];
  push: (text: string, kind?: Toast["kind"]) => void;
  dismiss: (id: number) => void;
}

let toastId = 0;

export const useToastStore = create<ToastState>((set) => ({
  toasts: [],
  push: (text, kind = "info") => {
    const id = ++toastId;
    set((state) => ({ toasts: [...state.toasts, { id, kind, text }] }));
    window.setTimeout(() => {
      set((state) => ({ toasts: state.toasts.filter((t) => t.id !== id) }));
    }, 5000);
  },
  dismiss: (id) =>
    set((state) => ({ toasts: state.toasts.filter((t) => t.id !== id) })),
}));

export const toast = {
  info: (text: string) => useToastStore.getState().push(text, "info"),
  success: (text: string) => useToastStore.getState().push(text, "success"),
  error: (text: string) => useToastStore.getState().push(text, "error"),
};
