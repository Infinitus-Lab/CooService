# CooService WebUI

技术栈：TypeScript + pnpm + React + daisyUI（Tailwind v4），构建成纯静态文件。

## 开发

```bash
pnpm install
cp .env.example .env      # 改 VITE_BASE_API 指向后端
pnpm dev                  # http://127.0.0.1:5173
```

开发服务器把 `/api` 代理到 `VITE_BASE_API`，所以浏览器里也是同源请求。

## 构建

```bash
pnpm build                # 产物在 dist/
pnpm preview
```

`VITE_BASE_API` 是**构建期**注入的（`import.meta.env`），改地址要重新构建，
不能靠运行时环境变量覆盖。留空则请求同源，适合和后端一起部署。

## 页面

| 页 | 用到 |
|---|---|
| 应用 | `admin/apps` 增删改查（需 Admin Key） |
| 发版通道 | `admin/channels` 增删改查（需 Admin Key）；详情预阅览 `app/channels` |
| 资源池 | `admin/pools` 增删改查（需 Admin Key） |

## 密钥门

进入管理台前是全屏的密钥输入页：填 Admin Key → 调 `/api/v1/admin/apps` 探活（无/错密钥返回 401，退回密钥门）→ 通过才进主界面。
任何接口返回 401（服务重启换了密钥、密钥填错）都会立刻退回密钥门。
密钥存 `sessionStorage`，关标签页失效，也可以点右上角「断开」清除。
服务重启会换密钥，取当前值：`podman logs coo-router | grep 'X-Admin-Key'`（docker compose 同理）。

资源池的 `secret` 永不回显，只能重设。

## 部署

`dist/` 是静态文件，扔到任意静态服务器或对象存储即可。
跨域由后端的 `CorsLayer::permissive()` 放行。
