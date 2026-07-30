import type { AppConfig } from '../../../types';

export type ConfigUpdater = (updater: (prev: AppConfig) => AppConfig) => void;

export interface EngineConfigProps {
  config: AppConfig;
  updateConfig: ConfigUpdater;
  saveConfig: () => Promise<void>;
  showSecrets?: Record<string, boolean>;
  toggleSecret?: (key: string) => void;
  onNavigate?: (sectionId: string) => void;
}

export interface EngineBadge {
  label: string;
  variant: 'success' | 'warning' | 'error' | 'info';
}

export interface EngineDisplayConfig {
  id: string;
  name: string;
  enabled: boolean;
  status: 'connected' | 'warning' | 'error';
  badges: EngineBadge[];
  description: string;
}
