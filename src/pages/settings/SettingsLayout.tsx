import { useState } from 'react';
import Sidebar from '../../components/Sidebar';
import {
  Globe,
  Languages,
  Eye,
  Keyboard,
  Palette,
  Settings as SettingsIcon,
  Puzzle,
  Sparkles,
  Filter,
  Gamepad2,
  Bell,
  Cloud,
} from 'lucide-react';
import BasicSettings from './BasicSettings';
import EngineSettings from './EngineSettings';
import OcrSettings from './OcrSettings';
import HotkeySettings from './HotkeySettings';
import AppearanceSettings from './AppearanceSettings';
import AdvancedSettings from './AdvancedSettings';
import PluginSettings from './PluginSettings';
import AiSettings from '../../components/AiSettings';
import PreProcessSettings from './PreProcessSettings';
import HookProfileSettings from './HookProfileSettings';
import SyncSettings from './SyncSettings';
import { NotificationManager } from '../../components/vocabulary';

export default function SettingsLayout() {
  const [activeSection, setActiveSection] = useState('basic');

  const sections = [
    { id: 'basic', icon: <Globe size={18} />, label: '基础设置' },
    { id: 'engines', icon: <Languages size={18} />, label: '翻译引擎' },
    { id: 'ai', icon: <Sparkles size={18} />, label: 'AI 增强' },
    { id: 'ocr', icon: <Eye size={18} />, label: 'OCR设置' },
    { id: 'hotkeys', icon: <Keyboard size={18} />, label: '快捷键' },
    { id: 'preprocess', icon: <Filter size={18} />, label: '预处理规则' },
    { id: 'hookprofiles', icon: <Gamepad2 size={18} />, label: 'Hook 配置' },
    { id: 'notifications', icon: <Bell size={18} />, label: '学习提醒' },
    { id: 'appearance', icon: <Palette size={18} />, label: '外观主题' },
    { id: 'sync', icon: <Cloud size={18} />, label: '云同步' },
    { id: 'plugins', icon: <Puzzle size={18} />, label: '插件管理' },
    { id: 'advanced', icon: <SettingsIcon size={18} />, label: '高级设置' },
  ];

  const renderContent = () => {
    switch (activeSection) {
      case 'basic':
        return <BasicSettings />;
      case 'engines':
        return <EngineSettings />;
      case 'ai':
        return (
          <div className="space-y-5">
            <div>
              <h1 className="text-xl font-semibold text-text-primary">AI 增强功能</h1>
              <p className="text-xs text-text-secondary mt-1">使用 AI 提升翻译质量和效率</p>
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
      case 'hookprofiles':
        return <HookProfileSettings />;
      case 'notifications':
        return <NotificationManager />;
      case 'appearance':
        return <AppearanceSettings />;
      case 'sync':
        return <SyncSettings />;
      case 'plugins':
        return <PluginSettings />;
      case 'advanced':
        return <AdvancedSettings />;
      default:
        return <BasicSettings />;
    }
  };

  return (
    <div className="flex h-full">
      <Sidebar items={sections} activeId={activeSection} onChange={setActiveSection} />

      <div className="flex-1 overflow-y-auto">
        <div className="max-w-4xl mx-auto p-6">{renderContent()}</div>
      </div>
    </div>
  );
}
