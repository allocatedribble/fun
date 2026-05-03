export interface UiControlContract {
  label: string;
  owningComponent: string;
  action: string;
  enabledCondition: string;
  disabledReason: string;
  expectedTransition: string;
}

export const uiControlContracts: UiControlContract[] = [
  contract('Editor swatch', 'ModeSwitchPane', 'editorStore.activateEditorShell', 'authorized', 'authorization unavailable', 'Editor pane system is visible'),
  contract('Game swatch', 'ModeSwitchPane', 'editorStore.selectGameAndShowLauncher', 'authorized game selected', 'authorization unavailable', 'Launcher game panes are visible'),
  contract('Game #1 pane', 'LauncherShell', 'editorStore.selectGame', 'always', '', 'Game #1 pane becomes selected'),
  contract('Game #2 pane', 'LauncherShell', 'editorStore.selectGame', 'always', '', 'Game #2 pane becomes selected'),
  contract('Editor pane', 'LauncherShell', 'editorStore.activateEditorShell', 'authorized', 'authorization unavailable', 'Editor shell opens'),
  contract('Minimize', 'WindowControls', 'minimizeWindow', 'always', '', 'Window minimizes; editor can return input to gameplay'),
  contract('Maximize', 'WindowControls', 'toggleWindowMaximize', 'always', '', 'Window maximizes or restores'),
  contract('Close', 'WindowControls', 'closeOrHideWindow', 'always', '', 'Window closes; editor can return input to gameplay'),
  contract('Commandbar', 'Commandbar', 'editorStore.focusCommandbar', 'always', '', 'Conversation surface opens from titlebar'),
  contract('Browse for Project', 'Commandbar', 'editorStore.pickProject', 'not opening', 'Project open is already running', 'Native project picker opens from command result'),
  contract('Open Fun Project', 'Commandbar', 'editorStore.openDefaultProject', 'not opening', 'Project open is already running', 'Fun opens and Preview prepares'),
  contract('Open Viewport', 'Commandbar', 'editorStore.executeEditorCommand(editor.set_context.preview)', 'project selected', 'No project selected', 'Viewport tab becomes active'),
  contract('Open Graph', 'Commandbar', 'editorStore.executeEditorCommand(editor.set_context.graph)', 'project selected', 'No project selected', 'Material graph tab becomes active'),
  contract('Show Log', 'Commandbar', 'editorStore.executeEditorCommand(editor.show_log)', 'always', '', 'Log tab becomes active'),
  contract('Play Current Client', 'Commandbar', 'editorStore.launchClient', 'project selected', 'no project selected', 'Current Client context selected'),
  contract('Launch Server', 'Commandbar', 'editorStore.launchServer', 'project selected and launch idle', 'no project selected or server launch running', 'Server context selected'),
  contract('Pane drag handle', 'EditorPaneFrame', 'HTML drag pane docking', 'always', '', 'Pane docks into left, center, right, or bottom zone'),
  contract('Viewport pane', 'EditorPaneFrame', 'editorStore.setActiveEditorContext(preview)', 'project selected', 'No project selected', 'Current-client preview surface is active'),
  contract('Graph pane', 'EditorPaneFrame', 'editorStore.setActiveEditorContext(graph)', 'project selected', 'No project selected', 'Material graph surface is active'),
  contract('Log pane', 'EditorPaneFrame', 'editorStore.setActiveEditorContext(diagnostics)', 'always', '', 'Combined log surface renders'),
  contract('Overview row', 'OverviewPanel', 'editorStore.selectOverviewItem', 'row selectable', 'row not loaded', 'Scrollable Overview tooltip reflects selected project, asset, scene, diagnostic, or entity'),
  contract('Entity row', 'OverviewPanel', 'editorStore.selectEntity', 'row selectable', 'row not loaded', 'Scrollable Overview tooltip loads entity components'),
  contract('Account login', 'AccountLoginPanel', 'editorStore.requestAccountTicket', 'editor auth available', 'auth unavailable', 'Profile avatar receives a backend auth ticket'),
  contract('Apply Transform', 'DetailsPane', 'editorStore.patchSelectedTransform', 'selected mutable entity', 'no selected entity', 'Transform patch command result recorded'),
  contract('Combined log stream', 'LogPanel', 'editorStore.setActiveEditorContext(diagnostics)', 'always', '', 'Diagnostics, runtime trace, events, and command results appear in Log')
];

export function validateUiContracts(contracts = uiControlContracts): string[] {
  return contracts
    .filter((control) => !control.label || !control.owningComponent || !control.action || !control.expectedTransition)
    .map((control) => `${control.owningComponent}:${control.label}`);
}

function contract(
  label: string,
  owningComponent: string,
  action: string,
  enabledCondition: string,
  disabledReason: string,
  expectedTransition: string
): UiControlContract {
  return { label, owningComponent, action, enabledCondition, disabledReason, expectedTransition };
}
