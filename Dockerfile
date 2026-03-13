# 多阶段构建，减小最终镜像大小
# 构建阶段
FROM rust:1.94-slim as builder

WORKDIR /build

# 安装构建依赖
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# 复制 Cargo 配置
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

# 构建 release 版本
RUN cargo build --release

# 运行阶段
FROM debian:bookworm-slim

# 安装运行时依赖
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    sqlite3 \
    && rm -rf /var/lib/apt/lists/* \
    && update-ca-certificates

# 创建非 root 用户
RUN useradd -m -u 1000 -s /bin/bash -c /app uhorse

# 创建目录
RUN mkdir -p /app/data /app/logs /app/config

# 从构建阶段复制二进制文件
COPY --from=builder /build/target/release/uhorse /app/uhorse

# 设置权限
RUN chown -R uhorse:uhorse /app

# 切换到非 root 用户
USER uhorse
WORKDIR /app

# 暴露端口
EXPOSE 8080 9090

# 健康检查
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# 数据目录
VOLUME ["/app/data", "/app/logs"]

# 设置环境变量
ENV RUST_LOG=info \
    UHORSE_CONFIG=/app/config/config.toml \
    UHORSE_DATA_DIR=/app/data \
    UHORSE_LOG_DIR=/app/logs

# 启动应用
CMD ["/app/uhorse"]
