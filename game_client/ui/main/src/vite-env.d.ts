/// <reference types="svelte" />
/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_FUN_RVELTE_DEV?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
