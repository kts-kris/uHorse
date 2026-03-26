import React, { Suspense } from 'react';
import { BrowserRouter, Navigate, Route, Routes } from 'react-router-dom';
import { ConfigProvider, Spin } from 'antd';
import zhCN from 'antd/locale/zh_CN';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

const MainLayout = React.lazy(() => import('./components/Layout/MainLayout'));
const Dashboard = React.lazy(() => import('./pages/Dashboard'));
const Workspaces = React.lazy(() => import('./pages/Workspaces'));
const Versioning = React.lazy(() => import('./pages/Versioning'));
const Logs = React.lazy(() => import('./pages/Logs'));
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

function App() {
  return (
    <ConfigProvider locale={zhCN}>
      <QueryClientProvider client={queryClient}>
        <BrowserRouter>
          <Routes>
            <Route path="/" element={renderLazy(<MainLayout />)}>
              <Route index element={<Navigate to="/dashboard" replace />} />
              <Route path="dashboard" element={renderLazy(<Dashboard />)} />
              <Route path="workspaces" element={renderLazy(<Workspaces />)} />
              <Route path="versioning" element={renderLazy(<Versioning />)} />
              <Route path="logs" element={renderLazy(<Logs />)} />
              <Route path="settings" element={renderLazy(<Settings />)} />
            </Route>
          </Routes>
        </BrowserRouter>
      </QueryClientProvider>
    </ConfigProvider>
  );
}

export default App;
