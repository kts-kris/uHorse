import axios from 'axios';
import type { AxiosInstance, AxiosResponse, InternalAxiosRequestConfig } from 'axios';

// API 响应类型
export type ApiError =
  | string
  | {
      code?: string;
      message?: string;
      details?: unknown;
    };

export interface ApiResponse<T> {
  success: boolean;
  data: T | null;
  error?: ApiError;
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

// 分页响应
export interface PaginatedResponse<T> {
  items: T[];
  total: number;
  page: number;
  page_size: number;
  total_pages: number;
}

// API 基础配置
const client: AxiosInstance = axios.create({
  baseURL: import.meta.env.VITE_API_URL || 'http://localhost:3000',
  timeout: 30000,
  headers: {
    'Content-Type': 'application/json',
  },
});

// 请求拦截器 - 添加认证 Token
client.interceptors.request.use(
  (config: InternalAxiosRequestConfig) => {
    const token = localStorage.getItem('access_token');
    if (token && config.headers) {
      config.headers.Authorization = `Bearer ${token}`;
    }
    return config;
  },
  (error) => Promise.reject(error)
);

// 响应拦截器 - 处理错误
client.interceptors.response.use(
  (response: AxiosResponse<ApiResponse<unknown>>) => response,
  async (error) => {
    if (error.response?.status === 401) {
      const refreshToken = localStorage.getItem('refresh_token');
      if (refreshToken) {
        try {
          const response = await axios.post<ApiResponse<{ access_token: string; refresh_token: string }>>(
            '/api/v1/auth/refresh',
            { refresh_token: refreshToken }
          );
          if (response.data.success && response.data.data !== null) {
            const { access_token, refresh_token } = response.data.data;
            localStorage.setItem('access_token', access_token);
            localStorage.setItem('refresh_token', refresh_token);
            error.config.headers.Authorization = `Bearer ${access_token}`;
            return client.request(error.config);
          }
        } catch {
          localStorage.removeItem('access_token');
          localStorage.removeItem('refresh_token');
          window.location.href = '/login';
        }
      }
    }
    return Promise.reject(error);
  }
);

export default client;
