import { describe, expect, it, vi, beforeEach } from 'vitest';
import { readCommand, GatewayIpcError, gatewayApi } from './ipc';
import { invoke } from '@tauri-apps/api/core';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

describe('read-only IPC gateway', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    delete (globalThis as {window?: unknown}).window;
  });
  it('rejects a browser page without invoking the desktop bridge', async () => {
    Object.assign(globalThis, {window: {}});
    await expect(readCommand('app_get_bootstrap')).rejects.toMatchObject({code:'DESKTOP_REQUIRED'});
    expect(invoke).not.toHaveBeenCalled();
  });
  it('rejects an incompatible contract version', async () => {
    Object.assign(globalThis, {window: {__TAURI_INTERNALS__: {}}});
    vi.mocked(invoke).mockResolvedValue({ok:true,envelope:{schemaVersion:2,data:{coreState:'RUNNING_TUN'}}});
    await expect(readCommand('app_get_bootstrap')).rejects.toBeInstanceOf(GatewayIpcError);
  });
  it('propagates a backend failure rather than inventing a healthy state', async () => {
    Object.assign(globalThis, {window: {__TAURI_INTERNALS__: {}}});
    vi.mocked(invoke).mockResolvedValue({ok:false,error:{code:'NO_CORE',message:'内核不可用',retryable:false,traceId:'t'}});
    await expect(readCommand('core_get_snapshot')).rejects.toMatchObject({code:'NO_CORE'});
  });
  it('only asks for a read-only local controller session', async () => {
    Object.assign(globalThis, {window: {__TAURI_INTERNALS__: {}}});
    vi.stubGlobal('crypto', {randomUUID: () => '11111111-1111-4111-8111-111111111111'});
    vi.mocked(invoke).mockResolvedValue({ok:true,envelope:{schemaVersion:3,data:{mode:'external',state:'EXTERNAL_READONLY'}}});
    await gatewayApi.connectExternal('http://127.0.0.1:9090');
    expect(invoke).toHaveBeenCalledWith('core_connect_external', {
      endpoint:'http://127.0.0.1:9090', readonly:true, requestId:'11111111-1111-4111-8111-111111111111',
    });
    vi.unstubAllGlobals();
  });
  it('passes pagination and filter to the read-only connections command', async () => {
    Object.assign(globalThis, {window: {__TAURI_INTERNALS__: {}}});
    vi.mocked(invoke).mockResolvedValue({ok:true,envelope:{schemaVersion:3,data:{items:[],total:0}}});
    await gatewayApi.connectionsList('25', 'example.org');
    expect(invoke).toHaveBeenCalledWith('connections_list', {cursor:'25', filter:'example.org', limit:25});
  });
});
