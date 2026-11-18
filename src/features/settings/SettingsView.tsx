import { useEffect, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { getVersion } from "@tauri-apps/api/app";
import { Download, Loader2, RefreshCw, Save, Trash2 } from "lucide-react";
import { composeApi, settingsApi, updaterApi } from "@/lib/api";
import type { AppSettings } from "@/lib/types";
import { formatBytes, formatFullDate, isTauri } from "@/lib/utils";
import { toast } from "@/store/app";
import { cn } from "@/lib/utils";

const DEFAULT_SETTINGS: AppSettings = {
  theme: "system",
  font_size: "md",
  sync_interval_secs: 300,
  signature: "",
  signature_per_account: false,
  close_to_tray: true,
  notifications_enabled: true,
  remote_images_trusted: false,
};

const SHORTCUTS: Array<[string, string]> = [
  ["C", "写新邮件"],
  ["R", "回复"],
  ["F", "转发"],
  ["/", "搜索"],
  ["E", "归档"],
  ["#", "删除"],
  ["U", "标为未读"],
  ["S", "切换星标"],
];

export function SettingsView() {
  const queryClient = useQueryClient();
  const [form, setForm] = useState<AppSettings>(DEFAULT_SETTINGS);
  const [saving, setSaving] = useState(false);
  const [version, setVersion] = useState("0.1.0");
  const [checkingUpdate, setCheckingUpdate] = useState(false);

  useEffect(() => {
    if (!isTauri()) return;
    void getVersion()
      .then(setVersion)
      .catch(() => {});
  }, []);

  const { data: settings, isLoading } = useQuery({
    queryKey: ["settings"],
    queryFn: settingsApi.get,
  });
  const { data: stats, refetch: refetchStats } = useQuery({
    queryKey: ["storage-stats"],
    queryFn: settingsApi.stats,
  });
  const { data: outbox } = useQuery({
    queryKey: ["outbox"],
    queryFn: composeApi.outbox,
  });

  useEffect(() => {
    if (settings) setForm(settings);
  }, [settings]);

  const set = <K extends keyof AppSettings>(key: K, value: AppSettings[K]) =>
    setForm((f) => ({ ...f, [key]: value }));

  const save = async () => {
    setSaving(true);
    try {
      await settingsApi.update(form);
      await queryClient.invalidateQueries({ queryKey: ["settings"] });
      toast.success("设置已保存");
    } catch (e) {
      toast.error(String(e));
    } finally {
      setSaving(false);
    }
  };

  const clearCache = async (scope: "attachments" | "messages") => {
    const message =
      scope === "attachments"
        ? "确定清理全部本地附件缓存吗？邮件正文会保留，可重新同步附件。"
        : "确定清理全部本地邮件缓存吗？本地邮件与附件将被删除，需要重新同步。";
    if (!window.confirm(message)) return;
    try {
      await settingsApi.clearCache(scope);
      await refetchStats();
      await queryClient.invalidateQueries({ queryKey: ["messages"] });
      await queryClient.invalidateQueries({ queryKey: ["folders"] });
      toast.success("缓存已清理");
    } catch (e) {
      toast.error(String(e));
    }
  };

  const retryOutbox = async (id: number) => {
    try {
      const result = await composeApi.retry(id);
      if (result.ok) toast.success(result.message);
      else toast.error(result.message);
      void queryClient.invalidateQueries({ queryKey: ["outbox"] });
    } catch (e) {
      toast.error(String(e));
    }
  };

  const checkUpdate = async () => {
    setCheckingUpdate(true);
    try {
      const update = await updaterApi.check();
      if (!update) {
        toast.info("当前已是最新版本");
        return;
      }
      const ok = window.confirm(
        `发现新版本 ${update.version}${update.body ? `：\n${update.body}` : ""}\n\n现在下载并安装吗？安装完成后需要重启应用。`,
      );
      if (!ok) return;
      await update.downloadAndInstall();
      toast.success("更新已下载，请重启 LiveEmail 完成安装");
    } catch (e) {
      toast.error(`检查更新失败：${String(e)}`);
    } finally {
      setCheckingUpdate(false);
    }
  };

  const removeOutbox = async (id: number) => {
    if (!window.confirm("确定从发送队列移除这封邮件吗？")) return;
    try {
      await composeApi.deleteOutbox(id);
      void queryClient.invalidateQueries({ queryKey: ["outbox"] });
    } catch (e) {
      toast.error(String(e));
    }
  };

  if (isLoading) {
    return (
      <div className="text-muted-foreground flex h-full items-center justify-center gap-2 text-sm">
        <Loader2 className="size-4 animate-spin" />
        正在加载设置…
      </div>
    );
  }

  return (
    <div className="mx-auto flex h-full w-full max-w-3xl flex-col gap-6 overflow-y-auto px-8 py-6">
      <div>
        <h1 className="text-xl font-semibold">设置</h1>
        <p className="text-muted-foreground text-sm">
          外观、通知、存储与发送队列
        </p>
      </div>

      <Section title="通用">
        <Row label="主题">
          <select
            value={form.theme}
            onChange={(e) => set("theme", e.target.value)}
            className="border-input bg-background h-8 w-44 rounded-md border px-2 text-sm"
          >
            <option value="system">跟随系统</option>
            <option value="light">浅色</option>
            <option value="dark">深色</option>
          </select>
        </Row>
        <Row label="字体大小">
          <select
            value={form.font_size}
            onChange={(e) => set("font_size", e.target.value)}
            className="border-input bg-background h-8 w-44 rounded-md border px-2 text-sm"
          >
            <option value="sm">小</option>
            <option value="md">标准</option>
            <option value="lg">大</option>
          </select>
        </Row>
        <Row label="同步间隔">
          <select
            value={form.sync_interval_secs}
            onChange={(e) => set("sync_interval_secs", Number(e.target.value))}
            className="border-input bg-background h-8 w-44 rounded-md border px-2 text-sm"
          >
            <option value={60}>1 分钟</option>
            <option value={300}>5 分钟</option>
            <option value={900}>15 分钟</option>
            <option value={1800}>30 分钟</option>
          </select>
        </Row>
        <Row label="默认签名">
          <textarea
            value={form.signature}
            onChange={(e) => set("signature", e.target.value)}
            rows={3}
            placeholder="写新邮件时自动插入"
            className="border-input bg-background w-full max-w-md rounded-md border px-2 py-1.5 text-sm"
          />
        </Row>
      </Section>

      <Section title="通知与窗口">
        <Toggle
          label="新邮件系统通知"
          checked={form.notifications_enabled}
          onChange={(v) => set("notifications_enabled", v)}
        />
        <Toggle
          label="关闭窗口时最小化到托盘"
          checked={form.close_to_tray}
          onChange={(v) => set("close_to_tray", v)}
        />
        <Toggle
          label="始终加载远程图片（会泄露阅读状态）"
          checked={form.remote_images_trusted}
          onChange={(v) => set("remote_images_trusted", v)}
        />
      </Section>

      <div className="flex justify-end">
        <button
          onClick={() => void save()}
          disabled={saving}
          className="bg-primary text-primary-foreground hover:bg-primary/90 inline-flex h-9 items-center gap-2 rounded-md px-4 text-sm font-medium disabled:opacity-50"
        >
          {saving ? <Loader2 className="size-4 animate-spin" /> : <Save className="size-4" />}
          保存设置
        </button>
      </div>

      <Section title="存储管理">
        <div className="text-muted-foreground grid grid-cols-2 gap-2 text-sm sm:grid-cols-4">
          <Stat label="数据库" value={formatBytes(stats?.db_bytes ?? 0)} />
          <Stat label="附件缓存" value={formatBytes(stats?.attachments_bytes ?? 0)} />
          <Stat label="缓存邮件" value={`${stats?.message_count ?? 0} 封`} />
          <Stat label="附件数量" value={`${stats?.attachment_count ?? 0} 个`} />
        </div>
        <div className="mt-3 flex flex-wrap gap-2">
          <button
            onClick={() => void refetchStats()}
            className="border-input bg-background hover:bg-accent inline-flex h-8 items-center gap-1.5 rounded-md border px-3 text-sm"
          >
            <RefreshCw className="size-3.5" />
            刷新统计
          </button>
          <button
            onClick={() => void clearCache("attachments")}
            className="border-input bg-background hover:bg-accent inline-flex h-8 items-center gap-1.5 rounded-md border px-3 text-sm"
          >
            <Trash2 className="size-3.5" />
            清理附件缓存
          </button>
          <button
            onClick={() => void clearCache("messages")}
            className="border-destructive/40 text-destructive hover:bg-destructive/10 inline-flex h-8 items-center gap-1.5 rounded-md border px-3 text-sm"
          >
            <Trash2 className="size-3.5" />
            清理全部邮件缓存
          </button>
        </div>
      </Section>

      <Section title="发送队列">
        {!outbox || outbox.length === 0 ? (
          <p className="text-muted-foreground text-sm">没有待重试的邮件</p>
        ) : (
          <ul className="divide-y">
            {outbox.map((item) => (
              <li key={item.id} className="flex items-center gap-3 py-2">
                <div className="min-w-0 flex-1">
                  <div className="truncate text-sm">
                    {item.subject || "(无主题)"} → {item.to_addrs}
                  </div>
                  <div
                    className={cn(
                      "truncate text-xs",
                      item.status === "failed" ? "text-destructive" : "text-muted-foreground",
                    )}
                  >
                    {item.status === "failed" ? "发送失败" : item.status} ·{" "}
                    {item.error ?? "—"} · {formatFullDate(item.updated_at)}
                  </div>
                </div>
                <button
                  onClick={() => void retryOutbox(item.id)}
                  className="border-input bg-background hover:bg-accent inline-flex h-8 items-center rounded-md border px-3 text-xs"
                >
                  重试
                </button>
                <button
                  onClick={() => void removeOutbox(item.id)}
                  className="text-destructive hover:bg-destructive/10 inline-flex size-8 items-center justify-center rounded"
                  aria-label="移除"
                >
                  <Trash2 className="size-3.5" />
                </button>
              </li>
            ))}
          </ul>
        )}
      </Section>

      <Section title="快捷键">
        <div className="grid grid-cols-2 gap-2 text-sm sm:grid-cols-4">
          {SHORTCUTS.map(([key, label]) => (
            <div key={key} className="flex items-center gap-2">
              <kbd className="bg-secondary rounded px-1.5 py-0.5 font-mono text-xs">{key}</kbd>
              <span className="text-muted-foreground text-xs">{label}</span>
            </div>
          ))}
        </div>
      </Section>

      <Section title="安全">
        <p className="text-muted-foreground text-sm">
          密码 / 授权码 / OAuth2 令牌仅保存在 Windows 凭据管理器，不写入数据库与日志。
          邮件正文渲染前经 DOMPurify 净化；默认拦截远程图片，点击邮件中的链接会使用系统浏览器打开。
        </p>
      </Section>

      <Section title="关于与更新">
        <Row label="当前版本">
          <span className="text-muted-foreground text-sm">v{version}</span>
        </Row>
        <div>
          <button
            onClick={() => void checkUpdate()}
            disabled={checkingUpdate || !isTauri()}
            className="border-input bg-background hover:bg-accent inline-flex h-8 items-center gap-1.5 rounded-md border px-3 text-sm disabled:opacity-50"
          >
            {checkingUpdate ? (
              <Loader2 className="size-3.5 animate-spin" />
            ) : (
              <Download className="size-3.5" />
            )}
            检查更新
          </button>
          {!isTauri() && (
            <p className="text-muted-foreground mt-2 text-xs">
              更新检查仅在桌面应用内可用。
            </p>
          )}
        </div>
      </Section>
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="bg-card rounded-xl border px-5 py-4 shadow-sm">
      <h2 className="mb-3 text-sm font-semibold">{title}</h2>
      <div className="flex flex-col gap-3">{children}</div>
    </section>
  );
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-4">
      <span className="text-sm">{label}</span>
      {children}
    </div>
  );
}

function Toggle({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (value: boolean) => void;
}) {
  return (
    <label className="flex cursor-pointer items-center justify-between gap-4 text-sm">
      <span>{label}</span>
      <input
        type="checkbox"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
        className="size-4 accent-current"
      />
    </label>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="bg-secondary/50 rounded-lg px-3 py-2">
      <div className="text-xs">{label}</div>
      <div className="font-medium">{value}</div>
    </div>
  );
}
