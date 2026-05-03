import type { EditorUiState, ExperienceRecord, ExperienceScope } from '../types';

export function defaultExperienceScope(state: EditorUiState): ExperienceScope {
  if (state.project && (state.liveClientStatus || state.serverRuntime)) {
    return {
      type: 'project_client_server',
      projectId: state.project.id,
      clientId: state.liveClientStatus?.id,
      serverId: state.serverRuntime?.id
    };
  }
  if (state.project) {
    return { type: 'current_project', projectId: state.project.id };
  }
  return { type: 'all_projects' };
}

export function scopeId(scope: ExperienceScope): string {
  switch (scope.type) {
    case 'current_project':
      return `project:${scope.projectId}`;
    case 'all_projects':
      return 'projects:all';
    case 'client':
      return `client:${scope.clientId}`;
    case 'server':
      return `server:${scope.serverId}`;
    case 'project_client_server':
      return `project-runtime:${scope.projectId}:${scope.clientId ?? 'no-client'}:${scope.serverId ?? 'no-server'}`;
    case 'all_connected':
      return 'connected:all';
  }
}

export function scopeLabel(scope: ExperienceScope): string {
  switch (scope.type) {
    case 'current_project':
      return 'Current Project';
    case 'all_projects':
      return 'All Projects';
    case 'client':
      return 'Client';
    case 'server':
      return 'Server';
    case 'project_client_server':
      return 'Project + Runtime';
    case 'all_connected':
      return 'All Connected';
  }
}

export function scopeTargetLabel(scope: ExperienceScope): string {
  switch (scope.type) {
    case 'current_project':
      return `Project ${scope.projectId}`;
    case 'all_projects':
      return 'All Projects';
    case 'client':
      return `Client ${scope.clientId}`;
    case 'server':
      return `Server ${scope.serverId}`;
    case 'project_client_server':
      return `Project ${scope.projectId}`;
    case 'all_connected':
      return 'All Connected';
  }
}

export function experienceRecordsFromState(state: EditorUiState): ExperienceRecord[] {
  const now = Date.now();
  const records: ExperienceRecord[] = [];
  if (state.project) {
    records.push({
      id: state.project.id,
      kind: 'project',
      title: state.project.display_name,
      subtitle: state.project.root_path,
      status: state.previewRenderStatus.status === 'failed' ? 'failed' : 'ready',
      scope: { type: 'current_project', projectId: state.project.id },
      capabilities: ['scenes', 'entities', 'materials', 'diagnostics'],
      tools: ['project.current.get', 'project.bsn.index', 'material.shader.list'],
      lastSeenAt: now
    });
  }
  if (state.previewRenderStatus) {
    records.push({
      id: 'current-client-preview',
      kind: 'preview',
      title: 'Current Client Preview',
      subtitle: state.previewRenderStatus.renderer?.frame.readback_status ?? state.previewRenderStatus.status,
      status: state.previewRenderStatus.status === 'ready' ? 'ready' : state.previewRenderStatus.status === 'failed' ? 'failed' : 'connecting',
      scope: defaultExperienceScope(state),
      capabilities: ['preview', 'render-status'],
      tools: ['preview.renderer.status.get', 'preview.renderer.scene.set', 'preview.renderer.resize'],
      lastSeenAt: now
    });
  }
  if (state.liveClientStatus) {
    records.push({
      id: state.liveClientStatus.id,
      kind: 'client',
      title: 'Current Fun Client',
      subtitle: `${state.liveClientStatus.status} - in host`,
      status: state.liveClientStatus.status === 'running' ? 'running' : 'stopped',
      scope: { type: 'client', clientId: state.liveClientStatus.id },
      capabilities: ['entity-stream', 'runtime-state', 'logs'],
      tools: ['viewport.client.launch', 'viewport.client.stop', 'runtime.inspector.attach'],
      lastSeenAt: now
    });
  }
  if (state.serverRuntime) {
    records.push({
      id: state.serverRuntime.id,
      kind: 'server',
      title: 'Local Fun Server',
      subtitle: `${state.serverRuntime.process_status} - ${state.serverRuntime.inspector_status}`,
      status: state.serverRuntime.process_status === 'running' ? 'running' : 'stopped',
      scope: { type: 'server', serverId: state.serverRuntime.id },
      capabilities: ['authority-state', 'diagnostics', 'logs'],
      tools: ['runtime.server.launch', 'runtime.diagnostics.list'],
      lastSeenAt: now
    });
  }
  records.push({
    id: 'backend-placeholder',
    kind: 'backend_experience',
    title: 'Hosted Experiences',
    subtitle: 'Backend hosting tools are registered but disabled until a backend connection exists.',
    status: 'stopped',
    scope: { type: 'all_connected' },
    capabilities: ['host', 'publish', 'sync'],
    tools: ['backend.experience.host', 'backend.experience.publish', 'backend.experience.sync'],
    lastSeenAt: now
  });
  return records;
}
