import { emitHost } from './host/commands';

const noDragSelector = [
  'button',
  'input',
  'select',
  'textarea',
  'a',
  '[role="button"]',
  '[data-no-drag]',
  '.window-controls',
  '.editor-mode-rail',
  '.topbar-menu'
].join(',');

export function isWindowDragExcluded(target: EventTarget | null): boolean {
  return target instanceof Element && target.closest(noDragSelector) !== null;
}

export function startWindowDrag(event: PointerEvent): void {
  if (event.button !== 0 || isWindowDragExcluded(event.target)) {
    return;
  }

  event.preventDefault();
  emitHost('host.window.drag.start', {
    x: Math.round(event.clientX),
    y: Math.round(event.clientY)
  });
}
