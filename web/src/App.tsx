import React, { Suspense } from 'react';
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { ConfigProvider, Spin } from 'antd';
import zhCN from 'antd/locale/zh_CN';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

const MainLayout = React.lazy(() => import('./components/Layout/MainLayout'));
const Login = React.lazy(() => import('./pages/Login'));
const Dashboard = React.lazy(() => import('./pages/Dashboard'));
const Agents = React.lazy(() => import('./pages/Agents'));
const Skills = React.lazy(() => import('./pages/Skills'));
const Sessions = React.lazy(() => import('./pages/Sessions'));
const Channels = React.lazy(() => import('./pages/Channels'));
const Settings = React.lazy(() => import('./pages/Settings'));

const queryClient = new QueryClient();

const RouteFallback: React.FC = () => (
  <div
    style={{
      minHeight: 320,
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
    }}
  >
    <Spin size="large" />
  </div>
);

function renderLazy(element: React.ReactNode): React.ReactElement {
  return <Suspense fallback={<RouteFallback />}>{element}</Suspense>;
}

// 简单的路由守卫
const PrivateRoute: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const token = localStorage.getItem('access_token');
  return token ? <>{children}</> : <Navigate to="/login" />;
};

function App() {
  return (
    <ConfigProvider locale={zhCN}>
      <QueryClientProvider client={queryClient}>
        <BrowserRouter>
          <Routes>
            <Route path="/login" element={renderLazy(<Login />)} />
            <Route
              path="/"
              element={
                <PrivateRoute>
                  {renderLazy(<MainLayout />)}
                </PrivateRoute>
              }
            >
              <Route index element={<Navigate to="/dashboard" replace />} />
              <Route path="dashboard" element={renderLazy(<Dashboard />)} />
              <Route path="agents" element={renderLazy(<Agents />)} />
              <Route path="skills" element={renderLazy(<Skills />)} />
              <Route path="sessions" element={renderLazy(<Sessions />)} />
              <Route path="channels" element={renderLazy(<Channels />)} />
              <Route path="settings" element={renderLazy(<Settings />)} />
            </Route>
          </Routes>
        </BrowserRouter>
      </QueryClientProvider>
    </ConfigProvider>
  );
}

export default App;
