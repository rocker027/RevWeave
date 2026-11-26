
# RevWeave 借鑑 Lazygit：圖譜整合實作計畫

## 結論

依 2026-11-26 三方討論（Claude、Codex、agy 經 Orca，共三輪）的收斂結果，把可借鑑的 Lazygit 做法排成八個可獨立驗證的切片。第一片是「reflog 項目右鍵建立分支」：只新增 ref、可用 `Cmd-Z` 復原，沿用既有建分支對話框，風險與成本最低。

整合原則（三方共識）：

- 圖上不插入幽靈節點，也不重排 lane。
- 拖放與單鍵都不直接改寫歷史；放下後只開選單或編輯器。
- 滑過節點時不即時計算可達性或 rebase 結果。
- 圖上的回饋只用三個輕量原語：模式狀態列、可見列 chip、只作用於已載入列的範圍淡化。

本文件只有計畫，沒有修改程式碼。實機 GUI 驗證為 **NOT RUN**。

## 需要使用者看的選擇

| 項目 | 狀態 | 說明 |
| --- | --- | --- |
| reflog 建分支預設**不簽出** | 已採用的預設 | 與 commit 右鍵相反。理由：開啟對話框時簽出預設為 `!bare`（`view/panels/popover/host/mod.rs:2154`）；`CreateBranchAndCheckout` 會清空 Git Undo 紀錄（`gitcomet-state/src/store/reducer/git_undo.rs:231`），只建分支則會記入 Undo（同檔 `:64-86`）。使用者仍可在對話框勾選簽出。 |
| 救回分支的名稱前綴 | 待決，預設留空 | Codex 提議 `rescue/`，其他兩方未表態。留空與 commit 右鍵一致；要預填就改 `name_prefix`。 |
| 行級丟棄的做法 | **受阻：待使用者裁決** | Codex：確認框。Claude：不加確認框，改 toast 復原。agy：確認框並禁止從檔案清單觸發，二期改存 blob。`docs/shortcuts.md:295` 寫明現行「不詢問」是既有設計。 |
| 互動式 rebase 的 F 鍵 | **受阻：待使用者裁決** | 補 F 會推翻 Q10 與 `interactive_rebase.rs:1691` 的測試斷言。 |
| 擴大 CAS Undo | **受阻：G3 審查狀態未知** | 對標計畫 `2026-11-18-RevWeave-gitkraken-parity-plan.md` 第 173 行仍寫「L3 獨立審查判定阻擋，待修」。`9ed7879d` 併入 master 不等於審查通過。本項屬該計畫 G3 的後續，以該計畫為準，本文件不另立版本。 |

## 範圍與版本

| 項目 | 值 |
| --- | --- |
| repo | `RevWeave`（本機） |
| 分支 | `feat/graph-multi-ref-gitkraken` |
| base commit | `4b66a31e` |
| 工作區 | 乾淨，未提交差異只有本文件 |
| 路徑簡寫 | `view/` 指 `crates/gitcomet-ui-gpui/src/view/` |
| 本次分級 | 唯讀規劃，不改行為，不建 SDLC 紀錄。各切片分級為暫定，開工時依 harness「工作流程」重判 |
| 範圍外 | 幽靈節點預覽層、拖曳 commit 節點重排、通用 customCommands、固定五面板與數字鍵跳面板、圖上單鍵 drop／squash |

## 切片總覽

| # | 切片 | 依賴 | 暫定分級／成本 | 狀態 |
| --- | --- | --- | --- | --- |
| S1 | reflog 右鍵建立分支 | 無 | L2／S | 已提交 `0622ca71`，測試與審查通過 |
| S2 | commit 右鍵「用已暫存變更建立 fixup commit」 | 無 | L2／S–M | 已提交 `d5f58d75`，測試通過，審查中 |
| S3 | 圖譜模式：模式狀態列＋列 chip 泛化 | 無 | L2／M | 併入 S4：沒有第二個使用者前不先抽象 |
| S4 | 引導式 bisect | S3 | L2 或 L3 待重判／M–L | 待 S3 |
| S5 | rebase 編輯器 `Alt-↑/↓` 移列 | 無 | L1／S | 已提交 `1d84be36`，測試通過 |
| S6 | 行級丟棄安全 | 使用者裁決 | 一期 L2／S，可復原 L3／M | 受阻 |
| S7 | 把已暫存變更併入舊 commit | S2、S3 | L3／M–L | 待前置 |
| S8 | 擴大 CAS Undo 至 checkout／rebase | G3 審查解除 | L3／L | 受阻 |

候補（不排程）：未推送 commit 搬到新分支（Lazygit `N`）、純鍵盤選行暫存、reflog 右鍵 cherry-pick 與「在圖上定位」、WIP 列拖到節點。

## S1：reflog 右鍵建立分支（詳細）

### 目標

在 reflog 面板任一列按右鍵，可從該項目指向的 commit 建立本機分支。這樣即使 Undo 鏈已被 checkout、rebase 等操作清空，仍能救回被 reset 或 rebase 丟掉、但物件還在的 commit。

### 已核對的現況

- reflog 列右鍵開 `PopoverKind::ReflogEntryMenu { repo_id, target, selector }`（`view/panes/reflog.rs:580-597`）。點列會送 `Msg::SelectCommit`（同檔 `:576`）。
- 選單模型只有三種 reset（`view/panels/popover/context_menu/reflog_entry.rs`），沒有傳入 `language`。呼叫點在 `view/panels/popover/context_menu.rs:729`，旁邊的選單都用 `localized_menu(cx, |language| …)`。
- commit 右鍵的「從此提交建立分支」開 `PopoverKind::CreateBranchFromRefPrompt { repo_id, target, source_selectable: false, name_prefix }`（`context_menu/commit.rs:763-778`）。
- 對話框開啟時，簽出預設為 `!bare`（`host/mod.rs:2147-2160`）。送出時依開關送 `Msg::CreateBranch` 或 `Msg::CreateBranchAndCheckout`，並套用 Worktree Finish 保護（`host/branch_prompts.rs:306-354`）。
- 後端 `create_branch_impl` 以 rev-parse 解析目標（`gitcomet-git-gix/src/repo/porcelain.rs:290`、`:520-551`）。不要求目標可由任何 ref 到達，只要物件存在。目前沒找到「對不可達 commit 建分支」的整合測試。
- reflog 項目的目標是完整 SHA，`resolve_commit` 直接採用（`git_undo.rs:37-40`），所以建分支會記入 Undo。

### 做法

1. **對話框加開啟時的簽出預設。** `PopoverKind::CreateBranchFromRefPrompt` 新增欄位 `checkout_on_open: bool`。開啟時改為 `checkout_on_open && !bare`。
   - 既有建構點一律傳 `true`，行為不變。正式建構點有 6 處：`gitcomet_view_commands.rs:365`、`panels/action_bar.rs:1563`、`context_menu/branch.rs:302`、`context_menu/commit.rs:775`、`context_menu/branch_group.rs:182`、`mod_helpers/mode_impls.rs:169`。另有約 15 處測試要補欄位。
   - `panels/popover/fingerprint.rs:779` 的 hash 要納入新欄位。
2. **reflog 選單加入口。** `reflog_entry::model` 加 `language` 參數，呼叫點改用 `localized_menu`。在 header 之後、三個 reset 之前加入「Create branch here…／在此建立分支…」，圖示 `icons/git_branch.svg`，開 `CreateBranchFromRefPrompt { target: 完整 SHA, source_selectable: false, name_prefix: 待決（預設空字串）, checkout_on_open: false }`。新入口與 reset 之間加分隔線，非破壞性操作在前。三個 reset 的標籤是否一併在地化，實作時隨 `language` 決定，不改其行為。
3. **後端補整合測試。** 在 `gitcomet-git-gix/tests` 建立一個 commit，再以 `reset --hard` 讓它不可達，然後以其 SHA 建分支，斷言分支指向該 commit。

### 驗收

- reflog 列右鍵，選單第一個動作是「在此建立分支…」，在三個 reset 之前。
- 對話框開啟時，簽出開關為關。送出後 dispatch 的是 `Msg::CreateBranch`，`target` 等於該 reflog 項目的完整 SHA。
- 建立後按 `Cmd-Z` 可移除該分支（沿用現有 Undo，前提是期間沒有其他清空 Undo 的操作）。
- commit 右鍵、側欄、動作列、命令面板開的建分支對話框，簽出預設維持原狀。
- bare repo 中仍可建分支，簽出開關停用，與現行相同。
- 沒有套用篩選或 Focus 時，新分支出現在歷史圖上。
- 不可達 commit 可以建立分支（後端整合測試）。

### 驗證方式

- 更新 `reflog_entry.rs` 內兩個單元測試：標籤清單加入新項，reset 目標斷言維持。新增一個測試，斷言新項的 `PopoverKind` 欄位。
- popover 測試（參考 `panels/popover/tests/branch.rs:1480-1495` 的簽出開關測試）：從 reflog 開啟時簽出為關，送出後 dispatch `Msg::CreateBranch`。另加一個測試確認從 commit 右鍵開啟時簽出仍為開。
- 執行 `CONTRIBUTING.md` 的流程：`python3 scripts/ci/run.py compile --context ui`、`python3 scripts/ci/run.py test --context ui`、`--context backend`，再跑 `CONTRIBUTING.md:151-153` 的 clippy。`--context` 的可選值已由 `run.py --help` 核對；`ui`、`backend` 是否涵蓋上述測試，開工時再確認。
- 實機 GUI 操作需另行授權，目前 NOT RUN。

### 文件

repo 內沒有 reflog 面板的說明文件。在 `docs/` 新增一段短說明，寫明 reflog 右鍵可救回 commit，並註明救不回未提交的內容；或併入既有文件。實作時擇一，不新增其他文件。

### 風險與回退

只新增 ref，不動工作區與既有分支。新增欄位使用預設值 `true`，回退只要移除選單項。開工時依 harness 規則建立 L2 的 SDLC 紀錄（plan＋verification）。

## S2–S8：概要

| # | 輸入與入口 | 可觀察結果 | 驗證方式 | 待決或備註 |
| --- | --- | --- | --- | --- |
| S2 | commit 節點右鍵「用已暫存變更建立 fixup commit」；沒有暫存內容就停用 | HEAD 上新增 `fixup! <目標標題>` 的 commit，可用 `Cmd-Z` 復原 | 單元測試加 popover 測試 | **待決**：既有 autosquash 依標題分組（`interactive_rebase.rs:73`），同名提交會配錯。Codex 建議改依 SHA 配對，這會影響 S7 |
| S3 | 無新入口；把比較基準 chip（`rows/history_canvas.rs:998`）與 `sequencer_banner`（`panels/action_bar.rs:182`）抽成每個 worktree 一份的「圖譜模式」狀態 | 既有比較標記與 merge／rebase 提示列外觀不變 | 既有測試全過，另加模式狀態的單元測試 | 淡化沿用既有「無關列」機制（`history_canvas.rs:692-695`），只作用於已載入列 |
| S4 | 節點右鍵「標為 good／bad」、命令面板「開始 bisect」 | 狀態列顯示剩餘筆數與 Good／Bad／Skip／結束；good、bad 節點加 chip；結束後回到原分支 | 後端整合測試加狀態測試 | 開始前工作區必須乾淨。是否淡化排除範圍：Claude 要，agy 反對，owner 折衷為範圍由後端一次算好、只淡化已載入列。分級：Codex 評高風險，另兩方評 L2 |
| S5 | 互動式 rebase 編輯器內按 `Alt-↑/↓` | 選取列移動，沿用上下按鈕的排序與 squash 驗證 | 編輯器單元測試 | 編輯器目前遇修飾鍵就返回（Claude 引 `interactive_rebase.rs:1432-1435`，**未核對**） |
| S6 | 未暫存 diff 的 `U` | 依裁決而定 | 依裁決而定 | 受阻，見「需要使用者看的選擇」 |
| S7 | commit 右鍵「把已暫存變更併入此 commit」 | 先建 fixup，再開已排好的互動式 rebase 編輯器預覽，確認後套用 | 整合測試；L3 需要獨立驗收 | 沿用改訊息的防呆（first-parent、已推送先確認）。有未暫存變更時只允許建 fixup。Codex 引 `history_rebase.rs:770` 的 `run_planned_rebase`（**未核對**） |
| S8 | 工具列 Undo 擴及 checkout、rebase | 滑到 Undo 時以 chip 標出分支會回到哪裡 | L3 完整鏈 | 受阻。Claude 引 `git_undo.rs:299-309` 的 worktree 阻擋（**未核對**） |

## 狀態與證據

- 2026-11-26：計畫建立。來源是同日 Orca 三方討論的第 1 到第 3 輪統整總結；各方原文在當次 session，沒有落檔。
- 本文件的檔案位置與行號，除了標「未核對」的之外，都由 owner 在 `4b66a31e` 讀過原始碼。
- 尚未實作，也沒有建立 SDLC 紀錄。
- 2026-11-27：使用者裁決 F 鍵依 GitKraken 不綁，並授權依序實作。S1、S2、S5 已在 `feat/graph-multi-ref-gitkraken` 實作並提交（`0622ca71`、`d5f58d75`、`1d84be36`）。SDLC 紀錄：`AISDLC-2026-1126-revweave-lazygit-reflog-branch`（S1，已完成）、`AISDLC-2026-1127-revweave-fixup-commit`（S2）。
- S2 採直接提交、不開對話框，訊息為 `fixup! <標題>`；同名提交的配對問題留給 S7。
- S5 順帶補上 `docs/shortcuts.md` 既有 `Shift-F10` 列缺少的繁中翻譯，原本 `traditional_chinese_reference_covers_documented_groups_actions_and_notes` 因此失敗。
- 下一步：S3＋S4（bisect）跨 core、後端、狀態與 UI，開工前確認範圍。S6 等使用者裁決；S8 等 G3 審查狀態查明。
