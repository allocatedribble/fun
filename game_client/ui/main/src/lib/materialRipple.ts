const RIPPLE_SELECTOR = 'button, .button, [role="button"], [data-ripple]';
const RIPPLE_CLASS = 'material-ripple';
const RIPPLE_TIMEOUT_MS = 700;

export function installMaterialRipples(root: Document | HTMLElement = document): () => void {
  const onPointerDown: EventListener = (event): void => {
    if (!(event instanceof PointerEvent)) {
      return;
    }
    if (event.pointerType === 'mouse' && event.button !== 0) {
      return;
    }

    const host = rippleHost(event.target);
    if (!host) {
      return;
    }

    addRipple(host, event.clientX, event.clientY);
  };

  const onKeyDown: EventListener = (event): void => {
    if (!(event instanceof KeyboardEvent)) {
      return;
    }
    if (event.repeat || (event.key !== 'Enter' && event.key !== ' ')) {
      return;
    }

    const host = rippleHost(event.target);
    if (!host) {
      return;
    }

    addRipple(host);
  };

  root.addEventListener('pointerdown', onPointerDown, { capture: true });
  root.addEventListener('keydown', onKeyDown, { capture: true });

  return () => {
    root.removeEventListener('pointerdown', onPointerDown, { capture: true });
    root.removeEventListener('keydown', onKeyDown, { capture: true });
  };
}

function rippleHost(target: EventTarget | null): HTMLElement | null {
  if (!(target instanceof HTMLElement)) {
    return null;
  }

  const host = target.closest<HTMLElement>(RIPPLE_SELECTOR);
  if (!host || host.matches(':disabled, [disabled], [aria-disabled="true"], .is-disabled')) {
    return null;
  }
  if (host.dataset.ripple === 'off' || host.hasAttribute('data-no-ripple')) {
    return null;
  }

  return host;
}

function addRipple(host: HTMLElement, clientX?: number, clientY?: number): void {
  const rect = host.getBoundingClientRect();
  if (rect.width <= 0 || rect.height <= 0) {
    return;
  }

  const diameter = Math.ceil(Math.hypot(rect.width, rect.height) * 2);
  const x = clientX ?? rect.left + rect.width / 2;
  const y = clientY ?? rect.top + rect.height / 2;
  const ripple = document.createElement('span');

  ripple.className = RIPPLE_CLASS;
  ripple.setAttribute('aria-hidden', 'true');
  ripple.style.width = `${diameter}px`;
  ripple.style.height = `${diameter}px`;
  ripple.style.left = `${x - rect.left - diameter / 2}px`;
  ripple.style.top = `${y - rect.top - diameter / 2}px`;

  host.querySelectorAll(`:scope > .${RIPPLE_CLASS}`).forEach((staleRipple) => {
    staleRipple.remove();
  });
  host.appendChild(ripple);

  const removeRipple = (): void => {
    ripple.remove();
  };
  ripple.addEventListener('animationend', removeRipple, { once: true });
  window.setTimeout(removeRipple, RIPPLE_TIMEOUT_MS);
}
