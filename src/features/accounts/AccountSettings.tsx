import { useCallback, useEffect, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import {
  Plus,
  Mail,
  Loader2,
  Trash2,
  Pencil,
  Plug,
  RefreshCw,
  Star,
  KeyRound,
} from "lucide-react";
import { accountsApi, mailApi } from "@/lib/api";
import { cn } from "@/lib/utils";
import type { Account, ProviderPreset } from "@/features/accounts/types";
import { AccountForm } from "@/features/accounts/AccountForm";

type DialogMode =
  | { kind: "closed" }
  | { kind: "create" }
  | { kind: "edit"; account: Account };

export function AccountSettings() {
  const queryClient = useQueryClient();
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [presets, setPresets] = useState<ProviderPreset[]>([]);
  const [loading, setLoading] = useState(true);
  const [syncingId, setSyncingId] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [dialog, setDialog] = useState<DialogMode>({ kind: "closed" });
  const [testingId, setTestingId] = useState<number | null>(null);
  const [authorizingId, setAuthorizingId] = useState<number | null>(null);
  const [testResult, setTestResult] = useState<{
    accountId: number;
    ok: boolean;
    message: string;
  } | null>(null);

  const refresh = useCallback(async () => {
    try {
      const [list, presetList] = await Promise.all([
        accountsApi.list(),
        accountsApi.presets(),
      ]);
      setAccounts(list);
      setPresets(presetList);
      setError(null);
      void queryClient.invalidateQueries({ queryKey: ["accounts"] });
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [queryClient]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const handleDelete = async (account: Account) => {
    const ok = window.confirm(
      `确定删除账户 ${account.email} 吗？\n本地邮件缓存与已保存的凭据将一并删除。`,
    );
    if (!ok) return;
    try {
      await accountsApi.remove(account.id);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const handleSetDefault = async (account: Account) => {
    try {
      await accountsApi.setDefault(account.id);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const handleSync = async (account: Account) => {
    setSyncingId(account.id);
    try {
      const result = await mailApi.sync(account.id);
      setTestResult({
        accountId: account.id,
        ok: true,
        message: `同步完成：新增 ${result.new_messages} 封，共 ${result.folders} 个文件夹`,
      });
      await refresh();
      void queryClient.invalidateQueries({ queryKey: ["messages"] });
      void queryClient.invalidateQueries({ queryKey: ["folders"] });
    } catch (e) {
      setTestResult({ accountId: account.id, ok: false, message: String(e) });
      await refresh();
    } finally {
      setSyncingId(null);
    }
  };

  const handleAuthorize = async (account: Account) => {
    setAuthorizingId(account.id);
    setTestResult(null);
    try {
      const result = await accountsApi.oauthAuthorize(account.id);
      setTestResult({ accountId: account.id, ok: true, message: result.message });
      await refresh();
    } catch (e) {
      setTestResult({ accountId: account.id, ok: false, message: String(e) });
    } finally {
      setAuthorizingId(null);
    }
  };

  const handleTest = async (account: Account) => {
    setTestingId(account.id);
    setTestResult(null);
    try {
      const result = await accountsApi.testConnection({
        account_id: account.id,
        email: account.email,
        imap_host: account.imap_host,
        imap_port: account.imap_port,
        imap_encryption: account.imap_encryption,
        smtp_host: account.smtp_host,
        smtp_port: account.smtp_port,
        smtp_encryption: account.smtp_encryption,
        auth_type: account.auth_type,
      });
      setTestResult({
        accountId: account.id,
        ok: result.ok,
        message: summarize(result.imap_ok, result.imap_error, result.smtp_ok, result.smtp_error),
      });
    } catch (e) {
      setTestResult({ accountId: account.id, ok: false, message: String(e) });
    } finally {
      setTestingId(null);
    }
  };

  return (
    <div className="mx-auto flex h-full w-full max-w-3xl flex-col gap-6 px-8 py-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-xl font-semibold">账户</h1>
          <p className="text-muted-foreground text-sm">
            管理你的邮箱账户（IMAP / SMTP）
          </p>
        </div>
        <button
          onClick={() => setDialog({ kind: "create" })}
          className="bg-primary text-primary-foreground hover:bg-primary/90 inline-flex h-9 items-center gap-2 rounded-md px-4 text-sm font-medium"
        >
          <Plus className="size-4" />
          添加账户
        </button>
      </div>

      {error && (
        <div className="text-destructive border-destructive/30 bg-destructive/10 rounded-md border px-4 py-3 text-sm">
          {error}
        </div>
      )}

      {loading ? (
        <div className="text-muted-foreground flex items-center gap-2 py-12 text-sm">
          <Loader2 className="size-4 animate-spin" />
          正在加载账户…
        </div>
      ) : accounts.length === 0 ? (
        <div className="border-border flex flex-col items-center gap-3 rounded-xl border border-dashed px-6 py-16 text-center">
          <Mail className="text-muted-foreground size-8" />
          <p className="font-medium">还没有添加账户</p>
          <p className="text-muted-foreground text-sm">
            支持 Gmail、Outlook、QQ、163 及自定义 IMAP/SMTP 服务
          </p>
          <button
            onClick={() => setDialog({ kind: "create" })}
            className="border-input bg-background hover:bg-accent mt-2 inline-flex h-9 items-center gap-2 rounded-md border px-4 text-sm font-medium"
          >
            <Plus className="size-4" />
            添加第一个账户
          </button>
        </div>
      ) : (
        <ul className="flex flex-col gap-3">
          {accounts.map((account) => (
            <li
              key={account.id}
              className="bg-card rounded-xl border py-4 shadow-sm"
            >
              <div className="flex items-center justify-between gap-4 px-5">
                <div className="min-w-0">
                  <div className="flex items-center gap-2">
                    <span className="truncate font-medium">{account.email}</span>
                    <span className="bg-secondary text-secondary-foreground rounded px-1.5 py-0.5 text-xs">
                      {account.provider}
                    </span>
                    {account.is_default && (
                      <span className="bg-primary text-primary-foreground rounded px-1.5 py-0.5 text-xs">
                        默认
                      </span>
                    )}
                  </div>
                  <p className="text-muted-foreground truncate text-sm">
                    {account.display_name || "—"} · IMAP {account.imap_host}:
                    {account.imap_port}
                  </p>
                  {testResult?.accountId === account.id && (
                    <p
                      className={cn(
                        "mt-1 text-xs",
                        testResult.ok ? "text-green-600" : "text-destructive",
                      )}
                    >
                      {testResult.ok ? "✓ " : "✕ "}
                      {testResult.message}
                    </p>
                  )}
                </div>
                <div className="flex shrink-0 items-center gap-2">
                  {!account.is_default && (
                    <button
                      onClick={() => void handleSetDefault(account)}
                      className="border-input bg-background hover:bg-accent inline-flex size-8 items-center justify-center rounded-md border"
                      aria-label="设为默认账户"
                      title="设为默认账户"
                    >
                      <Star className="size-3.5" />
                    </button>
                  )}
                  <button
                    onClick={() => void handleSync(account)}
                    disabled={syncingId === account.id}
                    className="border-input bg-background hover:bg-accent inline-flex h-8 items-center gap-1.5 rounded-md border px-3 text-sm disabled:opacity-50"
                  >
                    {syncingId === account.id ? (
                      <Loader2 className="size-3.5 animate-spin" />
                    ) : (
                      <RefreshCw className="size-3.5" />
                    )}
                    同步
                  </button>
                  {account.auth_type === "oauth2" && (
                    <button
                      onClick={() => void handleAuthorize(account)}
                      disabled={authorizingId === account.id}
                      className="border-input bg-background hover:bg-accent inline-flex h-8 items-center gap-1.5 rounded-md border px-3 text-sm disabled:opacity-50"
                    >
                      {authorizingId === account.id ? (
                        <Loader2 className="size-3.5 animate-spin" />
                      ) : (
                        <KeyRound className="size-3.5" />
                      )}
                      授权
                    </button>
                  )}
                  <button
                    onClick={() => void handleTest(account)}
                    disabled={testingId === account.id}
                    className="border-input bg-background hover:bg-accent inline-flex h-8 items-center gap-1.5 rounded-md border px-3 text-sm disabled:opacity-50"
                  >
                    {testingId === account.id ? (
                      <Loader2 className="size-3.5 animate-spin" />
                    ) : (
                      <Plug className="size-3.5" />
                    )}
                    测试
                  </button>
                  <button
                    onClick={() => setDialog({ kind: "edit", account })}
                    className="border-input bg-background hover:bg-accent inline-flex size-8 items-center justify-center rounded-md border"
                    aria-label="编辑"
                  >
                    <Pencil className="size-3.5" />
                  </button>
                  <button
                    onClick={() => void handleDelete(account)}
                    className="text-destructive hover:bg-destructive/10 inline-flex size-8 items-center justify-center rounded-md"
                    aria-label="删除"
                  >
                    <Trash2 className="size-3.5" />
                  </button>
                </div>
              </div>
            </li>
          ))}
        </ul>
      )}

      {dialog.kind !== "closed" && (
        <AccountForm
          presets={presets}
          account={dialog.kind === "edit" ? dialog.account : null}
          onClose={() => setDialog({ kind: "closed" })}
          onSaved={async () => {
            setDialog({ kind: "closed" });
            await refresh();
          }}
        />
      )}
    </div>
  );
}

function summarize(
  imapOk: boolean,
  imapError: string | null,
  smtpOk: boolean,
  smtpError: string | null,
): string {
  const parts: string[] = [];
  parts.push(imapOk ? "IMAP ✓" : `IMAP ✗ ${imapError ?? "失败"}`);
  parts.push(smtpOk ? "SMTP ✓" : `SMTP ✗ ${smtpError ?? "失败"}`);
  return parts.join("；");
}
