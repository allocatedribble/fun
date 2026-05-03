import { invokeHost } from './host/commands';
import { returnToGame } from './commands';

export type ChromeMode = 'launcher' | 'editor';

async function returnInputWhenAllowed(mode: ChromeMode, canReturnToGame: boolean): Promise<void> {
  if (mode === 'editor' && canReturnToGame) {
    await returnToGame();
  }
}

async function invokeWindowCommand(command: string): Promise<void> {
  try {
    await invokeHost(command);
  } catch {
    // Browser preview has no native window host. The visual state still stays local.
  }
}

export async function minimizeWindow(mode: ChromeMode = 'editor', canReturnToGame = false): Promise<void> {
  await returnInputWhenAllowed(mode, canReturnToGame);
  await invokeWindowCommand('window.minimize');
}

export async function toggleWindowMaximize(): Promise<void> {
  await invokeWindowCommand('window.maximize.toggle');
}

export async function hideWindow(mode: ChromeMode = 'editor', canReturnToGame = false): Promise<void> {
  await returnInputWhenAllowed(mode, canReturnToGame);
  await invokeWindowCommand('window.hide');
}

export async function closeWindow(mode: ChromeMode = 'editor', canReturnToGame = false): Promise<void> {
  await returnInputWhenAllowed(mode, canReturnToGame);
  await invokeWindowCommand('window.close');
}

export async function closeOrHideWindow(mode: ChromeMode = 'editor', canReturnToGame = false): Promise<void> {
  await closeWindow(mode, canReturnToGame);
}
