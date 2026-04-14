Feature: cta-usage-pattern Skill 建議生成
  作為 Claude Code 使用者
  我想要根據 session 分類結果取得可執行的 harness 優化建議
  以便改善我的工作流效率

  Background:
    Given CTA DB 已同步
    And cta-usage-pattern skill 已載入
    And skill 可存取 harness-signals-to-advice.md 對應表

  # ================================================================
  # Cold Session 建議
  # ================================================================

  Scenario: cold session 觸發 cache 保溫建議
    Given classify_session_pattern 返回 pattern "cold_session" severity "warn"
    And signals.cache_hit_rate = 0.25
    When cta-usage-pattern skill 合成建議報告
    Then 報告應包含關鍵詞 "cache"
    And 報告應包含至少一條可執行建議，涉及:
      | 建議類別 |
      | CLAUDE.md 穩定性 |
      | session 持續時間 |
      | 避免 mid-session 切換 model |
    And 報告格式應為：「⚠ 偵測到 Cold Session...」

  Scenario: cold session alert 等級觸發更緊急建議
    Given classify_session_pattern 返回 pattern "cold_session" severity "alert"
    And signals.cache_hit_rate = 0.08
    When cta-usage-pattern skill 合成建議報告
    Then 報告首行應標示 "🔴 ALERT"
    And 建議數量應 ≥ 3 條

  # ================================================================
  # Correction Spiral 建議
  # ================================================================

  Scenario: correction_spiral 觸發 diff 輸出建議
    Given classify_session_pattern 返回 pattern "correction_spiral" severity "warn"
    And signals.repeated_edit_peak = 6
    And signals.output_token_ratio = 0.52
    When cta-usage-pattern skill 合成建議報告
    Then 報告應包含關鍵詞 "diff"
    And 報告應包含關鍵詞 "context window"
    And 報告應說明 "同一檔案被修改 6 次"

  # ================================================================
  # Subagent Swarm 建議
  # ================================================================

  Scenario: subagent_swarm 觸發 subagent 協調建議
    Given classify_session_pattern 返回 pattern "subagent_swarm" severity "warn"
    And signals.subagent_count = 15
    When cta-usage-pattern skill 合成建議報告
    Then 報告應包含關鍵詞 "subagent"
    And 報告應包含數值 "15"
    And 建議應提及合理的 subagent 使用場景

  # ================================================================
  # Kitchen Sink 建議
  # ================================================================

  Scenario: kitchen_sink 觸發聚焦建議
    Given classify_session_pattern 返回 pattern "kitchen_sink" severity "info"
    And signals.topic_shift_count = 5
    When cta-usage-pattern skill 合成建議報告
    Then 報告應包含關鍵詞 "checkpoint"
    And 報告嚴重程度標示應為 "ℹ️ INFO"（非 alert/warn）

  # ================================================================
  # 正常 Pattern 建議（無警示）
  # ================================================================

  Scenario: marathon session 輸出資訊性摘要（無警示建議）
    Given classify_session_pattern 返回 pattern "marathon" severity "info"
    When cta-usage-pattern skill 合成建議報告
    Then 報告應正向確認工作模式
    And 報告不應包含 "⚠" 或 "🔴" 標記
    And 報告仍應顯示 signals 數值摘要

  Scenario: normal session 輸出簡短摘要
    Given classify_session_pattern 返回 pattern "normal" severity "info"
    When cta-usage-pattern skill 合成建議報告
    Then 報告應說明「未偵測到異常使用模式」
    And 報告長度應少於 200 字

  # ================================================================
  # 報告格式規範
  # ================================================================

  Scenario: 報告格式包含 ASCII sparkline（有趨勢資料時）
    Given classify_session_pattern 返回任意 pattern
    And 使用者要求「過去 14 天趨勢」
    When cta-usage-pattern skill 合成報告並呼叫 trend_report
    Then 報告中應包含 sparkline 字元（▁▂▃▄▅▆▇█ 其中之一）
    And sparkline 應標示時間範圍

  Scenario: 報告格式統一使用繁體中文
    Given classify_session_pattern 返回任意 pattern
    When cta-usage-pattern skill 合成建議報告
    Then 報告主體應為繁體中文
    And 技術術語（session_id、cache、sparkline）保留英文原文
    And 費用格式應為 "$X.XX USD"
