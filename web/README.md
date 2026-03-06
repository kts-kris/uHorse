# uHorse Web 管理界面

uHorse 的前端管理界面，基于 React + TypeScript + Vite + Ant Design 构建。

## 技术栈

- **React 19** - UI 框架
- **TypeScript** - 类型安全
- **Vite 7** - 构建工具
- **Ant Design 6** - UI 组件库
- **TanStack Query** - 数据请求和缓存
- **React Router 7** - 路由管理
- **Axios** - HTTP 客户端
- **Day.js** - 日期处理

## 项目结构

```
src/
├── App.tsx                 # 应用入口
├── main.tsx               # 渲染入口
├── types.ts               # 类型定义
├── components/
│   └── Layout/
│       └── MainLayout.tsx # 主布局
└── pages/
    ├── Dashboard.tsx      # 仪表盘
    ├── Agents.tsx         # Agent 管理
    ├── Skills.tsx         # 技能管理
    ├── Sessions.tsx       # 会话管理
    ├── Channels.tsx       # 通道管理
    ├── Settings.tsx       # 系统设置
    └── Login.tsx          # 登录页
```

## 页面功能

### Dashboard（仪表盘）
- 系统运行时间
- 今日消息统计
- Agent/会话数量
- 最近活动

### Agents（Agent 管理）
- Agent 列表展示
- 创建/编辑/删除 Agent
- 启用/禁用 Agent
- 配置 Agent 参数

### Skills（技能管理）
- 技能列表
- 创建/编辑技能
- 技能参数定义
- 启用/禁用技能

### Sessions（会话管理）
- 会话列表和筛选
- 查看消息历史
- 按状态/Agent 筛选

### Channels（通道管理）
- 通道状态监控
- 通道配置
- 连接测试

### Settings（系统设置）

#### 系统概览 Tab
- 运行时间
- 今日消息数
- 平均响应时间
- 系统版本信息

#### 通用设置 Tab
- 服务地址/端口
- 日志级别
- 最大连接数

#### LLM 设置 Tab
- 默认模型选择
- API Key 配置
- 温度/Max Tokens
- 请求超时

#### 安全设置 Tab
- JWT 认证开关
- JWT Secret
- Token 过期时间
- 速率限制
- CORS 配置

## 开发

```bash
# 安装依赖
npm install

# 启动开发服务器
npm run dev

# 构建生产版本
npm run build

# 预览生产版本
npm run preview

# 代码检查
npm run lint
```

## API 集成

前端通过 REST API 与后端通信：

```typescript
// 示例：获取 Agent 列表
const { data } = useQuery({
  queryKey: ['agents'],
  queryFn: async () => {
    const response = await fetch('/api/v1/agents');
    return response.json();
  },
});
```

## 认证

使用 JWT Token 进行认证：

```typescript
// 登录后存储 token
localStorage.setItem('access_token', token);

// 路由守卫检查 token
const PrivateRoute = ({ children }) => {
  const token = localStorage.getItem('access_token');
  return token ? children : <Navigate to="/login" />;
};
```

## 环境变量

创建 `.env.local` 文件：

```env
VITE_API_BASE_URL=http://localhost:8080
VITE_WS_URL=ws://localhost:8080/ws
```

## 构建部署

```bash
# 构建
npm run build

# 输出到 dist/ 目录
# 部署到任意静态文件服务器
```

## 浏览器支持

- Chrome (最新版)
- Firefox (最新版)
- Safari (最新版)
- Edge (最新版)
