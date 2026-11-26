//! Traditional Chinese presentation for the shortcut reference assembled from the Markdown docs.
//! Keep the English source searchable and use it as the fallback for new shortcuts.

pub(super) fn category(source: &str) -> Option<&'static str> {
    Some(match source {
        "Window" => "視窗",
        "Repository & layout" => "儲存庫與版面配置",
        "Images & diagrams" => "圖片與圖表",
        "Text input shortcuts" => "文字輸入",
        "Text editing" => "文字編輯",
        "Cursor & selection" => "游標與選取",
        "Word editing" => "字詞編輯",
        "Commit composer" => "撰寫提交",
        "Commit message editor" => "提交訊息編輯器",
        "Commit history" => "提交歷史",
        "Review navigation" => "變更檢閱導覽",
        "Pickers" => "選擇器",
        "File history" => "檔案歷史",
        "Terminal" => "終端機",
        "Diff & conflicts" => "差異與衝突",
        "Interactive rebase editor" => "互動式變基編輯器",
        "Branch drag and drop" => "分支拖放",
        "Context menus" => "右鍵選單",
        "Focused diff window" => "獨立差異視窗",
        "File explorer" => "檔案總管",
        _ => return None,
    })
}

pub(super) fn action(source: &str) -> Option<&'static str> {
    Some(match source {
        "Activate an inline menu shortcut" => "使用選單內的快捷鍵",
        "Activate selected menu item" => "執行選取的選單項目",
        "Activate selected result" => "開啟選取的結果",
        "Actual size (100%)" => "實際大小（100%）",
        "Align selected lines manually" => "手動對齊選取的行",
        "Cancel editing" => "取消編輯",
        "Cancel inline entry, pending cut, or transfer" => "取消行內輸入、尚未貼上的剪下項目或傳輸",
        "Checkout branch" => "切換分支",
        "Choose a conflict source everywhere" => "對所有衝突選用同一來源",
        "Clear all manual alignments" => "清除所有手動對齊",
        "Clear the filter" => "清除篩選",
        "Close menu or picker" => "關閉選單或選擇器",
        "Close the active repository tab, or close the window if no repo tab can close" => {
            "關閉目前的儲存庫分頁，若無分頁可關閉則關閉視窗"
        }
        "Close the active window" => "關閉目前視窗",
        "Close the find bar" => "關閉搜尋列",
        "Close the menu" => "關閉選單",
        "Close the window" => "關閉視窗",
        "Commit staged changes" => "提交已暫存的變更",
        "Commit staged changes and push" => "提交已暫存的變更並推送",
        "Copy" => "複製",
        "Copy / Cut / Paste" => "複製／剪下／貼上",
        "Copy selected diff text" => "複製選取的差異文字",
        "Copy terminal selection" => "複製終端機中選取的內容",
        "Copy the current file path" => "複製目前檔案路徑",
        "Create branch" => "建立分支",
        "Cut" => "剪下",
        "Decrease UI scale" => "縮小介面",
        "Delete to line start / end" => "刪除至行首／行尾",
        "Delete word to the left / right" => "刪除左／右側的字詞",
        "Discard current file changes" => "捨棄目前檔案的變更",
        "Discard the file's changes" => "捨棄檔案的變更",
        "Discard the selected lines" => "捨棄選取的行",
        "Drop the selected commit" => "捨棄選取的提交",
        "Move the selected commit up / down" => "上移／下移選取的提交",
        "Open the selected commit, worktree, or file menu" => "開啟選取的提交、工作樹或檔案選單",
        "Duplicate" => "製作副本",
        "Enter or leave the file editor" => "進入或離開檔案編輯器",
        "Expand / collapse or navigate folders" => "展開／收合或瀏覽資料夾",
        "Extend selection" => "延伸選取範圍",
        "Fetch all remotes" => "擷取所有遠端",
        "File list: expand / collapse folder" => "檔案清單：展開／收合資料夾",
        "File list: first / last row" => "檔案清單：第一列／最後一列",
        "File list: focus the diff" => "檔案清單：焦點移至差異檢視",
        "File list: previous / next page" => "檔案清單：上一頁／下一頁",
        "File list: previous / next row" => "檔案清單：上一列／下一列",
        "File list: return to graph" => "檔案清單：回到提交圖",
        "Filter the focused file list" => "篩選焦點所在的檔案清單",
        "Find a commit" => "搜尋提交",
        "First / last change block in this file" => "此檔案的第一個／最後一個變更區塊",
        "First / last commit" => "第一筆／最後一筆提交",
        "First / last delta" => "第一個／最後一個差異",
        "First / last result" => "第一筆／最後一筆結果",
        "First / last row" => "第一列／最後一列",
        "Fit" => "符合畫面大小",
        "Focus the commit message" => "焦點移至提交訊息",
        "Go to a commit" => "前往指定提交",
        "Graph: focus changed files" => "提交圖：焦點移至變更的檔案",
        "Graph: show working-copy changes" => "提交圖：顯示工作副本的變更",
        "Hide other applications" => "隱藏其他應用程式",
        "Hide the application" => "隱藏此應用程式",
        "Increase UI scale" => "放大介面",
        "Insert a newline in diff search" => "在差異搜尋中插入換行",
        "Mark the current conflict unresolved" => "將目前衝突標為未解決",
        "Merge, squash, rebase or push between two branches" => {
            "在兩個分支間合併、壓縮合併、變基或推送"
        }
        "Minimize the active window" => "最小化目前視窗",
        "Move by character or line" => "按字元或行移動",
        "Move by page" => "按頁移動",
        "Move focus and selection" => "移動焦點及選取位置",
        "Move focus without changing selection" => "移動焦點並保留選取範圍",
        "Move left / right by word" => "向左／向右移動一個字詞",
        "Move menu selection" => "移動選單選取位置",
        "Move previously copied files here" => "將先前複製的檔案移到這裡",
        "Move to document start / end" => "移至文件開頭／結尾",
        "Move to line start / end" => "移至行首／行尾",
        "Next / previous pane" => "下一個／上一個窗格",
        "Next change" => "下一項變更",
        "Next file" => "下一個檔案",
        "Next match" => "下一筆相符結果",
        "Next repository tab" => "下一個儲存庫分頁",
        "Next search match" => "下一筆搜尋結果",
        "Next unresolved conflict" => "下一個未解決的衝突",
        "Newer / older commit on the first-parent path" => "沿第一父提交路徑前往較新／較舊提交",
        "Open Settings" => "開啟設定",
        "Open a new window" => "開啟新視窗",
        "Open a repository" => "開啟儲存庫",
        "Open a workspace" => "開啟工作區",
        "Open active repository in external code editor" => "在外部程式碼編輯器中開啟目前的儲存庫",
        "Open commit dialog" => "開啟提交對話框",
        "Open external terminal" => "開啟外部終端機",
        "Open focused item" => "開啟焦點所在的項目",
        "Open the command palette" => "開啟命令面板",
        "Open the current file in the code editor" => "在程式碼編輯器中開啟目前檔案",
        "Open the file history tab" => "開啟檔案歷史分頁",
        "Open the repository's remote in a web browser" => "在瀏覽器中開啟儲存庫的遠端",
        "Open the selected commit/file menu" => "開啟選取的提交或檔案選單",
        "Paste" => "貼上",
        "Paste clipboard text" => "貼上剪貼簿文字",
        "Pick a conflict source" => "選擇衝突來源",
        "Pick conflict result Base / Ours / Theirs / Both" => {
            "選擇衝突結果：基底／我方／對方／兩者"
        }
        "Pick the selected commit" => "採用選取的提交",
        "Previous / next change block" => "上一個／下一個變更區塊",
        "Previous / next commit" => "上一筆／下一筆提交",
        "Previous / next page" => "上一頁／下一頁",
        "Previous / next result" => "上一筆／下一筆結果",
        "Previous / next row" => "上一列／下一列",
        "Previous change" => "上一項變更",
        "Previous file" => "上一個檔案",
        "Previous match" => "上一筆相符結果",
        "Previous repository tab" => "上一個儲存庫分頁",
        "Previous search match" => "上一筆搜尋結果",
        "Previous unresolved conflict" => "上一個未解決的衝突",
        "Pull" => "拉取",
        "Push" => "推送",
        "Quit application" => "結束應用程式",
        "Redo" => "重做",
        "Redo the last undone Git operation" => "重做上一個已復原的 Git 操作",
        "Reload repository" => "重新載入儲存庫",
        "Rename" => "重新命名",
        "Reset UI scale" => "重設介面縮放",
        "Return to the file list" => "回到檔案清單",
        "Reword the selected commit" => "編輯選取的提交訊息",
        "Save the edited file" => "儲存編輯過的檔案",
        "Scroll terminal history" => "捲動終端機歷史",
        "Scroll the diff by one page" => "捲動差異一頁",
        "Search the current diff" => "搜尋目前的差異",
        "Select all" => "全選",
        "Select all diff text" => "全選差異文字",
        "Select all terminal buffer" => "全選終端機緩衝區內容",
        "Select all visible rows" => "全選可見列",
        "Select by character or line" => "按字元或行選取",
        "Select by page" => "按頁選取",
        "Select left / right by word" => "向左／向右選取一個字詞",
        "Select to line start / end" => "選取至行首／行尾",
        "Show keyboard shortcuts" => "顯示鍵盤快捷鍵",
        "Show the character palette" => "顯示字元檢視器",
        "Show the open file in the file explorer" => "在檔案總管中顯示已開啟的檔案",
        "Squash the selected commit into the one below" => "將選取的提交壓縮合併到下方的提交",
        "Stage all changes" => "暫存所有變更",
        "Stage or unstage the current working-tree file and advance to the adjacent file" => {
            "暫存或取消暫存目前工作樹檔案，並移至相鄰檔案"
        }
        "Stage the current or selected working-tree files" => "暫存目前或選取的工作樹檔案",
        "Stage the selected lines" => "暫存選取的行",
        "Stash changes" => "貯藏變更",
        "Switch to inline diff" => "切換至單欄差異",
        "Switch to repository tab 1–9" => "切換至第 1–9 個儲存庫分頁",
        "Switch to split diff" => "切換至並排差異",
        "Toggle blame / annotation" => "切換逐行提交資訊",
        "Toggle details pane" => "切換詳情面板",
        "Toggle embedded terminal" => "切換內嵌終端機",
        "Toggle full screen" => "切換全螢幕",
        "Toggle open and recently closed repositories" => "切換已開啟與最近關閉的儲存庫",
        "Toggle sidebar" => "切換側欄",
        "Toggle the terminal" => "切換終端機",
        "Toggle whitespace characters" => "切換空白字元顯示",
        "Trash / permanent deletion" => "移至垃圾桶／永久刪除",
        "Undo" => "復原",
        "Undo / Redo file operation" => "復原／重做檔案操作",
        "Undo the last Git operation" => "復原上一次 Git 操作",
        "Unstage all changes" => "取消暫存所有變更",
        "Unstage the current or selected files" => "取消暫存目前或選取的檔案",
        "Unstage the selected lines" => "取消暫存選取的行",
        "Update commit" => "更新提交",
        "Zoom around pointer" => "以游標位置為中心縮放",
        "Zoom in / out" => "放大／縮小",
        _ => return None,
    })
}

pub(super) fn note(source: &str) -> Option<&'static str> {
    Some(match source {
        "Also the keyboard button in the bottom bar. Opens the searchable cheat sheet; Escape closes it and restores focus. With terminal focus on Windows/Linux, use Ctrl-Shift-/." => {
            "也可從底部狀態列的鍵盤按鈕開啟這份可搜尋的快捷鍵說明。按 Escape 關閉並還原焦點。Windows／Linux 的終端機取得焦點時，請按 Ctrl-Shift-/。"
        }
        "Also the toolbar's Redo button. Cmd-Y / Ctrl-Y stays with the conflict resolver's manual alignment." => {
            "也可使用工具列上的「重做」。Cmd-Y／Ctrl-Y 保留給衝突解決器的手動對齊。"
        }
        "Also the × button. Focus returns to the history list." => {
            "也可按 ×。焦點會回到歷史清單。"
        }
        "As in GitKraken. The conflict resolver keeps Cmd-1–Cmd-3 / Ctrl-1–Ctrl-3 for picking a side." => {
            "操作方式與 GitKraken 相同。Cmd-1 至 Cmd-3／Ctrl-1 至 Ctrl-3 在衝突解決器中用於選擇來源。"
        }
        "Available from the palette and terminal controls." => "可從命令面板與終端機控制項開啟。",
        "Case-insensitive path substring. Enter or Down focuses the results; Escape clears the filter. Diff navigation follows the filtered list." => {
            "搜尋路徑中相符的片段，忽略大小寫。按 Enter 或 Down 將焦點移至結果，按 Escape 清除篩選。差異導覽會按篩選後的清單移動。"
        }
        "Clipboard CRLF/CR line endings are normalized to LF. Bracketed paste is used when the shell enables it." => {
            "剪貼簿的 CRLF／CR 換行會轉成 LF。Shell 啟用括號式貼上時會沿用該方式。"
        }
        "Closes a row's context menu first, keeping the picker open." => {
            "若列的右鍵選單已開啟，會先關閉選單，保留選擇器。"
        }
        "Closes file search first. Restores the same commit and graph position." => {
            "先關閉檔案搜尋，再回到同一筆提交及提交圖位置。"
        }
        "Closes the active search first; otherwise replaces Commit details with Unstaged/Staged while keeping the graph selection." => {
            "先關閉目前的搜尋。若沒有搜尋，顯示未暫存與已暫存的變更，並保留提交圖的選取位置。"
        }
        "Closing a running command keeps its confirmation. Plain Ctrl-J reaches the shell." => {
            "若有執行中的命令，關閉時會要求確認。單獨按 Ctrl-J 會交給 Shell。"
        }
        "Cmd-L / Ctrl-L match GitKraken." => "Cmd-L／Ctrl-L 與 GitKraken 相同。",
        "Cmd-Shift-U / Ctrl-Shift-U match GitKraken." => {
            "Cmd-Shift-U／Ctrl-Shift-U 與 GitKraken 相同。"
        }
        "Commit message input only, and only when the Commit & Push action is enabled. It never stages for you: with nothing staged it stays off." => {
            "只在提交訊息欄位且「提交並推送」可用時生效。沒有暫存變更時無法執行，快捷鍵也不會替你暫存。"
        }
        "Commit message input only, and only when the Commit action is enabled. With nothing staged, the button reads Stage All & Commit and this stages every change, untracked files included, before committing. Not during an amend, a merge or rebase, or with a conflict." => {
            "只在提交訊息欄位且「提交」可用時生效。沒有暫存時按鈕顯示「全部暫存並提交」，會先暫存所有變更，包括未追蹤檔案。修訂提交、合併、變基或遇到衝突時不執行。"
        }
        "Commits, branch creation and deletion, and soft resets made since the application started, as in GitKraken. Also the toolbar's Undo button. A commit or soft reset can only be undone with its branch checked out; its changes go back to the staged files. Refused, without changing anything, when a branch moved since or is checked out in another worktree. Any other operation that moves a branch or HEAD (a checkout, merge, rebase, pull, mixed or hard reset...) ends what can be undone. A recreated branch gets its commit back, not its upstream setting. Text inputs and the Files explorer keep the key for their own undo." => {
            "可復原應用程式啟動後的提交、分支建立與刪除、以及軟重設，操作和工具列的「復原」相同。提交與軟重設須在原分支上復原，變更會回到暫存區。若分支已移動或由其他工作樹簽出，會拒絕執行並保持原狀。切換分支、合併、變基、拉取或混合／硬重設等移動分支或 HEAD 的操作，會清除可復原的紀錄。重建分支會還原到原提交，上游設定需另外設定。文字欄位及檔案總管各自保留復原快捷鍵。"
        }
        "Conflict resolver only (kdiff3 manual diff help)." => {
            "僅適用於衝突解決器（kdiff3 手動對齊）。"
        }
        "Conflict resolver only, outside text inputs." => {
            "僅適用於衝突解決器，且焦點須在文字欄位之外。"
        }
        "Conflict resolver only." => "僅適用於衝突解決器。",
        "Conflict resolver only. Not active while the resolved-output editor has focus, which keeps them for cursor movement." => {
            "僅適用於衝突解決器。輸出編輯器取得焦點時，這些按鍵保留給游標移動。"
        }
        "Conflict resolver only. Skips conflicts you have already resolved; works while the resolved-output editor has focus." => {
            "僅適用於衝突解決器。會跳過已解決的衝突，輸出編輯器取得焦點時也可使用。"
        }
        "Conflict resolver only; also works from the resolved-output editor. Ctrl aliases work on macOS too. Source choices depend on the two-way or three-way layout." => {
            "僅適用於衝突解決器，輸出編輯器中也可使用。macOS 也接受 Ctrl 組合。可用來源依兩方或三方檢視而定。"
        }
        "Ctrl-A is also accepted on macOS." => "macOS 也接受 Ctrl-A。",
        "Ctrl-C is also accepted on macOS." => "macOS 也接受 Ctrl-C。",
        "Ctrl-Home / Ctrl-End also work on macOS." => "macOS 也接受 Ctrl-Home／Ctrl-End。",
        "Ctrl-V is also accepted on macOS." => "macOS 也接受 Ctrl-V。",
        "Ctrl-W remains accepted on macOS as an extra alias." => "macOS 也接受 Ctrl-W。",
        "Ctrl-X is also accepted on macOS." => "macOS 也接受 Ctrl-X。",
        "Diff search only. The search box also has Match Case, Whole Word, and Regex toggles." => {
            "僅適用於差異搜尋。搜尋欄也能切換區分大小寫、全字相符與規則運算式。"
        }
        "Disabled/no-op without a terminal selection." => "未選取終端機內容時無法執行。",
        "Discards the edited message." => "捨棄已編輯的訊息。",
        "Escape restores terminal focus. Plain Ctrl-/ reaches the shell." => {
            "按 Escape 將焦點還給終端機。單獨按 Ctrl-/ 會交給 Shell。"
        }
        "File preview and text-selection flows." => "適用於檔案預覽與文字選取。",
        "Focused diff only, outside the conflict resolver." => {
            "僅在差異檢視取得焦點且焦點不在衝突解決器中時生效。"
        }
        "Focused diff only. At a file boundary, continues to the previous file’s last block or the next file’s first block. Stops at the outermost files. Images and files without text changes remain review stops. Conflicts retain their own navigation." => {
            "僅在差異檢視取得焦點時生效。到達檔案邊界時，繼續至上一個檔案的最後一個區塊，或下一個檔案的第一個區塊。到達清單兩端即停止。圖片與沒有文字變更的檔案也會停留。衝突沿用原有的導覽方式。"
        }
        "In an embedded terminal on Windows/Linux, Ctrl-Shift-A keeps its terminal “Select All” behavior." => {
            "在 Windows／Linux 的內嵌終端機中，Ctrl-Shift-A 保留終端機的全選功能。"
        }
        "Includes automatically selected blocks. Ctrl aliases work on macOS too." => {
            "包含自動選取的區塊。macOS 也接受 Ctrl 組合。"
        }
        "Left, Right, Up, Down" => "Left、Right、Up、Down",
        "Lists saved workspaces. In an empty window the chosen workspace opens there; otherwise its own window is focused or opened. Also in the command palette and the app menu." => {
            "列出已儲存的工作區。若目前視窗為空，便在此開啟選取的工作區。否則切換到該工作區視窗，或為它開啟新視窗。也可從命令面板與應用程式選單開啟。"
        }
        "Moves by one visible page and shows the commit." => "移動一頁可見內容，並顯示對應的提交。",
        "Moves by one visible page and stops at the first / last result." => {
            "移動一頁可見內容，到第一筆或最後一筆結果停止。"
        }
        "Moves one change block (a run of consecutive changed lines) at a time, landing on its first line and outlining the block, in Full, Collapsed, and whole-commit diffs alike. Pressed outside a text field, also moves keyboard focus into the diff, so S and U act on the outlined block. The conflict resolver moves by conflict instead." => {
            "每次移動一個變更區塊（相連的變更行），選中第一行並框出區塊。完整、收合與整筆提交差異都適用。焦點在文字欄位外時，也會把焦點移到差異，讓 S 和 U 作用於框出的區塊。衝突解決器會按衝突移動。"
        }
        "Needs an earlier commit to fold into." => "必須有較早的提交才可合併。",
        "Not while a text field has focus. Escape also leaves the editor." => {
            "僅在文字欄位沒有焦點時生效。也可按 Escape 離開編輯器。"
        }
        "One change block at a time, as in the main diff view." => {
            "與主要差異檢視相同，每次移動一個變更區塊。"
        }
        "Only active when an external code editor is configured." => {
            "設定外部程式碼編輯器後才可用。"
        }
        "Only in the normal screen buffer." => "僅適用於終端機的一般畫面緩衝區。",
        "Opens a menu for the pair. Merge, squash and push start at once, as in the branch menu and the toolbar; rebase asks first; interactive rebase opens the editor. Merges go into the checked-out branch and rebases move it, so pairs that need another branch checked out are listed greyed out with the reason. Push goes to the checked-out branch's upstream; a branch without one can push to the remote branch of its own name, through the first-push prompt (directly when git-annex manages push)." => {
            "開啟兩個分支的操作選單。合併、壓縮合併和推送立即執行，與分支選單及工具列相同。變基會先詢問，互動式變基則開啟編輯器。合併會納入已切換至的分支，變基會移動該分支。必須先切換到另一個分支的項目會停用並顯示原因。推送目標為目前分支的上游。若未設定上游，可透過首次推送視窗推送至同名遠端分支。由 git-annex 管理時直接推送。"
        }
        "Opens the Go to dialog. Accepts a full or short SHA (4+ characters, unique), a branch, a tag, or any revision such as HEAD~3; matches are exact. In an embedded terminal the shell keeps Ctrl-G." => {
            "開啟「前往提交」對話框。可輸入完整或至少 4 碼且唯一的短 SHA、分支、標籤或 HEAD~3 等修訂版本，並精確比對。內嵌終端機中的 Ctrl-G 會交給 Shell。"
        }
        "Opens the branch creation dialog." => "開啟建立分支對話框。",
        "Opens the branch picker." => "開啟分支選擇器。",
        "Opens the details pane if hidden." => "若詳情面板已隱藏，會先開啟。",
        "Opens the existing confirmation dialog." => "開啟既有確認對話框。",
        "Opens the find bar over the history. Matches the commit summary or the author (each on its own), or the start of the SHA (4+ hex characters), across the whole history, not only the loaded rows. Typing selects the first match. With the bar already open, refocuses it with the query selected." => {
            "在提交歷史上方開啟搜尋列。搜尋涵蓋完整歷史，包括尚未載入的列。可比對提交摘要、作者，或至少 4 碼的十六進位 SHA 開頭。輸入時會選取第一筆結果。若搜尋列已開啟，焦點會回到搜尋列並全選查詢。"
        }
        "Opens the message dialog." => "開啟提交訊息對話框。",
        "Opens the remote's page on its host (GitHub, GitLab, Bitbucket, Azure DevOps, Gitea/Codeberg, AWS CodeCommit, or a self-hosted forge). With several such remotes a menu lists them, origin first; 1–9 pick directly. Also in the command palette, the app menu, and a remote's right-click menu in the sidebar. In an embedded terminal on Windows/Linux the shell keeps Ctrl-K." => {
            "開啟遠端託管服務上的儲存庫網頁，例如 GitHub、GitLab、Bitbucket、Azure DevOps、Gitea／Codeberg、AWS CodeCommit 或自架服務。有多個可開啟的遠端時，選單把 origin 放在第一個，可按 1–9 選取。命令面板、應用程式選單與側欄的遠端右鍵選單也可開啟。Windows／Linux 的內嵌終端機會把 Ctrl-K 交給 Shell。"
        }
        "Opens the stash dialog." => "開啟貯藏對話框。",
        "Opens upstream setup when required. Available from the palette and buttons." => {
            "需要時會開啟上游設定。也可從命令面板與按鈕執行。"
        }
        "Option/Alt-Down from the focused diff lands at the next file’s first change. When the open file has left the list (its last change was staged or discarded), opens the list's first file." => {
            "差異檢視取得焦點時，按 Option/Alt-Down 會移到下一個檔案的第一項變更。若目前檔案已因最後一項變更被暫存或捨棄而離開清單，會開啟清單的第一個檔案。"
        }
        "Option/Alt-Up from the focused diff lands at the previous file’s last change. When the open file has left the list (its last change was staged or discarded), opens the list's first file." => {
            "差異檢視取得焦點時，按 Option/Alt-Up 會移到上一個檔案的最後一項變更。若目前檔案已因最後一項變更被暫存或捨棄而離開清單，會開啟清單的第一個檔案。"
        }
        "Outside the file editor. Conflicts with unresolved markers require confirmation." => {
            "僅在檔案編輯器關閉時適用。有未解決的衝突標記時會要求確認。"
        }
        "Picker search inputs also select the first / last result." => {
            "選擇器的搜尋欄也會選取第一筆或最後一筆結果。"
        }
        "Previews files and keeps focus in the list." => "預覽檔案，焦點留在清單。",
        "Raw file diff only." => "僅適用於原始檔案差異。",
        "Raw file diff only. Conflict resolver keeps split layout." => {
            "僅適用於原始檔案差異。衝突解決器保留並排檢視。"
        }
        "Raw text diff." => "適用於原始文字差異。",
        "Raw working-tree file diff only, and not while the diff search input has focus." => {
            "僅適用於原始工作樹檔案差異，且焦點須在差異搜尋欄位之外。"
        }
        "Rendered Markdown previews search their rendered text and expand disclosures containing the current match." => {
            "Markdown 預覽會搜尋轉譯後的文字，並展開包含目前結果的區段。"
        }
        "Requires a configured external editor." => "須先設定外部編輯器。",
        "Review and submit in the dialog; Cmd-Enter / Ctrl-Enter still submits from the commit message input. With nothing staged, the dialog stages every change first, like the Commit button; the palette's Stage All and Commit opens the same dialog." => {
            "在對話框檢查並提交。提交訊息欄仍可按 Cmd-Enter／Ctrl-Enter 直接提交。若沒有已暫存的變更，會像「提交」按鈕一樣先暫存所有變更。命令面板的「全部暫存並提交」也會開啟此對話框。"
        }
        "Right enters an expanded folder; Left returns to its parent. Enter toggles folders and groups." => {
            "按 Right 進入已展開的資料夾，按 Left 回到上層。按 Enter 展開或收合資料夾與群組。"
        }
        "Same unit as Previous change. With nothing selected, goes to the first change block." => {
            "與「上一項變更」採相同的移動單位。未選取時會移至第一個變更區塊。"
        }
        "Search for an action; its shortcut is shown beside it." => {
            "搜尋操作名稱，對應的快捷鍵會顯示在操作旁。"
        }
        "Selects the commit and shows it in the main diff at once." => {
            "選取提交，並立即在主要差異檢視中顯示。"
        }
        "Selects the next match below the selected commit, wrapping to the top. Enter only from the find bar; F3 also from the list. Also the ↓ button." => {
            "選取目前提交下方的下一筆相符結果，到底後回到頂端。Enter 僅適用於搜尋列，F3 也可從清單使用。也可按 ↓。"
        }
        "Selects the previous match above the selected commit, wrapping to the bottom. Shift-Enter only from the find bar; F2 also from the list. Also the ↑ button." => {
            "選取目前提交上方的上一筆相符結果，到頂後回到底端。Shift-Enter 僅適用於搜尋列，F2 也可從清單使用。也可按 ↑。"
        }
        "Selects the scrollback buffer plus the visible terminal grid." => {
            "選取終端機的捲動歷史與目前畫面。"
        }
        "Shift-Tab / Tab also navigate." => "也可按 Shift-Tab／Tab 移動。",
        "Shows the commit. Ctrl-Home / Ctrl-End also work on macOS." => {
            "顯示提交。macOS 也接受 Ctrl-Home／Ctrl-End。"
        }
        "Shows the working-copy changes in the details pane and puts the cursor in the commit message, as in GitKraken." => {
            "顯示詳情面板中的工作副本變更，並將游標移至提交訊息欄，方式與 GitKraken 相同。"
        }
        "Staged diff, while the diff has focus. No modifiers. The same as \"Unstage lines\" in the right-click menu. In an unstaged diff the same key discards instead (next row)." => {
            "已暫存差異取得焦點時，無須修飾鍵。等同於右鍵選單的「取消暫存選取的行」。在未暫存差異中，同一個鍵用於捨棄。"
        }
        "Staged file diff or status section." => "適用於已暫存檔案的差異或狀態區段。",
        "Stops at the ends." => "移至兩端即停止。",
        "Switches the sidebar to Files, expands the folders leading to the file, and scrolls it into view." => {
            "把側欄切換到「檔案」，展開通往該檔案的資料夾，並捲動到檔案位置。"
        }
        "Text diff / conflict diff only." => "僅適用於文字差異或衝突差異。",
        "The last remaining commit cannot be dropped." => "無法捨棄最後一筆提交。",
        "The same as the row's ▲/▼ buttons; the first and last rows stay." => {
            "與該列的 ▲／▼ 按鈕相同；第一列與最後一列不會移動。"
        }
        "The selected file stays open." => "選取的檔案會保持開啟。",
        "The tab stays open." => "分頁會保持開啟。",
        "Unavailable while later commits squash into it." => {
            "有後續提交要壓縮合併至這筆提交時不可用。"
        }
        "Unresolved conflict markers still require confirmation." => {
            "若仍有未解決的衝突標記，會要求確認。"
        }
        "Unstaged working-tree diff or a focused unstaged file list, after a confirmation. In a staged diff or list the same chord unstages, so a menu shows one or the other, never both. Cmd-D / Ctrl-D no longer discards: it is next to Cmd-S / Ctrl-S." => {
            "在未暫存的工作樹差異或取得焦點的未暫存檔案清單中生效，須先確認。相同按鍵在已暫存差異或清單中會取消暫存，選單只會顯示目前適用的操作。Cmd-D／Ctrl-D 與 Cmd-S／Ctrl-S 相鄰，已不再用於捨棄。"
        }
        "Unstaged working-tree diff, while the diff has focus. No modifiers. Acts on the lines selected by dragging over text, by selecting rows, or by right-clicking a line: a right-click focuses the diff, and once its menu closes the keys act on the lines that menu did until the selection changes. With nothing selected it acts on the change block outlined by change navigation or by opening the diff; a mouse selection removes that outline. Pressed outside the diff (e.g. in the file list), but not in a text field, it acts only on that outlined block; the same holds for U. After a plain click (a caret only) it does nothing. Once the diff reloads, the next change block is outlined, so S, S, S stages a file block by block. The same as \"Stage lines\" in the diff's right-click menu." => {
            "未暫存的工作樹差異取得焦點時，無須修飾鍵。作用於滑鼠拖曳、選取列或右鍵所選取的行。右鍵會把焦點帶到差異，關閉選單後快捷鍵仍對同一批行生效，直到選取變更。沒有選取時，作用於導覽或開啟差異時框出的變更區塊。用滑鼠選取會清除外框。焦點在差異外（例如檔案清單）且不在文字欄位時，只會作用於框出的區塊，U 也相同。只點一下放置游標時不會執行。重新載入差異後會框出下一個變更區塊，連續按 S 可逐區塊暫存。等同於差異右鍵選單的「暫存選取的行」。"
        }
        "Unstaged working-tree diff, while the diff has focus. No modifiers. Reverts the selected lines, or the outlined change block, in the file without asking, as \"Discard lines\" in the right-click menu does. U rather than D, so discard is not next to S; unstage shares the key but only applies in a staged diff. D does nothing in a diff (in the conflict resolver it picks Both)." => {
            "未暫存的工作樹差異取得焦點時，無須修飾鍵。等同於右鍵選單的「捨棄選取的行」，會直接還原選取的行或框出的變更區塊。捨棄使用 U，和 S 不相鄰。同一個鍵只在已暫存差異中用於取消暫存。D 在差異中沒有作用，在衝突解決器中則會選擇兩者。"
        }
        "Uses the configured upstream. Available from the palette and buttons." => {
            "使用已設定的上游。可從命令面板與按鈕執行。"
        }
        "Uses the existing context menu and its confirmations." => "沿用既有的右鍵選單及確認流程。",
        "Uses the first enabled item if nothing is selected." => {
            "若未選取項目，會執行第一個可用項目。"
        }
        "Uses the visible list height." => "依清單的可見高度移動。",
        "Visible panes only; terminal input and modal dialogs keep focus." => {
            "僅在可見的窗格間移動。終端機的輸入欄與強制回應對話框會保留焦點。"
        }
        "While diff search is open. The ↑ button in the search box does the same." => {
            "在差異搜尋開啟時生效。也可按搜尋欄的 ↑。"
        }
        "While diff search is open. The ↓ button in the search box does the same." => {
            "在差異搜尋開啟時生效。也可按搜尋欄的 ↓。"
        }
        "While editing a commit's message in Commit details, and only when Update commit is enabled. For a commit already on the upstream, the first press asks to confirm." => {
            "在提交詳情中編輯提交訊息，且「更新提交」可用時生效。若該提交已在上游，第一次按下時會要求確認。"
        }
        "While the file editor is open, wherever focus is. Outside the editor the same chord stages the file." => {
            "檔案編輯器開啟時，不限焦點位置。編輯器關閉時，相同按鍵會暫存檔案。"
        }
        "While the history list has focus." => "在歷史清單取得焦點時生效。",
        "While the history list has focus. Without modifiers, K moves up one row and J moves down one row. Plain Up/Down keep their existing row behavior." => {
            "在歷史清單取得焦點時生效。未按修飾鍵時，K 往上一列，J 往下一列。單獨按上／下方向鍵仍沿用原有的逐列操作。"
        }
        "While the graph has focus and the current selection is a visible commit. Shift-Up/Shift-K moves to a newer commit; Shift-Down/Shift-J moves to an older commit along the first-parent path. Reversing at a fork prefers the path just followed. Skips worktree rows and stops at the current graph boundary. Shift-click keeps its existing range-selection behavior." => {
            "圖譜取得焦點，且目前選取的是可見提交時生效。Shift-Up／Shift-K 沿第一父提交路徑前往較新提交，Shift-Down／Shift-J 前往較舊提交。在分岔處反向移動時，優先沿剛走過的路徑返回。會跳過工作樹列，並在目前圖譜的邊界停止。Shift-click 仍沿用原有的範圍選取操作。"
        }
        "While the history list has focus. Also with nothing selected." => {
            "在歷史清單取得焦點時生效，即使尚未選取也可使用。"
        }
        "While the history list has focus. Moves selection by one visible page, updates the selected row's details, and stops at the first / last available row. With nothing selected, selects the first row." => {
            "在歷史清單取得焦點時生效。選取位置會移動一頁可見內容，更新該列的詳情，並在清單兩端停止。若尚未選取，會選取第一列。"
        }
        "With diff focus, returns to the matching row and keeps its preview open. If no row matches, closes the preview and focuses the graph. Closes search, an editor, or an inline submodule first. Standalone diff windows retain their close behavior." => {
            "差異檢視取得焦點時，返回對應的列並保留預覽。沒有相符列時關閉預覽，回到提交圖。若搜尋、編輯器或行內子模組已開啟，會先關閉它。獨立差異視窗保留原有關閉方式。"
        }
        "With terminal focus on Windows/Linux, use Ctrl-Shift-J. Closing an active session keeps the existing confirmation." => {
            "Windows／Linux 終端機取得焦點時按 Ctrl-Shift-J。結束進行中的工作階段時仍會要求確認。"
        }
        "With the caret at the end of a single-line input, such as a search box, deleting to the line start clears it. At the start or end of a line, deletes the line break instead." => {
            "在搜尋框等單行欄位的末尾，刪除至開頭會清空欄位。在多行文字的行首或行尾會刪除換行。"
        }
        "Working-tree file diff." => "適用於工作樹的檔案差異。",
        "Working-tree files and files viewed at a commit; not while a text field has focus." => {
            "適用於工作樹檔案及指定提交中的檔案。文字欄位有焦點時不適用。"
        }
        "macOS only." => "僅限 macOS。",
        _ => return None,
    })
}
