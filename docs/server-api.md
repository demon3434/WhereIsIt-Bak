# API 协议说明

本文档按当前 Rust 客户端 `client-win32-rust/src/main.rs` 中实际调用的接口整理。

## 健康检查
- `GET /api/health`
- `GET /health`

说明：
- 客户端优先请求 `/api/health`，失败时回退到 `/health`。

## 认证
- `POST /api/auth/login`
- `GET /api/auth/me`

## 任务
- `GET /api/tasks/{taskId}`

## 数据库备份/恢复
- `POST /api/backup/database/preflight`
- `POST /api/backup/database`
- `GET /api/backup/database/{taskId}/download`
- `GET /api/backup/database/{taskId}/metadata`
- `POST /api/restore/database/upload`
- `POST /api/restore/database/preflight`
- `POST /api/restore/database`

## 图片目录备份/恢复
- `POST /api/backup/uploads/create-manifest`
- `GET /api/backup/uploads/file/{fileId}`
- `POST /api/restore/uploads/create-task`
- `POST /api/restore/uploads/{taskId}/upload-file`
- `POST /api/restore/uploads/{taskId}/complete`

## 说明
- 本仓库当前仅保留客户端代码；服务端已独立部署，不再包含在仓库中。
- 如果服务端接口发生变动，请以客户端源码中的实际请求路径为准同步更新本文档。
