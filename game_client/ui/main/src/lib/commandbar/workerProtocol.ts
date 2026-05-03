import type {
  CommandbarContextSnapshot,
  CommandbarIntent,
  CommandbarResult,
  EditorToolDefinition,
  ExperienceScope,
  SearchIndexRecord
} from '../types';

export type CommandbarWorkerRequest =
  | {
      kind: 'query';
      requestId: number;
      query: string;
      scope: ExperienceScope;
      context: CommandbarContextSnapshot;
    }
  | {
      kind: 'index_update';
      records: SearchIndexRecord[];
    }
  | {
      kind: 'tools_update';
      tools: EditorToolDefinition[];
    };

export type CommandbarWorkerResponse =
  | {
      kind: 'results';
      requestId: number;
      intent: CommandbarIntent;
      results: CommandbarResult[];
    }
  | {
      kind: 'partial_results';
      requestId: number;
      results: CommandbarResult[];
    };
