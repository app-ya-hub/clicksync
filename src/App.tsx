/// App.tsx — YouTube投稿効率化コックピット
/// Landing: SourceInput（音声・字幕・台本） (+ 生成履歴)
/// Results: サムネイル用プロンプト3パターン | SEOメタデータ

import { useAppStore } from './stores/appStore';
import { useGeminiCatalogBootstrap } from './hooks/useGeminiCatalog';

import { Header } from './components/layout/Header';
import { Sidebar } from './components/layout/Sidebar';
import { CockpitWorkspace } from './components/cockpit/CockpitWorkspace';
import { SourceInput } from './components/input/SourceInput';
import { SettingsModal } from './components/settings/SettingsModal';

export default function App() {
  const assets = useAppStore((s) => s.assets);
  useGeminiCatalogBootstrap();

  // 旧形式（画像生成時代・URL入力時代）の結果は表示しない
  const showResults =
    !!assets?.seo && Array.isArray(assets.thumbnail_prompts) && typeof assets.source_label === 'string';

  return (
    <div className="flex flex-col h-screen bg-surface text-white font-sans overflow-hidden select-none">
      <Header />

      <div className="flex flex-1 overflow-hidden min-h-0">
        {!showResults && <Sidebar />}

        <main className="flex flex-1 flex-col overflow-hidden min-h-0">
          {showResults && assets ? <CockpitWorkspace assets={assets} /> : <SourceInput />}
        </main>
      </div>

      <SettingsModal />
    </div>
  );
}
