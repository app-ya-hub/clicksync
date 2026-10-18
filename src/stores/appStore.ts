import { create } from 'zustand';
import { devtools } from 'zustand/middleware';
import type {
  AppError,
  AppSettings,
  GeminiModelCatalog,
  GeneratedAssets,
  GenerationStatus,
  HistoryEntry,
  VideoData,
} from '../types';

interface AppState {
  // ── Pipeline status ────────────────────────────────────────────────────
  status: GenerationStatus;
  setStatus: (s: GenerationStatus) => void;
  error: AppError | null;
  setError: (e: AppError | null) => void;

  // ── Data ───────────────────────────────────────────────────────────────
  videoData: VideoData | null;
  setVideoData: (v: VideoData | null) => void;
  assets: GeneratedAssets | null;
  setAssets: (a: GeneratedAssets | null) => void;

  // ── History ────────────────────────────────────────────────────────────
  history: HistoryEntry[];
  addToHistory: (entry: HistoryEntry) => void;
  clearHistory: () => void;

  // ── Settings ───────────────────────────────────────────────────────────
  settings: AppSettings;
  setSettings: (s: AppSettings) => void;
  hasApiKey: boolean;
  setHasApiKey: (v: boolean) => void;
  geminiCatalog: GeminiModelCatalog | null;
  catalogStatus: 'idle' | 'loading' | 'ready' | 'error';
  catalogError: string | null;
  setGeminiCatalog: (
    catalog: GeminiModelCatalog | null,
    status: 'idle' | 'loading' | 'ready' | 'error',
    error?: string | null,
  ) => void;
  /** 直近のリクエストが実際に成功したモデル（フォールバック結果を反映） */
  runtimeModelId: string | null;
  setRuntimeModelId: (id: string | null) => void;

  // ── UI ─────────────────────────────────────────────────────────────────
  settingsOpen: boolean;
  setSettingsOpen: (v: boolean) => void;

  // ── Actions ────────────────────────────────────────────────────────────
  reset: () => void;
}

const DEFAULT_SETTINGS: AppSettings = {
  gemini_api_key: '',
  gemini_model: 'auto',
  fixed_boilerplate: '',
  model_image_path: null,
};

export const useAppStore = create<AppState>()(
  devtools(
    (set) => ({
      // Pipeline
      status: 'idle',
      setStatus: (status) => set({ status }),
      error: null,
      setError: (error) => set({ error }),

      // Data
      videoData: null,
      setVideoData: (videoData) => set({ videoData }),
      assets: null,
      setAssets: (assets) => set({ assets }),

      // History (persisted in memory; Part 4 will add localStorage)
      history: [],
      addToHistory: (entry) =>
        set((state) => ({
          history: [entry, ...state.history].slice(0, 50), // keep last 50
        })),
      clearHistory: () => set({ history: [] }),

      // Settings
      settings: DEFAULT_SETTINGS,
      setSettings: (settings) => set({ settings }),
      hasApiKey: false,
      setHasApiKey: (hasApiKey) => set({ hasApiKey }),
      geminiCatalog: null,
      catalogStatus: 'idle',
      catalogError: null,
      setGeminiCatalog: (geminiCatalog, catalogStatus, catalogError = null) =>
        set({ geminiCatalog, catalogStatus, catalogError }),
      runtimeModelId: null,
      setRuntimeModelId: (runtimeModelId) => set({ runtimeModelId }),

      // UI
      settingsOpen: false,
      setSettingsOpen: (settingsOpen) => set({ settingsOpen }),

      // Reset to idle (keep history + settings)
      reset: () =>
        set({
          status: 'idle',
          error: null,
          videoData: null,
          assets: null,
        }),
    }),
    { name: 'ClickSync' }
  )
);
