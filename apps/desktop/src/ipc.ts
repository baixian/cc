import { invoke } from '@tauri-apps/api/core';
import type { BootstrapDto, CommandResult, CoreSessionDto, CoreSnapshotDto, EnvironmentDto } from './generated/contracts';

export class GatewayIpcError extends Error {
  constructor(readonly code: string, message: string, readonly traceId = '') { super(message); }
}

export async function readCommand<T>(name: string, args?: Record<string, unknown>): Promise<T> {
  if (!('__TAURI_INTERNALS__' in window)) {
    throw new GatewayIpcError('DESKTOP_REQUIRED', '请在 Mac 桌面应用中打开。浏览器页面没有系统状态。');
  }
  const result = await invoke<CommandResult<T>>(name, args);
  if (!result.ok || !result.envelope || result.envelope.schemaVersion !== 3) {
    throw new GatewayIpcError(result.error?.code ?? 'IPC_CONTRACT_ERROR', result.error?.message ?? '接口版本或响应不正确', result.error?.traceId);
  }
  return result.envelope.data;
}
export const gatewayApi = {
  bootstrap: () => readCommand<BootstrapDto>('app_get_bootstrap'),
  environment: () => readCommand<EnvironmentDto>('system_get_environment'),
  coreSnapshot: () => readCommand<CoreSnapshotDto>('core_get_snapshot'),
  connectExternal: (endpoint: string) => readCommand<CoreSessionDto>('core_connect_external', {endpoint, readonly:true, requestId:crypto.randomUUID()}),
  disconnect: () => readCommand<boolean>('core_disconnect', {requestId:crypto.randomUUID()}),
};
