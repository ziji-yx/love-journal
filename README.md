# 我们的手账

一个只属于两个人的私密恋爱手账。后端使用 Rust + Axum + SQLite，前端是原生 HTML、CSS 和 JavaScript，不需要 Node.js 构建步骤。

## 功能

- 口令登录与自动登录
- 按日期记录共同回忆
- 首页统计在一起的天数和下一个纪念日
- 时间轴与月历浏览
- 照片上传、预览与删除
- 留言和贴纸
- 冲突保护，防止两个人同时修改同一页时互相覆盖

## 环境要求

- Rust 1.85 或更高版本

安装依赖并首次启动只需要执行：

```powershell
cargo run
```

启动后打开 <http://127.0.0.1:8080>。

## 配置

项目会读取根目录的 `.env`。首次使用时可以复制示例文件：

```powershell
Copy-Item .env.example .env
```

需要修改的配置：

```dotenv
APP_PASSWORD=换成你们自己的口令
LOVE_START=2025-10-02
HOST=127.0.0.1
PORT=8080
DATABASE_URL=sqlite://data/journal.db
UPLOAD_DIR=data/uploads
RUST_LOG=love_journal=info,tower_http=info
```

- `APP_PASSWORD`：登录口令，至少 8 个字符。默认只监听本机；如果需要对外监听，请先换成强口令。
- `LOVE_START`：在一起的日期，格式为 `YYYY-MM-DD`。
- `HOST`：默认 `127.0.0.1`。只有需要在局域网内访问时才改成 `0.0.0.0`。
- `DATABASE_URL` 和 `UPLOAD_DIR`：数据会保存在项目的 `data` 目录下。

## 发布版本

```powershell
cargo build --release
.\target\release\love-journal.exe
```

数据库和上传的照片都保存在 `data` 目录。备份时请一起备份 `data/journal.db` 和 `data/uploads`。

## 开发检查

```powershell
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features
```

修改端口后，访问地址中的端口也要同步修改。若启动失败，最常见的原因是端口被占用或 `.env` 中的日期格式不正确。
