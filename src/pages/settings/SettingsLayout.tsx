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
} from 'lucide-react';
import BasicSettings from './BasicSettings';
import EngineSettings from './EngineSettings';
import OcrSettings from './OcrSettings';
import HotkeySettings from './HotkeySettings';
import AppearanceSettings from './AppearanceSettings';
import AdvancedSettings from './AdvancedSettings';
import CollectionSettings from './CollectionSettings';
import AiSettings from '../../components/AiSettings';
import PreProcessSettings from './PreProcessSettings';
import PostProcessSettings from './PostProcessSettings';
import HookProfileSettings from './HookProfileSettings';
import SyncSettings from './SyncSettings';
import { NotificationManager } from '../../components/vocabulary';

export default function SettingsLayout() {
  const [activeSection, setActiveSection] = useState('basic');

  const groups = [
    { key: 'translate', label: '翻译' },
    { key: 'interact', label: '交互' },
    { key: 'learn', label: '学习' },
    { key: 'system', label: '系统' },
  ];

  const sections = [
    { id: 'basic', icon: <Globe size={16} />, label: '基础设置', group: 'translate' },
    { id: 'engines', icon: <Languages size={16} />, label: '翻译引擎', group: 'translate' },
    { id: 'ai', icon: <Sparkles size={16} />, label: 'AI 增强', group: 'translate' },
    { id: 'ocr', icon: <Eye size={16} />, label: 'OCR', group: 'translate' },
    { id: 'preprocess', icon: <Filter size={16} />, label: '预处理', group: 'translate' },
    { id: 'postprocess', icon: <Wand2 size={16} />, label: '后处理', group: 'translate' },
    { id: 'hotkeys', icon: <Keyboard size={16} />, label: '快捷键', group: 'interact' },
    { id: 'hookprofiles', icon: <Gamepad2 size={16} />, label: 'Hook', group: 'interact' },
    { id: 'appearance', icon: <Palette size={16} />, label: '外观', group: 'interact' },
    { id: 'notifications', icon: <Bell size={16} />, label: '学习提醒', group: 'learn' },
    { id: 'collection', icon: <BookMarked size={16} />, label: '外部生词本', group: 'learn' },
    { id: 'sync', icon: <Cloud size={16} />, label: '云同步', group: 'system' },
    { id: 'advanced', icon: <SettingsIcon size={16} />, label: '高级', group: 'system' },
  ];

  const renderContent = () => {
    switch (activeSection) {
      case 'basic':
        return <BasicSettings />;
      case 'engines':
        return <EngineSettings />;
      case 'ai':
        return (
          <div className="space-y-5 animate-fadeIn">
            <div>
              <h1 className="ui-page-title">AI 增强</h1>
              <p className="ui-page-desc">使用 AI 提升翻译质量与效率</p>
            </div>
            <AiSettings />
          </div>
        );
      case 'ocr':
        return <OcrSettings />;
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
        <div className="max-w-3xl mx-auto p-6 animate-fadeIn">{renderContent()}</div>
      </div>
    </div>
  );
}
