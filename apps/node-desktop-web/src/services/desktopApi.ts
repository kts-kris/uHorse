import type {
  ApiError,
  ApiResponse,
  CancelAccountPairingRequest,
  DefaultSettings,
  DesktopAccountStatus,
  DesktopCapabilityStatus,
  DesktopLogEntry,
  DesktopNodeStatus,
  DesktopPairingRequest,
  DesktopSettings,
  DesktopVersionSummary,
  DesktopWorkspaceStatus,
  DirectoryPickerResponse,
  WorkspaceValidation,
  WorkspaceValidationRequest,
} from '../types/desktop';

const DEFAULT_BASE_URL = 'http://127.0.0.1:8757';

function getBaseUrl(): string {
  if (import.meta.env.VITE_DESKTOP_API_URL) {
    return import.meta.env.VITE_DESKTOP_API_URL;
  }

  if (!import.meta.env.DEV && typeof window !== 'undefined' && /^https?:$/.test(window.location.protocol)) {
    return window.location.origin;
  }

  return DEFAULT_BASE_URL;
}

export function getApiErrorMessage(error: ApiError | undefined, fallback: string): string {
  if (!error) return fallback;
  if (typeof error === 'string') return error || fallback;
  return error.message || fallback;
}

export function unwrapApiResponse<T>(response: ApiResponse<T>, fallback: string): T {
  if (response.success && response.data !== null) {
    return response.data;
  }

  throw new Error(getApiErrorMessage(response.error, fallback));
}

async function request<T>(path: string, init?: RequestInit, fallback = '请求失败'): Promise<T> {
  const response = await fetch(`${getBaseUrl()}${path}`, {
    ...init,
    headers: {
      'Content-Type': 'application/json',
      ...(init?.headers || {}),
    },
  });

  let payload: ApiResponse<T>;
  try {
    payload = (await response.json()) as ApiResponse<T>;
  } catch {
    throw new Error(fallback);
  }

  if (!response.ok) {
    throw new Error(getApiErrorMessage(payload.error, fallback));
  }

  return unwrapApiResponse(payload, fallback);
}

export const desktopApi = {
  getSettings(): Promise<DesktopSettings> {
    return request('/api/settings', undefined, '加载设置失败');
  },
  getDefaultSettings(): Promise<DefaultSettings> {
    return request('/api/settings/defaults', undefined, '加载默认设置失败');
  },
  getCapabilityStatus(): Promise<DesktopCapabilityStatus> {
    return request('/api/settings/capabilities', undefined, '加载桌面能力状态失败');
  },
  saveSettings(payload: DesktopSettings): Promise<DesktopSettings> {
    return request(
      '/api/settings',
      {
        method: 'POST',
        body: JSON.stringify(payload),
      },
      '保存设置失败',
    );
  },
  testNotification(): Promise<string> {
    return request('/api/settings/notifications/test', { method: 'POST' }, '发送测试通知失败');
  },
  validateWorkspace(payload: WorkspaceValidationRequest): Promise<WorkspaceValidation> {
    return request(
      '/api/workspace/validate',
      {
        method: 'POST',
        body: JSON.stringify(payload),
      },
      '校验工作区失败',
    );
  },
  pickWorkspace(): Promise<DirectoryPickerResponse> {
    return request('/api/workspace/pick', { method: 'POST' }, '选择工作区失败');
  },
  getRuntimeStatus(): Promise<DesktopNodeStatus> {
    return request('/api/runtime/status', undefined, '加载运行时状态失败');
  },
  startNode(): Promise<DesktopNodeStatus> {
    return request('/api/runtime/start', { method: 'POST' }, '启动 Node 失败');
  },
  stopNode(): Promise<DesktopNodeStatus> {
    return request('/api/runtime/stop', { method: 'POST' }, '停止 Node 失败');
  },
  getWorkspaceStatus(): Promise<DesktopWorkspaceStatus> {
    return request('/api/workspace/status', undefined, '加载工作区状态失败');
  },
  getVersionSummary(): Promise<DesktopVersionSummary> {
    return request('/api/versioning/summary', undefined, '加载版本摘要失败');
  },
  getLogs(): Promise<DesktopLogEntry[]> {
    return request('/api/logs', undefined, '加载日志失败');
  },
  startAccountPairing(): Promise<DesktopPairingRequest> {
    return request('/api/account/pairing/start', { method: 'POST' }, '发起账号绑定失败');
  },
  cancelAccountPairing(payload: CancelAccountPairingRequest): Promise<string> {
    return request(
      '/api/account/pairing/cancel',
      {
        method: 'POST',
        body: JSON.stringify(payload),
      },
      '取消账号绑定失败',
    );
  },
  getAccountStatus(): Promise<DesktopAccountStatus> {
    return request('/api/account/status', undefined, '加载账号绑定状态失败');
  },
  deleteAccountBinding(): Promise<string> {
    return request('/api/account/binding', { method: 'DELETE' }, '解绑账号失败');
  },
};
