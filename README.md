# dsh-pet Tauri 独立桌宠（第三方打包，非官方）

> **本项目不是 dsh-pet 官方项目。**
>
> 这是一个**第三方打包项目**：把开源项目 **[dsh-pet](https://github.com/PC2005-cloud/dsh-pet)**（MIT License，Copyright (c) 2026 PC2005-cloud）的动画、表情和配置素材，用 **Tauri** 打包成 Windows 独立桌面应用。桌宠素材、角色形象和原项目代码的版权归原作者；本项目只负责 Tauri 壳、Windows 打包和少量独立实现，不是官方发行版。
>
> - 原项目：https://github.com/PC2005-cloud/dsh-pet
> - 原项目协议：MIT
> - 界面字体：站酷快乐体 ZCOOL KuaiLe（SIL OFL 1.1）

## 下载

- **Releases：** https://github.com/elysia-ren/dsh-pet-tauri/releases
- **安装包：** `dsh-pet_0.2.0_x64-setup.exe`（中文 NSIS 向导，可选安装目录）
- **免安装版：** `dsh-pet-tauri.exe`

运行环境：Windows 10/11 + WebView2 Runtime（Windows 11 自带；Windows 10 通常随 Edge 已安装）。

## 功能

- 透明、无边框、置顶、跳过任务栏的桌面宠物，支持多显示器位置记忆
- 同步 dsh-pet 0.4.5 的 **114 段动画**：待机 / 转向 / 走动 / 点击回应 / 分类动作 / 睡觉 / 干饭 / 余额档位 / 碎碎念等
- 自主随机链：会自己在桌面上走动、转向、做动作
- 点击 Q 弹 + 彩色粒子反馈；拖拽跟手，松手可甩出，带重力、边缘反弹和落地回弹
- 碰撞边界按可见身体计算，支持「整屏」或「工作区（避开任务栏）」；飞行中也能直接抓住
- 右键级联动作菜单，可点播具体动画；自定义动画会出现在「动作 → 自定义动画」
- DeepSeek 官方余额查询 + 额度档位动画 + 峰/谷气泡
- 碎碎念：可配置间隔，随机配表情包
- 对话：完整聊天面板，本地记忆，按最近 N 轮进上下文
- 思考动画、Windows 系统通知、单实例、开机自启、置顶看门狗
- 设置页：DeepSeek / OpenAI / Ollama 一键预设、API Key 本机 DPAPI 加密、桌宠尺寸/位置/置顶/自启、弹性物理参数、记忆查看/导出/清空、打开数据文件夹
- 本地 LLM：任意 OpenAI 兼容端点，Ollama 可不填 Key

## 截图

![右键动作菜单](screenshots/menu-anims2.png)

![对话](screenshots/chat-new.png)

![余额气泡](screenshots/balance-new.png)

![中文安装向导](screenshots/installer-020-welcome.png)

## 使用

1. 运行安装包或免安装 exe。
2. 右键宠物 → **设置…**。
3. 填写 AI 模型（任选其一）：
   - **DeepSeek**：`https://api.deepseek.com`，模型 `deepseek-chat`，填 API Key
   - **OpenAI**：`https://api.openai.com/v1`，填模型名和 Key
   - **Ollama**：`http://127.0.0.1:11434/v1`，模型如 `qwen2.5:7b`，Key 可留空
4. **保存 Key → 测试连接 → 保存**。
5. 回到宠物，右键可用余额、碎碎念、对话；拖动可以换位置，甩出去会弹跳落地。

## 数据目录

| 内容 | 路径 |
|---|---|
| 设置 | `%APPDATA%\dsh-pet-tauri\settings.json` |
| 对话记忆 | `%APPDATA%\dsh-pet-tauri\memory.json` |
| 自定义动画 | `%APPDATA%\dsh-pet-tauri\animations\*.webm` |
| 自定义表情 | `%APPDATA%\dsh-pet-tauri\memes\*.png` |
| 日志 | `%TEMP%\dsh-pet-tauri.log` |

设置页里的「打开数据文件夹」可以直接打开上面的目录。

## 构建

需要 Rust、Node.js、Tauri CLI 和 NSIS。

```powershell
cd src-tauri
cargo build --release      # 免安装 exe
cargo tauri build          # NSIS 安装包
```

安装包构建完成后，运行：

```powershell
python tools/patch_nsis.py
```

它会应用中文字体/文案、安装包图标和中文卸载器设置，并重新生成安装包。

## 开源与致谢

- 桌宠素材、动画、表情和动画/物理配置来自 **[dsh-pet](https://github.com/PC2005-cloud/dsh-pet)**，MIT License，Copyright (c) 2026 PC2005-cloud。
- 界面字体：**站酷快乐体 / ZCOOL KuaiLe**，SIL Open Font License 1.1。
- 完整许可见 [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md)。

再次说明：本项目只是第三方打包，不是 dsh-pet 官方发行版。
