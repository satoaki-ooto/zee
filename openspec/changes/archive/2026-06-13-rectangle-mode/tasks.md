## 1. 安全網（着手前・依存ゼロ・いつでも）

- [x] 1.1 `zee-edit/src/lib.rs` の既存 selection 保護テストを追加（`begin_selection` / `clear_selection` / `select_all` / `selection()` 正規化の回帰テスト）
- [x] 1.2 `delete_selection` の保護テストを追加（削除範囲・カーソル・selection リセットの現挙動を固定）
- [x] 1.3 既存の単一 diff / undo / redo の挙動を固定するテストを `tree.rs` に確認・追加（C-3 の基準点）

## 2. Phase 0 — 継ぎ目の型（衝突核の手前・最小宣言）

- [x] 2.1 矩形座標型（行範囲 × visual column 範囲）の最小宣言を `zee-edit/src/lib.rs` に置く（振る舞いは持たせない、`Cursor` から参照される前提）
- [x] 2.2 多領域 diff の形（compound diff: 単一連続 `OpaqueDiff` の順序付き列）の最小宣言を `zee-edit/src/diff.rs` に置く（D1/D2）
- [x] 2.3 通常編集が「列の長さ1」の退化ケースとして既存経路を通ることを確認（C-3 の構造的担保）

## 3. Scope 2 — 列計算（[A] 孤立・純粋追加・即着手可）

- [x] 3.1 `zee-edit/src/graphemes.rs` に矩形 (visual column `[left, right)`, 行) → char range 写像を純粋追加（grapheme 幅準拠、CJK=2 / tab=`tab_width`、C-4）
- [x] 3.2 ragged line ポリシー（空扱い ↔ 空白パディング）を単一の分岐点として隔離（D5、後から1箇所で差し替え可能に）
- [x] 3.3 列写像の単体テスト: ASCII / CJK / tab / 行末 < left / 行末が範囲途中まで（spec `rectangle-column-mapping` の各シナリオ）
- [x] 3.4 同一矩形でハイライト判定・コピー切り出し・カット削除が同一 char range を返すことのテスト（3者規則共有）

## 4. Scope 1 — 編集モデルの不連続複数領域対応（[B] クリティカルパス）

- [x] 4.1 compound diff の確定形を決める（`CompoundDiff` 新規型か `Revision.diffs: Vec<OpaqueDiff>` か、D1 Open Question）
- [x] 4.2 `EditTree::create_revision()` を diff 列対応に拡張（`tree.rs:69`、1リビジョンに複数領域を束ねる）
- [x] 4.3 `undo` / `redo`（`tree.rs:98` / `:115`）を、各サブ diff の `reverse()` を逆順適用する原子操作に拡張
- [x] 4.4 多領域削除の適用順序を実装（元 Rope に対し全 char range を一括計算し char_index 降順で適用、undo は逆順）
- [x] 4.5 `Cursor::reconcile()`（`lib.rs:105`）を compound 対応に拡張。`modified_range = char_index..max(old,new)` の次元混在疑いを単体テストで確認し、踏むなら是正
- [x] 4.6 `zee/src/editor/buffer.rs` 451-463 の単一 diff 契約を compound 受け入れに拡張、`syntax/parse.rs` の parse tree 更新連携を確認
- [x] 4.7 C-1 テスト: N行不連続削除を1リビジョンとして undo 1回で全行同時に戻り、redo 1回で全行再適用（spec `block-edit-model`）
- [x] 4.8 C-2 テスト: 範囲外（`[0,left)` / `[right,行末]` / 選択行外）が変更されないこと
- [x] 4.9 C-3 テスト: 通常の単一連続編集の undo 粒度・カーソル追従・selection 正規化が不変（1.1-1.3 の基準点と一致）

## 5. Scope 3 — 矩形選択状態（衝突核・波1で先に land）

- [x] 5.1 矩形モード用の (行, visual column) アンカー状態を `Cursor` に追加。通常選択（`selection: Option<CharIndex>`）と排他にする（D3）
- [x] 5.2 進入操作: 進入時点のカーソルをアンカー角に固定、幅0・高さ1から開始（spec `rectangle-selection` 進入）
- [x] 5.3 対角更新と正規化: 移動キーで対角を動かし (行範囲 × visual column 範囲) を min..max で正規化（向き非依存）。`movement.rs` の `pub(crate)` フィールドアクセスへの波及を抑える
- [x] 5.4 EOL を越える列を許す（右端列はカーソル visual column で決まり行の文字長に縛られない）
- [x] 5.5 キャンセル（`C-g` 相当）で矩形破棄・カーソル不動・バッファ不変。幅0矩形は選択状態として成立（no padding）
- [x] 5.6 `CursorMessage` 追加とハンドラを `zee/src/editor/buffer.rs` に実装（Phase 1 stateless 相当、diff empty）
- [x] 5.7 キーバインド: `select-all` 同様の多段プレフィックス方式で進入を `zee/src/editor/bindings.rs` に割り当て（D6、第一打鍵を確定）
- [x] 5.8 テスト: 進入後アンカー不動・向き非依存正規化・キャンセル非破壊・矩形モード中バッファ不変（spec `rectangle-selection` 各シナリオ）

## 6. Scope 4 — ハイライト描画（描画層・Scope 0 後に Scope 3 と並列可）

- [x] 6.1 `text_style_at_char()`（`syntax/highlight.rs`）と `draw_line()`（`textarea.rs`）に `visual_x` を渡せるよう拡張
- [x] 6.2 矩形 active 時、各行の `[left, right)` に重なる grapheme を selection 背景色でハイライト（Scope 2 の列写像で判定）
- [x] 6.3 短い行・行末より右の仮想空間を塗らない（C-5）。通常連続選択のハイライト経路は不変（C-3）
- [x] 6.4 幅0矩形を細い縦線（カーソル様マーカー）で表示
- [x] 6.5 テスト/目視確認: ハイライト列範囲＝切り出し列範囲の一致（C-4）、モード離脱でハイライト消去（spec `rectangle-highlight`）

## 7. Scope 5 — 矩形コピー（読み取り系・Scope 2+3 後）

- [x] 7.1 `zee/src/editor/buffer.rs` に矩形コピーメソッドを追加。各行を Scope 2 規則で切り出し `\n` 連結して clipboard に set（diff empty、非破壊）
- [x] 7.2 末尾改行を付けない（最終行 r1 の末尾に改行なし）。短い行は空扱いで空行を維持（C-5）
- [x] 7.3 コピー後に矩形選択解除して通常編集へ。幅0矩形は no-op（clipboard を触らない）
- [x] 7.4 system/local clipboard の改行コード（`\n`/`\r\n`）差を確認
- [x] 7.5 テスト: 切り出し連結・非破壊（undo 履歴不変）・末尾改行なし・短い行空扱い・幅0 no-op（spec `rectangle-copy`）

## 8. Scope 6 — 矩形カット/削除（全部乗せ・波4）

- [x] 8.1 矩形削除を `zee-edit/src/lib.rs` に実装。各行の `[left, right)` を Scope 2 と同一規則で削除し `[0,left)` と `[right,行末]` を連結
- [x] 8.2 複数行削除を Scope 1 の compound diff で生成し1リビジョンに束ねる（char_index 降順適用）
- [x] 8.3 カット = 切り出しを clipboard へ（copy と同一規則）、削除 = clipboard に書かない。編集の振る舞いは両者同一
- [x] 8.4 カット/削除後にカーソルを左上角（`r0`, visual column `left`）へ、矩形選択解除（Emacs `kill-rectangle` 準拠）
- [x] 8.5 短い行: 行末 < left は何も削除しない / 行末が範囲途中までは存在文字だけ削除（no padding、C-5）。幅0は no-op（バッファ/clipboard/undo 不変）
- [x] 8.6 `cut` メソッド＋ハンドラと（必要なら別キーの）キーバインドを `buffer.rs` / `bindings.rs` に追加（D6）
- [x] 8.7 テスト: 各行の穴閉じ・範囲外不変（C-2）・undo 1回で全行同時復元 / redo 1回で再適用（C-1）・カーソル左上角・中途半端状態を残さない・幅0 no-op（spec `rectangle-cut`）

## 9. 仕上げ

- [x] 9.1 `cargo test`（zee-edit / zee 両 crate）と `cargo clippy` を通す
- [x] 9.2 通常編集の手動回帰確認（入力・連続選択/削除/コピー・undo/redo が従来どおり、C-3）
- [x] 9.3 矩形 選択→ハイライト→コピー→カット/削除→undo の一連を実アプリで目視確認
- [x] 9.4 ROADMAP の区間「状態」を更新し、この区間で確定した事項（キーバインド方式・compound diff 機構）を反映
