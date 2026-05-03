import { emitHost } from './commands';

export type HostHitRegionMode = 'gameplay' | 'hud_passive' | 'ui_modal' | 'text_entry';

export function reportHostHitRegions(mode: HostHitRegionMode = 'hud_passive'): void {
  if (typeof document === 'undefined') {
    return;
  }
  const regions = Array.from(document.querySelectorAll<HTMLElement>('[data-hit-region]'))
    .map((element) => {
      const rect = element.getBoundingClientRect();
      return {
        id: element.dataset.hitRegion ?? 'devtools',
        x: Math.max(0, Math.round(rect.left)),
        y: Math.max(0, Math.round(rect.top)),
        w: Math.max(1, Math.round(rect.width)),
        h: Math.max(1, Math.round(rect.height))
      };
    })
    .filter((region) => region.w > 0 && region.h > 0)
    .slice(0, 64);
  emitHost('ui.hit_regions.changed', { mode, regions });
}
