# SnapX

> ⚡ 轻量高效的截图 + 标注 + 贴图工具 — PixPin 的开源替代品

![Tauri v2](https://img.shields.io/badge/Tauri-v2-blue?logo=tauri)
![Rust](https://img.shields.io/badge/Rust-1.77+-orange?logo=rust)
![Svelte 5](https://img.shields.io/badge/Svelte-5-FF3E00?logo=svelte)
![License](https://img.shields.io/badge/License-MIT-green)

## ✨ 特性

- 📸 **极速截图** — 快捷键一按即截，零等待
- 🎨 **强大标注** — 矩形、圆形、箭头、文字、序号、马赛克、荧光笔、画笔
- 📌 **贴图参考** — 截图贴在屏幕上，随时参考
- ⚡ **极致性能** — Tauri v2 + Rust，内存占用低，启动快
- 💚 **免费开源** — 社区驱动，永久免费，无功能阉割

## 🛠 技术栈

| 层级 | 技术 |
|------|------|
| 框架 | Tauri v2 |
| 后端 | Rust |
| 前端 | Svelte 5 + TypeScript |
| 样式 | TailwindCSS 4 |
| 构建 | Vite |
| 截图引擎 | xcap |

## 📦 项目结构

```
snapx/
├── src/                          # Svelte 前端
│   ├── App.svelte                # 主入口（双窗口路由）
│   ├── main.ts                   # 前端启动
│   ├── app.css                   # TailwindCSS 入口
│   └── lib/components/
│       ├── CaptureOverlay.svelte  # 截图覆盖层（区域选择）
│       ├── SizeIndicator.svelte   # 选区尺寸显示
│       └── Toolbar.svelte         # 操作工具栏
├── src-tauri/                    # Rust 后端
│   ├── src/
│   │   ├── main.rs               # 应用入口
│   │   ├── lib.rs                # Tauri 配置（托盘、快捷键、窗口）
│   │   ├── capture.rs            # 截图引擎（xcap 封装）
│   │   └── commands.rs           # Tauri 命令（前端调用）
│   ├── capabilities/
│   │   └── default.json          # 窗口权限声明
│   ├── icons/                    # 应用图标
│   ├── Cargo.toml                # Rust 依赖
│   └── tauri.conf.json           # Tauri 配置
├── index.html                    # HTML 入口
├── vite.config.ts                # Vite 构建配置
├── svelte.config.js              # Svelte 配置
├── tsconfig.json                 # TypeScript 配置
└── package.json                  # Node.js 依赖
```

## 🚀 快速开始

### 前置要求

- [Node.js](https://nodejs.org/) 18+
- [pnpm](https://pnpm.io/) 8+
- [Rust](https://www.rust-lang.org/tools/install) 1.77+
- [Tauri v2 Prerequisites](https://v2.tauri.app/start/prerequisites/)

### 安装依赖

```bash
# 安装前端依赖
pnpm install

# 安装 Rust 依赖（首次构建时自动安装）
cd src-tauri && cargo build
```

### 开发模式

```bash
pnpm tauri dev
```

### 构建发布

```bash
pnpm tauri build
```

## 🎮 使用方式

1. **启动后常驻系统托盘** — 托盘图标右键菜单操作
2. **快捷键截图** — `Ctrl+Shift+A` 触发区域截图
3. **区域选择** — 鼠标拖拽框选，松手确认
4. **双击全屏** — 双击截图全屏
5. **ESC / 右键取消**
6. **自动保存到剪贴板**

## 📋 开发计划

详见 [ROADMAP.md](./ROADMAP.md)

## 🤝 贡献

欢迎 PR！请确保：

1. 代码通过 `cargo clippy` 检查
2. 前端代码通过 `pnpm check`（svelte-check）
3. 提交信息遵循 [Conventional Commits](https://www.conventionalcommits.org/)

## 📄 许可证

MIT License（待确认）

---

*SnapX — 让截图更简单*
