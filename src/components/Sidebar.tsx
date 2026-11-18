import { useMemo } from "react";
import { useQueryClient } from "@tanstack/react-query";
import {
  Archive,
  FileText,
  Folder as FolderIcon,
  Inbox,
  Mail,
  PenSquare,
  Plus,
  RefreshCw,
  Send,
  Settings,
  ShieldAlert,
  Trash2,
  UserRound,
  X,
} from "lucide-react";
import type { Account, Folder } from "@/lib/types";
import { mailApi } from "@/lib/api";
import { useAppStore, toast } from "@/store/app";
import { cn } from "@/lib/utils";

interface Props {
  accounts: Account[];
  folders: Folder[];
  syncing: boolean;
  onSync: () => void;
}

const KIND_ORDER: Record<string, number> = {
  inbox: 0,
  drafts: 1,
  sent: 2,
  archive: 3,
  junk: 4,
  trash: 5,
  other: 6,
};

function FolderIconFor({ kind }: { kind: string }) {
  switch (kind) {
    case "inbox":
      return <Inbox className="size-4 shrink-0" />;
    case "drafts":
      return <FileText className="size-4 shrink-0" />;
    case "sent":
      return <Send className="size-4 shrink-0" />;
    case "archive":
      return <Archive className="size-4 shrink-0" />;
    case "junk":
      return <ShieldAlert className="size-4 shrink-0" />;
    case "trash":
      return <Trash2 className="size-4 shrink-0" />;
    default:
      return <FolderIcon className="size-4 shrink-0" />;
  }
}

export function Sidebar({ accounts, folders, syncing, onSync }: Props) {
  const {
    view,
    accountId,
    folderId,
    selectAccount,
    selectFolder,
    setView,
    openCompose,
    setShowDrafts,
  } = useAppStore();

  const queryClient = useQueryClient();
  const activeAccount = accounts.find((a) => a.id === accountId) ?? null;

  const createFolder = async () => {
    if (!accountId) return;
    const name = window.prompt("新建文件夹名称");
    if (!name?.trim()) return;
    try {
      await mailApi.createFolder(accountId, name.trim());
      await queryClient.invalidateQueries({ queryKey: ["folders"] });
      toast.success("文件夹已创建");
    } catch (e) {
      toast.error(String(e));
    }
  };

  const removeFolder = async (folder: Folder) => {
    if (!window.confirm(`确定删除文件夹「${folder.name}」吗？`)) return;
    try {
      await mailApi.deleteFolder(folder.account_id, folder.id);
      await queryClient.invalidateQueries({ queryKey: ["folders"] });
      toast.success("文件夹已删除");
    } catch (e) {
      toast.error(String(e));
    }
  };
  const visibleFolders = useMemo(() => {
    if (!accountId) return [];
    return [...folders]
      .filter((f) => f.account_id === accountId)
      .sort(
        (a, b) =>
          (KIND_ORDER[a.kind] ?? 9) - (KIND_ORDER[b.kind] ?? 9) ||
          a.name.localeCompare(b.name),
      );
  }, [folders, accountId]);

  const inboxUnread = (id: number) =>
    folders
      .filter((f) => f.account_id === id && f.kind === "inbox")
      .reduce((sum, f) => sum + f.unread_count, 0);

  return (
    <aside className="bg-sidebar text-sidebar-foreground flex w-60 shrink-0 flex-col border-r">
      <div className="flex h-14 items-center gap-2 border-b px-4">
        <Mail className="text-primary size-5" />
        <span className="font-semibold">LiveEmail</span>
        <button
          onClick={onSync}
          disabled={syncing}
          className="hover:bg-sidebar-accent ml-auto rounded-md p-1.5 disabled:opacity-50"
          title="立即同步"
        >
          <RefreshCw className={cn("size-4", syncing && "animate-spin")} />
        </button>
      </div>

      <div className="p-2">
        <button
          onClick={() => openCompose({ mode: "new", accountId })}
          className="bg-primary text-primary-foreground hover:bg-primary/90 flex h-9 w-full items-center justify-center gap-2 rounded-md text-sm font-medium"
        >
          <PenSquare className="size-4" />
          写邮件
        </button>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-2 pb-2">
        <button
          onClick={() => selectAccount(null)}
          className={cn(
            "flex h-9 w-full items-center gap-2 rounded-md px-3 text-sm",
            view === "mail" && !accountId && !folderId
              ? "bg-sidebar-accent text-sidebar-accent-foreground font-medium"
              : "hover:bg-sidebar-accent/60",
          )}
        >
          <Inbox className="size-4" />
          统一收件箱
        </button>

        <div className="text-muted-foreground mt-4 px-3 text-xs font-medium">
          账户
        </div>
        <div className="mt-1 flex flex-col gap-0.5">
          {accounts.map((account) => {
            const isActive =
              view === "mail" && accountId === account.id && folderId === null;
            const unread = inboxUnread(account.id);
            return (
              <button
                key={account.id}
                onClick={() => selectAccount(account.id)}
                className={cn(
                  "flex h-9 items-center gap-2 rounded-md px-3 text-sm",
                  isActive
                    ? "bg-sidebar-accent text-sidebar-accent-foreground font-medium"
                    : "hover:bg-sidebar-accent/60",
                )}
                title={`${account.email}${account.last_error ? `\n${account.last_error}` : ""}`}
              >
                <UserRound className="size-4 shrink-0" />
                <span className="truncate">
                  {account.display_name || account.email}
                </span>
                {unread > 0 && (
                  <span className="bg-primary text-primary-foreground ml-auto rounded-full px-1.5 text-[10px] leading-4">
                    {unread > 99 ? "99+" : unread}
                  </span>
                )}
              </button>
            );
          })}
        </div>

        {activeAccount && (
          <>
            <div className="mt-4 flex items-center px-3">
              <span className="text-muted-foreground text-xs font-medium">
                文件夹
              </span>
              <button
                onClick={() => void createFolder()}
                className="hover:bg-sidebar-accent ml-auto rounded p-0.5"
                title="新建文件夹"
              >
                <Plus className="size-3.5" />
              </button>
            </div>
            <div className="mt-1 flex flex-col gap-0.5">
              {visibleFolders.map((folder) => {
                const depth = Math.min(
                  folder.path
                    .split(folder.delimiter || "/")
                    .filter(Boolean).length - 1,
                  3,
                );
                const editable = folder.kind === "other";
                return (
                  <div
                    key={folder.id}
                    className={cn(
                      "group flex h-8 items-center gap-2 rounded-md pr-2 text-sm",
                      folderId === folder.id
                        ? "bg-sidebar-accent text-sidebar-accent-foreground font-medium"
                        : "hover:bg-sidebar-accent/60",
                    )}
                    style={{ paddingLeft: 12 + depth * 12 }}
                  >
                    <button
                      onClick={() => selectFolder(folder.account_id, folder.id)}
                      className="flex min-w-0 flex-1 items-center gap-2 text-left"
                    >
                      <FolderIconFor kind={folder.kind} />
                      <span className="truncate">{folder.name}</span>
                      {folder.unread_count > 0 && (
                        <span className="text-muted-foreground ml-auto text-xs">
                          {folder.unread_count}
                        </span>
                      )}
                    </button>
                    {editable && (
                      <button
                        onClick={() => void removeFolder(folder)}
                        className="hover:bg-background hidden rounded p-0.5 group-hover:block"
                        title="删除文件夹"
                      >
                        <X className="size-3" />
                      </button>
                    )}
                  </div>
                );
              })}
            </div>
          </>
        )}
      </div>

      <div className="flex flex-col gap-1 border-t p-2">
        <button
          onClick={() => setShowDrafts(true)}
          className="hover:bg-sidebar-accent/60 flex h-9 items-center gap-2 rounded-md px-3 text-sm"
        >
          <FileText className="size-4" />
          草稿箱
        </button>
        <button
          onClick={() => setView("accounts")}
          className={cn(
            "flex h-9 items-center gap-2 rounded-md px-3 text-sm",
            view === "accounts"
              ? "bg-sidebar-accent text-sidebar-accent-foreground font-medium"
              : "hover:bg-sidebar-accent/60",
          )}
        >
          <UserRound className="size-4" />
          账户管理
        </button>
        <button
          onClick={() => setView("settings")}
          className={cn(
            "flex h-9 items-center gap-2 rounded-md px-3 text-sm",
            view === "settings"
              ? "bg-sidebar-accent text-sidebar-accent-foreground font-medium"
              : "hover:bg-sidebar-accent/60",
          )}
        >
          <Settings className="size-4" />
          设置
        </button>
      </div>
    </aside>
  );
}
