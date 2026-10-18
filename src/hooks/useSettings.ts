import { invoke } from '@tauri-apps/api/core';
import { useCallback, useEffect } from 'react';
import type { AppSettings } from '../types';
import { useAppStore } from '../stores/appStore';
import { refreshGeminiCatalog } from './useGeminiCatalog';

export function useSettings() {
  const { settings, setSettings, hasApiKey, setHasApiKey, settingsOpen, setSettingsOpen } =
    useAppStore();

  // Load on mount
  useEffect(() => {
    (async () => {
      try {
        const [loaded, keyExists] = await Promise.all([
          invoke<AppSettings>('cmd_load_settings'),
          invoke<boolean>('cmd_has_api_key'),
        ]);
        setSettings(loaded);
        setHasApiKey(keyExists);
      } catch {
        // First launch: no settings yet
      }
    })();
  }, [setSettings, setHasApiKey]);

  const saveSettings = useCallback(
    async (newSettings: AppSettings) => {
      await invoke('cmd_save_settings', { settings: newSettings });
      setSettings(newSettings);
      const keyExists = await invoke<boolean>('cmd_has_api_key');
      setHasApiKey(keyExists);
      if (keyExists) {
        const typed = newSettings.gemini_api_key;
        await refreshGeminiCatalog({
          apiKey: typed.includes('*') ? null : typed,
          force: true,
        });
      }
      setSettingsOpen(false);
    },
    [setSettings, setHasApiKey, setSettingsOpen]
  );

  return {
    settings,
    hasApiKey,
    settingsOpen,
    setSettingsOpen,
    saveSettings,
  };
}
