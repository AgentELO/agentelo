import { useState, useEffect, useRef } from "react";
import type { UserInfo } from "../lib/types";
import { getApiKeyConfig, saveApiKeyConfig, clearApiKeyConfig } from "../lib/api";

interface SettingsProps {
  user: UserInfo | null;
  onLogin: () => void;
  onLogout: () => void;
}

export function Settings({ user, onLogin, onLogout }: SettingsProps) {
  const isSignedIn = !!user?.email;

  const [maskedKey, setMaskedKey] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const keyRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    getApiKeyConfig().then((config) => {
      setMaskedKey(config?.masked_key ?? null);
    });
  }, []);

  const handleSave = async () => {
    const key = keyRef.current?.value?.trim();
    if (!key) return;
    setSaving(true);
    setError(null);
    try {
      await saveApiKeyConfig(key);
      const config = await getApiKeyConfig();
      setMaskedKey(config?.masked_key ?? null);
      if (keyRef.current) keyRef.current.value = "";
    } catch (e) {
      setError(String(e));
    }
    setSaving(false);
  };

  const handleClear = async () => {
    await clearApiKeyConfig();
    setMaskedKey(null);
    setError(null);
  };

  return (
    <div className="space-y-6">
      {/* Account */}
      <div className="bg-surface-raised rounded-xl border border-border p-6">
        <h3 className="text-sm font-medium text-text-secondary mb-4">Account</h3>

        {isSignedIn ? (
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              {user!.avatar_url ? (
                <img src={user!.avatar_url} className="w-10 h-10 rounded-full" alt="" />
              ) : (
                <div className="w-10 h-10 rounded-full bg-brand/20 flex items-center justify-center text-sm font-bold">
                  {user!.name.charAt(0)}
                </div>
              )}
              <div>
                <div className="text-sm font-medium">{user!.name}</div>
                <div className="text-xs text-text-muted">{user!.email}</div>
              </div>
            </div>
            <button
              onClick={onLogout}
              className="text-xs text-text-muted hover:text-danger transition-colors"
            >
              Sign out
            </button>
          </div>
        ) : (
          <div>
            <p className="text-xs text-text-muted mb-4">
              Link your account to show your name on the leaderboard and sync across devices.
            </p>
            <button
              onClick={onLogin}
              className="flex items-center gap-2 px-4 py-2 rounded-lg bg-white/[0.05] border border-border text-sm font-medium hover:bg-white/[0.08] transition-colors"
            >
              <svg className="w-4 h-4" viewBox="0 0 24 24">
                <path
                  fill="#4285F4"
                  d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92a5.06 5.06 0 0 1-2.2 3.32v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.1z"
                />
                <path
                  fill="#34A853"
                  d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z"
                />
                <path
                  fill="#FBBC05"
                  d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z"
                />
                <path
                  fill="#EA4335"
                  d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z"
                />
              </svg>
              Sign in with Google
            </button>
          </div>
        )}
      </div>

      {/* BYOK — Gemini API Key */}
      <div className="bg-surface-raised rounded-xl border border-border p-6">
        <h3 className="text-sm font-medium text-text-secondary mb-4">AI Scoring</h3>

        {maskedKey ? (
          <div>
            <div className="flex items-center justify-between mb-3">
              <div>
                <div className="text-sm font-medium">Gemini API Key</div>
                <div className="text-xs text-text-muted font-mono mt-1">{maskedKey}</div>
              </div>
              <button
                onClick={handleClear}
                className="text-xs text-text-muted hover:text-danger transition-colors"
              >
                Clear
              </button>
            </div>
            <p className="text-xs text-brand">
              Using your own key — screenshots never leave your device.
            </p>
          </div>
        ) : (
          <div>
            <p className="text-xs text-text-muted mb-4">
              Screenshots are sent to AgentELO cloud for scoring. Add your own Gemini API key to keep screenshots on your device.
            </p>
            <div className="flex gap-2">
              <input
                ref={keyRef}
                type="password"
                placeholder="Gemini API key"
                className="flex-1 px-3 py-2 rounded-lg bg-white/[0.05] border border-border text-sm font-mono placeholder:text-text-muted/50 focus:outline-none focus:border-brand/40"
              />
              <button
                onClick={handleSave}
                disabled={saving}
                className="px-4 py-2 rounded-lg bg-brand text-[#09090b] text-sm font-medium hover:bg-brand-light transition-colors disabled:opacity-50"
              >
                {saving ? "..." : "Save"}
              </button>
            </div>
            {error && (
              <p className="text-xs text-danger mt-2">{error}</p>
            )}
            <p className="text-xs text-text-muted mt-3">
              Get a free key at{" "}
              <span className="text-text-secondary">aistudio.google.com</span>
            </p>
          </div>
        )}
      </div>
    </div>
  );
}
