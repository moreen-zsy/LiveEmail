import { useMemo, useState } from "react";
import { Loader2, X } from "lucide-react";
import { accountsApi } from "@/lib/api";
import { cn } from "@/lib/utils";
import type { Account, AccountInput, ProviderPreset } from "@/features/accounts/types";

interface Props {
  presets: ProviderPreset[];
  /** 编辑时传入，新增为 null */
  account: Account | null;
  onClose: () => void;
  onSaved: () => void | Promise<void>;
}

const ENCRYPTION_OPTIONS = [
  { value: "ssl", label: "SSL/TLS（通常 993 / 465）" },
  { value: "starttls", label: "STARTTLS（通常 143 / 587）" },
  { value: "none", label: "不加密（仅内网，不推荐）" },
];

interface FormState {
  provider: string;
  email: string;
  display_name: string;
  imap_host: string;
  imap_port: number;
  imap_encryption: string;
  smtp_host: string;
  smtp_port: number;
  smtp_encryption: string;
  auth_type: string;
  secret: string;
  oauth_client_id: string;
  oauth_tenant: string;
  oauth_client_secret: string;
}

function fromAccount(account: Account): FormState {
  return {
    provider: account.provider,
    email: account.email,
    display_name: account.display_name,
    imap_host: account.imap_host,
    imap_port: account.imap_port,
    imap_encryption: account.imap_encryption,
    smtp_host: account.smtp_host,
    smtp_port: account.smtp_port,
    smtp_encryption: account.smtp_encryption,
    auth_type: account.auth_type,
    secret: "",
    oauth_client_id: account.oauth_client_id ?? "",
    oauth_tenant: account.oauth_tenant ?? "",
    oauth_client_secret: account.oauth_client_secret ?? "",
  };
}

function empty(preset?: ProviderPreset): FormState {
  return {
    provider: preset?.id ?? "custom",
    email: "",
    display_name: "",
    imap_host: preset?.imap_host ?? "",
    imap_port: preset?.imap_port ?? 993,
    imap_encryption: preset?.imap_encryption ?? "ssl",
    smtp_host: preset?.smtp_host ?? "",
    smtp_port: preset?.smtp_port ?? 465,
    smtp_encryption: preset?.smtp_encryption ?? "ssl",
    auth_type: preset?.auth_type ?? "password",
    secret: "",
    oauth_client_id: "",
    oauth_tenant: "",
    oauth_client_secret: "",
  };
}

export function AccountForm({ presets, account, onClose, onSaved }: Props) {
  const initialPreset = useMemo(
    () => presets.find((p) => p.id === (account?.provider ?? "custom")),
    [presets, account],
  );
  const [form, setForm] = useState<FormState>(
    account ? fromAccount(account) : empty(initialPreset),
  );
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [testMsg, setTestMsg] = useState<{ ok: boolean; text: string } | null>(null);

  const preset = presets.find((p) => p.id === form.provider);
  const isOauth = form.auth_type === "oauth2";
  const needsSecret = !isOauth;
  const secretHint =
    form.provider === "qq"
      ? "请输入 QQ 邮箱授权码（设置 → 账户 → 开启 IMAP/SMTP → 生成授权码）"
      : form.provider === "163"
        ? "请输入 163 邮箱客户端授权码"
        : form.provider === "custom"
          ? "请输入密码或客户端授权码"
          : "请输入密码 / 授权码";

  const set = <K extends keyof FormState>(key: K, value: FormState[K]) =>
    setForm((f) => ({ ...f, [key]: value }));

  const applyPreset = (id: string) => {
    const p = presets.find((x) => x.id === id);
    if (!p) {
      set("provider", id);
      return;
    }
    setForm((f) => ({
      ...f,
      provider: p.id,
      imap_host: p.imap_host || f.imap_host,
      imap_port: p.imap_port,
      imap_encryption: p.imap_encryption,
      smtp_host: p.smtp_host || f.smtp_host,
      smtp_port: p.smtp_port,
      smtp_encryption: p.smtp_encryption,
      auth_type: p.auth_type,
    }));
  };

  const validate = (): string | null => {
    if (!form.email.trim()) return "请填写邮箱地址";
    if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(form.email.trim()))
      return "邮箱地址格式不正确";
    if (!form.imap_host.trim()) return "请填写 IMAP 服务器地址";
    if (!form.smtp_host.trim()) return "请填写 SMTP 服务器地址";
    if (needsSecret && !account && !form.secret.trim())
      return "请填写密码 / 授权码";
    return null;
  };

  const buildInput = (): AccountInput => ({
    email: form.email.trim(),
    display_name: form.display_name.trim(),
    provider: form.provider,
    imap_host: form.imap_host.trim(),
    imap_port: Number(form.imap_port),
    imap_encryption: form.imap_encryption,
    smtp_host: form.smtp_host.trim(),
    smtp_port: Number(form.smtp_port),
    smtp_encryption: form.smtp_encryption,
    auth_type: form.auth_type,
    secret: form.secret.trim() ? form.secret.trim() : null,
    oauth_client_id: form.oauth_client_id.trim() || null,
    oauth_tenant: form.oauth_tenant.trim() || null,
    oauth_client_secret: form.oauth_client_secret.trim() || null,
  });

  const handleTest = async () => {
    const invalid = validate();
    if (invalid) {
      setError(invalid);
      return;
    }
    if (isOauth && !account) {
      setError("请先保存账户，再在账户列表点击「授权」完成 OAuth2 登录");
      return;
    }
    setError(null);
    setTesting(true);
    setTestMsg(null);
    try {
      const result = await accountsApi.testConnection({
        account_id: account?.id ?? null,
        email: form.email.trim(),
        imap_host: form.imap_host.trim(),
        imap_port: Number(form.imap_port),
        imap_encryption: form.imap_encryption,
        smtp_host: form.smtp_host.trim(),
        smtp_port: Number(form.smtp_port),
        smtp_encryption: form.smtp_encryption,
        auth_type: form.auth_type,
        secret: form.secret.trim(),
      });
      const parts = [
        result.imap_ok
          ? "IMAP ✓"
          : `IMAP ✗ ${result.imap_error ?? "失败"}`,
        result.smtp_ok
          ? "SMTP ✓"
          : `SMTP ✗ ${result.smtp_error ?? "失败"}`,
      ];
      setTestMsg({ ok: result.ok, text: parts.join("；") });
    } catch (e) {
      setTestMsg({ ok: false, text: String(e) });
    } finally {
      setTesting(false);
    }
  };

  const handleSave = async () => {
    const invalid = validate();
    if (invalid) {
      setError(invalid);
      return;
    }
    setError(null);
    setSaving(true);
    try {
      const input = buildInput();
      if (account) {
        await accountsApi.update(account.id, input);
      } else {
        await accountsApi.add(input);
      }
      await onSaved();
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="bg-background/80 fixed inset-0 z-50 flex items-center justify-center p-6 backdrop-blur-sm">
      <div className="bg-card max-h-[85vh] w-full max-w-lg overflow-y-auto rounded-xl border shadow-lg">
        <div className="flex items-center justify-between border-b px-6 py-4">
          <h2 className="font-semibold">{account ? "编辑账户" : "添加账户"}</h2>
          <button
            onClick={onClose}
            className="hover:bg-accent rounded-md p-1"
            aria-label="关闭"
          >
            <X className="size-4" />
          </button>
        </div>

        <div className="flex flex-col gap-4 px-6 py-5">
          <Field label="服务商">
            <select
              value={form.provider}
              onChange={(e) => applyPreset(e.target.value)}
              className="border-input bg-background h-9 w-full rounded-md border px-3 text-sm"
            >
              {presets.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                </option>
              ))}
            </select>
            {preset?.note && (
              <p className="text-muted-foreground mt-1 text-xs">{preset.note}</p>
            )}
          </Field>

          <div className="grid grid-cols-2 gap-3">
            <Field label="邮箱地址">
              <input
                type="email"
                value={form.email}
                onChange={(e) => set("email", e.target.value)}
                placeholder="you@example.com"
                className="border-input bg-background h-9 w-full rounded-md border px-3 text-sm"
              />
            </Field>
            <Field label="显示名称（可选）">
              <input
                value={form.display_name}
                onChange={(e) => set("display_name", e.target.value)}
                placeholder="张三"
                className="border-input bg-background h-9 w-full rounded-md border px-3 text-sm"
              />
            </Field>
          </div>

          {needsSecret && (
            <Field label={form.provider === "custom" ? "密码 / 授权码" : "授权码"}>
              <input
                type="password"
                value={form.secret}
                onChange={(e) => set("secret", e.target.value)}
                placeholder={account ? "留空则保持原凭据不变" : ""}
                autoComplete="off"
                className="border-input bg-background h-9 w-full rounded-md border px-3 text-sm"
              />
              <p className="text-muted-foreground mt-1 text-xs">
                {account ? "留空则保持已保存的凭据不变。" : secretHint}
              </p>
            </Field>
          )}

          <Field label="认证方式">
            <select
              value={form.auth_type}
              onChange={(e) => set("auth_type", e.target.value)}
              className="border-input bg-background h-9 w-full rounded-md border px-3 text-sm"
            >
              <option value="password">密码 / 授权码</option>
              <option value="oauth2">OAuth2（Gmail / Outlook）</option>
            </select>
          </Field>

          {isOauth && (
            <>
              <div className="grid grid-cols-2 gap-3">
                <Field label="OAuth2 Client ID">
                  <input
                    value={form.oauth_client_id}
                    onChange={(e) => set("oauth_client_id", e.target.value)}
                    placeholder="在服务商控制台创建桌面应用获得"
                    className="border-input bg-background h-9 w-full rounded-md border px-3 text-sm"
                  />
                </Field>
                <Field label="Tenant（Outlook 可选）">
                  <input
                    value={form.oauth_tenant}
                    onChange={(e) => set("oauth_tenant", e.target.value)}
                    placeholder="common / 租户 ID"
                    className="border-input bg-background h-9 w-full rounded-md border px-3 text-sm"
                  />
                </Field>
              </div>
              <Field label="Client Secret（Google 桌面客户端必填）">
                <input
                  type="password"
                  value={form.oauth_client_secret}
                  onChange={(e) => set("oauth_client_secret", e.target.value)}
                  autoComplete="off"
                  className="border-input bg-background h-9 w-full rounded-md border px-3 text-sm"
                />
                <p className="text-muted-foreground mt-1 text-xs">
                  桌面客户端为公开客户端，此值不视为用户机密。保存后在账户列表点击「授权」，
                  将打开系统浏览器完成登录，令牌写入 Windows 凭据管理器。
                </p>
              </Field>
            </>
          )}

          <fieldset className="border-border rounded-md border p-3">
            <legend className="px-1 text-xs font-medium">IMAP（接收）</legend>
            <div className="grid grid-cols-[1fr_88px] gap-2">
              <input
                value={form.imap_host}
                onChange={(e) => set("imap_host", e.target.value)}
                placeholder="imap.example.com"
                className="border-input bg-background h-9 rounded-md border px-3 text-sm"
              />
              <input
                type="number"
                value={form.imap_port}
                onChange={(e) => set("imap_port", Number(e.target.value))}
                className="border-input bg-background h-9 rounded-md border px-3 text-sm"
              />
            </div>
            <select
              value={form.imap_encryption}
              onChange={(e) => set("imap_encryption", e.target.value)}
              className="border-input bg-background mt-2 h-9 w-full rounded-md border px-3 text-sm"
            >
              {ENCRYPTION_OPTIONS.map((o) => (
                <option key={o.value} value={o.value}>
                  {o.label}
                </option>
              ))}
            </select>
          </fieldset>

          <fieldset className="border-border rounded-md border p-3">
            <legend className="px-1 text-xs font-medium">SMTP（发送）</legend>
            <div className="grid grid-cols-[1fr_88px] gap-2">
              <input
                value={form.smtp_host}
                onChange={(e) => set("smtp_host", e.target.value)}
                placeholder="smtp.example.com"
                className="border-input bg-background h-9 rounded-md border px-3 text-sm"
              />
              <input
                type="number"
                value={form.smtp_port}
                onChange={(e) => set("smtp_port", Number(e.target.value))}
                className="border-input bg-background h-9 rounded-md border px-3 text-sm"
              />
            </div>
            <select
              value={form.smtp_encryption}
              onChange={(e) => set("smtp_encryption", e.target.value)}
              className="border-input bg-background mt-2 h-9 w-full rounded-md border px-3 text-sm"
            >
              {ENCRYPTION_OPTIONS.map((o) => (
                <option key={o.value} value={o.value}>
                  {o.label}
                </option>
              ))}
            </select>
          </fieldset>

          {error && (
            <p className="text-destructive text-sm">{error}</p>
          )}
          {testMsg && (
            <p
              className={cn(
                "text-sm",
                testMsg.ok ? "text-green-600" : "text-destructive",
              )}
            >
              {testMsg.ok ? "✓ " : "✕ "}
              {testMsg.text}
            </p>
          )}
        </div>

        <div className="flex items-center justify-between gap-2 border-t px-6 py-4">
          <button
            onClick={() => void handleTest()}
            disabled={testing || saving}
            className="border-input bg-background hover:bg-accent inline-flex h-9 items-center gap-2 rounded-md border px-4 text-sm font-medium disabled:opacity-50"
          >
            {testing && <Loader2 className="size-4 animate-spin" />}
            测试连接
          </button>
          <div className="flex items-center gap-2">
            <button
              onClick={onClose}
              disabled={saving}
              className="hover:bg-accent h-9 rounded-md px-4 text-sm"
            >
              取消
            </button>
            <button
              onClick={() => void handleSave()}
              disabled={saving || testing}
              className="bg-primary text-primary-foreground hover:bg-primary/90 inline-flex h-9 items-center gap-2 rounded-md px-4 text-sm font-medium disabled:opacity-50"
            >
              {saving && <Loader2 className="size-4 animate-spin" />}
              保存
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

function Field({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <label className="flex flex-col gap-1.5 text-sm">
      <span className="font-medium">{label}</span>
      {children}
    </label>
  );
}
