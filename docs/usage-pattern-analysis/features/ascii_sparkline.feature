Feature: ASCII Sparkline 視覺化
  作為 CTA 使用者
  我想要在 terminal 中看到 token 使用趨勢的 ASCII 圖
  以便一眼辨識用量模式

  Background:
    Given sparkline::render 函式已實作
    And 使用 Unicode block characters: ▁▂▃▄▅▆▇█

  # ================================================================
  # 正常輸入
  # ================================================================

  Scenario: 正常數據輸入生成 sparkline
    Given data = [10.0, 20.0, 15.0, 40.0, 35.0, 50.0, 45.0, 60.0]
    When 呼叫 sparkline::render(data)
    Then 應返回長度為 8 的字串
    And 所有字元都在 {▁, ▂, ▃, ▄, ▅, ▆, ▇, █} 集合中
    And 第一個字元應為較低的 block（對應值 10.0）
    And 最後一個字元應為較高的 block（對應值 60.0）
    And 第四個字元應為最高的 block '█'（對應最大值 60.0）

  Scenario: 遞增序列生成升坡 sparkline
    Given data = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]
    When 呼叫 sparkline::render(data)
    Then 應返回 "▁▂▃▄▅▆▇█"

  Scenario: 遞減序列生成降坡 sparkline
    Given data = [8.0, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0]
    When 呼叫 sparkline::render(data)
    Then 應返回 "█▇▆▅▄▃▂▁"

  # ================================================================
  # 邊界條件
  # ================================================================

  Scenario: 空陣列返回空字串
    Given data = []
    When 呼叫 sparkline::render(data)
    Then 應返回空字串 ""

  Scenario: 單一元素返回中間 block
    Given data = [42.0]
    When 呼叫 sparkline::render(data)
    Then 應返回長度為 1 的字串
    And 該字元應為 '▄'（中間值，表示無法比較相對大小）

  Scenario: 全相同值返回中間 block
    Given data = [5.0, 5.0, 5.0, 5.0, 5.0]
    When 呼叫 sparkline::render(data)
    Then 應返回 "▄▄▄▄▄"
    # 所有值相同時，映射到中間等級 (3/7)

  Scenario: 零值序列正確處理
    Given data = [0.0, 10.0, 0.0, 10.0]
    When 呼叫 sparkline::render(data)
    Then 應返回長度為 4 的字串
    And 第一個字元應為 '▁'（最小值 0.0）
    And 第二個字元應為 '█'（最大值 10.0）

  # ================================================================
  # 特殊值處理
  # ================================================================

  Scenario: NaN 值以 '·' 表示缺失
    Given data = [10.0, NaN, 30.0]
    When 呼叫 sparkline::render(data)
    Then 輸出長度應為 3
    And 第二個字元應為 '·'（缺失標記）
    And 第一和第三個字元應正常從 block characters 選取

  Scenario: 大數值範圍正確縮放
    Given data = [1.0, 1000000.0]
    When 呼叫 sparkline::render(data)
    Then 應返回 "▁█"
    And 不應 panic 或返回 out-of-range 字元

  Scenario: 負數值正確處理（平移後縮放）
    Given data = [-50.0, -25.0, 0.0, 25.0, 50.0]
    When 呼叫 sparkline::render(data)
    Then 應返回長度為 5 的字串
    And 第一個字元應為 '▁'（最小值）
    And 最後一個字元應為 '█'（最大值）

  # ================================================================
  # 在 Skill 輸出中的整合
  # ================================================================

  Scenario: cta trend 命令輸出包含 sparkline
    Given 過去 14 天的每日 token 使用資料已存在 DB 中
    When 執行 CLI 命令 "cta trend --granularity daily --days 14"
    Then 輸出應包含至少一行 sparkline 字元
    And sparkline 長度應等於資料點數量

  Scenario: sparkline 超過 terminal 寬度時截斷（80 字元限制）
    Given data 包含 200 個點
    When 呼叫 sparkline::render(data) 並指定 max_width = 80
    Then 應返回長度 ≤ 80 的字串
    And 應優先保留最近的資料點（右側）
