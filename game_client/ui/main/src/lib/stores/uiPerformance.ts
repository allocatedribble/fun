import { readable } from 'svelte/store';

export type UiFrameRateSample = {
  fps: number;
  frame_ms: number;
  sample_frames: number;
  sample_window_ms: number;
};

const EMPTY_SAMPLE: UiFrameRateSample = {
  fps: 0,
  frame_ms: 0,
  sample_frames: 0,
  sample_window_ms: 0
};

export const uiFrameRate = readable<UiFrameRateSample>(EMPTY_SAMPLE, (set) => {
  if (
    typeof window === 'undefined' ||
    typeof window.requestAnimationFrame !== 'function' ||
    typeof window.cancelAnimationFrame !== 'function'
  ) {
    set(EMPTY_SAMPLE);
    return () => {};
  }

  let animationFrame = 0;
  let sampleStartMs = performance.now();
  let sampleFrames = 0;
  let running = true;

  const publishSample = (nowMs: number) => {
    const sampleWindowMs = nowMs - sampleStartMs;
    if (sampleWindowMs < 1000) {
      return;
    }

    const fps = (sampleFrames * 1000) / sampleWindowMs;
    set({
      fps,
      frame_ms: fps > 0 ? 1000 / fps : 0,
      sample_frames: sampleFrames,
      sample_window_ms: sampleWindowMs
    });
    sampleFrames = 0;
    sampleStartMs = nowMs;
  };

  const tick = (nowMs: number) => {
    if (!running) {
      return;
    }

    sampleFrames += 1;
    publishSample(nowMs);
    animationFrame = window.requestAnimationFrame(tick);
  };

  animationFrame = window.requestAnimationFrame((nowMs) => {
    sampleStartMs = nowMs;
    animationFrame = window.requestAnimationFrame(tick);
  });

  return () => {
    running = false;
    window.cancelAnimationFrame(animationFrame);
  };
});
