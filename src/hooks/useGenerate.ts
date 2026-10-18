/// useGenerate.ts
///
/// Orchestrates the full generation pipeline via Tauri IPC commands.
/// Sequence: idle → fetching (素材の読み込み・文字起こし) → generating (Gemini) → done | error

import { invoke } from '@tauri-apps/api/core';
import { nanoid } from 'nanoid';
import { useCallback } from 'react';
import type { GeneratedAssets, SourceInput, VideoData } from '../types';
import { useAppStore } from '../stores/appStore';
import { syncRuntimeModel } from './useGeminiCatalog';

function errorMessage(err: unknown): string {
  if (typeof err === 'string') return err;
  if (err instanceof Error) return err.message;
  return '予期しないエラーが発生しました';
}

export function useGenerate() {
  const { setStatus, setError, setVideoData, setAssets, addToHistory, status } = useAppStore();
  const isRunning = status === 'fetching' || status === 'generating';

  const runGeneration = useCallback(
    async (videoData: VideoData) => {
      setStatus('generating');
      const assets = await invoke<GeneratedAssets>('cmd_generate', { videoData });
      setAssets(assets);
      setStatus('done');
      void syncRuntimeModel();
      addToHistory({
        id: nanoid(),
        title: assets.title,
        generated_at: assets.generated_at,
        assets,
        video_data: videoData,
      });
    },
    [setStatus, setAssets, addToHistory],
  );

  const fail = useCallback(
    (err: unknown) => {
      setError({ message: errorMessage(err) });
      setStatus('error');
      void syncRuntimeModel();
    },
    [setError, setStatus],
  );

  /** 素材を読み込み（音声なら文字起こしまで）、その内容で生成する */
  const generate = useCallback(
    async (source: SourceInput, topic: string) => {
      if (isRunning) return;
      setError(null);
      setStatus('fetching');
      try {
        const videoData = await invoke<VideoData>('cmd_load_source', { source, topic });
        setVideoData(videoData);
        await runGeneration(videoData);
      } catch (err: unknown) {
        fail(err);
      }
    },
    [isRunning, setError, setStatus, setVideoData, runGeneration, fail],
  );

  /** 読み込み済みの素材のまま作り直す（音声の文字起こしはやり直さない） */
  const regenerate = useCallback(async () => {
    const videoData = useAppStore.getState().videoData;
    if (isRunning || !videoData) return;
    setError(null);
    try {
      await runGeneration(videoData);
    } catch (err: unknown) {
      fail(err);
    }
  }, [isRunning, setError, runGeneration, fail]);

  return { generate, regenerate, status };
}
