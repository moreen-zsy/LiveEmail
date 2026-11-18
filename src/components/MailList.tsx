import { useEffect, useMemo, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useVirtualizer } from "@tanstack/react-virtual";
import {
  Archive,
  Flag,
  Inbox,
  Loader2,
  Mail,
  MailOpen,
  Paperclip,
  ShieldAlert,
  Star,
  Trash2,
} from "lucide-react";
import { mailApi } from "@/lib/api";
import type { MessageQuery } from "@/lib/types";
import { useAppStore, toast } from "@/store/app";
import { cn, formatMailDate, senderLabel } from "@/lib/utils";

const PAGE_SIZE = 200;

export function MailList() {
  const {
    accountId,
    folderId,
    selectedId,
    search,
    unreadOnly,
    flaggedOnly,
    hasAttachment,
    selection,
    selectMessage,
    toggleSelection,
    setSelection,
    clearSelection,
    setUnreadOnly,
    setFlaggedOnly,
    setHasAttachment,
    setSearch,
  } = useAppStore();
  const queryClient = useQueryClient();
  const [searchInput, setSearchInput] = useState(search);
  const [busy, setBusy] = useState(false);
  const parentRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const timer = window.setTimeout(() => setSearch(searchInput.trim()), 300);
    return () => window.clearTimeout(timer);
  }, [searchInput, setSearch]);

  useEffect(() => {
    setSearchInput(search);
  }, [search]);

  const query: MessageQuery = useMemo(
    () => ({
      account_id: accountId,
      folder_id: folderId,
      unified: true,
      search: search || null,
      unread_only: unreadOnly,
      flagged_only: flaggedOnly,
      has_attachment: hasAttachment,
      limit: PAGE_SIZE,
      offset: 0,
    }),
    [accountId, folderId, search, unreadOnly, flaggedOnly, hasAttachment],
  );

  const { data, isLoading, isFetching, error } = useQuery({
    queryKey: ["messages", query],
    queryFn: () => mailApi.list(query),
    refetchOnWindowFocus: false,
  });

  const items = data?.items ?? [];
  const total = data?.total ?? 0;

  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 78,
    overscan: 8,
  });

  const invalidate = () => {
    void queryClient.invalidateQueries({ queryKey: ["messages"] });
    void queryClient.invalidateQueries({ queryKey: ["folders"] });
    void queryClient.invalidateQueries({ queryKey: ["unread"] });
  };

  const handleSelect = async (id: number, isRead: boolean) => {
    selectMessage(id);
    if (!isRead) {
      try {
        await mailApi.setFlags({ message_ids: [id], read: true });
        invalidate();
        void queryClient.invalidateQueries({ queryKey: ["message", id] });
      } catch {
        // 标记已读失败不阻断阅读
      }
    }
  };

  const runFlags = async (read?: boolean, flagged?: boolean) => {
    const ids = selection.size
      ? [...selection]
      : selectedId
        ? [selectedId]
        : [];
    if (!ids.length) return;
    setBusy(true);
    try {
      const result = await mailApi.setFlags({
        message_ids: ids,
        read,
        flagged,
      });
      toast.success(result.message);
      clearSelection();
      invalidate();
      ids.forEach((id) =>
        queryClient.invalidateQueries({ queryKey: ["message", id] }),
      );
    } catch (e) {
      toast.error(String(e));
    } finally {
      setBusy(false);
    }
  };

  const runMove = async (action: "archive" | "delete" | "junk") => {
    const ids = selection.size
      ? [...selection]
      : selectedId
        ? [selectedId]
        : [];
    if (!ids.length) return;
    if (action === "delete") {
      const ok = window.confirm(`确定删除选中的 ${ids.length} 封邮件吗？`);
      if (!ok) return;
    }
    setBusy(true);
    try {
      const result = await mailApi.move({
        message_ids: ids,
        target_folder_id: null,
        action,
      });
      toast.success(result.message);
      clearSelection();
      selectMessage(null);
      invalidate();
    } catch (e) {
      toast.error(String(e));
    } finally {
      setBusy(false);
    }
  };

  const allSelected = items.length > 0 && items.every((m) => selection.has(m.id));

  return (
    <section className="flex w-[400px] shrink-0 flex-col border-r">
      <div className="flex h-14 items-center gap-2 border-b px-3">
        <div className="relative flex-1">
          <Mail className="text-muted-foreground pointer-events-none absolute top-2.5 left-2.5 size-4" />
          <input
            id="mail-search"
            value={searchInput}
            onChange={(e) => setSearchInput(e.target.value)}
            placeholder="搜索主题、发件人、正文…"
            className="border-input bg-background h-9 w-full rounded-md border pr-3 pl-8 text-sm"
          />
        </div>
        {isFetching && <Loader2 className="text-muted-foreground size-4 animate-spin" />}
      </div>

      <div className="flex items-center gap-1 border-b px-2 py-1.5">
        <FilterChip active={unreadOnly} onClick={() => setUnreadOnly(!unreadOnly)}>
          未读
        </FilterChip>
        <FilterChip active={flaggedOnly} onClick={() => setFlaggedOnly(!flaggedOnly)}>
          星标
        </FilterChip>
        <FilterChip
          active={hasAttachment}
          onClick={() => setHasAttachment(!hasAttachment)}
        >
          附件
        </FilterChip>
        <span className="text-muted-foreground ml-auto pr-1 text-xs">
          {total} 封
        </span>
      </div>

      {selection.size > 0 ? (
        <div className="bg-accent/60 flex items-center gap-1 border-b px-2 py-1.5 text-sm">
          <span className="px-1 text-xs">已选 {selection.size} 封</span>
          <div className="ml-auto flex items-center gap-0.5">
            <IconBtn title="归档" onClick={() => void runMove("archive")}>
              <Archive className="size-4" />
            </IconBtn>
            <IconBtn title="标记垃圾" onClick={() => void runMove("junk")}>
              <ShieldAlert className="size-4" />
            </IconBtn>
            <IconBtn title="删除" onClick={() => void runMove("delete")}>
              <Trash2 className="size-4" />
            </IconBtn>
            <IconBtn title="标为已读" onClick={() => void runFlags(true)}>
              <MailOpen className="size-4" />
            </IconBtn>
            <IconBtn title="标为未读" onClick={() => void runFlags(false)}>
              <Mail className="size-4" />
            </IconBtn>
            <IconBtn title="星标" onClick={() => void runFlags(undefined, true)}>
              <Star className="size-4" />
            </IconBtn>
            <button
              onClick={clearSelection}
              className="text-muted-foreground ml-1 px-2 text-xs hover:underline"
            >
              取消
            </button>
          </div>
        </div>
      ) : (
        <div className="flex items-center gap-2 border-b px-3 py-1.5">
          <input
            type="checkbox"
            checked={allSelected}
            onChange={() => {
              if (allSelected) clearSelection();
              else setSelection(items.map((m) => m.id));
            }}
            className="size-3.5 accent-current"
          />
          <span className="text-muted-foreground text-xs">全选</span>
          {busy && <Loader2 className="size-3.5 animate-spin" />}
        </div>
      )}

      <div ref={parentRef} className="min-h-0 flex-1 overflow-y-auto">
        {isLoading ? (
          <div className="text-muted-foreground flex items-center gap-2 p-6 text-sm">
            <Loader2 className="size-4 animate-spin" />
            正在加载邮件…
          </div>
        ) : error ? (
          <div className="text-destructive p-6 text-sm">{String(error)}</div>
        ) : items.length === 0 ? (
          <EmptyList searching={Boolean(search)} />
        ) : (
          <div
            style={{ height: virtualizer.getTotalSize(), position: "relative" }}
          >
            {virtualizer.getVirtualItems().map((row) => {
              const message = items[row.index];
              return (
                <div
                  key={message.id}
                  style={{
                    position: "absolute",
                    top: 0,
                    left: 0,
                    width: "100%",
                    height: row.size,
                    transform: `translateY(${row.start}px)`,
                  }}
                >
                  <MessageRow
                    message={message}
                    active={selectedId === message.id}
                    checked={selection.has(message.id)}
                    onToggle={() => toggleSelection(message.id)}
                    onOpen={() => void handleSelect(message.id, message.is_read)}
                    showAccount={!accountId}
                  />
                </div>
              );
            })}
          </div>
        )}
      </div>
    </section>
  );
}

function MessageRow({
  message,
  active,
  checked,
  onToggle,
  onOpen,
  showAccount,
}: {
  message: import("@/lib/types").MessageSummary;
  active: boolean;
  checked: boolean;
  onToggle: () => void;
  onOpen: () => void;
  showAccount: boolean;
}) {
  return (
    <div
      onClick={onOpen}
      className={cn(
        "flex h-full cursor-pointer gap-2 border-b px-3 py-2 transition-colors",
        active ? "bg-accent" : "hover:bg-accent/40",
        !message.is_read && "bg-primary/[0.03]",
      )}
    >
      <div className="flex items-start pt-1">
        <input
          type="checkbox"
          checked={checked}
          onChange={onToggle}
          onClick={(e) => e.stopPropagation()}
          className="size-3.5 accent-current"
        />
      </div>
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-1.5">
          {!message.is_read && (
            <span className="bg-primary size-1.5 shrink-0 rounded-full" />
          )}
          <span
            className={cn(
              "truncate text-sm",
              message.is_read ? "font-normal" : "font-semibold",
            )}
          >
            {senderLabel(message.from_name, message.from_addr)}
          </span>
          {message.is_flagged && (
            <Flag className="size-3 shrink-0 fill-amber-400 text-amber-500" />
          )}
          {message.has_attachment && (
            <Paperclip className="text-muted-foreground size-3 shrink-0" />
          )}
          <span className="text-muted-foreground ml-auto shrink-0 text-xs">
            {formatMailDate(message.date)}
          </span>
        </div>
        <div
          className={cn(
            "truncate text-sm",
            message.is_read ? "" : "font-medium",
          )}
        >
          {message.subject || "(无主题)"}
        </div>
        <div className="text-muted-foreground flex items-center gap-2 text-xs">
          <span className="truncate">{message.preview || "—"}</span>
          {showAccount && (
            <span className="ml-auto shrink-0 rounded bg-secondary px-1 text-[10px]">
              {message.account_email}
            </span>
          )}
        </div>
      </div>
    </div>
  );
}

function FilterChip({
  active,
  onClick,
  children,
}: {
  active: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "rounded-full border px-2.5 py-0.5 text-xs transition-colors",
        active
          ? "bg-primary text-primary-foreground border-primary"
          : "text-muted-foreground hover:bg-accent",
      )}
    >
      {children}
    </button>
  );
}

function IconBtn({
  title,
  onClick,
  children,
}: {
  title: string;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      title={title}
      onClick={onClick}
      className="hover:bg-background inline-flex size-7 items-center justify-center rounded"
    >
      {children}
    </button>
  );
}

function EmptyList({ searching }: { searching: boolean }) {
  return (
    <div className="flex flex-col items-center gap-2 px-6 py-16 text-center">
      <Inbox className="text-muted-foreground size-8" />
      <p className="text-sm font-medium">
        {searching ? "没有匹配的邮件" : "这里还没有邮件"}
      </p>
      <p className="text-muted-foreground text-xs">
        {searching ? "换个关键词，或清除筛选条件" : "点击右上角同步按钮拉取邮件"}
      </p>
    </div>
  );
}
