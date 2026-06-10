# tenhou-bot

天凤 (Tenhou) 实时 AI 打牌建议工具 + 自动麻将对局机器人。

> ⚠️ 教育用途。使用者自行承担账号风险。

## 项目状态

| 组件 | 状态 |
|------|------|
| 天凤协议逆向 (WebSocket JSON) | ✅ 完成 |
| Rust 麻将引擎 (向听/役/状态) | ✅ 22 测试通过 |
| Playwright 实时牌局监听 | ✅ 框架跑通 |
| akochan AI 推荐 | ⚠️ 框架正确，解析待修 |
| Akagi (Mortal 模型) | ✅ 预编译版已下载 |

## 项目结构

```
src/                      # Rust — 核心引擎
├── tile.rs               # 牌型定义 (136/34)
├── shanten.rs            # 向听数计算 (递归最优搜索)
├── yaku.rs               # 役种判定 (立直/断幺/役牌/门前自摸)
├── meld.rs               # 副露
├── state.rs              # 游戏状态追踪
├── tactics.rs            # 决策引擎 (自定义策略入口)
├── protocol.rs           # 天凤 JSON 协议 (解析+构建)
├── client.rs             # WebSocket 客户端 (tokio-tungstenite)
├── sim.rs                # 本地游戏模拟器 (测试用)
├── advisor.rs            # Advisor 模式 (stdin JSON → stdout 推荐)
├── config.rs             # 配置加载
├── ai.rs                 # AI trait 接口
└── main.rs               # CLI 入口

advisor_bridge.js         # Playwright 拦截天凤 WebSocket → 实时推荐
akochan_advisor.py        # Python 桥接 → akochan pipe_detailed
capture_ws.js             # 天凤协议抓包工具
STRATEGY.md               # 策略自定义指南
CLAUDE.md                 # Claude Code 会话配置
```

## 技术栈

- **Rust** — 向听计算、役判定、状态管理、协议解析
- **Node.js (Playwright)** — 浏览器自动化，拦截 WebSocket
- **Python** — akochan AI 引擎桥接
- **C++ (akochan)** — 编译自 `critter-mj/akochan`

## 快速开始

### 1. 编译 Rust 引擎

```bash
cargo build --release
```

### 2. 编译 akochan (如果未编译)

```bash
git clone https://github.com/critter-mj/akochan.git /tmp/akochan
cd /tmp/akochan/ai_src && make -f Makefile_Linux -j$(nproc)
cd .. && make -f Makefile_Linux -j$(nproc)
```

### 3. 安装 Node 依赖

```bash
npm install
npx playwright install chromium
```

### 4. 运行 Advisor Bridge

```bash
LD_LIBRARY_PATH=/tmp/akochan:$HOME/.local/lib node advisor_bridge.js
```

Chrome 窗口自动打开天凤 → 登录 → 打牌 → 终端实时显示推荐。

### 5. 本地模拟 (不需要网络)

```bash
cargo run --release -- --sim --games 1
```

## 关键发现

### 天凤 HTML5 协议

- **连接**: `wss://b-ww.mjv.jp/` (JSON WebSocket)
- **认证**: `{"tag":"HELO","name":"NoName","sx":"M"}` → `{"tag":"LN"}`
- **对局流程**: `HELO → LN → JOIN → GO → GOK → NEXTREADY → [INIT → T/D 循环] → AGARI/RYUUKYOKU`
- **消息编码**: tag 首字母 = 类型，后续数字 = 牌号 (136 格式)
  - `T78` = 自己摸牌 78
  - `D116` = 自己手出切牌 116
  - `d116` = 自己摸切 116
  - `U/V/W` = 对手摸牌
  - `E/e/F/f/G/g` + 数字 = 对手切牌 (大小写 = 手出/摸切)

### Mortal 模型

Mortal (~天凤八段) 的模型权重由作者 **不公开分发**，原因是顾虑作弊。Akagi (shinkuan) 通过 Discord 频道提供权重下载。

### 旧 TCP 协议

旧地址 `133.242.10.78:10080` 已于 2024 年关闭，Tenhou 全部迁移至 WebSocket。

## CLI 用法

```bash
# 本地模拟 3 局
cargo run --release -- --sim --games 3

# 单次推荐 (stdin JSON → stdout JSON)
echo '{"hand":[0,4,8,...],"dora":[...]}' | cargo run --release -- --advise

# 连天凤 (需先修好 WebSocket 握手)
cargo run --release -- -v
```

## 鸣谢

- [mjai-reviewer](https://github.com/Equim-chan/mjai-reviewer) — 牌谱分析框架
- [critter-mj/akochan](https://github.com/critter-mj/akochan) — AI 引擎
- [shinkuan/Akagi](https://github.com/shinkuan/Akagi) — Mortal 桌面工具
- [MahjongAI](https://github.com/erreurt/MahjongAI) — 天凤协议参考
