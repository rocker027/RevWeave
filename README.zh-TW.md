# <img alt="RevWeave 圖示" src="assets/revweave.svg" width="32" /> RevWeave

[English](README.md) | **繁體中文**

**開源桌面 Git 客戶端**

RevWeave 以提交歷史、分支與差異檢視為核心，由 rocker027 維護，基於 AutoExplore Oy 開發的 [GitComet](https://github.com/Auto-Explore/GitComet)。修改內容與日期記錄於 [NOTICE](NOTICE)。

支援 macOS、Windows 與 Linux。Linux 可使用 Wayland、X11 或 WSLg。

### 下載

有發行版本時，安裝包及其對應原始碼會放在 [RevWeave 的 GitHub Releases](https://github.com/rocker027/RevWeave/releases)。

RevWeave 使用獨立的設定與工作階段目錄，不會讀寫原 GitComet 的設定。命令列與 Git diff／merge 工具使用 `revweave` 指令。

從舊版升級：舊版的指令名稱是 `gitcomet`。若曾執行 `gitcomet setup`，升級前先用舊版執行 `gitcomet uninstall`。如果已經升級、舊指令不在了，改用下列指令手動清除舊設定，再執行 `revweave setup`：

```bash
git config --global --remove-section mergetool.gitcomet
git config --global --remove-section mergetool.gitcomet-gui
git config --global --remove-section difftool.gitcomet
git config --global --remove-section difftool.gitcomet-gui
git config --global --remove-section gitcomet.backup
# 若 merge.tool／diff.tool／merge.guitool／diff.guitool 仍指向 gitcomet，一併移除
git config --global --get-regexp '^(merge|diff)\.(gui)?tool$'
```

不存在的區段會回報錯誤，可以忽略。若使用 `--local` 設定過，請在該 repository 內把 `--global` 換成 `--local` 再執行一次。

### 系統需求

Git 2.53 以上可使用完整功能。使用較舊的 Git 時，依賴新版 Git 的功能會停用。

### 功能

- 完整的本機優先桌面工作流程
- Git 遠端、[以分支為範圍的 Pull](docs/pull.md)、push、暫存與提交
- 可設定的開啟中工作區自動背景 fetch（[說明](docs/automatic-fetch.md)）
- 工作樹、分支與完整歷史，可隱藏 agent 檢查點與其他工具產生的 ref（[說明](docs/hidden-refs.md)）
- 瀏覽與轉換 bare repository（[說明](docs/bare-repositories.md)）
- 多 repository 工作區與多視窗
- 行內與並排差異、逐行暫存，以及 [Difftastic](docs/difftastic.md)
- 雙向與三向合併工具
- 互動式 rebase、cherry-pick、blame、stash 與子模組
- Reflog 與提交救回（[說明](docs/reflog.md)）、引導式 bisect（[說明](docs/bisect.md)）
- 內建編輯器、內嵌終端機與自訂佈景主題
- Git LFS：檔案狀態、內容下載、鎖定與追蹤（[說明](docs/git-lfs.md)）
- git-annex：內容 get／drop／copy／move、特殊遠端、adjusted 分支（[說明](docs/git-annex.md)）

### 從原始碼建置

Linux 請先安裝 Clang 與 mold，詳見[開發環境設定](CONTRIBUTING.md#getting-started)。

```bash
cargo build -p gitcomet --features ui-gpui,gix
cargo run -p gitcomet --features ui-gpui,gix -- /path/to/repo
```

Cargo 套件沿用上游名稱 `gitcomet`，建置出的執行檔是 `revweave`。

### 參與貢獻

開發環境設定、工作區結構、測試與覆蓋率說明都在 `CONTRIBUTING.md`。

### 作為 Git difftool／mergetool 使用

RevWeave 可以作為獨立的差異與合併工具，由 `git difftool` 和 `git mergetool` 呼叫。它支援無畫面（只跑演算法）與 GUI（互動式 GPUI 視窗）兩種模式。

#### 設定與移除（建議做法）

```bash
# 全域設定 Git 以 RevWeave 作為 difftool 與 mergetool
revweave setup

# 安全移除 RevWeave 整合
revweave uninstall
```

- 加上 `--local` 只設定目前的 repository，不動全域設定。
- 加上 `--dry-run` 先印出指令，不實際套用。

這個設定會同時註冊無畫面與 GUI 兩種工具，並設定 `guiDefault=auto`：有顯示環境時 Git 使用 GUI，否則改用無畫面模式。
`setup` 與 `uninstall` 都可以重複執行，結果相同。

<details>
<summary>顯示 setup／uninstall 的詳細行為與手動指令</summary>

內建的 `setup` 會寫入下列 Git 設定：

```bash
REVWEAVE_BIN="/absolute/path/to/revweave"

# 無畫面工具：只跑演算法的合併／差異，適用 CI、腳本與沒有顯示環境的情況
git config --global merge.tool revweave
git config --global mergetool.revweave.cmd \
  "'$REVWEAVE_BIN' mergetool --base \"\$BASE\" --local \"\$LOCAL\" --remote \"\$REMOTE\" --merged \"\$MERGED\""
git config --global mergetool.trustExitCode true
git config --global mergetool.revweave.trustExitCode true
git config --global mergetool.prompt false

git config --global diff.tool revweave
git config --global difftool.revweave.cmd \
  "'$REVWEAVE_BIN' difftool --local \"\$LOCAL\" --remote \"\$REMOTE\" --path \"\$MERGED\""
git config --global difftool.trustExitCode true
git config --global difftool.revweave.trustExitCode true
git config --global difftool.prompt false

# GUI 工具：開啟專用的 GPUI 視窗，互動式比對與合併
git config --global merge.guitool revweave-gui
git config --global mergetool.revweave-gui.cmd \
  "'$REVWEAVE_BIN' mergetool --gui --base \"\$BASE\" --local \"\$LOCAL\" --remote \"\$REMOTE\" --merged \"\$MERGED\""
git config --global mergetool.revweave-gui.trustExitCode true

git config --global diff.guitool revweave-gui
git config --global difftool.revweave-gui.cmd \
  "'$REVWEAVE_BIN' difftool --gui --local \"\$LOCAL\" --remote \"\$REMOTE\" --path \"\$MERGED\""
git config --global difftool.revweave-gui.trustExitCode true

# 有 DISPLAY 時自動選 GUI 工具，否則使用無畫面工具
git config --global mergetool.guiDefault auto
git config --global difftool.guiDefault auto
```

內建的 `setup` 會在需要時，把共用設定原本的值備份到 `revweave.backup.*`。
內建的 `uninstall` 只在設定值仍是 setup 寫入的值時才還原備份。如果使用者在 setup 之後改過設定，uninstall 會保留使用者的值，再移除 RevWeave 專屬的設定。

</details>

#### 命令列模式

**瀏覽器（預設）：**

```bash
revweave [path] [--foreground]
```

在 Unix 上，release 版本若標準輸入是終端機，會在獨立工作階段重新啟動瀏覽器；debug 版本預設不分離。分離啟動時，第一個原生視窗開啟（包含首次執行對話框）或既有瀏覽器接受請求後，指令就回傳成功，啟動診斷訊息仍會顯示。在這之前失敗會回傳結束碼 2；啟動超過 60 秒會逾時，並停止已啟動的程序群組。若完全無法啟動分離程序（例如沒有可寫入的 socket 目錄），瀏覽器會改在前景執行。指令不會等待 repository 載入、新手引導完成或視窗關閉。

加上 `--foreground`（或設定 `REVWEAVE_FOREGROUND=1`），新啟動的瀏覽器會留在前景，方便持續看診斷訊息，或讓腳本等待它結束。從終端機啟動的腳本也可能繼承終端機輸入而分離；要等待腳本啟動的瀏覽器，請用 `revweave --foreground "$repo"`。轉給既有瀏覽器的請求，仍會在對方接受後就回傳。從 Finder、`open` 或沒有終端機輸入的方式啟動時，不會自動分離。專用的 difftool 與 mergetool 模式一律等待，並回報最終結束碼。

`REVWEAVE_FOREGROUND` 接受 `1`／`true` 與 `0`／`false`（不分大小寫）。空值或未設定時採用建置預設值：啟用 debug assertions 的版本不分離，其餘自動分離。在 debug 版本設定 `REVWEAVE_FOREGROUND=0` 可改為分離。其他值會報錯；`--foreground` 優先於環境變數。未設定 `REVWEAVE_FOREGROUND` 時，仍會讀取舊的 `GITCOMET_FOREGROUND`。

以這套函式庫建置的產品，可用 `AppLaunch::run_cli()` 或 `App::run()` 啟用終端機分離。既有的 `AppLaunch::run()` 與 `run_with_args()` 呼叫不會自動分離。請在執行檔重新啟動時能重建設定的程序進入點使用 `run_cli()`。

送往既有瀏覽器的命令列請求，會在準備 hook 與擴充註冊之前轉送。新的瀏覽器只會在自己的程序裡做一次準備。啟動診斷訊息寫到呼叫端的標準錯誤；在就緒前啟動的輔助程序，可能在啟動器回傳後仍持有這個串流。

**Difftool：**

```bash
revweave difftool --local <path> --remote <path> [--path <display_name>] [--label-left <label>] [--label-right <label>]
```

由 Git 呼叫時，也會從環境變數 `LOCAL`／`REMOTE` 取得路徑作為備用。

**Mergetool：**

```bash
revweave mergetool --local <path> --remote <path> --merged <path> [--base <path>] [--label-local <label>] [--label-remote <label>] [--label-base <label>]
```

也會從環境變數 `LOCAL`／`REMOTE`／`MERGED`／`BASE` 讀取。add/add 衝突時可以不提供 base。

#### 相容性

支援 KDiff3 與 Meld 的呼叫格式（`--L1/--L2/--L3`、`-o/--output/--out`、`--base`、位置參數），所以 RevWeave 可以直接取代它們。

### 提交簽章驗證

驗證會拖慢 Git 歷史的顯示，所以預設關閉。開啟 **設定 → Git 歷史 → 驗證提交簽章** 後會顯示驗證標章。

RevWeave 會在已簽章的提交上顯示驗證標章。檢查由 Git 執行：GPG 與 X.509 簽章用 `gpg`，SSH 簽章用 `ssh-keygen`，所以需要這些程式以及簽署者的公鑰。**設定 → 執行檔** 會列出 RevWeave 找到的程式。

安裝 GnuPG、信任 GitHub 的簽章金鑰，以及設定 SSH allowed signers，請見[提交簽章指南](docs/commit-signatures.md)。

### Difftastic

在 **設定 → 差異 → 差異比對引擎** 把 Difftastic 設為預設，或用差異檢視器的齒輪暫時切換。並排與行內兩種版面都支援。部分暫存與備援行為請見 [Difftastic](docs/difftastic.md)。

### 佈景主題

RevWeave 支援內建佈景主題與使用者自訂佈景主題。

內建佈景主題包含在 RevWeave 執行檔裡。自訂佈景主題從每位使用者的佈景主題目錄載入 JSON 檔，RevWeave 啟動時會建立這個目錄。

完整的佈景主題說明，包括檔案位置、格式、範例與覆寫規則，請見 [THEMES.md](docs/themes.md)。

### 當機紀錄

RevWeave 將當機紀錄寫在以下路徑：

- Linux：`$XDG_STATE_HOME/revweave/crashes/`（預設 `~/.local/state/revweave/crashes/`）
- macOS：`~/Library/Logs/revweave/crashes/`
- Windows：`%LOCALAPPDATA%\revweave\crashes`（備用 `%APPDATA%\revweave\crashes`）

發生異常結束時，RevWeave 會在下次啟動顯示復原報告，並在終端機印出記錄路徑。報告包含 App 版本、平台、錯誤資訊及精簡的堆疊追蹤。

### 參考與靈感來源

SourceTree、GitKraken、Zed、GPUI、KDiff3、Meld、Github Desktop、Git、Gix、Rust、Smol 等等。

本專案在 AI 工具協助下開發，包括 OpenAI Codex 與 Claude Code。貢獻者如何使用這些工具，請見 [AI 政策](AI_POLICY.md)。

### 授權

RevWeave 依 [GNU Affero General Public License v3.0 only](LICENSE-AGPL-3.0)（AGPL-3.0-only）發行，可自由使用、修改與再散布，散布修改版時須同樣以 AGPL-3.0-only 提供對應原始碼。

RevWeave 基於 AutoExplore Oy 開發的 [GitComet](https://github.com/Auto-Explore/GitComet)。上游著作權為 Copyright (C) 2026 AutoExplore Oy，RevWeave 的修改著作權屬 rocker027，修改內容與日期見 [NOTICE](NOTICE)。

每個發行版本都須隨安裝包提供對應原始碼。App 設定視窗的「連結」區塊列出依賴名稱與授權代號，並非完整的第三方著作權及授權聲明。準備發行時仍須核對各安裝包內的聲明。

RevWeave 的名稱與圖示依 AGPL-3.0 第 7(e) 條不在授權範圍內。AGPL 也不授予 GitComet 或 AutoExplore Oy 名稱與圖示的商標權。RevWeave 是獨立維護的分支，未經 AutoExplore Oy 背書。
