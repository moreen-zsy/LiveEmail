import { useEffect, useState } from "react";
import {
  QueryClient,
  QueryClientProvider,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { Mail, Plus, X } from "lucide-react";
import { Sidebar } from "@/components/Sidebar";
import { MailList } from "@/components/MailList";
import { ReaderPane } from "@/components/ReaderPane";
import { AccountSettings } from "@/features/accounts/AccountSettings";
import { SettingsView } from "@/features/settings/SettingsView";
import { ComposeDialog } from "@/features/compose/ComposeDialog";
import { DraftsDialog } from "@/features/compose/DraftsDialog";
import { accountsApi, mailApi, settingsApi } from "@/lib/api";
import type { MessageDetail, MessagePage, MessageSummary } from "@/lib/types";
import { isTauri } from "@/lib/utils";
import { toast, useAppStore, useToastStore } from "@/store/app";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 10_000,
      refetchOnWindowFocus: false,
      retry: 1,
    },
  },
});

function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <AppShell />
    </QueryClientProvider>
  );
}

function AppShell() {
  const {
    view,
    accountId,
    selectedId,
    selection,
    compose,
    showDrafts,
    setView,
    closeCompose,
    selectMessage,
    clearSelection,
  } = useAppStore();
  const client = useQueryClient();
  const [syncing, setSyncing] = useState(false);

  const { data: accounts = [], isLoading: accountsLoading } = useQuery({
    queryKey: ["accounts"],
    queryFn: accountsApi.list,
  });

  const { data: settings } = useQuery({
    queryKey: ["settings"],
    queryFn: settingsApi.get,
  });

  const { data: folders = [] } = useQuery({
    queryKey: ["folders", "all"],
    queryFn: () => mailApi.folders(null),
    refetchInterval: 30_000,
  });

  // 主题
  useEffect(() => {
    const theme = settings?.theme ?? "system";
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () => {
      const dark = theme === "dark" || (theme === "system" && media.matches);
      document.documentElement.classList.toggle("dark", dark);
    };
    apply();
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [settings?.theme]);

  // 字号
  useEffect(() => {
    const map: Record<string, string> = { sm: "14px", md: "16px", lg: "18px" };
    document.documentElement.style.fontSize = map[settings?.font_size ?? "md"] ?? "16px";
  }, [settings?.font_size]);

  // 没有账户时引导到账户页
  useEffect(() => {
    if (!accountsLoading && accounts.length === 0 && view === "mail") {
      setView("accounts");
    }
  }, [accountsLoading, accounts.length, view, setView]);

  // 同步事件
  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false;
    const unlisteners: Array<() => void> = [];
    const setup = async () => {
      try {
        const { listen } = await import("@tauri-apps/api/event");
        const done = await listen("sync:done", () => {
          setSyncing(false);
          void client.invalidateQueries({ queryKey: ["messages"] });
          void client.invalidateQueries({ queryKey: ["folders"] });
          void client.invalidateQueries({ queryKey: ["unread"] });
          void client.invalidateQueries({ queryKey: ["storage-stats"] });
        });
        const error = await listen<{ error: string }>("sync:error", (event) => {
          toast.error(`同步失败：${event.payload.error}`);
        });
        const start = await listen("sync:progress", () => setSyncing(true));
        if (disposed) {
          done();
          error();
          start();
        } else {
          unlisteners.push(done, error, start);
        }
      } catch {
        // 非 Tauri 环境忽略
      }
    };
    void setup();
    return () => {
      disposed = true;
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, [client]);

  const invalidateAll = () => {
    void client.invalidateQueries({ queryKey: ["messages"] });
    void client.invalidateQueries({ queryKey: ["folders"] });
    void client.invalidateQueries({ queryKey: ["unread"] });
  };

  const syncNow = async () => {
    if (!accounts.length) {
      toast.info("请先添加一个邮箱账户");
      setView("accounts");
      return;
    }
    setSyncing(true);
    try {
      await mailApi.syncAll();
      window.setTimeout(() => setSyncing(false), 15_000);
    } catch (e) {
      toast.error(String(e));
      setSyncing(false);
    }
  };

  // 快捷键
  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      if (
        target?.closest("input, textarea, select, [contenteditable='true']") ||
        event.ctrlKey ||
        event.metaKey ||
        event.altKey
      ) {
        return;
      }
      const state = useAppStore.getState();
      const ids = state.selection.size
        ? [...state.selection]
        : state.selectedId
          ? [state.selectedId]
          : [];
      const findMessage = (): MessageSummary | null => {
        if (!state.selectedId) return null;
        const detail = client.getQueryData<MessageDetail>([
          "message",
          state.selectedId,
        ]);
        if (detail) return detail;
        const pages = client.getQueriesData<MessagePage>({
          queryKey: ["messages"],
        });
        for (const [, page] of pages) {
          const hit = page?.items.find((item) => item.id === state.selectedId);
          if (hit) return hit;
        }
        return null;
      };

      switch (event.key) {
        case "c":
        case "C":
          event.preventDefault();
          state.openCompose({ mode: "new", accountId: state.accountId });
          break;
        case "r":
        case "R": {
          const message = findMessage();
          if (message) {
            event.preventDefault();
            state.openCompose({
              mode: "reply",
              accountId: message.account_id,
              message,
            });
          }
          break;
        }
        case "f":
        case "F": {
          const message = findMessage();
          if (message) {
            event.preventDefault();
            state.openCompose({
              mode: "forward",
              accountId: message.account_id,
              message,
            });
          }
          break;
        }
        case "/":
          event.preventDefault();
          document.getElementById("mail-search")?.focus();
          break;
        case "e":
        case "E":
          if (ids.length) {
            event.preventDefault();
            void runMove("archive", ids);
          }
          break;
        case "#":
          if (ids.length) {
            event.preventDefault();
            void runMove("delete", ids);
          }
          break;
        case "u":
        case "U":
          if (ids.length) {
            event.preventDefault();
            void mailApi
              .setFlags({ message_ids: ids, read: false })
              .then(() => {
                toast.success("已标为未读");
                state.clearSelection();
                invalidateAll();
              })
              .catch((e) => toast.error(String(e)));
          }
          break;
        case "s":
        case "S": {
          if (ids.length) {
            event.preventDefault();
            const detail = state.selectedId
              ? client.getQueryData<MessageDetail>(["message", state.selectedId])
              : null;
            void mailApi
              .setFlags({ message_ids: ids, flagged: !detail?.is_flagged })
              .then(() => {
                toast.success("星标已更新");
                state.clearSelection();
                invalidateAll();
              })
              .catch((e) => toast.error(String(e)));
          }
          break;
        }
        default:
          break;
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [client, selectedId, selection, accountId]);

  const runMove = async (
    action: "archive" | "delete" | "junk",
    ids: number[],
  ) => {
    if (action === "delete") {
      const ok = window.confirm(`确定删除选中的 ${ids.length} 封邮件吗？`);
      if (!ok) return;
    }
    try {
      const result = await mailApi.move({
        message_ids: ids,
        target_folder_id: null,
        action,
      });
      toast.success(result.message);
      clearSelection();
      if (selectedId && ids.includes(selectedId)) selectMessage(null);
      invalidateAll();
    } catch (e) {
      toast.error(String(e));
    }
  };

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-background text-foreground">
      <Sidebar
        accounts={accounts}
        folders={folders}
        syncing={syncing}
        onSync={() => void syncNow()}
      />

      <main className="flex min-w-0 flex-1">
        {view === "mail" ? (
          accounts.length === 0 ? (
            <Onboarding onAdd={() => setView("accounts")} />
          ) : (
            <>
              <MailList />
              <ReaderPane />
            </>
          )
        ) : view === "accounts" ? (
          <div className="min-w-0 flex-1 overflow-y-auto">
            <AccountSettings />
          </div>
        ) : (
          <div className="min-w-0 flex-1 overflow-y-auto">
            <SettingsView />
          </div>
        )}
      </main>

      {compose && (
        <ComposeDialog
          intent={compose}
          accounts={accounts}
          signature={settings?.signature ?? ""}
          onClose={closeCompose}
        />
      )}
      {showDrafts && <DraftsDialog />}
      <Toasts />
    </div>
  );
}

function Onboarding({ onAdd }: { onAdd: () => void }) {
  return (
    <div className="flex h-full flex-1 flex-col items-center justify-center gap-3 text-center">
      <Mail className="text-muted-foreground size-10" />
      <h2 className="text-lg font-semibold">欢迎使用 LiveEmail</h2>
      <p className="text-muted-foreground max-w-sm text-sm">
        添加一个邮箱账户开始收发邮件。支持 Gmail、Outlook、QQ、163
        及自定义 IMAP/SMTP 服务。
      </p>
      <button
        onClick={onAdd}
        className="bg-primary text-primary-foreground hover:bg-primary/90 mt-2 inline-flex h-9 items-center gap-2 rounded-md px-4 text-sm font-medium"
      >
        <Plus className="size-4" />
        添加账户
      </button>
    </div>
  );
}

function Toasts() {
  const { toasts, dismiss } = useToastStore();
  return (
    <div className="pointer-events-none fixed right-4 bottom-4 z-[100] flex w-80 flex-col gap-2">
      {toasts.map((item) => (
        <div
          key={item.id}
          className={[
            "pointer-events-auto flex items-start gap-2 rounded-lg border px-3 py-2 text-sm shadow-lg",
            item.kind === "error"
              ? "border-destructive/40 bg-destructive/10 text-destructive"
              : item.kind === "success"
                ? "border-green-600/30 bg-green-600/10 text-green-700 dark:text-green-400"
                : "bg-card",
          ].join(" ")}
        >
          <span className="min-w-0 flex-1 break-words">{item.text}</span>
          <button
            onClick={() => dismiss(item.id)}
            className="hover:bg-accent rounded p-0.5"
            aria-label="关闭提示"
          >
            <X className="size-3.5" />
          </button>
        </div>
      ))}
    </div>
  );
}

export default App;
