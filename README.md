# WhereIsIt-Bak

`WhereIsIt-Bak` 是一个 Windows 客户端工具，用来连接已部署的服务端，执行数据库和上传目录的备份与恢复。

## 功能
- 数据库备份
- 数据库恢复
- 上传目录备份
- 上传目录恢复

## 使用前准备
1. 准备好可访问的服务端地址。
2. 将 `config.json.example` 复制为 `config.json`。
3. 按实际环境修改 `config.json`。

最小配置示例：

```json
{
  "serverBaseUrl": "http://192.168.7.186:3000",
  "adminUsername": "admin",
  "token": "",
  "timeoutSeconds": 60,
  "defaultBackupRoot": "D:\\system-backups",
  "modules": {
    "database": {
      "dbName": "whereisit"
    }
  }
}
```

## 如何使用 EXE
1. 将编译好的 `exe` 文件和 `config.json` 放在同一目录。
2. 双击启动 `exe`。
3. 在登录窗口填写或确认服务端地址、管理员用户名和密码。
4. 登录成功后进入主界面。
5. 在 `备份` 页执行数据库备份或图片备份。
6. 在 `恢复` 页执行数据库恢复或图片恢复。

## 备份与恢复说明
- 数据库备份文件会下载到 `defaultBackupRoot` 指定目录。
- 上传目录备份默认保存到 `defaultBackupRoot\\uploads`。
- 数据库恢复会覆盖目标库内容，执行前请确认环境正确。
- 图片恢复支持覆盖策略选择。

## 注意事项
- 本仓库只包含客户端代码，不包含服务端源码。
- EXE 必须连接一个已部署且接口兼容的服务端才能工作。
- 如果服务端地址或数据库名不对，相关操作会失败。

## 相关文档
- [API 协议说明](docs/server-api.md)
- [Uploads Manifest 说明](docs/uploads-manifest.md)
