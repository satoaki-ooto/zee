## Why

zee には矩形（ブロック）選択・編集が無い。Emacs の rectangle 相当を最終地点とするロードマップの第一区間として、まず **矩形の選択 → ハイライト → コピー → カット/削除** までを、通常編集と同じ操作感（1操作＝1 undo）で行えるようにする。難しさは要件の珍しさではなく、zee の現行編集・undo モデル（`OpaqueDiff` / `EditTree` の revision）が **単一連続領域しか前提にしていない** 点にある。カット/削除がこのモデルに正面からぶつかるため、この区間の重心は編集モデルそのものの変更にある。

## What Changes

- 矩形選択モードに入り、(行範囲 × visual column 範囲) の2次元範囲をアンカー＋対角で選べる。進入は zi が Shift 修飾を持たないため `select-all` 同様の多段プレフィックス方式で行う。
- 矩形選択範囲が画面上でハイライトされる（見た目の列範囲＝切り出し列範囲が一致）。
- 矩形選択範囲をコピーできる（行ごとに visual column で切り出し `\n` 連結して clipboard へ）。
- 矩形選択範囲をカット／削除できる。**カット1回が1 undo で矩形全体ぴったり戻る**。カット＝clipboard へ書く、削除＝書かない、編集の振る舞いは両者同一。
- **BREAKING（内部モデル）**: 編集・undo モデルを、不連続な複数文字範囲の変更を **1操作 = 1リビジョン** として表現・適用・巻き戻しできるよう拡張する。通常の単一連続編集の観測可能な振る舞いは一切変えない（後方互換 C-3）。
- 列計算（visual column → 各行 char range）と短い行（ragged line）の扱いを、ハイライト/コピー/カットが共有する **単一の切り出し規則** として隔離する。

非目標（この区間では作らない、ROADMAP「やらないこと」）: 矩形ペースト/矩形挿入、clipboard の「矩形」メタ情報保持、複数カーソル併用、tab/CJK を列の途中でまたぐ変則エッジケースの作り込み（基本の幅整合までは含む）。

## Capabilities

### New Capabilities
- `block-edit-model`: 不連続な複数文字範囲の変更を1操作=1リビジョンとして表現・適用・巻き戻しできる編集・undo 土台（横断制約 C-1/C-2、後方互換 C-3）。カット/削除が生成し、この区間の中心かつ最高リスク。
- `rectangle-column-mapping`: 矩形 (行×visual column) → 各行の char range への写像規則。grapheme 幅準拠（CJK=2, tab=`tab_width`、C-4）と短い行=空扱い（C-5）の分岐を1箇所に隔離。ハイライト/コピー/カットが共有する単一規則。
- `rectangle-selection`: 矩形モード進入と2次元範囲選択の状態管理（アンカー固定・対角移動・正規化・キャンセル・幅0矩形）。矩形編集すべての入口。バッファは変えない。
- `rectangle-highlight`: 矩形選択中の (行×列) 領域を画面に描く。見た目の列範囲が切り出し範囲と完全一致する（C-4）。表示のみでバッファ・カーソルを変えない。
- `rectangle-copy`: 矩形範囲を共有の切り出し規則で行ごとに切り出し `\n` 連結して clipboard へ。非破壊（diff empty）。
- `rectangle-cut`: 各行の矩形範囲を共有規則で取り除き左右を繋ぎ、`block-edit-model` で1リビジョンに束ね、カーソルを左上角へ。カット＝clipboard へ書く、削除＝書かない。幅0は no-op。

### Modified Capabilities
<!-- 既存 spec は無し（openspec/specs/ は空）。通常編集の振る舞いは C-3 で不変のため delta なし。 -->

## Impact

- **編集・undo 層（衝突核・クリティカルパス）**: `zee-edit/src/diff.rs`（`OpaqueDiff` 単一レンジ）, `tree.rs`（revision）, `lib.rs::reconcile()`, `zee/src/editor/buffer.rs` 451-463（単一diff契約）, `syntax/parse.rs`。
- **列計算層（孤立・純粋追加）**: `zee-edit/src/graphemes.rs`。
- **選択状態・キーバインド（衝突核）**: `zee-edit/src/lib.rs`（`Cursor` の矩形選択状態）, `movement.rs`, `zee/src/editor/buffer.rs`（`CursorMessage` 追加＋ハンドラ）, `zee/src/editor/bindings.rs`。
- **描画層（編集層と非接触）**: `zee/src/syntax/highlight.rs`, `components/buffer/textarea.rs`, `components/buffer/mod.rs`。
- **clipboard**: `zee/src/clipboard.rs` の `String` 入出力 trait はそのまま利用（矩形メタ情報は持たせない）。
- **安全網**: 現状 `zee-edit` に selection の保護テストが無い。`Cursor` 構造変更（`movement.rs` の多数箇所に波及）の前に既存 selection の回帰テストを足す。
