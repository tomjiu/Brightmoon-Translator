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
/** labelKey / descriptionKey are i18n paths under settings.enginePage.* */
export const ROUTING_STRATEGIES: Array<{
  id: RoutingStrategy;
  labelKey: string;
  descriptionKey: string;
  recommended?: boolean;
}> = [
  {
    id: 'fallback_on_error',
    labelKey: 'settings.enginePage.routeFallback',
    descriptionKey: 'settings.enginePage.routeFallbackDesc',
    recommended: true,
  },
  {
    id: 'primary_only',
    labelKey: 'settings.enginePage.routePrimary',
    descriptionKey: 'settings.enginePage.routePrimaryDesc',
  },
  {
    id: 'parallel_compare',
    labelKey: 'settings.enginePage.routeParallel',
    descriptionKey: 'settings.enginePage.routeParallelDesc',
  },
];
