import { FormEvent, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  AlertCircle,
  ArrowUpRight,
  CheckCircle2,
  LoaderCircle,
  Monitor,
  Moon,
  Pencil,
  Plus,
  RefreshCw,
  Server,
  Settings2,
  Sun,
  Trash2,
  Wifi,
} from "lucide-react";

type OureaProfile = { id: string; name: string; url: string };
type ThemeMode = "system" | "light" | "dark";
type AppSnapshot = { profiles: OureaProfile[]; activeProfileId: string | null; themeMode: ThemeMode };
type ConnectionCheck = { ok: boolean; message: string; healthUrl: string };
type ProfileForm = { id: string; name: string; url: string };
type View = "settings" | "workbench" | "fallback";
type Status = "idle" | "loading" | "saving" | "testing" | "connecting";

const DEFAULT_URL = "http://127.0.0.1:8008";
const DEFAULT_SNAPSHOT: AppSnapshot = {
  profiles: [{ id: "local", name: "本地 Ourea", url: DEFAULT_URL }],
  activeProfileId: "local",
  themeMode: "system",
};
const emptyForm = (): ProfileForm => ({ id: "", name: "", url: DEFAULT_URL });

function WindowControls({ onNavigate, showSettings, isTauri }: { onNavigate: () => void; showSettings: boolean; isTauri: boolean }) {
  async function action(actionName: "minimize" | "maximize" | "close") {
    if (isTauri) await invoke("window_action", { label: "main", action: actionName });
  }

  return (
    <div className="titlebar-actions">
      {isTauri && (
        <button className="titlebar-text-button" type="button" onClick={onNavigate}>
          {showSettings ? "设置" : "工作台"}
        </button>
      )}
      <button className="window-button" type="button" aria-label="最小化" onClick={() => void action("minimize")}><span className="minimize-icon" /></button>
      <button className="window-button" type="button" aria-label="最大化" onClick={() => void action("maximize")}><span className="maximize-icon" /></button>
      <button className="window-button close" type="button" aria-label="关闭" onClick={() => void action("close")}><span className="close-icon" /></button>
    </div>
  );
}

function TitleBar({ view, isTauri, onNavigate }: { view: View; isTauri: boolean; onNavigate: () => void }) {
  const isWorkbench = view === "workbench" || view === "fallback";

  async function handleMouseDown(event: React.MouseEvent<HTMLDivElement>) {
    if (!isTauri || event.button !== 0) return;
    try {
      if (event.detail === 2) await invoke("window_action", { label: "main", action: "maximize" });
      else await getCurrentWindow().startDragging();
    } catch (error) {
      console.error("Failed to handle titlebar interaction", error);
    }
  }

  return (
    <header className="titlebar">
      <div className="titlebar-drag-region" onMouseDown={(event) => void handleMouseDown(event)} aria-label="拖动窗口">
        <div className="titlebar-brand">
          <img src="/ourea-logo.svg" alt="" />
          <div className="titlebar-copy"><strong>Ourea Desktop</strong><span>{isWorkbench ? "工作台" : "连接设置"}</span></div>
        </div>
      </div>
      <WindowControls onNavigate={onNavigate} showSettings={isWorkbench} isTauri={isTauri} />
    </header>
  );
}

function ThemeOption({ mode, value, icon, label, onChange }: { mode: ThemeMode; value: ThemeMode; icon: React.ReactNode; label: string; onChange: (value: ThemeMode) => void }) {
  return <button type="button" className={`theme-option ${mode === value ? "active" : ""}`} onClick={() => onChange(value)} aria-pressed={mode === value}>{icon}<span>{label}</span></button>;
}

function ConnectionResult({ result }: { result: ConnectionCheck | null }) {
  if (!result) return null;
  return (
    <div className={`connection-result ${result.ok ? "success" : "failure"}`} role="status">
      {result.ok ? <CheckCircle2 size={17} /> : <AlertCircle size={17} />}
      <div><strong>{result.ok ? "连接正常" : "连接失败"}</strong><span>{result.ok ? "Ourea 服务已响应" : result.message}</span><code>{result.healthUrl}</code></div>
    </div>
  );
}

function App() {
  const isTauri = useMemo(() => "__TAURI_INTERNALS__" in window, []);
  const [view, setView] = useState<View>("settings");
  const [snapshot, setSnapshot] = useState<AppSnapshot>(DEFAULT_SNAPSHOT);
  const [themeMode, setThemeMode] = useState<ThemeMode>("system");
  const [form, setForm] = useState<ProfileForm>(emptyForm);
  const [status, setStatus] = useState<Status>("loading");
  const [message, setMessage] = useState("");
  const [connectionResult, setConnectionResult] = useState<ConnectionCheck | null>(null);
  const [fallbackReason, setFallbackReason] = useState("");
  const [busyId, setBusyId] = useState<string | null>(null);
  const [workbenchLoaded, setWorkbenchLoaded] = useState(false);

  const activeProfile = snapshot.profiles.find((profile) => profile.id === snapshot.activeProfileId) ?? null;

  async function refreshState() {
    if (!isTauri) { setStatus("idle"); setSnapshot(DEFAULT_SNAPSHOT); return; }
    const state = await invoke<AppSnapshot>("get_state");
    setSnapshot(state); setThemeMode(state.themeMode ?? "system"); setStatus("idle");
    if (!form.id && state.activeProfileId) {
      const profile = state.profiles.find((item) => item.id === state.activeProfileId);
      if (profile) setForm({ ...profile });
    }
  }

  useEffect(() => { void refreshState().catch((error) => { setStatus("idle"); setMessage(String(error)); }); }, [isTauri]);
  useEffect(() => {
    document.documentElement.dataset.theme = themeMode === "dark" ? "cyber" : themeMode === "light" ? "porcelain" : "system";
    document.documentElement.dataset.themeMode = themeMode;
  }, [themeMode]);

  async function changeTheme(nextTheme: ThemeMode) {
    setThemeMode(nextTheme);
    if (isTauri) try { setSnapshot(await invoke<AppSnapshot>("set_theme", { themeMode: nextTheme })); } catch (error) { setMessage(String(error)); }
  }

  async function handleSave(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); setStatus("saving"); setMessage(""); setConnectionResult(null);
    try {
      if (!isTauri) throw new Error("浏览器预览模式下无法保存桌面配置。");
      const next = await invoke<AppSnapshot>("upsert_profile", { input: { id: form.id || undefined, name: form.name, url: form.url } });
      setSnapshot(next); setForm(form.id ? ({ ...next.profiles.find((item) => item.id === form.id) } as ProfileForm) : emptyForm()); setMessage(form.id ? "配置已更新" : "配置已保存");
    } catch (error) { setMessage(String(error)); } finally { setStatus("idle"); }
  }

  async function openProfile(profileId: string) {
    setBusyId(profileId); setStatus("connecting"); setMessage(""); setConnectionResult(null); setWorkbenchLoaded(false);
    try {
      if (!isTauri) { const profile = snapshot.profiles.find((item) => item.id === profileId); if (profile) window.open(profile.url, "_blank", "noopener,noreferrer"); return; }
      const next = await invoke<AppSnapshot>("activate_profile", { profileId }); setSnapshot(next); setView("workbench");
    } catch (error) { setFallbackReason(String(error)); setView("fallback"); }
    finally { setBusyId(null); setStatus("idle"); }
  }

  async function testProfile(profileId: string) {
    setBusyId(profileId); setStatus("testing"); setMessage("");
    try { if (!isTauri) throw new Error("浏览器预览模式下无法执行桌面健康检查。"); setConnectionResult(await invoke<ConnectionCheck>("test_profile", { profileId })); }
    catch (error) { setMessage(String(error)); } finally { setBusyId(null); setStatus("idle"); }
  }

  async function testDraft() {
    setStatus("testing"); setMessage(""); setConnectionResult(null);
    try { if (!isTauri) throw new Error("浏览器预览模式下无法执行桌面健康检查。"); setConnectionResult(await invoke<ConnectionCheck>("test_ourea_url", { url: form.url })); }
    catch (error) { setMessage(String(error)); } finally { setStatus("idle"); }
  }

  async function deleteProfile(profileId: string) {
    if (!isTauri) { setMessage("浏览器预览模式下无法删除桌面配置。"); return; }
    setBusyId(profileId); setMessage("");
    try { setSnapshot(await invoke<AppSnapshot>("delete_profile", { profileId })); if (form.id === profileId) setForm(emptyForm()); setMessage("配置已删除"); }
    catch (error) { setMessage(String(error)); } finally { setBusyId(null); }
  }

  function editProfile(profile: OureaProfile) { setForm({ ...profile }); setConnectionResult(null); setMessage(`正在编辑：${profile.name}`); setView("settings"); }
  async function openDocs() { const url = "https://github.com/ourea-zone/ourea-desktop"; if (isTauri) await openUrl(url); else window.open(url, "_blank", "noopener,noreferrer"); }
  const navigate = () => setView((current) => current === "settings" ? "workbench" : "settings");
  const titlebar = <TitleBar view={view} isTauri={isTauri} onNavigate={navigate} />;

  if (view === "workbench" && activeProfile) return <main className="app-frame">{titlebar}<section className={`workbench-viewport ${workbenchLoaded ? "loaded" : "loading"}`}>
    <iframe className="workbench-embed" src={activeProfile.url} title="Ourea 工作台" allow="clipboard-read; clipboard-write; fullscreen" onLoad={() => setWorkbenchLoaded(true)} />
    <div className="workbench-loading" aria-live="polite"><div className="loading-mark"><img src="/ourea-logo.svg" alt="" /><LoaderCircle size={18} className="loading-spinner" /></div><strong>正在打开 Ourea 工作台</strong><span>{activeProfile.name} · 正在建立连接</span></div>
  </section></main>;

  if (view === "fallback") return <main className="app-frame">{titlebar}<div className="shell fallback-shell"><section className="fallback-panel"><div className="fallback-icon"><Wifi size={22} /></div><p className="eyebrow">WORKBENCH UNAVAILABLE</p><h1>暂时无法打开工作台</h1><p className="description">{activeProfile ? `${activeProfile.name} · ${activeProfile.url}` : "没有可用的 Ourea 配置。"}</p><div className="message error">{fallbackReason}</div><div className="action-row"><button type="button" onClick={() => activeProfile && void openProfile(activeProfile.id)}><RefreshCw size={15} />重试连接</button><button type="button" className="secondary" onClick={() => setView("settings")}><Settings2 size={15} />打开设置</button></div></section></div></main>;

  return <main className="app-frame">{titlebar}<div className="shell settings-shell">
    <header className="settings-intro"><div><p className="eyebrow">DESKTOP CONNECTION</p><h1>连接你的 Ourea 工作台</h1><p className="description">管理工作台地址与桌面外观。Ourea 页面本身的布局、主题和数据不会被 Desktop 改写。</p></div><div className="intro-mark"><img src="/ourea-logo.svg" alt="Ourea" /></div></header>
    <div className="overview-strip"><div><Server size={16} /><span><b>{snapshot.profiles.length}</b> 个连接配置</span></div><div><Wifi size={16} /><span>当前连接 <b>{activeProfile?.name ?? "未选择"}</b></span></div><button type="button" className="strip-action" onClick={() => void openDocs()}><ArrowUpRight size={14} />仓库</button></div>
    <div className="settings-grid">
      <section className="panel connection-panel"><div className="panel-heading"><div><p className="eyebrow">连接工作台</p><h2>{form.id ? "编辑连接" : "新增连接"}</h2></div><div className="panel-index">01</div></div><form className="profile-form" onSubmit={handleSave}><label>配置名称<input type="text" value={form.name} onChange={(event) => setForm((current) => ({ ...current, name: event.target.value }))} placeholder="本地 / 公司 / 测试" autoComplete="off" /></label><label>Ourea 地址<input type="url" value={form.url} onChange={(event) => setForm((current) => ({ ...current, url: event.target.value }))} placeholder={DEFAULT_URL} autoComplete="url" required /></label><div className="form-actions"><button type="submit" disabled={status === "saving" || status === "testing"}>{status === "saving" ? <><LoaderCircle size={15} className="spin" />保存中</> : <><Plus size={15} />{form.id ? "更新连接" : "保存连接"}</>}</button><button type="button" className="secondary" onClick={() => void testDraft()} disabled={status === "testing"}>{status === "testing" ? <><LoaderCircle size={15} className="spin" />检测中</> : <><Wifi size={15} />检测地址</>}</button>{form.id && <button type="button" className="ghost" onClick={() => { setForm(emptyForm()); setMessage(""); setConnectionResult(null); }}>取消编辑</button>}</div></form><ConnectionResult result={connectionResult} />{message && <p className={`message ${message.includes("失败") || message.includes("错误") ? "error" : ""}`}>{message}</p>}<div className="health-note"><span className="health-dot" />健康检查 endpoint <code>/api/health</code></div></section>
      <section className="panel profiles-panel"><div className="panel-heading"><div><p className="eyebrow">已保存的地址</p><h2>选择工作台</h2></div><button type="button" className="icon-button" onClick={() => void refreshState()} aria-label="刷新配置"><RefreshCw size={15} /></button></div><div className="profile-list">{snapshot.profiles.map((profile) => { const active = profile.id === snapshot.activeProfileId; const busy = busyId === profile.id; return <article key={profile.id} className={`profile-item ${active ? "active" : ""}`}><div className="profile-item-head"><div className={`profile-status ${active ? "online" : ""}`}><span />{active ? "当前连接" : "已保存"}</div>{active && <span className="active-mark">ACTIVE</span>}</div><h3>{profile.name}</h3><p>{profile.url}</p><div className="profile-actions"><button type="button" onClick={() => void openProfile(profile.id)} disabled={busy}>{busy && status === "connecting" ? <><LoaderCircle size={14} className="spin" />连接中</> : <>打开工作台<ArrowUpRight size={14} /></>}</button><button type="button" className="icon-button" onClick={() => void testProfile(profile.id)} disabled={busy} aria-label="检测连接"><Wifi size={14} /></button><button type="button" className="icon-button" onClick={() => editProfile(profile)} aria-label="编辑连接"><Pencil size={14} /></button><button type="button" className="icon-button danger-icon" onClick={() => void deleteProfile(profile.id)} disabled={busy} aria-label="删除连接"><Trash2 size={14} /></button></div></article>; })}</div></section>
      <section className="panel appearance-panel"><div className="panel-heading"><div><p className="eyebrow">外观</p><h2>Desktop 主题</h2></div><div className="panel-index">02</div></div><p className="panel-copy">设置页跟随这里的选择。工作台打开后，页面主题继续由 Ourea 自己控制。</p><div className="theme-options"><ThemeOption mode={themeMode} value="system" icon={<Monitor size={15} />} label="跟随系统" onChange={(value) => void changeTheme(value)} /><ThemeOption mode={themeMode} value="light" icon={<Sun size={15} />} label="浅色" onChange={(value) => void changeTheme(value)} /><ThemeOption mode={themeMode} value="dark" icon={<Moon size={15} />} label="深色" onChange={(value) => void changeTheme(value)} /></div></section>
    </div>
  </div></main>;
}

export default App;
