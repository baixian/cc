/* Contract snapshot for initial development. Regenerate and compare with Rust ts-rs exports on a Mac build. */
export interface CapabilityDto {
  externalRead: boolean; externalSelectProxy: boolean; managedCore: boolean;
  systemProxy: boolean; tun: boolean; diagnostics: boolean;
  networkRestore: boolean; update: boolean; reasonCodes: string[];
}
export interface BootstrapDto {
  appVersion: string; osVersion: string; architecture: string;
  firstRun: boolean; coreMode: string | null; coreState: string;
  ingress: string; capabilities: CapabilityDto; settingsRevision: number;
}
export interface EnvironmentDto {
  osVersion: string; architecture: string; isMacos: boolean;
  observedAt: string; networkStatus: string;
}
export interface CoreSnapshotDto {
  state: string; observedAt: string; coreVersion: string; connectionCount: number;
  uploadBytes: number | null; downloadBytes: number | null;
}
export interface CoreSessionDto { mode: string; state: string; version: string; endpoint: string }
export interface ErrorDto { code: string; message: string; retryable: boolean; traceId: string }
export type CommandResult<T> = {
  ok: boolean;
  envelope?: { schemaVersion: number; traceId: string; timestamp: string; data: T };
  error?: ErrorDto;
};
