import type { RoutingStrategy } from '../../../types';

export interface RoutingStrategyOption {
  id: RoutingStrategy;
  label: string;
  description: string;
  recommended?: boolean;
}

/**
 * Global strategy applies to selection / hook / clipboard / etc.
 * Product overrides (hardcoded in Rust):
 * - Main window (channel=ui): always multi-engine parallel results
 * - OCR frame (channel=ocr): always ordered fallback (single result)
 */
export const ROUTING_STRATEGIES: RoutingStrategyOption[] = [
  {
    id: 'fallback_on_error',
    label: '顺序回退',
    description:
      '按引擎列表从上到下尝试，第一个成功即返回。用于划词/Hook 等；OCR 框固定此策略；主页不受此影响',
    recommended: true,
  },
  {
    id: 'primary_only',
    label: '仅首位',
    description: '只用排序第一的引擎，失败不尝试后续（划词等非主页通道）',
  },
  {
    id: 'parallel_compare',
    label: '并行对比',
    description: '同时调用已启用引擎并返回多结果。主页复制翻译已固定多结果，无需依赖此项',
  },
];
