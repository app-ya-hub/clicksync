import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { AppSettings } from '../../types';
import { useSettings } from '../../hooks/useSettings';
import { AboutSection } from './AboutSection';

export function SettingsModal() {
  const { settings, settingsOpen, setSettingsOpen, saveSettings } = useSettings();

  const [form, setForm] = useState<AppSettings>({ ...settings });
  const [saving, setSaving] = useState(false);
  const [showKey, setShowKey] = useState(false);
  const fileRef = useRef<HTMLInputElement>(null);

  const [previewUrl, setPreviewUrl] = useState<string | null>(null);
  const [pickError, setPickError] = useState<string | null>(null);

  useEffect(() => {
    if (!settingsOpen) return;
    setForm({ ...settings });
  }, [settingsOpen, settings]);

  // 保存済みパス（または選択直後のパス）から、データURIでプレビューを復元する
  const imagePath = form.model_image_path;
  useEffect(() => {
    if (!settingsOpen || !imagePath) return;
    let cancelled = false;
    invoke<string | null>('cmd_read_image_data_url', { path: imagePath })
      .then((url) => {
        if (!cancelled) setPreviewUrl(url ?? null);
      })
      .catch(() => {
        if (!cancelled) setPreviewUrl(null);
      });
    return () => {
      cancelled = true;
    };
  }, [settingsOpen, imagePath]);

  if (!settingsOpen) return null;

  const handlePickReference = async (file: File) => {
    setPickError(null);
    try {
      const dataUrl = await readAsDataUrl(file);
      setPreviewUrl(dataUrl); // 選択直後に即プレビュー
      const path = await invoke<string>('cmd_save_reference_image', {
        filename: file.name,
        dataBase64: dataUrl.split(',')[1] ?? '',
      });
      setForm((f) => ({ ...f, model_image_path: path }));
    } catch (err) {
      setPickError(typeof err === 'string' ? err : '画像を読み込めませんでした');
      setPreviewUrl(null);
    }
  };

  const handleSave = async () => {
    setSaving(true);
    try {
      await saveSettings({
        ...form,
        gemini_model: 'auto',
      });
    } finally {
      setSaving(false);
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/70"
      onClick={(e) => e.target === e.currentTarget && setSettingsOpen(false)}
    >
      <div className="bg-[#1a1a1a] border border-white/10 rounded-2xl w-full max-w-md mx-4 flex flex-col shadow-2xl overflow-hidden max-h-[88vh]">
        <div className="flex items-center justify-between px-5 py-4 border-b border-white/8 shrink-0">
          <span className="font-bold text-[14px]">設定</span>
          <button
            onClick={() => setSettingsOpen(false)}
            className="text-white/30 hover:text-white/70 transition-colors text-lg leading-none"
          >
            ✕
          </button>
        </div>

        <div className="flex flex-col gap-5 p-5 overflow-y-auto">
          <FieldGroup
            label="Gemini API キー"
            hint="※Google AI Studio (aistudio.google.com) で取得したGemini APIキーを入力してください"
          >
            <div className="flex gap-2">
              <div className="relative flex-1">
                <input
                  type={showKey ? 'text' : 'password'}
                  value={form.gemini_api_key}
                  onChange={(e) => setForm((f) => ({ ...f, gemini_api_key: e.target.value }))}
                  placeholder="Gemini API キーを入力（AIza…）"
                  className="w-full bg-surface border border-white/10 rounded-lg px-3 py-2.5 text-[12px] text-white placeholder-white/20 focus:outline-none focus:border-accent/50 pr-10 transition-colors"
                />
                <button
                  type="button"
                  onClick={() => setShowKey((v) => !v)}
                  className="absolute right-2.5 top-1/2 -translate-y-1/2 text-white/30 hover:text-white/60 text-[11px]"
                >
                  {showKey ? '隠す' : '表示'}
                </button>
              </div>
            </div>
            <a
              href="https://aistudio.google.com/app/apikey"
              target="_blank"
              rel="noopener noreferrer"
              className="text-[11px] text-accent/70 hover:text-accent transition-colors"
            >
              → Google AI StudioでAPIキーを取得
            </a>
          </FieldGroup>

          <FieldGroup
            label="固定定型文"
            hint="概要欄の末尾に自動挿入されます（SNSリンク等）"
          >
            <textarea
              value={form.fixed_boilerplate}
              onChange={(e) => setForm((f) => ({ ...f, fixed_boilerplate: e.target.value }))}
              rows={4}
              placeholder={`📺 チャンネル登録はこちら → https://...\n🐦 Twitter: @...`}
              className="w-full bg-surface border border-white/10 rounded-lg px-3 py-2.5 text-[12px] text-white placeholder-white/20 focus:outline-none focus:border-accent/50 resize-none transition-colors"
            />
          </FieldGroup>

          <FieldGroup
            label="見本画像（サムネイルのデザイン見本）"
            hint="構図・配色・文字配置エリアを解析し、サムネイル用プロンプトに反映します。Web版Geminiにはこの画像を添付してください。"
          >
            <div className="flex items-center gap-3">
              <div className="w-40 aspect-video rounded-lg overflow-hidden border border-white/10 shrink-0 bg-white/5 flex items-center justify-center">
                {previewUrl ? (
                  <img
                    src={previewUrl}
                    alt="reference thumbnail"
                    className="w-full h-full object-contain"
                    onError={() => setPreviewUrl(null)}
                  />
                ) : (
                  <span className="text-white/20 text-[10px] text-center px-2">
                    {form.model_image_path ? '読み込み中…' : '16:9'}
                  </span>
                )}
              </div>

              <div className="flex flex-col gap-1.5">
                <button
                  onClick={() => fileRef.current?.click()}
                  className="px-3 py-1.5 bg-white/8 hover:bg-white/12 border border-white/10 rounded-lg text-[11px] text-white/70 transition-colors"
                >
                  画像を選択…
                </button>
                {form.model_image_path && (
                  <button
                    onClick={() => {
                      setForm((f) => ({ ...f, model_image_path: null }));
                      setPreviewUrl(null);
                    }}
                    className="text-[10px] text-white/30 hover:text-red-400 transition-colors text-left"
                  >
                    ✕ 削除
                  </button>
                )}
              </div>
            </div>
            <input
              ref={fileRef}
              type="file"
              accept="image/png,image/jpeg,image/webp"
              className="hidden"
              onChange={(e) => {
                const file = e.target.files?.[0];
                e.target.value = ''; // 同じファイルの再選択でも onChange が走るように
                if (file) void handlePickReference(file);
              }}
            />
            {pickError && <p className="text-[11px] text-red-400">{pickError}</p>}
          </FieldGroup>
        </div>

        <div className="flex items-center gap-3 px-5 py-4 border-t border-white/8 shrink-0">
          <div className="flex-1 min-w-0">
            <AboutSection />
          </div>
          <button
            onClick={() => setSettingsOpen(false)}
            className="px-4 py-2 text-[12px] text-white/40 hover:text-white/70 transition-colors"
          >
            キャンセル
          </button>
          <button
            onClick={handleSave}
            disabled={saving || !form.gemini_api_key}
            className="px-5 py-2 bg-accent hover:bg-accent-hover disabled:opacity-40 rounded-lg text-[12px] font-bold transition-colors"
          >
            {saving ? '保存中…' : '保存'}
          </button>
        </div>
      </div>
    </div>
  );
}

function readAsDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });
}

function FieldGroup({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex flex-col gap-1.5">
      <label className="text-[12px] font-semibold text-white/80">{label}</label>
      {hint && <p className="text-[11px] text-white/35">{hint}</p>}
      {children}
    </div>
  );
}
