import { invoke } from '@tauri-apps/api/core';
import { useCallback, useEffect } from 'react';
import type { GeminiModelCatalog } from '../types';
import { useAppStore } from '../stores/appStore';

function invokeErrorMessage(err: unknown): string {
  if (typeof err === 'string') return err;
  if (err && typeof err === 'object' && 'message' in err && typeof (err as { message: unknown }).message === 'string') {
    return (err as { message: string }).message;
  }
  try {
    return JSON.stringify(err);
  } catch {
    return 'モデル一覧を取得できませんでした';
  }
}

export async function refreshGeminiCatalog(opts?: {
  apiKey?: string | null;
  force?: boolean;
}): Promise<GeminiModelCatalog | null> {
  const { setGeminiCatalog, hasApiKey } = useAppStore.getState();
  const typed = opts?.apiKey?.trim() ?? '';
  const canCall = Boolean(typed && !typed.includes('*')) || hasApiKey;
  if (!canCall) {
    setGeminiCatalog(null, 'idle', null);
    return null;
  }

  setGeminiCatalog(useAppStore.getState().geminiCatalog, 'loading', null);
  try {
    const payload: { apiKey?: string; force: boolean } = {
      force: opts?.force ?? false,
    };
    if (typed && !typed.includes('*')) {
      payload.apiKey = typed;
    }
    const catalog = await invoke<GeminiModelCatalog>('cmd_refresh_gemini_catalog', payload);
    setGeminiCatalog(catalog, 'ready', null);
    return catalog;
  } catch (err: unknown) {
    setGeminiCatalog(null, 'error', invokeErrorMessage(err));
    return null;
  }
}

/** 実際に成功したモデルIDをバックエンドから取り込む */
export async function syncRuntimeModel(): Promise<string | null> {
  try {
    const id = await invoke<string | null>('cmd_active_model');
    useAppStore.getState().setRuntimeModelId(id ?? null);
    return id ?? null;
  } catch {
    return null;
  }
}

/** ストア上のカタログを読むだけ（取得は App の bootstrap が行う） */
export function useGeminiCatalog() {
  const geminiCatalog = useAppStore((s) => s.geminiCatalog);
  const catalogStatus = useAppStore((s) => s.catalogStatus);
  const catalogError = useAppStore((s) => s.catalogError);
  const runtimeModelId = useAppStore((s) => s.runtimeModelId);

  const refresh = useCallback(
    (apiKey?: string, force = true) => refreshGeminiCatalog({ apiKey, force }),
    [],
  );

  // フォールバックで別世代に切り替わった場合は、その実績値を優先表示する
  const activeModelId = runtimeModelId ?? geminiCatalog?.auto_selected_id ?? null;
  const usedFallback =
    !!runtimeModelId &&
    !!geminiCatalog &&
    runtimeModelId !== geminiCatalog.auto_selected_id;

  return {
    geminiCatalog,
    catalogStatus,
    catalogError,
    activeModelId,
    usedFallback,
    refresh,
  };
}

/** 起動時・APIキー有効化時に models.list をバックグラウンド取得する */
export function useGeminiCatalogBootstrap() {
  const hasApiKey = useAppStore((s) => s.hasApiKey);

  useEffect(() => {
    if (!hasApiKey) return;
    void refreshGeminiCatalog({ force: true });
    void syncRuntimeModel();
  }, [hasApiKey]);
}
