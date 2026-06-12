import { useState } from 'react';
import Sidebar from '../../components/Sidebar';
import { Globe, Languages, Eye, Keyboard, Palette, Settings as SettingsIcon } from 'lucide-react';
import BasicSettings from './BasicSettings';
import EngineSettings from './EngineSettings';
import OcrSettings from './OcrSettings';
import HotkeySettings from './HotkeySettings';
import AppearanceSettings from './AppearanceSettings';
import AdvancedSettings from './AdvancedSettings';

export default function SettingsLayout() {
  const [activeSection, setActiveSection] = useState('basic');

  const sections = [
    { id: 'basic', icon: <Globe size={18} />, label: '基础设置' },
    { id: 'engines', icon: <Languages size={18} />, label: '翻译引擎' },
    { id: 'ocr', icon: <Eye size={18} />, label: 'OCR设置' },
    { id: 'hotkeys', icon: <Keyboard size={18} />, label: '快捷键' },
    { id: 'appearance', icon: <Palette size={18} />, label: '外观主题' },
    { id: 'advanced', icon: <SettingsIcon size={18} />, label: '高级设置' },
  ];

  const renderContent = () => {
    switch (activeSection) {
      case 'basic':
        return <BasicSettings />;
      case 'engines':
        return <EngineSettings />;
      case 'ocr':
        return <OcrSettings />;
      case 'hotkeys':
        return <HotkeySettings />;
      case 'appearance':
        return <AppearanceSettings />;
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
        <div className="max-w-4xl mx-auto p-8">{renderContent()}</div>
      </div>
    </div>
  );
}
