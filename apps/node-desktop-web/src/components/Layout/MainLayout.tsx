import React, { useState } from 'react';
import { Outlet, useLocation, useNavigate } from 'react-router-dom';
import { Layout, Menu, Space, Tag, Typography, theme } from 'antd';
import {
  AppstoreOutlined,
  FolderOpenOutlined,
  HistoryOutlined,
  FileTextOutlined,
  SettingOutlined,
} from '@ant-design/icons';

const { Header, Sider, Content } = Layout;

const MainLayout: React.FC = () => {
  const [collapsed, setCollapsed] = useState(false);
  const navigate = useNavigate();
  const location = useLocation();
  const {
    token: { colorBgContainer, borderRadiusLG },
  } = theme.useToken();

  const menuItems = [
    { key: '/dashboard', icon: <AppstoreOutlined />, label: '仪表盘' },
    { key: '/workspaces', icon: <FolderOpenOutlined />, label: '工作区' },
    { key: '/versioning', icon: <HistoryOutlined />, label: '版本管理' },
    { key: '/logs', icon: <FileTextOutlined />, label: '日志中心' },
    { key: '/settings', icon: <SettingOutlined />, label: '设置' },
  ];

  return (
    <Layout style={{ minHeight: '100vh' }}>
      <Sider collapsible collapsed={collapsed} onCollapse={setCollapsed} theme="dark">
        <div
          style={{
            height: 40,
            margin: 16,
            borderRadius: 8,
            background: 'rgba(255, 255, 255, 0.16)',
            color: '#fff',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            fontWeight: 700,
          }}
        >
          {collapsed ? 'uN' : 'uHorse Node'}
        </div>
        <Menu
          theme="dark"
          selectedKeys={[location.pathname]}
          mode="inline"
          items={menuItems}
          onClick={({ key }) => navigate(key)}
        />
      </Sider>
      <Layout>
        <Header
          style={{
            padding: '0 16px',
            background: colorBgContainer,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
          }}
        >
          <Space>
            <Typography.Title level={5} style={{ margin: 0 }}>
              Node Desktop MVP
            </Typography.Title>
            <Tag color="processing">预览</Tag>
          </Space>
          <Tag color="green">本地运行时</Tag>
        </Header>
        <Content style={{ margin: 16 }}>
          <div
            style={{
              minHeight: 360,
              padding: 24,
              background: colorBgContainer,
              borderRadius: borderRadiusLG,
            }}
          >
            <Outlet />
          </div>
        </Content>
      </Layout>
    </Layout>
  );
};

export default MainLayout;
