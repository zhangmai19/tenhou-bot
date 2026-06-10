# 策略自定义指南

这是你注入自己打牌理解的地方。所有自定义点都在 `src/tactics.rs` 中，用 `**CUSTOMIZE**` 标记。

## 核心决策流程

```
摸牌 → 能和了? → 和了(ツモ)
     ↓
     能九種九牌? → 流局
     ↓
     能立直? → 立直
     ↓
     能槓? → 槓
     ↓
     選一張牌打出 ← 這是主要自定義點
```

對手的捨牌來時:
```
    能栄和? → 栄和
     ↓
     能吃/碰? → 鳴牌
     ↓
     過(pass)
```

## 如何修改打牌邏輯

### 1. 調整權重 (最簡單)

編輯 `config.toml`:

```toml
[strategy]
ukeire_weight = 2.0       # 更重視牌效（速度優先）
dora_weight = 1.0         # 更重視保留寶牌
danger_penalty = 5.0      # 更防守
```

### 2. 修改 discard 打分邏輯

在 `src/tactics.rs` 的 `evaluate_all_discards()` 方法中，有一個 `**CUSTOMIZE**` 區塊。

**例子 1: 早巡優先速度，晚巡優先防守**

```rust
// 在 custom_score 計算區塊中加入:
let junme = state.bot().discards.len(); // 巡目

if junme < 6 {
    // 早巡：優先速度，減少danger penalty
    if info.shanten <= 1 {
        custom_score -= 2.0; // 接近聽牌，加速
    }
} else if junme > 10 {
    // 晚巡：優先防守
    if is_dangerous {
        custom_score += 5.0; // 不打危險牌
    }
    if !is_dangerous {
        custom_score -= 1.0; // 傾向保留安全牌
    }
}
```

**例子 2: 點數差判斷**

```rust
// 根據點數差調整打法
let my_score = state.bot().score;
let target_score = state.players.iter().map(|p| p.score).max().unwrap_or(0);
let diff = target_score - my_score;

if diff > 10000 {
    // 落後很多，需要大牌 → 保留dora, 不輕易鳴牌
    if dora_tiles.contains(&tile34) {
        custom_score += 3.0; // 死保dora
    }
}
```

**例子 3: 副露手 vs 門前手**

```rust
let has_open = !state.bot().melds.is_empty();

if has_open {
    // 副露手：追求速度
    custom_score -= info.ukeire_count as f64 * 0.3;
} else {
    // 門前手：保留立直機會
    if info.shanten <= 1 {
        custom_score -= 1.0; // 加速進入聽牌
    }
}
```

### 3. 修改鳴牌策略

在 `try_call_meld()` 方法中:

```rust
// **CUSTOMIZE** 區塊:
// 例如：只在特定情況下鳴牌
if tile34.is_honor() && count_in_hand == 2 {
    // 有人打了役牌，即使降向聽少也碰
    // 因為碰了就有役
    return Some(MeldDecision { ... });
}
```

### 4. 修改立直判斷

在 `should_riichi()` 方法中:

```rust
// 對手已立直時不要立直（除非牌很大）
let opponent_riichi = state.players.iter().any(|p| !p.is_bot && p.is_riichi);
if opponent_riichi {
    // 檢查自己的聽牌型好不好
    if info.ukeire_count < 8 {
        return (false, None); // 默聽
    }
}
```

## 架構擴展

如果你想做大改（比如用 ML 模型替換規則引擎），實現 `AiStrategy` trait 即可:

```rust
struct MyMlAi {
    model: MyModel,
}

impl AiStrategy for MyMlAi {
    fn choose_discard(&self, state: &GameState) -> Tile136 {
        // 你的 ML 推理邏輯
    }
    // ... implement other methods
}
```

## 調試

啟動時加 `-v` 參數可以看到詳細的日誌:

```bash
cargo run --release -- -v
```

日誌中會顯示每張候選牌的評分細節:

```
Discard choice: 1m (shanten=2, ukeire=23, dora=0, dangerous=false, score=19.30)
```

## 牌譜復盤

你的 mjai-reviewer 項目可以用來復盤 bot 的對局，分析哪裡打得不好。打完後獲取牌譜 URL，用 mjai-reviewer 分析。
