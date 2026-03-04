import client from './api';
import type { ApiResponse } from './api';
import type { LoginRequest, TokenResponse } from '../types';

export const authService = {
  // 登录
  async login(credentials: LoginRequest): Promise<TokenResponse> {
    const response = await client.post<ApiResponse<TokenResponse>>('/api/v1/auth/login', credentials);
    if (response.data.success) {
      localStorage.setItem('access_token', response.data.data.access_token);
      localStorage.setItem('refresh_token', response.data.data.refresh_token);
      return response.data.data;
    }
    throw new Error(response.data.error?.message || '登录失败');
  },

  // 登出
  async logout(): Promise<void> {
    const refreshToken = localStorage.getItem('refresh_token');
    if (refreshToken) {
      try {
        await client.post('/api/v1/auth/logout', { refresh_token: refreshToken });
      } catch {
        // 忽略登出错误
      }
    }
    localStorage.removeItem('access_token');
    localStorage.removeItem('refresh_token');
  },

  // 刷新 Token
  async refreshToken(): Promise<TokenResponse> {
    const refreshToken = localStorage.getItem('refresh_token');
    if (!refreshToken) {
      throw new Error('No refresh token');
    }
    const response = await client.post<ApiResponse<TokenResponse>>('/api/v1/auth/refresh', {
      refresh_token: refreshToken,
    });
    if (response.data.success) {
      localStorage.setItem('access_token', response.data.data.access_token);
      localStorage.setItem('refresh_token', response.data.data.refresh_token);
      return response.data.data;
    }
    throw new Error(response.data.error?.message || 'Token 刷新失败');
  },

  // 检查是否已登录
  isAuthenticated(): boolean {
    return !!localStorage.getItem('access_token');
  },
};
