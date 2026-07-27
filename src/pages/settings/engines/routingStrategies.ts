import type { RoutingStrategy } from '../../../types';

export interface RoutingStrategyOption {
  id: RoutingStrategy;
  label: string;
  description: string;
  recommended?: boolean;
}

/** Product default: ordered fallback. Keep parallel for multi-result; drop cost/latency UI noise. */
export const ROUTING_STRATEGIES: RoutingStrategyOption[] = [
  {
    id: 'fallback_on_error',
    label: '顺序回退',
    description: '按下方列表从上到下尝试，第一个成功即返回',
    recommended: true,
  },
  {
    id: 'primary_only',
    label: '仅首位',
    description: '只用排序第一的引擎，失败不尝试后续',
  },
  {
    id: 'parallel_compare',
    label: '并行对比',
    description: '同时调用已启用引擎，主界面展示多结果',
  },
];
