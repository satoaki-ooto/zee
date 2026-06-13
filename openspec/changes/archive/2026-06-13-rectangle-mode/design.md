## Context

`@docs/rectangle-implementation-plan.md`（requirement-definition の結果）と `docs/behavior/*` の振る舞い定義、横断制約 `docs/behavior/_constraints.md`（C-1〜C-5）を土台に、ROADMAP「矩形のカット/削除まで」区間を実装する。

現行コードの裏取り済みの制約（`docs/rectangle-mode.md`）:
- `Cursor.selection: Option<CharIndex>`（`zee-edit/src/lib.rs:49`）は1次元アンカー1点のみ。`Cursor::selection()` は常に単一連続 `Range<CharIndex>` を返す。矩形（行ごとに不連続な range 群）は表現不可。
- `OpaqueDiff`（`zee-edit/src/diff.rs`）は単一連続領域（`char_index` + old/new length）のみ。`EditTree::create_revision()`（`tree.rs:69`）は `OpaqueDiff` を1つ受け取り、`Revision.diff: OpaqueDiff` を1つ持つ。複数 diff を1リビジョンに束ねる仕組みが無い。
- `Cursor::reconcile()`（`lib.rs:105`）は `modified_range = char_index..max(old_char_length, new_char_length)`。コメントは conservative bounding box だが、`char_index`(開始) と `max(len,len)`(終了) で **index と length が次元混在** している疑い（`docs/rectangle-mode.md` の指摘）。複数領域 reconcile を増やすと踏む可能性があり、この区間で触るか実装時に判定。
- `text_style_at_char()`（`syntax/highlight.rs`）は `char_index` だけ受け取り `visual_x` を受け取らない。`draw_line()`（`textarea.rs`）も `visual_x` を渡していない。
- zi は Shift 修飾を持たない（README TODO）。`Shift+移動で広げる` 方式は不可。
- `zee-edit` に selection の保護テストが無い（`begin_selection`/`clear_selection`/`select_all`/`delete_selection`/`selection()` の直接テスト無し）。

3層がファイルレベルで分離している（並列性の地形）:
- **[B] 編集・undo 層**（衝突核・クリティカルパス）: `diff.rs` / `tree.rs` / `lib.rs::reconcile()` / `buffer.rs` 451-463 / `parse.rs`。
- **[A] 列計算層**（孤立・純粋追加）: `graphemes.rs`。
- **描画層**（編集層と非接触）: `highlight.rs` / `textarea.rs` / `components/buffer/mod.rs`。
- 衝突核 = `Cursor`@`lib.rs` ＋ `Buffer::handle_message`/各メソッド@`buffer.rs`。selection / copy / cut が集中するので直列化する。

## Goals / Non-Goals

**Goals:**
- 矩形の選択 → ハイライト → コピー → カット/削除を、通常編集と同じ操作感（1操作＝1 undo）で成立させる。
- 編集・undo モデルを不連続複数領域対応に拡張しつつ、通常編集の観測可能な振る舞いを不変に保つ（C-3）。
- 列計算と ragged line 扱いを、ハイライト/コピー/カットが共有する単一規則として隔離する。
- コーディングエージェント並列を意識し、衝突核の手前に「継ぎ目の型」を置いてファイル分離して並列化する。

**Non-Goals:**
- 矩形ペースト/矩形挿入、clipboard の「矩形」メタ情報保持、複数カーソル併用（ROADMAP 次区間）。
- tab/CJK を列の途中でまたぐ網羅的エッジケース（基本の幅整合まではこの区間に含む、C-4）。
- 行選択モードや空白パディング（現状コードに無く、導入しない）。

## Decisions

### D1. C-1 機構: revision に「単一連続 diff の順序付き列」を束ねる（バウンディング近似は採らない）

**決定**: 1操作=1リビジョンを、`Revision` が **単一連続 `OpaqueDiff` の順序付き列**（compound diff）を保持する形で実現する。`OpaqueDiff` 自体は単一連続のまま変えない。通常の単一連続編集は「列の長さ1」の退化ケースとして同じ経路を通す。

- **継ぎ目の型（Scope 0）**: 多領域 diff を `OpaqueDiff` の Vec 化（順序付き列、例 `CompoundDiff(Vec<OpaqueDiff>)` ないし `Revision.diffs: Vec<OpaqueDiff>`）で表す。新規の複合 diff 専用型に振る舞いを盛り込みすぎず、最小宣言から始める。
- **適用順序**: 矩形削除は各行の char range を **元の Rope に対して一括計算** し、`char_index` の **降順（後ろから前へ）に適用** して、先行削除が後続の char index をずらさないようにする。
- **undo/redo**: 列の各 `OpaqueDiff.reverse()` を **逆順** に適用して原子的に巻き戻す/再適用する。`create_revision` / `undo` / `redo`（`tree.rs`）を列対応に拡張。
- **reconcile**: compound を、各サブ diff を順に畳んで反映する。単一カーソル運用なので他カーソル reconcile の組合せ爆発は無い。`modified_range` の次元混在疑い（`lib.rs:113`）は複数領域で踏むか実装時に確認し、踏むならこの区間で是正。

**理由 / 代替**:
- *単一バウンディング diff で近似* は C-2「絶対にこうならない（範囲外巻き込み）」に正面衝突する（`[left, right)` 外の `[right, 行末]` を消す）。却下。
- *`OpaqueDiff` を複合化（単一型に複数レンジ）* は `OpaqueDiff` の全消費者（reconcile/reverse/parse 連携）に多領域分岐を波及させ、通常編集（単一）の経路まで複雑化して C-3 リスクを上げる。退化ケースを別経路にできる「列を束ねる」方が、通常編集の経路を温存できる。
- 「列を束ねる」案は、通常編集 = 1要素列、矩形 = N要素列、という一様な扱いで C-3 を構造的に守れるのが最大の利点。

### D2. Scope 0 を先に land させ、継ぎ目の2型だけ宣言する

衝突核の手前に2つの型の輪郭だけ置き、振る舞いは持たせない。

1. **矩形座標型**（行範囲 × visual column 範囲）: 選択管理が埋め、描画/コピー/カットが読む。`Cursor` に持たせる前提。
2. **多領域 diff の形**（D1 の compound diff）: 編集モデルが振る舞いを実装し、カットが生成する。

これを先に置くことで、選択管理↔描画層がファイル分離して並列化でき、編集モデル↔カットも型を介して非同期に進む。型を盛りすぎると D1 の選択肢を先に縛るため、Phase 0 は最小宣言に留める。

### D3. 矩形選択状態の持ち方

`Cursor.selection: Option<CharIndex>` では2次元アンカーを表現できない。矩形モード用に **(行, visual column) のアンカー** を保持する状態を `Cursor`（または `Cursor` が抱える矩形状態）に追加する。通常選択（`selection: Option<CharIndex>`）とは **排他**（どちらか一方が active）にし、既存 `begin_selection`/`selection()` の経路と振る舞いを変えない（C-3）。`movement.rs` は `cursor.range` / `visual_horizontal_offset` に `pub(crate)` で直接アクセスしているため、フィールド追加の波及面を抑える形にする。

### D4. 列計算とハイライトのための visual column 受け渡し

`graphemes.rs` に、矩形 (visual column `[left, right)`, 行) → 各行 char range の写像を **純粋追加**（誰も書き換えない）で実装する。描画は `text_style_at_char()` / `draw_line()` に `visual_x` を渡せるよう拡張し、列で矩形内判定する。grapheme 幅は `graphemes::width()`（CJK=2, tab=`tab_width`）に準拠し、sticky column（`visual_horizontal_offset`）と同尺度（C-4）。

### D5. ragged line ポリシーの隔離

C-5 の「短い行=空扱い ↔ 空白パディング」を、**単一の分岐点**（1関数/1型）として column-mapping 層に隔離する。copy/cut/highlight はこの分岐点を通すだけにし、判断を各所に散らさない。後で padding に差し替えるとき1箇所で済む。具体手段（関数/trait）は実装裁量だが、3能力から重複実装しないことを要件とする。

### D6. キーバインド: select-all 同様の多段プレフィックス方式

zi が Shift 修飾を持たないため、矩形モード進入は既存 `select-all` と同様の **多段プレフィックス方式**（方式は確定、第一打鍵の具体キーは実装時に確定）。進入後は通常の移動キーで対角を動かすモーダル。カットと削除のキーを別々に用意するかは実装時に決める（編集の振る舞いは同一）。`bindings.rs` に割り当てを追加。

### D7. clipboard はフラット文字列のまま

`Clipboard` trait（`String` 入出力）はそのまま使う。矩形は行を `\n` 連結したフラット文字列として set し、矩形メタ情報は持たせない（次区間）。末尾改行は付けない（Emacs `copy-rectangle-as-kill` 準拠）。system/local clipboard の改行コード（`\n`/`\r\n`）差は実装時に確認。

### D8. 進める順序（波）と並列化

- **波1（並列）**: Scope 3 選択状態（衝突核を先に land）と、Scope 1 編集モデル（最重量・最優先で機構を潰す）。両者は触る層が直交。＋ Scope 2 列計算と安全網テストは依存ゼロで即着手可。
- **波2（並列）**: Scope 0 の座標型が固まったら、Scope 4 ハイライト（描画層）。Scope 3 とファイル分離。
- **波3**: Scope 5 コピー（`buffer.rs` を Scope 3 と共有するため Scope 3 後）。
- **波4（全部乗せ）**: Scope 6 カット/削除（Scope 0+1+2+3 全部の上）。
- **クリティカルパス**: D1 機構決定 → Scope 1 実装 → Scope 6。最長・最高リスク。

## Risks / Trade-offs

- **[reconcile の次元混在疑い]**（`lib.rs:113`）が複数領域 reconcile で顕在化 → Scope 1 着手時に単体テストで踏むか確認し、踏むならこの区間で是正。単一カーソル運用なので影響範囲は限定的。
- **[compound diff の適用順序ミスで char index がずれる]** → 元 Rope に対し全 char range を一括計算し降順適用、undo は逆順、を不変条件としてテストで固定。
- **[通常編集の C-3 後方互換が崩れる]** → 通常編集を「1要素列」の退化ケースとして同経路に通し、着手前に既存 selection/delete の保護テストを足してから `Cursor` を触る。
- **[衝突核（`Cursor`@lib.rs / `buffer.rs`）の同一ファイル衝突]** → Scope 0 の継ぎ目型と Scope 1 の編集契約を先に land させ、その後に消費者（Scope 4/5/6）を並列で乗せる。
- **[Scope 0 の型を盛りすぎ design の選択肢を縛る]** → Phase 0 は最小宣言＋表せる対象の合意に留め、確定形は Scope 1 実装と歩調を合わせる。
- **[ragged line 判断の散在]**（C-5 違反） → D5 の単一分岐点を通すことを copy/cut/highlight の共通要件にする。

## Open Questions

- 多領域 diff を `CompoundDiff` 新規型にするか `Revision.diffs: Vec<OpaqueDiff>` にするか（D1 の範囲内、Scope 1 実装初手で確定）。
- 矩形モード進入の第一打鍵の具体キー（D6、方式は確定）。
- カットと削除を別キーにするか（編集の振る舞いは同一）。
- アンカー端が画面外のときのスクロール（`ensure_cursor_in_view` は range.start のみ参照）。最低限「対角カーソルが見える」は要請、アンカー追従は実装裁量。
- system/local clipboard の改行コード差（`\n`/`\r\n`）。
