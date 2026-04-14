Feature: classify_session_pattern MCP Tool
  作為 Harness Engineer
  我想要透過 MCP tool 取得 session 的使用模式分類結果
  以便了解我的工作模式並採取針對性優化措施

  Background:
    Given projects_dir 下的 JSONL 可直接被 CTA 讀取
    And 所有可分類 session 至少包含 3 個 assistant turns

  # ================================================================
  # 正常 Pattern 分類
  # ================================================================

  Scenario: 偵測 marathon session（長時間深度工作）
    Given 一個 session 有以下特徵:
      | turn_count       | 150  |
      | duration_minutes | 185  |
      | cache_hit_rate   | 0.82 |
    When 呼叫 classify_session_pattern 傳入此 session_id
    Then pattern 應為 "marathon"
    And severity 應為 "info"
    And evidence 應包含:
      | metric           | direction |
      | turn_count       | above     |
      | duration_minutes | above     |
      | cache_hit_rate   | above     |
    And signals.cache_hit_rate 應等於 0.82

  Scenario: 偵測 observer session（輕量偵察模式）
    Given 一個 session 有以下特徵:
      | turn_count         | 12   |
      | repeated_edit_peak | 0    |
      | cache_hit_rate     | 0.55 |
    When 呼叫 classify_session_pattern 傳入此 session_id
    Then pattern 應為 "observer"
    And severity 應為 "info"
    And evidence 應包含:
      | metric             | direction |
      | turn_count         | below     |
      | repeated_edit_peak | below     |

  Scenario: 偵測 cold session — warn 等級（cache 保溫不足）
    Given 一個 session 有以下特徵:
      | cache_hit_rate | 0.25 |
      | turn_count     | 45   |
    When 呼叫 classify_session_pattern 傳入此 session_id
    Then pattern 應為 "cold_session"
    And severity 應為 "warn"
    And evidence 應包含 metric "cache_hit_rate" value 0.25 threshold 0.30 direction "below"

  Scenario: 偵測 cold session — alert 等級（cache 完全失效）
    Given 一個 session 有以下特徵:
      | cache_hit_rate | 0.08 |
      | turn_count     | 30   |
    When 呼叫 classify_session_pattern 傳入此 session_id
    Then pattern 應為 "cold_session"
    And severity 應為 "alert"
    And evidence.first.threshold 應等於 0.15

  Scenario: 偵測 correction_spiral（反覆修改同一檔案）
    Given 一個 session 有以下特徵:
      | repeated_edit_peak  | 6    |
      | output_token_ratio  | 0.52 |
      | turn_count          | 25   |
    When 呼叫 classify_session_pattern 傳入此 session_id
    Then pattern 應為 "correction_spiral"
    And severity 應為 "warn"
    And evidence 應包含 metric "repeated_edit_peak" value 6 threshold 4 direction "above"
    And evidence 應包含 metric "output_token_ratio" value 0.52 threshold 0.40 direction "above"

  Scenario: 偵測 correction_spiral — alert 等級（任一 alert threshold 即升級）
    Given 一個 session 有以下特徵:
      | repeated_edit_peak  | 8    |
      | output_token_ratio  | 0.45 |
    When 呼叫 classify_session_pattern 傳入此 session_id
    Then pattern 應為 "correction_spiral"
    And severity 應為 "alert"

  Scenario: 偵測 subagent_swarm（子代理過多）
    Given 一個 session 有以下特徵:
      | subagent_count | 15 |
      | turn_count     | 60 |
    When 呼叫 classify_session_pattern 傳入此 session_id
    Then pattern 應為 "subagent_swarm"
    And severity 應為 "warn"
    And evidence 應包含 metric "subagent_count" value 15 threshold 10 direction "above"

  Scenario: 偵測 kitchen_sink（話題散亂）
    Given 一個 session 有以下特徵:
      | topic_shift_count  | 5 |
      | repeated_edit_peak | 2 |
      | turn_count         | 40 |
    When 呼叫 classify_session_pattern 傳入此 session_id
    Then pattern 應為 "kitchen_sink"
    And severity 應為 "info"
    And evidence 應包含 metric "topic_shift_count" value 5 threshold 3 direction "above"

  Scenario: 偵測 normal session（無特殊 pattern）
    Given 一個 session 有以下特徵:
      | turn_count         | 35   |
      | cache_hit_rate     | 0.65 |
      | repeated_edit_peak | 2    |
      | subagent_count     | 3    |
      | topic_shift_count  | 1    |
      | output_token_ratio | 0.30 |
    When 呼叫 classify_session_pattern 傳入此 session_id
    Then pattern 應為 "normal"
    And severity 應為 "info"
    And evidence 應為空 list

  # ================================================================
  # 優先順序測試（多 pattern 同時觸發）
  # ================================================================

  Scenario: cold_session 優先於 correction_spiral（優先順序驗證）
    Given 一個 session 有以下特徵:
      | cache_hit_rate      | 0.12 |
      | repeated_edit_peak  | 5    |
      | output_token_ratio  | 0.45 |
    When 呼叫 classify_session_pattern 傳入此 session_id
    Then pattern 應為 "cold_session"
    And evidence 中主要 metric 應為 "cache_hit_rate"
    # cold_session 優先於 correction_spiral，即使兩個條件都滿足

  Scenario: correction_spiral 優先於 marathon（anti-pattern 優先）
    Given 一個 session 有以下特徵:
      | turn_count          | 150  |
      | duration_minutes    | 200  |
      | cache_hit_rate      | 0.75 |
      | repeated_edit_peak  | 5    |
      | output_token_ratio  | 0.45 |
    When 呼叫 classify_session_pattern 傳入此 session_id
    Then pattern 應為 "correction_spiral"
    And severity 應為 "warn"
    # marathon 訊號雖然存在，但 anti-pattern correction_spiral 優先

  # ================================================================
  # 邊界條件 & 錯誤處理
  # ================================================================

  Scenario: session 輪數不足（無法有意義分類）
    Given 一個 session 只有 2 個 assistant turns
    When 呼叫 classify_session_pattern 傳入此 session_id
    Then transport error.code 應為 "INVALID_PARAMS"
    And error.data.code 應為 "INSUFFICIENT_DATA"
    And error message 應說明「至少需要 3 個 assistant turns 才能分類」

  Scenario: session_id 不存在
    Given session_id "nonexistent0" 不存在於 projects_dir
    When 呼叫 classify_session_pattern 傳入 "nonexistent0"
    Then transport error.code 應為 "INVALID_PARAMS"
    And error.data.code 應為 "SESSION_NOT_FOUND"

  Scenario: session_id 前綴有衝突（多個 session 符合）
    Given projects_dir 中有兩個 session_id 以 "abc12345" 開頭
    When 呼叫 classify_session_pattern 傳入 "abc12345"
    Then transport error.code 應為 "INVALID_PARAMS"
    And error.data.code 應為 "AMBIGUOUS_SESSION_ID"
    And error message 應說明 session_id 不唯一，要求提供更長的 ID
