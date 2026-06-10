# CLAUDE.md

## Git 身份

```bash
git config user.name "zhangmai19"
git config user.email "zhangmai19@foxmail.com"
```

## 项目说明

天凤 (Tenhou) 实时 AI 打牌建议工具 + 自动玩牌机器人。

## 项目结构

| 文件 | 说明 | 状态 |
|------|------|------|
| `src/` Rust | tile/shanten/yaku/meld/state/protocol/client/sim/advisor | ✅ 编译+测试通过 |
| `advisor_bridge.js` | Playwright 开 Chromium,拦截天凤 WebSocket,每摸牌调用 akochan | ✅ 框架跑通 |
| `akochan_advisor.py` | 单次调用 akochan pipe_detailed,喂完整局况 | ✅ 能用 |
| `capture_ws.js` | 抓包工具:拦截天凤 WebSocket URL 和消息 | ✅ 已验证 |
| `STRATEGY.md` | 策略自定义指南 | ✅ |

## 外部依赖

| 组件 | 位置 | 说明 |
|------|------|------|
| akochan | `/tmp/akochan/` | 已编译 `system.exe` + `libai.so`, `pipe_detailed` 模式 |
| Akagi | `akagi-app/akagi-3.1.1-linux-x64/akagi` | 预编译版,需 libwebkit2gtk-4.1-0 + Discord 下 Mortal 权重 |
| Chromium | `~/.cache/ms-playwright/` | 需 `LD_LIBRARY_PATH=$HOME/.local/lib` (libnspr4 等) |
| mjai-reviewer | `/tmp/mjai-reviewer/` | 牌谱分析工具,已编译 |

## 关键发现

1. **天凤协议 (HTML5)**: WebSocket `wss://b-ww.mjv.jp/`, JSON 消息, tag 编码规则:
   - `T<num>` = 自己摸牌, `D/d<num>` = 自己切牌 (大/小写=手出/摸切)
   - `U/V/W` = 对手摸牌, `E/e/F/f/G/g<num>` = 对手切牌
   - `INIT` = 发牌, `DORA` = 新宝牌, `REACH` = 立直, `AGARI`/`RYUUKYOKU` = 局终

2. **旧 TCP 协议 (133.242.10.78:10080) 已废除**

3. **Mortal 模型不公开** (作者顾虑作弊), Akagi 通过 Discord 分发权重

4. **akochan `pipe_detailed`**: stdin 喂 mjai 事件流, actor=0 tsumo 时 stdout 输出 JSON 候选评价

## 当前问题

- akochan 推荐 `pt=-999` — 聚合解析逻辑改了,需验证 akochan stdout 是否返回完整 JSON
- 鸣牌评估未完成 (仅列出吃碰可能,未模拟推荐)

## 下一步建议

- 修好 akochan 解析 → 即可用
- 或者直接装 Akagi → 进 Discord 下 Mortal 权重 → 开箱即用

## 运行方式

```bash
cd /mnt/c/Users/zhang/tenhou-bot
LD_LIBRARY_PATH=$HOME/.local/lib node advisor_bridge.js
# 点 Guest Login → 开始打牌 → 终端看推荐
```
