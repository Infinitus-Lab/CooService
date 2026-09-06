# 资源以及更新服务设计

> **已废弃（2026-09）**：本稿为早期设计。池配置列分工（§4.1）、WebUI 配置
> 方式（API 地址/密钥在界面「设置」框输入）、部署拓扑（§7）等均以 [architecture.md](./architecture.md) 为准；
> 保留仅作文档沿革，不再追更加新。

## API 服务

技术栈：Rust + axum + tokio + PostgreSQL

### 路由层

负责对传入的连接进行路由，确定是自动化处理、资源访问、管理访问。

### 应用管理器

管理 App 的创建删除以及信息读写

### 一个应用所拥有的东西

- AppId
- Name
- Channels
- Resources
- Announces

### 资源池

抽象 S3 FTP 等远程服务为哈希池

只需要管理终结点、密钥以及公开终结点三个配置即可

## WebUI

WebUI 界面，用于方便访问管理 API。需要单独编译成静态文件额外部署，或者在本地使用。API 地址与管理密钥在界面「设置」框由使用者输入（非环境变量注入）。

技术栈：TypeScript + pnpm + React + daisyui
