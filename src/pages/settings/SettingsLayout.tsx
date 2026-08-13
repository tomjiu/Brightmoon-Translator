import { useState } from 'react';
import Sidebar from '../../components/Sidebar';
import {
  Globe,
  Languages,
  Eye,
  Keyboard,
  Palette,
  Settings as SettingsIcon,
  Sparkles,
  Filter,
  Wand2,
  Gamepad2,
  Bell,
  Cloud,
  BookMarked,
  MousePointer2,
} from 'lucide-react';
import BasicSettings from './BasicSettings';
import EngineSettings from './EngineSettings';
import OcrSettings from './OcrSettings';
import HotkeySettings from './HotkeySettings';
import AppearanceSettings from './AppearanceSettings';
import AdvancedSettings from './AdvancedSettings';
import CollectionSettings from './CollectionSettings';
import SelectionSettings from './SelectionSettings';
import AiSettings from '../../components/AiSettings';
import PreProcessSettings from './PreProcessSettings';
import PostProcessSettings from './PostProcessSettings';
import HookProfileSettings from './HookProfileSettings';
import SyncSettings from './SyncSettings';
import { NotificationManager } from '../../components/vocabulary';
import { useI18n } from '../../i18n';

export default function SettingsLayout() {
  // 默认打开划词翻译，避免用户找不到入口
  const [activeSection, setActiveSection] = useState('selection');
  const { t } = useI18n();

  const groups = [
    { key: 'translate', label: t('settings.nav.groups.translate') },
    { key: 'interact', label: t('settings.nav.groups.interact') },
    { key: 'learn', label: t('settings.nav.groups.learn') },
    { key: 'system', label: t('settings.nav.groups.system') },
  ];

  const sections = [
    { id: 'basic', icon: <Globe size={16} />, label: t('settings.nav.sections.basic'), group: 'translate' },
    { id: 'selection', icon: <MousePointer2 size={16} />, label: t('settings.nav.sections.selection'), group: 'translate' },
    { id: 'engines', icon: <Languages size={16} />, label: t('settings.nav.sections.engines'), group: 'translate' },
    { id: 'ai', icon: <Sparkles size={16} />, label: t('settings.nav.sections.ai'), group: 'translate' },
    { id: 'ocr', icon: <Eye size={16} />, label: t('settings.nav.sections.ocr'), group: 'translate' },
    { id: 'preprocess', icon: <Filter size={16} />, label: t('settings.nav.sections.preprocess'), group: 'translate' },
    { id: 'postprocess', icon: <Wand2 size={16} />, label: t('settings.nav.sections.postprocess'), group: 'translate' },
    { id: 'hotkeys', icon: <Keyboard size={16} />, label: t('settings.nav.sections.hotkeys'), group: 'interact' },
    { id: 'hookprofiles', icon: <Gamepad2 size={16} />, label: t('settings.nav.sections.hookprofiles'), group: 'interact' },
    { id: 'appearance', icon: <Palette size={16} />, label: t('settings.nav.sections.appearance'), group: 'interact' },
    { id: 'notifications', icon: <Bell size={16} />, label: t('settings.nav.sections.notifications'), group: 'learn' },
    { id: 'collection', icon: <BookMarked size={16} />, label: t('settings.nav.sections.collection'), group: 'learn' },
    { id: 'sync', icon: <Cloud size={16} />, label: t('settings.nav.sections.sync'), group: 'system' },
    { id: 'advanced', icon: <SettingsIcon size={16} />, label: t('settings.nav.sections.advanced'), group: 'system' },
  ];

  const go = (id: string) => setActiveSection(id);

  const renderContent = () => {
    switch (activeSection) {
      case 'basic':
        return <BasicSettings />;
      case 'engines':
        return <EngineSettings onNavigate={go} />;
      case 'ai':
        return (
          <div className="space-y-5 animate-fadeIn">
            <div>
              <h1 className="ui-page-title">{t('settings.nav.aiTitle')}</h1>
              <p className="ui-page-desc">{t('settings.nav.aiDesc')}</p>
            </div>
            <AiSettings onNavigate={go} />
          </div>
        );
      case 'ocr':
        return <OcrSettings onNavigate={go} />;
      case 'selection':
        return <SelectionSettings />;
      case 'hotkeys':
        return <HotkeySettings />;
      case 'preprocess':
        return <PreProcessSettings />;
      case 'postprocess':
        return <PostProcessSettings />;
      case 'hookprofiles':
        return <HookProfileSettings />;
      case 'notifications':
        return <NotificationManager />;
      case 'collection':
        return <CollectionSettings />;
      case 'appearance':
        return <AppearanceSettings />;
      case 'sync':
        return <SyncSettings />;
      case 'advanced':
        return <AdvancedSettings />;
      default:
        return <BasicSettings />;
    }
  };

  return (
    <div className="flex h-full">
      <Sidebar
        items={sections}
        activeId={activeSection}
        onChange={setActiveSection}
        groups={groups}
      />
      <div className="flex-1 overflow-y-auto bg-bg-primary">
        <div className="settings-content w-full p-4 md:p-5 lg:p-6 animate-fadeIn">
          {renderContent()}
        </div>
      </div>
    </div>
  );
}
