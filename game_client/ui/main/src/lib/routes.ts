import type { EditorUiState } from './types';

export type FunClientRoute =
  | 'loading'
  | 'game.hud'
  | 'game.pause'
  | 'game.chat'
  | 'game.scoreboard'
  | 'launcher.home'
  | 'launcher.projects'
  | 'launcher.games'
  | 'editor.overview'
  | 'editor.preview'
  | 'editor.live_client'
  | 'editor.server'
  | 'editor.graph'
  | 'editor.diagnostics';

export const funClientRoutes: readonly FunClientRoute[] = [
  'loading',
  'game.hud',
  'game.pause',
  'game.chat',
  'game.scoreboard',
  'launcher.home',
  'launcher.projects',
  'launcher.games',
  'editor.overview',
  'editor.preview',
  'editor.live_client',
  'editor.server',
  'editor.graph',
  'editor.diagnostics'
];

export function routeForState(state: EditorUiState): FunClientRoute {
  if ((state.hostMode === 'loading' || state.hostMode === 'boot') || (state.loading && !state.status)) {
    return 'loading';
  }
  if (state.hostInputOwner === 'game_menu_ui') {
    return 'game.pause';
  }
  if (state.hostInputOwner === 'text_entry' && state.hostMode === 'game') {
    return 'game.chat';
  }
  if (state.hostMode === 'game') {
    return 'game.hud';
  }
  if (state.launcherMode === 'hidden') {
    return 'game.hud';
  }
  if (state.launcherMode === 'editor') {
    switch (state.activeEditorContextId) {
      case 'preview':
        return 'editor.preview';
      case 'live_client':
        return 'editor.live_client';
      case 'server':
        return 'editor.server';
      case 'graph':
        return 'editor.graph';
      case 'diagnostics':
        return 'editor.diagnostics';
      case 'overview':
      default:
        return 'editor.overview';
    }
  }
  if (state.projects.length > 0 && state.games.length === 0) {
    return 'launcher.projects';
  }
  if (state.games.length > 0) {
    return 'launcher.games';
  }
  return 'launcher.home';
}
