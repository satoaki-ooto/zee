## Context

要件定義 `docs/rectangle-paste-scope.md`（S1）と振る舞い定義 `docs/behavior/rectangle-paste.md`、横断制約 `docs/behavior/_constraints.md`（C-3 後方互換 / C-5 ragged line）を土台に、矩形ペーストの前提となる **矩形 kill ストア** だけを実装する。

現行コードの裏取り:
- `Clipboard` trait は `get_contents() -> Result<String>` / `set_contents(String)` の2メソッドのみ（`zee/src/clipboard.rs:5-8`）。`LocalClipboard` は `RwLock<String>`、`SystemClipboard` は `crossclip` 経由でいずれも**プレーン文字列のみ**。
- 矩形コピー/カットは行ごとに `parts: Vec<String>` を作り `parts.join("\n")` でフラット化して `set_contents` する（`buffer.rs:594`, `:719`）。**行リストは join 直前に既に手元にある**。
- clipboard は `Context.clipboard: Arc<dyn Clipboard>` に保持され、`Context` 自体は `ContextHandle(pub &'static Context)`（`editor/mod.rs:108`）= **不変参照**で共有される。よって Context に足す可変状態は、clipboard と同じく**内部可変（`RwLock` 等）**でなければならない。
- 通常 paste は `paste_from_clipboard`（`buffer.rs:538`）で `get_contents()` → `insert_chars` のフラット挿入。`CursorMessage::Yank`（`:428`）。

未解決だった曖昧さ（scope の「残された曖昧さ」）= **外部クリップボードとの相互作用**（別アプリでコピーして戻ったとき古い矩形を誤判定しないか）を、本 design の D2 で閉じる。

## Goals / Non-Goals

**Goals:**
- 矩形コピー/カットの結果を「矩形 kill」として、行リストと種別を内部に記録する。
- 「直近の kill が矩形 kill か」を判定し、矩形 kill の各行を復元できる。
- 通常 kill・外部コピーで clipboard が上書きされたら矩形 kill 判定が自動的に外れる（誤判定しない）。
- clipboard に書く文字列表現（`\n` 連結・末尾改行なし）と、通常 copy/cut/yank の観測可能な振る舞いを一切変えない（C-3）。

**Non-Goals:**
- 矩形ペーストの挿入動作そのもの（S2）、カーソル3変種（S3）。本 change はストアと判定/復元 API までで、貼り付けは行わない。
- `Clipboard` trait のシグネチャ変更／矩形メタ情報を OS クリップボードへ載せること。
- SystemClipboard をまたいだ矩形性の往復「保存」（別アプリ→zee へ矩形として持ち込む等）。本 change は zee 内で記録した矩形 kill の判定までで、外部由来を矩形と解釈しない。

## Decisions

### D1. 矩形 kill ストアは Context に内部可変フィールドとして足す（trait は壊さない）

**決定**: `Clipboard` trait は不変のまま、`Context` に clipboard と並ぶ**内部可変ストア**を1つ足す。`Context` は `&'static` 共有なので、clipboard の `RwLock` パターンに倣い `Arc<...>` + `parking_lot::RwLock`（既存依存）で内部可変にする。

- データモデル: `RectangleKill { lines: Vec<String>, clipboard_text: String }`。
  - `lines` = 矩形コピー/カットが `join` する直前の `parts`（各行の切り出し。短い行は空文字列＝C-5 の空扱いがそのまま入る）。
  - `clipboard_text` = 記録時に実際に clipboard へ書いた `\n` 連結文字列（D2 の照合鍵）。
- ストア型: `Option<RectangleKill>` を `RwLock` で包む。記録で `Some(..)` を上書き、判定/復元で読む。
- 記録箇所: `rectangle_copy`（`buffer.rs:594` 付近）/ `rectangle_cut`（`:719` 付近）で `set_contents(clipboard_text.clone())` の直後に `lines = parts`・`clipboard_text` を記録。幅0矩形コピーは clipboard を触らない no-op なので**記録もしない**。

**理由 / 代替**:
- *`Clipboard` trait に矩形メソッドを追加* → `String` I/O の責務を汚し、`SystemClipboard`（crossclip）は OS クリップボード越しに構造化データを運べないため実体を持てない。却下。
- *clipboard 文字列にマーカーを埋め込む* → 貼り付け結果や外部アプリに漏れ、往復で壊れやすい。却下。
- *内部の別ストア（content snapshot で照合）* → trait を壊さず、OS クリップボードに依存せず、外部相互作用も D2 で安全に閉じられる。採用。

### D2. 矩形 kill の有効性は「記録時の clipboard 文字列との一致」で判定する（外部相互作用の解決）

**決定**: 「直近の kill が矩形 kill か」の判定は、ストアが `Some` であることに加えて、**現在の `clipboard.get_contents()` が記録した `clipboard_text` と一致すること**を条件にする。一致しなければ「矩形 kill ではない」とみなす（ストアは古い＝stale）。

- 通常コピー/カット（`copy_selection_to_clipboard` / `cut_selection_to_clipboard`）が新しい文字列を `set_contents` すると、現在の clipboard 内容が記録 `clipboard_text` と食い違う → 矩形 kill 判定が自動的に外れる。**通常 copy/cut/yank 側のコード変更は不要**（C-3 を構造的に担保）。
- 別アプリのコピーで OS クリップボードが変わった場合も同様に食い違い → 外部由来を矩形と誤判定しない。**外部変更を監視/ポーリングする必要がない**のが利点。
- 矩形 kill 判定が有効なら `lines` を復元として返す（**非消費＝peek**。yank-rectangle は繰り返せるべきなので、判定/復元で `take` しない）。

**理由 / 代替**: *新規 kill のたびにストアを明示的にクリア* も可能だが、通常 copy/cut/yank の各経路に無効化を挿す必要があり C-3 リスクと記述箇所が増える。clipboard 文字列照合は1箇所（判定 API 内）で済み、外部変更も同じ仕組みで吸収できる。

### D3. 公開する操作の最小形

ストアが提供するのは次の最小 API（名前は実装裁量、振る舞いを固定）:
- **記録**: 行リストと clipboard 文字列を矩形 kill として保存する（矩形 copy/cut から呼ぶ）。
- **判定付き取得（peek）**: 現在の clipboard 内容と照合し、矩形 kill が有効なら行リストの参照/複製を返す。無効/未記録なら「無し」を返す。消費しない。

S2（ペースト）はこの「判定付き取得」を使い、「無し」のとき no-op に分岐する。

## Risks / Trade-offs

- **[SystemClipboard の改行コード正規化（`\n`↔`\r\n`）で照合が外れる]** → 記録した `clipboard_text` と `get_contents()` の比較を、同一の改行正規化を通してから行う（または比較前に `\r\n`→`\n` 正規化）。万一プラットフォームがクリップボードを書き換えても、最悪「矩形 kill 無し＝no-op」へ安全に縮退するだけで、誤った貼り付けは起きない。実装時に system-clipboard feature で要確認。
- **[判定のたびに `get_contents()` を呼ぶコスト]** → 判定が走るのはペースト操作時のみで頻度は低い。許容。
- **[`Context` 共有下の競合]** → clipboard と同じく `RwLock` で保護。単一カーソル・単一 UI スレッド運用で競合は限定的。
- **[`lines` の空文字列（C-5 空扱い）保持]** → 記録する `parts` は短い行が空文字列のまま。S2 の padding は貼り付け側で行う設計（本 change の責務外）。ストアは「コピー時に切り出した行」を忠実に保持するだけで、padding 情報は持たない。

## Open Questions

- SystemClipboard（crossclip）の改行コード実挙動。比較前正規化の要否は system-clipboard feature 有効ビルドでの実測で確定（リスク欄の縮退で安全側に倒れるため、ブロッカーではない）。
- ストアの物理的な置き場が `Context` 直下フィールドか、clipboard を包む薄いラッパか。どちらでも D1/D2 の振る舞いは同一。実装時に波及の小さい方を選ぶ。
