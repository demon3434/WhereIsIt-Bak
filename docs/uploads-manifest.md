# Uploads Manifest 说明

manifest 核心字段：
- `fileId`
- `relativePath`
- `size`
- `sha256`
- `modifiedAt`
- `downloadUrl`
- 可选：`etag`、`objectKey`、`supportsRange`

客户端行为：
- 获取 manifest 后并发下载单文件。
- 下载先写 `.part`，完成后 rename。
- 可按失败项重试。

恢复行为：
- 本地扫描目录并并发上传。
- 服务端按覆盖策略处理，逐文件记录状态。
