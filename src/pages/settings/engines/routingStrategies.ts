import type { RoutingStrategy } from '../../../types';

export interface RoutingStrategyOption {
  id: RoutingStrategy;
  label: string;
  description: string;
  recommended?: boolean;
}

export const ROUTING_STRATEGIES: RoutingStrategyOption[] = [
  {
    id: 'fallback_on_error',
    label: '回退模式',
    description: '按引擎顺序尝试，第一个成功就返回（推荐）',
    recommended: true,
  },
  {
    id: 'primary_only',
    label: '仅主引擎',
    description: '只使用排序第一的引擎，失败不回退',
  },
  {
    id: 'parallel_compare',
    label: '并行模式',
    description: '同时调用多个引擎，显示所有结果',
  },
  {
    id: 'cost_aware',
    label: '成本优先',
    description: '优先使用免费引擎，失败后尝试付费引擎',
  },
  {
    id: 'latency_first',
    label: '延迟优先',
    description: '优先使用历史延迟更低的引擎',
  },
];
