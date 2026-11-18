import { useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import DOMPurify from "dompurify";
import {
  Archive,
  Download,
  Forward,
  Inbox,
  Loader2,
  Mail,
  MailOpen,
  Paperclip,
  Reply,
  ReplyAll,
  ShieldAlert,
  Star,
  Trash2,
} from "lucide-react";
import { openPath, openUrl } from "@tauri-apps/plugin-opener";
import { save } from "@tauri-apps/plugin-dialog";
import { mailApi, settingsApi } from "@/lib/api";
import type { AttachmentMeta } from "@/lib/types";
import { useAppStore, toast } from "@/store/app";
import {
  cn,
  formatBytes,
  formatFullDate,
  isTauri,
  senderLabel,
} from "@/lib/utils";

export function ReaderPane() {
  const { selectedId, openCompose, selectMessage } = useAppStore();
  const queryClient = useQueryClient();
  const [busy, setBusy] = useState(false);

  const { data: settings } = useQuery({
    queryKey: ["settings"],
    queryFn: settingsApi.get,
  });

  const { data: message, isLoading } = useQuery({
    queryKey: ["message", selectedId],
    queryFn: () => mailApi.get(selectedId as number),
    enabled: selectedId != null,
  });

  const html = useMemo(
    () =>
      message?.html_body
        ? sanitizeHtml(message.html_body, settings?.remote_images_trusted ?? false)
        : null,
    [message, settings?.remote_images_trusted],
  );

  const invalidate = () => {
    void queryClient.invalidateQueries({ queryKey: ["messages"] });
    void queryClient.invalidateQueries({ queryKey: ["folders"] });
  };

  const runFlags = async (read?: boolean, flagged?: boolean) => {
    if (!message) return;
    setBusy(true);
    try {
      await mailApi.setFlags({ message_ids: [message.id], read, flagged });
      invalidate();
      void queryClient.invalidateQueries({ queryKey: ["message", message.id] });
    } catch (e) {
      toast.error(String(e));
    } finally {
      setBusy(false);
    }
  };

  const runMove = async (action: "archive" | "delete" | "junk") => {
    if (!message) return;
    if (action === "delete" && !window.confirm("确定删除这封邮件吗？")) return;
    setBusy(true);
    try {
      const result = await mailApi.move({
        message_ids: [message.id],
        target_folder_id: null,
        action,
      });
      toast.success(result.message);
      selectMessage(null);
      invalidate();
    } catch (e) {
      toast.error(String(e));
    } finally {
      setBusy(false);
    }
  };

  const openAttachment = async (attachment: AttachmentMeta) => {
    if (!attachment.local_path) {
      toast.error("附件尚未缓存到本地，请先同步该邮件");
      return;
    }
    try {
      if (isTauri()) await openPath(attachment.local_path);
    } catch (e) {
      toast.error(String(e));
    }
  };

  const downloadAttachment = async (attachment: AttachmentMeta) => {
    try {
      if (!isTauri()) return;
      const dest = await save({ defaultPath: attachment.filename });
      if (!dest) return;
      await mailApi.saveAttachment(attachment.id, dest);
      toast.success("附件已保存");
    } catch (e) {
      toast.error(String(e));
    }
  };

  const handleContentClick = (event: React.MouseEvent<HTMLDivElement>) => {
    const anchor = (event.target as HTMLElement).closest("a");
    if (!anchor) return;
    const href = anchor.getAttribute("href");
    if (!href || href.startsWith("data:")) return;
    event.preventDefault();
    if (isTauri()) {
      void openUrl(href).catch((e) => toast.error(String(e)));
    } else {
      window.open(href, "_blank", "noopener,noreferrer");
    }
  };

  if (selectedId == null) {
    return (
      <div className="flex h-full flex-1 flex-col items-center justify-center gap-3 text-center">
        <Inbox className="text-muted-foreground size-10" />
        <p className="text-sm font-medium">选择一封邮件开始阅读</p>
        <p className="text-muted-foreground max-w-xs text-xs">
          支持全文搜索、附件下载、回复、转发与快捷操作
        </p>
      </div>
    );
  }

  if (isLoading) {
    return (
      <div className="text-muted-foreground flex flex-1 items-center justify-center gap-2 text-sm">
        <Loader2 className="size-4 animate-spin" />
        正在加载邮件…
      </div>
    );
  }

  if (!message) {
    return (
      <div className="text-muted-foreground flex flex-1 items-center justify-center text-sm">
        邮件不存在或已被删除
      </div>
    );
  }

  return (
    <section className="flex min-w-0 flex-1 flex-col">
      <div className="flex h-14 items-center gap-1 border-b px-3">
        <ToolbarButton
          title="回复 (R)"
          onClick={() =>
            openCompose({
              mode: "reply",
              accountId: message.account_id,
              message,
            })
          }
        >
          <Reply className="size-4" />
        </ToolbarButton>
        <ToolbarButton
          title="全部回复"
          onClick={() =>
            openCompose({
              mode: "replyAll",
              accountId: message.account_id,
              message,
            })
          }
        >
          <ReplyAll className="size-4" />
        </ToolbarButton>
        <ToolbarButton
          title="转发 (F)"
          onClick={() =>
            openCompose({
              mode: "forward",
              accountId: message.account_id,
              message,
            })
          }
        >
          <Forward className="size-4" />
        </ToolbarButton>
        <div className="bg-border mx-1 h-5 w-px" />
        <ToolbarButton title="归档 (E)" onClick={() => void runMove("archive")}>
          <Archive className="size-4" />
        </ToolbarButton>
        <ToolbarButton title="标记垃圾" onClick={() => void runMove("junk")}>
          <ShieldAlert className="size-4" />
        </ToolbarButton>
        <ToolbarButton title="删除 (#)" onClick={() => void runMove("delete")}>
          <Trash2 className="size-4" />
        </ToolbarButton>
        <div className="bg-border mx-1 h-5 w-px" />
        <ToolbarButton
          title={message.is_read ? "标为未读 (U)" : "标为已读"}
          onClick={() => void runFlags(!message.is_read)}
        >
          {message.is_read ? <Mail className="size-4" /> : <MailOpen className="size-4" />}
        </ToolbarButton>
        <ToolbarButton
          title={message.is_flagged ? "取消星标 (S)" : "星标 (S)"}
          onClick={() => void runFlags(undefined, !message.is_flagged)}
        >
          <Star
            className={cn(
              "size-4",
              message.is_flagged && "fill-amber-400 text-amber-500",
            )}
          />
        </ToolbarButton>
        {busy && <Loader2 className="text-muted-foreground ml-1 size-4 animate-spin" />}
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto">
        <div className="border-b px-6 py-4">
          <h1 className="text-lg font-semibold">
            {message.subject || "(无主题)"}
          </h1>
          <div className="text-muted-foreground mt-2 space-y-1 text-sm">
            <div className="flex flex-wrap items-center gap-2">
              <span className="text-foreground font-medium">
                {senderLabel(message.from_name, message.from_addr)}
              </span>
              <span>&lt;{message.from_addr}&gt;</span>
              <span className="ml-auto text-xs">
                {formatFullDate(message.date)}
              </span>
            </div>
            <div className="text-xs">收件人：{message.to_addrs || "—"}</div>
            {message.cc_addrs && (
              <div className="text-xs">抄送：{message.cc_addrs}</div>
            )}
            <div className="text-xs">
              账户：{message.account_email} · 文件夹：{message.folder_name}
            </div>
          </div>

          {message.attachments.length > 0 && (
            <div className="mt-3 flex flex-wrap gap-2">
              {message.attachments.map((attachment) => (
                <div
                  key={attachment.id}
                  className="bg-secondary flex items-center gap-2 rounded-md px-2 py-1 text-xs"
                >
                  <Paperclip className="size-3.5" />
                  <button
                    className="max-w-52 truncate hover:underline"
                    onClick={() => void openAttachment(attachment)}
                    title={attachment.filename}
                  >
                    {attachment.filename}
                  </button>
                  <span className="text-muted-foreground">
                    {formatBytes(attachment.size)}
                  </span>
                  <button
                    title="保存附件"
                    onClick={() => void downloadAttachment(attachment)}
                    className="hover:bg-background rounded p-0.5"
                  >
                    <Download className="size-3.5" />
                  </button>
                </div>
              ))}
            </div>
          )}
        </div>

        {html ? (
          <div
            className="mail-html px-6 py-5 text-sm leading-relaxed"
            onClick={handleContentClick}
            dangerouslySetInnerHTML={{ __html: html }}
          />
        ) : (
          <pre className="px-6 py-5 font-sans text-sm leading-relaxed whitespace-pre-wrap">
            {message.text_body ?? "(此邮件没有正文)"}
          </pre>
        )}
      </div>
    </section>
  );
}

function ToolbarButton({
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
      className="hover:bg-accent inline-flex size-8 items-center justify-center rounded-md"
    >
      {children}
    </button>
  );
}

function sanitizeHtml(html: string, trustRemoteImages: boolean): string {
  const clean = DOMPurify.sanitize(html, {
    USE_PROFILES: { html: true },
    ADD_ATTR: ["target", "rel"],
    FORBID_TAGS: ["script", "iframe", "object", "embed", "form"],
  });
  if (trustRemoteImages) return clean;
  const doc = new DOMParser().parseFromString(clean, "text/html");
  doc.querySelectorAll("img").forEach((img) => {
    const src = img.getAttribute("src") ?? "";
    if (/^https?:/i.test(src)) {
      img.removeAttribute("src");
      img.setAttribute("data-blocked", "1");
      img.setAttribute("alt", "远程图片已拦截");
      img.setAttribute(
        "style",
        "border:1px dashed currentColor;padding:8px;font-size:12px;opacity:.5",
      );
    }
    img.setAttribute("loading", "lazy");
  });
  doc.querySelectorAll("a").forEach((a) => {
    a.setAttribute("rel", "noopener noreferrer");
  });
  return doc.body.innerHTML;
}
