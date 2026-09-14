# CooService WebUI

技术栈：TypeScript + pnpm + Vue 3 + Nuxt UI 4（Tailwind v4）+ vue-router，构建成纯静态文件。

## 开发

```bash
pnpm install
pnpm dev                  # http://127.0.0.1:5173
```

开发服务器不代理 `/api`：API 地址由界面「设置」框配置（浏览器直连，靠后端 CORS 放行）。
本地后端在 `http://127.0.0.1:8081` 时，首次请求失败后弹出的「API 设置」框里填入该地址即可；
地址留空 = 同源。

## 构建

```bash
pnpm typecheck            # vue-tsc --noEmit
pnpm build                # vue-tsc --noEmit && vite build，产物在 dist/
pnpm preview
```

纯静态产物，无构建期注入配置；API 地址与密钥全部运行时在界面配置。

## 路由

| 路径 | 页 |
|---|---|
| `/` | 重定向到 `/apps` |
| `/apps` | 应用列表 |
| `/apps/:id` | 应用详情（左侧栏：概览 / 发版通道 / 资源 / 公告） |
| `/pools` | 资源池 |
| `/resources` | 资源 |
| `/announces` | 公告 |

可直达、可刷新（Caddy 的 `try_files {path} /index.html` 兜底）；未知路径回落 `/apps`。

## 目录

```
src/
  api/            # types.ts（接口信封类型）与 client.ts（fetch 封装、错误中文映射）
  components/     # HashChip / ConfirmDialog / ApiSetupDialog / ResourcePicker / ChannelDetail
  composables/    # useAsync / useSession / useConfirm
  lib/            # format.ts（哈希与字节格式化）、announce.ts（公告状态与有效期）
  pages/          # 六个路由页
  App.vue         # 顶栏导航 + RouterView + 密钥门
  router.ts, main.ts
```

## 页面

| 页 | 用到 |
|---|---|
| 应用 | `admin/apps` 增删改查（需 Admin Key） |
| 发版通道 | `admin/channels` 增删改查（需 Admin Key）；详情预阅览 `app/channels` |
| 资源池 | `admin/pools` 增删改查（需 Admin Key） |

## API 设置框

首页直接渲染业务页。任意请求地址不可达（或返回的不是 CooService API），或管理接口返回
401，都会弹出全屏「API 设置」框，需同屏配置 API 地址与管理密钥：

- API 地址：留空 = 同源；也可填完整地址（`http(s)://` 开头），浏览器直连
- 管理密钥：与部署方 `ADMIN_KEY` 环境变量一致
- 提交即探活（`/api/v1/admin/apps`），地址错聚焦地址框、密钥错聚焦密钥框，验证成功才放行
- 右上角「设置」按钮可随时主动修改

地址与密钥都存 `sessionStorage`，关标签页失效；「断开」只清密钥，地址保留。
服务重启会换密钥，取当前值：`podman logs coo-router | grep 'X-Admin-Key'`（docker compose 同理）。

资源池的 `secret` 永不回显，只能重设。

## 主题

Nuxt UI 4 默认主题：浅色为默认，深色由 `@nuxt/ui/vue-plugin` 内置的 color-mode 跟随系统
`prefers-color-scheme`（在 `<html>` 上切 `.dark`），无需额外配置。

## 部署

`dist/` 是静态文件，扔到任意静态服务器或对象存储即可。
跨域由后端的 `CorsLayer::permissive()` 放行。
