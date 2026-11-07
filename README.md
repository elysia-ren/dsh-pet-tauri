# dsh-pet Tauri 独立桌宠（第三方打包，非官方）

> **重要说明：本项目不是 dsh-pet 官方项目。**
>
> 这只是把开源项目 **[dsh-pet](https://github.com/PC2005-cloud/dsh-pet)**（MIT License，
> Copyright (c) 2026 PC2005-cloud）的动画/表情/配置素材，用 **Tauri** 重新打包成
> Windows 独立桌面应用。桌宠素材、角色形象和原项目代码的版权归原作者；
> 本项目只提供 Tauri 壳、Windows 打包和少量独立实现，不是官方发行版。
>
> 原项目：https://github.com/PC2005-cloud/dsh-pet
> 原项目协议：MIT（见 [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md)）
> 界面字体：站酷快乐体 ZCOOL KuaiLe（SIL OFL 1.1）

把 dsh-pet 从 DSH 插件里「提出来」的独立桌面应用：**不打开 DeepSeek Harness**，双击就能养一只透明置顶的桌宠。
动画、点击 Q 弹、拖拽、右键菜单、余额、碎碎念、对话、记忆、设置页都已经接上；LLM 直连任意 OpenAI 兼容端点（DeepSeek / OpenAI / 本地 Ollama）。

## 功能

- **桌宠**：透明、无边框、置顶、跳过任务栏；VP9-alpha 视频透明播放；**同步 dsh-pet 0.4.5，114 段动画全量内嵌**
- **界面字体**：站酷快乐体（ZCOOL KuaiLe，SIL OFL 1.1，可免费随软件分发）
- **安装包**：中文 NSIS 向导、桌宠自定义图标/侧边图，不再是默认 NSIS 外观
- **单文件 exe**：使用桌宠图标（资源管理器/任务栏）
- **自主行为**：按权重随机链（待机/转向/走动/随机动作分类），**会自己在桌面上走过去**
- **点击**：Q 弹（压扁回弹）+ 随机点击动作 + **彩色粒子爆发**；等第一帧解码后才交叉淡入，无白闪
- **拖拽 / 甩抛**：rAF 每帧跟手；松手按速度甩出去，带重力、边缘反弹、地面摩擦、落地 Q 弹；**碰撞按可见身体算，能贴到屏幕最上/最下**；**飞行中也能直接抓住**；位置会记住
- **右键菜单**：设置… / 刷新余额 / 碎碎念 / 对话… / **动作（级联：待机/转向/拖拽/点击/移动/分类/事件/睡觉/干饭/自定义动画）** / 回家 / 切换置顶 / 退出
- **余额**：DeepSeek 官方 `GET /user/balance`，按额度档位播 6 段余额动画 + 富气泡（峰/谷标签）
- **碎碎念**：可配置间隔，人设提示词来自 DSH 配置（可改）；**随机配一张表情包**
- **对话**：完整聊天面板，记忆存本地 JSON，按最近 N 轮进上下文；**模型按语境选表情包**
- **思考动画**：等模型回复时循环「工作状态-思考冒泡」
- **系统通知**：窗口没聚焦时，回复会弹 Windows 通知
- **设置页**：AI 模型/API Key（DeepSeek/OpenAI/Ollama 一键预设）、桌宠尺寸/停靠/置顶/开机自启、**弹性物理（重力/弹性/摩擦/抛掷力度/顶部反弹）**、余额与碎碎念、配图开关、记忆查看/导出/清空、从 DSH 导入配置和 Key、重置位置、打开数据文件夹
- **自定义素材**：`%APPDATA%\dsh-pet-tauri\animations\*.webm` 放自定义动画、`memes\*.png` 放自定义表情包，重启/打开菜单自动收录（动画会出现在「动作 → 自定义动画」）
- **屏幕自动适配**：碰撞边界取当前显示器，可选「整屏（含任务栏区域，能贴最上/最下）」或「工作区（避开任务栏）」
- **置顶看门狗**：每 2 秒重设一次 always-on-top，被别的置顶窗口挤下来会自动回去
- **安全**：API Key 用 Windows DPAPI 加密后存本机；日志与 UI 都不回显明文
- **本地 LLM**：任意 OpenAI 兼容端点；Ollama 的 `http://127.0.0.1:11434/v1` 可**不填 Key**
- **单实例**：重复启动第二个进程会直接退出，不会出现两只
- **首次运行**：没有 Key 时气泡提示去哪配置（可以从 DSH 一键导入）

> 与原版 dsh-pet 的逐项功能对照见 [FEATURE-MATRIX.md](FEATURE-MATRIX.md)。

## 产物

| 文件 | 大小 | 说明 |
|---|---|---|
| `src-tauri/target/release/dsh-pet-tauri.exe` | **65.1 MB** | 免安装（114 段动画 + 27 张表情包已内嵌） |
| `src-tauri/target/release/bundle/nsis/dsh-pet_0.2.0_x64-setup.exe` | **62.8 MB** | NSIS 安装包 |

依赖：Windows 10/11 + WebView2 Runtime（Win11 自带；Win10 一般随 Edge 已有）。

## 快速开始

1. 双击 `dsh-pet-tauri.exe`（或安装包）。
2. 右键宠物 → **设置…**。
3. 填 AI 模型：
   - DeepSeek：`https://api.deepseek.com` + `deepseek-chat` + API Key
   - OpenAI：`https://api.openai.com/v1` + 模型名 + Key
   - Ollama：`http://127.0.0.1:11434/v1` + `qwen2.5:7b`（Key 留空）
   - 也可以点 **从 DSH 导入**：读取 `$DSH_HOME/.credentials.yaml` 里的 `DEEPSEEK_API_KEY`（只读，一次点击）
4. **保存 Key** → **测试连接** → **保存**。
5. 回到宠物：右键 → 余额 / 碎碎念 / 对话。

## 数据与路径

| 内容 | 路径 |
|---|---|
| 设置 | `%APPDATA%\dsh-pet-tauri\settings.json` |
| 对话记忆 | `%APPDATA%\dsh-pet-tauri\memory.json` |
| 自定义动画 | `%APPDATA%\dsh-pet-tauri\animations\*.webm` |
| 自定义表情 | `%APPDATA%\dsh-pet-tauri\memes\*.png` |
| 日志 | `%TEMP%\dsh-pet-tauri.log` |
| 动画素材 | 打包进 exe（`web/assets/webm/`，20 段：待机/点击/拖拽/动作/余额/碎碎念） |

首次运行会自动从 `$DSH_HOME/dsh-pet/main-config.jsonc` 导入：名字、尺寸、停靠、边距、人设提示词、记忆轮数、余额/碎碎念间隔（只读，不写回 DSH）。

## 架构（独立侧，P1 的 port/adapter 版）

```
src-tauri/src/
├─ main.rs       壳：窗口 / 点击穿透 / 拖拽 / 托盘 / 定时器 / 命令注册
├─ settings.rs   本地配置适配器 + DSH 导入
├─ llm.rs        LLM 端口：OpenAI 兼容 /chat/completions（流式未做）
├─ balance.rs    余额端口：DeepSeek 官方余额 + 档位映射
├─ memory.rs     记忆存储：JSON
└─ secrets.rs    凭证端口：Windows DPAPI（CryptProtectData）
web/
├─ index.html / app.js / style.css   宠物窗口：视频精灵、菜单、气泡、对话
└─ settings.html / settings.js        设置窗口
```

> 说明：这一版在**独立应用侧**完成了端口/适配器拆分（原来的 DSH 插件宿主逻辑被换成
> 本地 LLM / 余额 / 记忆 / DPAPI 实现）。DSH 插件本身仍是它自己的 TypeScript 核，
> 要做到「插件与独立应用共享同一份 core」还需要把 TS 侧再抽一层（P1.5，未完成）。

## 构建 / 测试

```powershell
cd src-tauri
cargo build --release      # 免安装 exe
cargo tauri build          # NSIS 安装包
cargo test --release       # DPAPI / 余额档位等纯逻辑单测
```

本地联调（不花真 token）：

```powershell
python tools/mock_server.py          # http://127.0.0.1:3199/v1
$env:DSH_PET_API_KEY="dummy"         # 临时覆盖 Key（不落盘）
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=9223 --remote-allow-origins=*"
.\src-tauri\target\release\dsh-pet-tauri.exe
python tools/cdp.py eval "settings.html" "JSON.stringify(await __TAURI__.core.invoke('get_settings'))"
```

把设置里的接口地址改成 `http://127.0.0.1:3199/v1`、模型 `mock`，即可用 mock 跑通对话/余额/碎碎念。

## 截图

- `screenshots/standalone-base.png`：透明窗叠在其它窗口上
- `screenshots/menu-new.png`：新右键菜单（白底系统风）
- `screenshots/menu-anims2.png`：三级级联动作菜单（分类 → 具体动画）
- `screenshots/picked-anim.png`：从菜单点播动画
- `screenshots/chat-new.png`：对话面板（白底软糖体）
- `screenshots/balance-new.png`：余额富气泡（金额 + 峰/谷）
- `screenshots/chat-meme.png`：对话配图（模型选择的表情包）
- `screenshots/whisper-meme.png`：碎碎念随机配图
- `screenshots/notify-test.png`：窗口未聚焦时的系统通知
- `screenshots/click-bounce.png`：点击 Q 弹（压扁帧）

## 已知限制 / 下一步

- 单宠物：多窗口/多宠物按你的要求暂时不做
- **P1.5 未完成**：DSH 插件仍是它自己的 TypeScript 核，和独立应用目前是两套实现（行为对齐、代码未共享）
- 拖拽是「直接跟手」而不是原版的弹簧跟随，手感略硬
- 点击积分/粒子爆发、置顶看门狗、打开网站/重载配置还没做
- 自定义动画/表情包目录已支持；pet pack 额外「种类」、多服务商余额还没做
- dsh-pet 0.4.5 新增的自动睡觉/干饭触发、语音、点击音效、倾斜重力/摇一摇还没接（动画本身可在「动作」菜单点播）
- 仅 Windows；macOS 需要 WKWebView 的 HEVC-alpha 素材链
- 没有自动更新/代码签名（需要证书与更新服务器）
- DSH 工作状态联动没有（独立应用没有 DSH 会话事件；用「思考动画」覆盖等模型的场景）
- 多显示器/DPI 矩阵只做了主屏 + 100% 缩放验证
- 为了全量 114 段动画，exe 约 65.1 MB（仍远小于 Electron 的 347 MB）

## 实测（P0 壳阶段，1 宠 idle）

| 指标 | Electron Helper | Tauri 壳 |
|---|---|---|
| 安装体积 | 347 MB 运行时 + 62.5 MB 包 | P0 壳 8.6 MB；当前完整版 **65.1 MB exe / 62.8 MB 安装包**（114 段动画） |
| 私有工作集 | 67–69 MB | 89–91 MB |
| 空闲 CPU | 10.2–10.6% 单核 = 0.51–0.53% 整机 | 11.9–13.4% 单核 = 0.59–0.67% 整机 |
| 启动到窗口 | 未单独测 | 0.46–0.56 s |

> 结论：Tauri 赢在安装体积/启动/分发，不赢在内存/CPU。完整对比见
> `../dsh-pet-standalone/PLAN.md`。

## 开源声明

本项目的桌宠素材/配置来自 [dsh-pet](https://github.com/PC2005-cloud/dsh-pet)（MIT License，Copyright (c) 2026 PC2005-cloud）。完整许可见 [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md)。

## 安装包后处理

`cargo tauri build` 之后需要运行 `python tools/patch_nsis.py`。它会：

- 给 NSIS 脚本换微软雅黑字体，缩短欢迎/目录页文案（避免中文压字/截断）；
- 显式设置安装包/卸载包图标；
- 强制卸载程序简体中文（`$LANGUAGE 2052`）；
- 跳过维护选择页：同版本重装直接继续；升级时自动静默卸载旧版后继续安装（不需要先点卸载再重跑安装包）。

最终生成 `dsh-pet_0.2.0_x64-setup.exe`。
