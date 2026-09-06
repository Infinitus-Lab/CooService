#!/bin/sh
# caddy 容器启动入口：把运行时 API 基址物化成 /usr/share/caddy/config.js 后启动 caddy。
# 前端运行时读 window.API_BASE_URL；本地 vite dev 场景没有此文件，走构建期 VITE_BASE_API。
set -eu

# 运行时 API_BASE_URL 优先，回退镜像构建期注入的 VITE_BASE_API（可能为空 = 同源）
api="${API_BASE_URL:-${VITE_BASE_API:-}}"
escaped=$(printf '%s' "$api" | sed 's/"/\\"/g')
printf 'window.API_BASE_URL="%s";\n' "$escaped" > /usr/share/caddy/config.js

exec caddy run --config /etc/caddy/Caddyfile --adapter caddyfile