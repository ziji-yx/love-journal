# 我们的手账

一个只属于两个人的私密恋爱手账，适合用作纪念日礼物。后端使用 Rust + Axum + SQLite，前端使用原生 HTML、CSS 和 JavaScript，不需要 Node.js 构建步骤。

支持 Windows、macOS 和 Linux。Docker 部署是可选功能，本地运行不依赖 Docker。

## 功能

- 口令登录与自动登录
- 按日期记录共同回忆
- 首页统计在一起的天数、手账数、照片数和留言数
- 时间轴、月历、关键词搜索和随机回忆
- 照片上传、预览、说明文字和删除
- 留言、贴纸和删除留言
- 自定义纪念日和首页倒计时
- 一周年年度回顾：月份记录、第一篇、最近一篇、照片墙和时间胶囊回声
- 时间胶囊：写给未来的信，可以设置开启日期
- 时间胶囊支持文字和声音一起封存，到期前服务端不会返回正文或音频
- 时间胶囊到期提醒：首页提醒条、导航计数和可选的浏览器系统通知
- 声音日记：每篇手账都可以录音或上传音频
- 深色模式
- 冲突保护，防止两个人同时修改同一篇手账时互相覆盖

## 技术栈

- Rust 1.98 或更高版本
- Axum
- SQLx + SQLite
- 原生 HTML / CSS / JavaScript
- Docker + Caddy，可选 Cloudflare Tunnel

## 目录说明

```text
src/                 Rust 后端
static/              前端页面、样式和脚本
migrations/          数据库迁移，启动时自动执行
data/                本地数据库和上传文件，不提交到 Git
Dockerfile           应用镜像
compose.yaml         Docker + Caddy 部署
compose.tunnel.yaml  Docker + Cloudflare Tunnel 部署
Caddyfile            Caddy 配置
```

## 运行要求

- Rust 1.98 或更高版本
- 任意现代浏览器
- 如果要使用麦克风录音，需要允许浏览器访问麦克风
- Docker 仅在服务器部署时需要

检查 Rust 是否安装：

```bash
rustc --version
cargo --version
```

如果提示找不到命令，请先安装 Rust 工具链。

推荐通过 <https://rustup.rs/> 安装 Rust。不同系统可能还需要基础编译工具：

- Windows：安装 Rust MSVC 工具链和 Visual Studio C++ Build Tools。
- macOS：运行 `xcode-select --install` 安装 Xcode Command Line Tools。
- Linux：安装 `build-essential`、`pkg-config` 等基础构建工具，具体包名取决于发行版。

## 本地运行

在项目根目录打开终端。

### 1. 创建配置文件

macOS / Linux：

```bash
cp .env.example .env
```

Windows PowerShell：

```powershell
Copy-Item .env.example .env
```

也可以手动创建 `.env` 文件。

### 2. 修改配置

至少设置下面两项：

```dotenv
APP_PASSWORD=换成你们自己的口令
LOVE_START=2025-10-02
```

默认配置如下：

```dotenv
HOST=127.0.0.1
PORT=8080
DATABASE_URL=sqlite://data/journal.db
UPLOAD_DIR=data/uploads
COOKIE_SECURE=false
RUST_LOG=love_journal=info,tower_http=info
```

### 3. 启动开发版本

所有系统都使用同一个 Cargo 命令：

```bash
cargo run
```

按 `Ctrl+C` 停止服务。

### 4. 打开浏览器

默认访问地址：

```text
http://127.0.0.1:8080
```

这里的地址不是固定的，由 `.env` 中的 `HOST` 和 `PORT` 决定：

- `HOST=127.0.0.1`：只有本机可以访问，本机使用 `http://127.0.0.1:端口`。
- `HOST=0.0.0.0`：本机仍可用 `127.0.0.1`，其他设备使用这台电脑的局域网 IP 或公网 IP。
- `PORT=9000`：访问地址变成 `http://127.0.0.1:9000`。
- Docker + Caddy 或 Cloudflare Tunnel 部署时，外部访问的是域名，不是容器内部的 `8080`。

## 构建发布版本

所有系统都使用：

```bash
cargo build --release
```

构建完成后，发布文件的路径不同：

macOS / Linux：

```bash
./target/release/love-journal
```

Windows PowerShell：

```powershell
.\target\release\love-journal.exe
```

Windows CMD：

```bat
target\release\love-journal.exe
```

程序会自动定位项目根目录，因此从项目根目录启动时可以正确读取 `static`、`.env` 和 `data`。

## 配置说明

```dotenv
APP_PASSWORD=换成你们自己的口令
LOVE_START=2025-10-02
HOST=127.0.0.1
PORT=8080
DATABASE_URL=sqlite://data/journal.db
UPLOAD_DIR=data/uploads
COOKIE_SECURE=false
RUST_LOG=love_journal=info,tower_http=info
```

- `APP_PASSWORD`：登录口令，至少 8 个字符。
- `LOVE_START`：在一起的日期，格式为 `YYYY-MM-DD`。
- `HOST`：默认 `127.0.0.1`，只允许本机访问。局域网访问时改成 `0.0.0.0`，同时必须使用强口令并设置系统防火墙。
- `PORT`：监听端口，默认 `8080`。
- `DATABASE_URL`：SQLite 数据库地址。
- `UPLOAD_DIR`：照片和声音文件的保存目录。
- `COOKIE_SECURE`：使用 HTTPS 时设为 `true`；仅本机 HTTP 使用时设为 `false`。
- `RUST_LOG`：日志级别。

## 数据与备份

所有个人数据都在 `data` 目录：

```text
data/journal.db      SQLite 数据库
data/uploads/        照片和声音文件
```

备份时请同时复制数据库和上传目录。不要只备份其中一项。

## 局域网使用

如果想让同一 Wi-Fi 下的手机或另一台电脑访问：

1. 把 `.env` 中的 `HOST` 改成 `0.0.0.0`。
2. 把 `APP_PASSWORD` 改成强口令。
3. 在操作系统防火墙中放行对应 TCP 端口，默认是 `8080`。
4. 找到这台电脑的局域网 IP，在其他设备上访问：

```text
http://这台电脑的局域网IP:8080
```

查看局域网 IP 的方式：

macOS：

```bash
ipconfig getifaddr en0
```

Linux：

```bash
hostname -I
```

Windows PowerShell：

```powershell
ipconfig
```

不要在没有强口令的情况下把服务直接暴露到公网。

## Docker + Caddy 部署

适用于有域名、有公网 IP 的云服务器。Caddy 会自动申请 HTTPS 证书。

1. 准备域名并添加 A 记录，指向服务器公网 IP。
2. 云服务器安全组开放 `80`、`443`，不要向公网开放应用内部的 `8080`。
3. 在服务器项目目录创建生产配置：

macOS / Linux：

```bash
cp .env.production.example .env.production
```

Windows PowerShell：

```powershell
Copy-Item .env.production.example .env.production
```

4. 编辑 `.env.production`，至少修改：

```dotenv
DOMAIN=love.your-domain.com
APP_PASSWORD=replace-with-a-long-random-password
LOVE_START=2025-10-02
COOKIE_SECURE=true
```

5. 启动：

```bash
docker compose --env-file .env.production up -d --build
```

6. 打开 `https://你的域名`。

数据保存在 Docker 卷 `journal_data` 中。备份示例：

```bash
docker run --rm -v love-journal_journal_data:/data -v "$PWD/backup:/backup" alpine tar czf /backup/journal.tar.gz -C /data .
```

Windows PowerShell 中 `$PWD` 也可以使用，若路径包含空格，请用引号包住挂载路径。

## Cloudflare Tunnel 部署

适合没有公网 IP，或不想在路由器上开放端口的场景。服务器只主动向 Cloudflare 建立出站连接，不需要开放 80/443，也不需要启动 Caddy。

1. 在 Cloudflare 中添加并托管域名。
2. 在 Zero Trust 控制台创建 Tunnel，复制 token。
3. 在 Public Hostname 中添加域名，Service 填 `http://app:8080`。
4. 创建生产配置：

```bash
cp .env.production.example .env.production
```

Windows PowerShell 使用：

```powershell
Copy-Item .env.production.example .env.production
```

至少修改 `APP_PASSWORD`、`LOVE_START` 和 `CLOUDFLARE_TUNNEL_TOKEN`，保持 `COOKIE_SECURE=true`。

5. 启动：

```bash
docker compose -f compose.tunnel.yaml --env-file .env.production up -d --build
```

建议在 Cloudflare Zero Trust 中再给域名加一条 Access 策略，例如邮箱一次性验证码，形成双重保护。

## 时间胶囊与声音提醒

- 新建时间胶囊时可以填写标题、正文、署名和开启日期。
- 时间胶囊支持直接录音，也可以上传音频。
- 未到开启日期时，正文不会返回给前端，音频地址也无法访问。
- 到期后，首页会出现提醒条，时间胶囊导航也会显示数量。
- 点击“去查看”后，信件会标记为已查看，并从提醒列表中消失。
- 浏览器允许通知时，可以点击“开启浏览器提醒”接收系统通知。
- 年度回顾只收录已经到达开启日期的时间胶囊，并展示其中的声音。

## 开发检查

所有系统都可以使用：

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features
```

如果本机安装了 Node.js，还可以检查前端脚本：

```bash
node --check static/js/app.js
node --check static/js/features.js
```

## 安全说明

- 默认只监听 `127.0.0.1`。
- 对外监听前必须更换示例口令。
- 登录失败会触发临时限流，降低公网口令暴力尝试风险。
- 会话保存在 SQLite 中，服务重启后登录状态仍有数据库记录。
- 浏览器会话优先使用 `HttpOnly` Cookie，不再把登录令牌长期保存在 `localStorage`。
- 照片和声音接口都需要登录。
- 照片和声音会根据文件签名校验类型，私密媒体响应使用 `no-store`，避免退出后仍从浏览器缓存直接打开。
- 时间胶囊的锁定逻辑在服务端执行，不是只靠前端隐藏。
- 公网部署建议使用 HTTPS、Cloudflare Access 或 Tailscale 等额外访问控制。
- 使用麦克风时必须由浏览器授权；如果不授权，可以改为上传音频文件。

## 产品定位

这个项目适合两个人长期记录共同生活，不适合作为多人公开社区。SQLite 足以承担两个人的日常使用，部署时优先做好数据备份和隐私保护。
