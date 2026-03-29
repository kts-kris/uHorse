# uHorse Playground

30 秒快速体验 uHorse AI Gateway。

## 🚀 快速开始

### 方式一：Docker Compose（推荐）

```bash
# 进入 playground 目录
cd playground

# 启动服务
docker-compose up -d

# 访问
open http://localhost:8080
```

### 方式二：Docker 直接运行

```bash
# 构建镜像
docker build -f playground/Dockerfile.playground -t uhorse/playground:latest .

# 运行容器
docker run -it --rm -p 8080:8080 uhorse/playground:latest
```

### 方式三：使用预构建镜像

```bash
docker run -it --rm -p 8080:8080 ghcr.io/uhorse/uhorse:playground
```

## ⚙️ 配置 LLM

### 使用 OpenAI

```bash
docker run -it --rm -p 8080:8080 \
  -e LLM_PROVIDER=openai \
  -e LLM_API_KEY=sk-your-api-key \
  -e LLM_MODEL=gpt-4 \
  uhorse/playground:latest
```

### 使用 Anthropic Claude

```bash
docker run -it --rm -p 8080:8080 \
  -e LLM_PROVIDER=anthropic \
  -e LLM_API_KEY=sk-ant-xxx \
  -e LLM_MODEL=claude-3-sonnet-20240229 \
  uhorse/playground:latest
```

### 使用本地 Ollama

```bash
docker run -it --rm -p 8080:8080 \
  -e LLM_PROVIDER=ollama \
  -e LLM_BASE_URL=http://host.docker.internal:11434 \
  -e LLM_MODEL=llama2 \
  --add-host=host.docker.internal:host-gateway \
  uhorse/playground:latest
```

## 🎮 功能体验

启动后访问 http://localhost:8080，你可以：

1. **基础对话**
   - 发送 "你好"
   - 发送 "介绍一下你自己"

2. **工具调用**
   - 发送 "现在几点？"
   - 发送 "123 * 456 等于多少？"
   - 发送 "帮我翻译 Hello World"

3. **上下文记忆**
   - 告诉它你的名字
   - 下一条消息问 "我叫什么名字？"

## 📁 目录结构

```
playground/
├── Dockerfile.playground  # Docker 构建文件
├── docker-compose.yml     # Docker Compose 配置
├── entrypoint.sh          # 入口脚本
├── config.toml            # Playground 配置
├── workspace/             # Agent 工作空间
│   └── default/
│       └── SOUL.md        # Agent 人设
└── README.md              # 本文档
```

## ⚠️ 注意事项

- **数据不持久**: 默认配置下，容器重启后数据会丢失
- **Mock 模式**: 默认使用 Mock LLM，响应为预设内容
- **仅供测试**: 不建议在生产环境使用 Playground 配置

## 🔗 相关链接

- [完整文档](https://uhorse.ai/docs)
- [GitHub](https://github.com/kts-kris/uHorse)
- [问题反馈](https://github.com/kts-kris/uHorse/issues)
