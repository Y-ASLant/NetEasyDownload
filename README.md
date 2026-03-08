# NetEasyDownload

<div align=”center”>

**基于 Rust + ratatui 的网易云音乐终端下载工具**

[![Rust](https://img.shields.io/badge/Rust-1.70+-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

</div>

## ✨ 特性

- 🔍 **关键词搜索**：快速搜索歌曲，支持歌名、歌手名
- 📥 **高音质下载**：支持 标准 / 极高 / 无损 / Hi-Res 四种音质
- 🔐 **短信验证码登录**：安全便捷的登录方式，自动保存 Cookie
- 🎨 **终端 UI**：基于 ratatui 的现代化终端界面
- ⚡ **异步下载**：带进度条的文件下载，实时显示下载状态
- 💾 **自动保存**：登录信息自动保存到本地，无需重复登录

## 📦 安装

### 前置要求

- Rust 1.70 或更高版本
- Cargo（Rust 包管理器）

### 从源码构建

```bash
# 克隆仓库
git clone https://github.com/Y-ASLant/NetEasyDownload.git
cd NetEasyDownload

# 构建项目
cargo build --release

# 运行
cargo run --release
```

## 🚀 快速开始

### 基础使用

```bash
# 直接运行（首次使用需要登录）
cargo run --release
```

### 使用环境变量

```bash
# Windows PowerShell
$env:NETEASYDOWNLOAD_COOKIE='你的cookie'
$env:NETEASYDOWNLOAD_LEVEL='hires'
cargo run --release

# Linux/macOS
export NETEASYDOWNLOAD_COOKIE='你的cookie'
export NETEASYDOWNLOAD_LEVEL='hires'
cargo run --release
```

### 环境变量说明

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `NETEASYDOWNLOAD_COOKIE` | 网易云登录 Cookie（可选） | 无 |
| `NETEASYDOWNLOAD_LEVEL` | 默认音质等级 | `hires` |

兼容性说明：当前版本仍兼容旧变量名 `CLOUDX_COOKIE` / `CLOUDX_LEVEL`。

**音质等级**：
- `standard`：标准音质（128kbps）
- `exhigh`：极高音质（320kbps）
- `lossless`：无损音质（FLAC）
- `hires`：Hi-Res 高解析度音质

## 📖 使用指南

### 登录流程

1. **启动程序**：首次运行会进入登录界面
2. **输入手机号**：输入 11 位手机号，按 `Enter` 发送验证码
3. **输入验证码**：收到短信后输入验证码，按 `Enter` 完成登录
4. **自动保存**：登录成功后 Cookie 自动保存到 `.neteasydownload_cookie` 文件

### 主界面操作

#### 键盘快捷键

| 按键 | 功能 |
|------|------|
| `Enter` | 搜索歌曲 |
| `↑` / `↓` | 选择歌曲 |
| `d` | 下载选中的歌曲 |
| `1` | 切换到标准音质 |
| `2` | 切换到极高音质 |
| `3` | 切换到无损音质 |
| `4` | 切换到 Hi-Res 音质 |
| `q` | 退出程序 |

#### 搜索歌曲

1. 在输入框输入歌曲名或歌手名
2. 按 `Enter` 开始搜索
3. 使用 `↑` / `↓` 键选择歌曲
4. 按 `d` 键下载选中的歌曲

#### 下载管理

- 下载的文件保存在当前运行目录的 `downloads/` 目录
- 文件名格式：`歌曲名-歌手名.扩展名`
- 支持下载进度显示
- 自动过滤文件名中的非法字符

## 🔧 技术细节

### 项目架构

```
src/main.rs    # 应用入口、事件循环、任务调度
src/client.rs  # HTTP 客户端，封装网易云 API
src/models.rs  # 应用状态与数据模型
src/crypto.rs  # 网易云 eapi 接口加密
src/ui.rs      # ratatui 终端界面渲染
```

### 核心依赖

- **ratatui**：终端 UI 框架
- **crossterm**：跨平台终端控制
- **reqwest**：HTTP 客户端
- **aes + ecb**：AES-128-ECB 加密
- **serde_json**：JSON 解析
- **anyhow**：错误处理

### 网易云 API 说明

本项目使用网易云音乐的 `eapi` 接口：
- 请求和响应均使用 AES-128-ECB 加密
- 加密密钥：`e82ckenh8dichen8`
- 需要模拟网易云桌面客户端的 User-Agent
- 登录接口需要特定的设备指纹信息

## ❓ 常见问题

### Q: 为什么需要登录？

A: 部分高音质歌曲（极高/无损/Hi-Res）需要登录后才能下载。登录后可以获取更完整的音乐资源。

### Q: Cookie 保存在哪里？

A: Cookie 保存在程序所在目录下的 `.neteasydownload_cookie` 文件中。优先级：环境变量 > 文件 > 未设置。当前也兼容读取旧的 `.cloudx_cookie` 文件。

### Q: 下载失败怎么办？

A: 可能的原因：
1. 歌曲需要 VIP 权限
2. 歌曲在当前地区不可用
3. 网络连接问题
4. Cookie 已过期（重新登录即可）

### Q: 支持歌词下载吗？

A: 当前版本专注于音频下载，暂不支持歌词、封面、标签等功能。

### Q: 可以批量下载歌单吗？

A: 当前版本仅支持单曲下载，歌单批量下载功能计划在后续版本中添加。

## 🛠️ 开发计划

- [ ] 歌词下载和嵌入
- [ ] 封面下载和嵌入
- [ ] 歌单批量下载
- [ ] 下载历史记录
- [ ] 配置文件支持
- [ ] 多线程并发下载

## 📄 许可证

本项目仅供学习交流使用，请勿用于商业用途。

## ⚠️ 免责声明

本项目仅供学习和研究使用，请勿用于非法用途。使用本工具下载的音乐文件仅供个人学习使用，请支持正版音乐。

## 🙏 致谢

- [ratatui](https://github.com/ratatui-org/ratatui) - 优秀的终端 UI 框架
- [NeteaseCloudMusicApi](https://github.com/Binaryify/NeteaseCloudMusicApi) - 网易云音乐 API 参考
