import { useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { EditorContent, useEditor } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import Link from "@tiptap/extension-link";
import Image from "@tiptap/extension-image";
import Placeholder from "@tiptap/extension-placeholder";
import {
  Bold,
  Code,
  ImagePlus,
  Italic,
  Link2,
  List,
  ListOrdered,
  Loader2,
  Paperclip,
  Quote,
  Redo2,
  Send,
  Strikethrough,
  Undo2,
  X,
} from "lucide-react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { composeApi, mailApi } from "@/lib/api";
import type { Account, Draft, MessageDetail } from "@/lib/types";
import type { ComposeIntent } from "@/store/app";
import { toast } from "@/store/app";
import { cn, escapeHtml, htmlToText, isTauri, quoteHtml, textToHtml } from "@/lib/utils";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

interface Props {
  intent: ComposeIntent;
  accounts: Account[];
  signature: string;
  draft?: Draft | null;
  onClose: () => void;
}

export function ComposeDialog({ intent, accounts, signature, draft, onClose }: Props) {
  const queryClient = useQueryClient();
  const [accountId, setAccountId] = useState<number | null>(
    intent.accountId ?? accounts.find((a) => a.is_default)?.id ?? accounts[0]?.id ?? null,
  );
  const [to, setTo] = useState("");
  const [cc, setCc] = useState("");
  const [bcc, setBcc] = useState("");
  const [showCc, setShowCc] = useState(false);
  const [subject, setSubject] = useState("");
  const [bodyHtml, setBodyHtml] = useState("");
  const [plain, setPlain] = useState(false);
  const [plainText, setPlainText] = useState("");
  const [attachments, setAttachments] = useState<string[]>([]);
  const [inReplyTo, setInReplyTo] = useState<string | null>(null);
  const [references, setReferences] = useState<string | null>(null);
  const [sending, setSending] = useState(false);
  const [draftSaving, setDraftSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const initialized = useRef(false);
  const dirty = useRef(false);
  const draftId = useRef<number | null>(intent.draftId ?? null);
  const saveTimer = useRef<number | undefined>(undefined);

  const needSource =
    intent.mode === "reply" || intent.mode === "replyAll" || intent.mode === "forward";
  const { data: source } = useQuery({
    queryKey: ["compose-source", intent.message?.id],
    queryFn: () => mailApi.get(intent.message!.id),
    enabled: needSource && intent.message != null,
  });
  const { data: drafts } = useQuery({
    queryKey: ["drafts"],
    queryFn: composeApi.drafts,
    enabled: intent.mode === "draft" && !draft,
  });
  const effectiveDraft = draft ?? drafts?.find((d) => d.id === intent.draftId) ?? null;

  const editor = useEditor({
    extensions: [
      StarterKit.configure({ link: false }),
      Link.configure({ openOnClick: false, autolink: true }),
      Image,
      Placeholder.configure({ placeholder: "写点什么…" }),
    ],
    content: "",
    editorProps: {
      attributes: {
        class: "mail-editor prose-editor min-h-[220px] px-4 py-3 text-sm focus:outline-none",
      },
    },
    onUpdate({ editor: instance }) {
      dirty.current = true;
      setBodyHtml(instance.getHTML());
    },
  });

  const signatureHtml = () =>
    signature.trim()
      ? `<p><br></p><p>--<br>${escapeHtml(signature).replace(/\n/g, "<br>")}</p>`
      : "";

  // 初始化：新邮件写入签名
  useEffect(() => {
    if (intent.mode === "new" && !initialized.current) {
      initialized.current = true;
      setBodyHtml(`<p><br></p>${signatureHtml()}`);
    }
  }, [intent.mode, signature]);

  // 初始化：草稿恢复
  useEffect(() => {
    if (intent.mode !== "draft" || !effectiveDraft || initialized.current) return;
    initialized.current = true;
    draftId.current = effectiveDraft.id;
    setAccountId(effectiveDraft.account_id ?? accountId);
    setTo(effectiveDraft.to_addrs);
    setCc(effectiveDraft.cc_addrs);
    setBcc(effectiveDraft.bcc_addrs);
    setShowCc(Boolean(effectiveDraft.cc_addrs || effectiveDraft.bcc_addrs));
    setSubject(effectiveDraft.subject);
    setBodyHtml(effectiveDraft.body_html);
    setPlainText(effectiveDraft.body_text || htmlToText(effectiveDraft.body_html));
    setAttachments(effectiveDraft.attachments);
  }, [intent.mode, effectiveDraft, accountId]);

  // 初始化：回复 / 全部回复 / 转发
  useEffect(() => {
    if (!needSource || !source || initialized.current) return;
    initialized.current = true;
    const own = accounts.find((a) => a.id === accountId)?.email.toLowerCase() ?? "";
    const split = (value: string) =>
      value
        .split(/[,;]/)
        .map((s) => s.trim())
        .filter(Boolean);

    if (intent.mode === "reply") {
      setTo(source.from_addr);
    } else if (intent.mode === "replyAll") {
      const all = [source.from_addr, ...split(source.to_addrs), ...split(source.cc_addrs)];
      setTo(
        [...new Set(all.filter((a) => a && a.toLowerCase() !== own))].join(", "),
      );
      setCc("");
      setShowCc(true);
    }

    const prefix = /^(re|fwd?):/i.test(source.subject)
      ? ""
      : intent.mode === "forward"
        ? "Fwd: "
        : "Re: ";
    setSubject(prefix + (source.subject || ""));

    const meta = `${senderLabelOf(source)} 写道：`;
    setBodyHtml(`<p><br></p>${quoteHtml(source.html_body, source.text_body, meta)}`);

    if (intent.mode === "forward") {
      setAttachments(
        source.attachments
          .map((a) => a.local_path)
          .filter((p): p is string => Boolean(p)),
      );
      setInReplyTo(null);
      setReferences(null);
    } else {
      setInReplyTo(source.message_id);
      setReferences(
        [source.references, source.message_id].filter(Boolean).join(" ").trim() || null,
      );
    }
  }, [needSource, source, initialized, intent.mode, accountId, accounts]);

  // 编辑器内容与状态同步（外部初始化时）
  useEffect(() => {
    if (!editor) return;
    if (bodyHtml !== editor.getHTML()) {
      editor.commands.setContent(bodyHtml, { emitUpdate: false });
    }
  }, [editor, bodyHtml]);

  // 草稿自动保存（防抖 1.2s）
  useEffect(() => {
    if (!dirty.current || accountId == null) return;
    if (!to && !cc && !bcc && !subject && !bodyHtml.trim() && attachments.length === 0) {
      return;
    }
    window.clearTimeout(saveTimer.current);
    saveTimer.current = window.setTimeout(() => {
      void persistDraft(false);
    }, 1200);
    return () => window.clearTimeout(saveTimer.current);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [to, cc, bcc, subject, bodyHtml, attachments, accountId]);

  useEffect(() => {
    return () => window.clearTimeout(saveTimer.current);
  }, []);

  const currentHtml = () =>
    plain ? textToHtml(plainText) : (editor?.getHTML() ?? bodyHtml);
  const currentText = () =>
    plain ? plainText : (editor?.getText() ?? htmlToText(bodyHtml));

  async function persistDraft(showToast: boolean) {
    if (accountId == null) return;
    setDraftSaving(true);
    try {
      const saved = await composeApi.saveDraft({
        id: draftId.current,
        account_id: accountId,
        subject,
        to_addrs: to,
        cc_addrs: cc,
        bcc_addrs: bcc,
        body_html: currentHtml(),
        body_text: currentText(),
        attachments,
      });
      draftId.current = saved.id;
      if (showToast) toast.success("草稿已保存");
      void queryClient.invalidateQueries({ queryKey: ["drafts"] });
    } catch (e) {
      if (showToast) toast.error(String(e));
    } finally {
      setDraftSaving(false);
    }
  }

  const addAttachments = async () => {
    try {
      if (!isTauri()) return;
      const selected = await openDialog({ multiple: true, title: "选择附件" });
      if (!selected) return;
      const paths = Array.isArray(selected) ? selected : [selected];
      setAttachments((current) => [...new Set([...current, ...paths])]);
      dirty.current = true;
    } catch (e) {
      toast.error(String(e));
    }
  };

  const setLink = () => {
    if (!editor) return;
    const previous = editor.getAttributes("link").href as string | undefined;
    const url = window.prompt("链接地址", previous ?? "https://");
    if (url === null) return;
    if (!url.trim()) {
      editor.chain().focus().unsetLink().run();
      return;
    }
    editor.chain().focus().extendMarkRange("link").setLink({ href: url.trim() }).run();
  };

  const insertImage = () => {
    if (!editor) return;
    const url = window.prompt("图片链接（https://）");
    if (!url || !/^https?:\/\//i.test(url.trim())) return;
    editor.chain().focus().setImage({ src: url.trim() }).run();
  };

  const togglePlain = () => {
    if (!plain) {
      setPlainText(editor?.getText() ?? htmlToText(bodyHtml));
      setPlain(true);
    } else {
      setBodyHtml(textToHtml(plainText));
      setPlain(false);
    }
  };

  const handleSend = async () => {
    setError(null);
    if (accountId == null) {
      setError("请选择发件账户");
      return;
    }
    if (!to.trim()) {
      setError("请填写收件人");
      return;
    }
    setSending(true);
    try {
      const html = currentHtml();
      const text = currentText();
      const result = await composeApi.send({
        account_id: accountId,
        to_addrs: to,
        cc_addrs: cc,
        bcc_addrs: bcc,
        subject,
        body_html: html,
        body_text: text,
        attachments,
        in_reply_to: inReplyTo,
        references,
        draft_id: draftId.current,
      });
      if (result.ok) {
        toast.success(result.message);
        void queryClient.invalidateQueries({ queryKey: ["messages"] });
        void queryClient.invalidateQueries({ queryKey: ["folders"] });
        void queryClient.invalidateQueries({ queryKey: ["drafts"] });
        void queryClient.invalidateQueries({ queryKey: ["outbox"] });
        onClose();
      } else {
        toast.error(result.message);
        setError(result.message);
        void queryClient.invalidateQueries({ queryKey: ["outbox"] });
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setSending(false);
    }
  };

  return (
    <Dialog open onOpenChange={(value) => !value && onClose()}>
      <DialogContent className="flex h-[88vh] max-w-3xl flex-col gap-0 p-0">
        <DialogHeader className="border-b px-5 py-3">
          <DialogTitle className="text-base">
            {intent.mode === "reply"
              ? "回复"
              : intent.mode === "replyAll"
                ? "全部回复"
                : intent.mode === "forward"
                  ? "转发"
                  : intent.mode === "draft"
                    ? "编辑草稿"
                    : "写邮件"}
          </DialogTitle>
        </DialogHeader>

        <div className="flex flex-col gap-1.5 border-b px-5 py-3 text-sm">
          <div className="flex items-center gap-2">
            <span className="text-muted-foreground w-12 shrink-0 text-xs">发件人</span>
            <select
              value={accountId ?? ""}
              onChange={(e) => setAccountId(Number(e.target.value))}
              className="border-input bg-background h-8 min-w-0 flex-1 rounded-md border px-2 text-sm"
            >
              {accounts.map((a) => (
                <option key={a.id} value={a.id}>
                  {a.display_name ? `${a.display_name} <${a.email}>` : a.email}
                </option>
              ))}
            </select>
            <button
              className="text-muted-foreground text-xs hover:underline"
              onClick={() => setShowCc((v) => !v)}
            >
              抄送/密送
            </button>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-muted-foreground w-12 shrink-0 text-xs">收件人</span>
            <input
              value={to}
              onChange={(e) => {
                dirty.current = true;
                setTo(e.target.value);
              }}
              placeholder="多个地址用逗号分隔"
              className="border-input bg-background h-8 min-w-0 flex-1 rounded-md border px-2 text-sm"
            />
          </div>
          {showCc && (
            <>
              <div className="flex items-center gap-2">
                <span className="text-muted-foreground w-12 shrink-0 text-xs">抄送</span>
                <input
                  value={cc}
                  onChange={(e) => {
                    dirty.current = true;
                    setCc(e.target.value);
                  }}
                  className="border-input bg-background h-8 min-w-0 flex-1 rounded-md border px-2 text-sm"
                />
              </div>
              <div className="flex items-center gap-2">
                <span className="text-muted-foreground w-12 shrink-0 text-xs">密送</span>
                <input
                  value={bcc}
                  onChange={(e) => {
                    dirty.current = true;
                    setBcc(e.target.value);
                  }}
                  className="border-input bg-background h-8 min-w-0 flex-1 rounded-md border px-2 text-sm"
                />
              </div>
            </>
          )}
          <div className="flex items-center gap-2">
            <span className="text-muted-foreground w-12 shrink-0 text-xs">主题</span>
            <input
              value={subject}
              onChange={(e) => {
                dirty.current = true;
                setSubject(e.target.value);
              }}
              placeholder="邮件主题"
              className="border-input bg-background h-8 min-w-0 flex-1 rounded-md border px-2 text-sm"
            />
          </div>
        </div>

        <div className="flex flex-wrap items-center gap-0.5 border-b px-3 py-1.5">
          <EditorButton
            title="加粗"
            active={!plain && Boolean(editor?.isActive("bold"))}
            disabled={plain}
            onClick={() => editor?.chain().focus().toggleBold().run()}
          >
            <Bold className="size-4" />
          </EditorButton>
          <EditorButton
            title="斜体"
            active={!plain && Boolean(editor?.isActive("italic"))}
            disabled={plain}
            onClick={() => editor?.chain().focus().toggleItalic().run()}
          >
            <Italic className="size-4" />
          </EditorButton>
          <EditorButton
            title="删除线"
            active={!plain && Boolean(editor?.isActive("strike"))}
            disabled={plain}
            onClick={() => editor?.chain().focus().toggleStrike().run()}
          >
            <Strikethrough className="size-4" />
          </EditorButton>
          <EditorButton
            title="无序列表"
            active={!plain && Boolean(editor?.isActive("bulletList"))}
            disabled={plain}
            onClick={() => editor?.chain().focus().toggleBulletList().run()}
          >
            <List className="size-4" />
          </EditorButton>
          <EditorButton
            title="有序列表"
            active={!plain && Boolean(editor?.isActive("orderedList"))}
            disabled={plain}
            onClick={() => editor?.chain().focus().toggleOrderedList().run()}
          >
            <ListOrdered className="size-4" />
          </EditorButton>
          <EditorButton
            title="引用"
            active={!plain && Boolean(editor?.isActive("blockquote"))}
            disabled={plain}
            onClick={() => editor?.chain().focus().toggleBlockquote().run()}
          >
            <Quote className="size-4" />
          </EditorButton>
          <EditorButton
            title="代码块"
            active={!plain && Boolean(editor?.isActive("codeBlock"))}
            disabled={plain}
            onClick={() => editor?.chain().focus().toggleCodeBlock().run()}
          >
            <Code className="size-4" />
          </EditorButton>
          <EditorButton title="链接" disabled={plain} onClick={setLink}>
            <Link2 className="size-4" />
          </EditorButton>
          <EditorButton title="插入图片" disabled={plain} onClick={insertImage}>
            <ImagePlus className="size-4" />
          </EditorButton>
          <div className="bg-border mx-1 h-5 w-px" />
          <EditorButton
            title="撤销"
            disabled={plain}
            onClick={() => editor?.chain().focus().undo().run()}
          >
            <Undo2 className="size-4" />
          </EditorButton>
          <EditorButton
            title="重做"
            disabled={plain}
            onClick={() => editor?.chain().focus().redo().run()}
          >
            <Redo2 className="size-4" />
          </EditorButton>
          <button
            onClick={togglePlain}
            className={cn(
              "ml-auto rounded px-2 py-1 text-xs",
              plain ? "bg-primary text-primary-foreground" : "hover:bg-accent",
            )}
          >
            {plain ? "富文本" : "纯文本"}
          </button>
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto">
          {plain ? (
            <textarea
              value={plainText}
              onChange={(e) => {
                dirty.current = true;
                setPlainText(e.target.value);
              }}
              className="h-full min-h-[240px] w-full resize-none bg-transparent px-4 py-3 font-mono text-sm focus:outline-none"
            />
          ) : (
            <EditorContent editor={editor} />
          )}
        </div>

        {attachments.length > 0 && (
          <div className="flex flex-wrap gap-2 border-t px-5 py-2">
            {attachments.map((path) => (
              <span
                key={path}
                className="bg-secondary flex max-w-64 items-center gap-1.5 rounded px-2 py-1 text-xs"
              >
                <Paperclip className="size-3 shrink-0" />
                <span className="truncate">{path.split(/[\\/]/).pop()}</span>
                <button
                  onClick={() => {
                    dirty.current = true;
                    setAttachments((list) => list.filter((p) => p !== path));
                  }}
                  className="hover:bg-background rounded p-0.5"
                  aria-label="移除附件"
                >
                  <X className="size-3" />
                </button>
              </span>
            ))}
          </div>
        )}

        {error && (
          <div className="text-destructive border-t px-5 py-2 text-xs">{error}</div>
        )}

        <div className="flex items-center gap-2 border-t px-5 py-3">
          <button
            onClick={() => void addAttachments()}
            className="border-input bg-background hover:bg-accent inline-flex h-8 items-center gap-1.5 rounded-md border px-3 text-sm"
          >
            <Paperclip className="size-4" />
            附件
          </button>
          <button
            onClick={() => void persistDraft(true)}
            disabled={draftSaving || accountId == null}
            className="border-input bg-background hover:bg-accent inline-flex h-8 items-center gap-1.5 rounded-md border px-3 text-sm disabled:opacity-50"
          >
            {draftSaving && <Loader2 className="size-3.5 animate-spin" />}
            存草稿
          </button>
          <div className="ml-auto flex items-center gap-2">
            <button
              onClick={onClose}
              className="hover:bg-accent h-8 rounded-md px-3 text-sm"
            >
              取消
            </button>
            <button
              onClick={() => void handleSend()}
              disabled={sending}
              className="bg-primary text-primary-foreground hover:bg-primary/90 inline-flex h-8 items-center gap-1.5 rounded-md px-4 text-sm font-medium disabled:opacity-50"
            >
              {sending ? <Loader2 className="size-4 animate-spin" /> : <Send className="size-4" />}
              发送
            </button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

function EditorButton({
  title,
  active,
  disabled,
  onClick,
  children,
}: {
  title: string;
  active?: boolean;
  disabled?: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      title={title}
      onClick={onClick}
      disabled={disabled}
      className={cn(
        "inline-flex size-7 items-center justify-center rounded",
        active ? "bg-accent" : "hover:bg-accent",
        disabled ? "opacity-40" : "",
      )}
    >
      {children}
    </button>
  );
}

function senderLabelOf(message: MessageDetail): string {
  return message.from_name || message.from_addr || "对方";
}
