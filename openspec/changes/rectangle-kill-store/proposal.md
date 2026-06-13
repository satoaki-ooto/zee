## Why

矩形ペースト（yank-rectangle）を載せるには、clipboard が「直近の kill が矩形 kill か」を通常 kill と区別でき、矩形 kill の各行を復元できる必要がある。現状の clipboard は `String` の出し入れだけで、矩形コピー/カットは `parts.join("\n")` でフラット文字列に潰しており、矩形であった事実・行構造を一切持てない（`docs/behavior/rectangle-paste.md`「矩形 kill が無いとき no-op」の前提が成立しない）。本 change は、その土台＝**矩形 kill ストア**だけを先に切り出す（要件定義 S1）。ペースト本体（S2）・カーソル3変種（S3）は後続 change。

## What Changes

- 矩形コピー / 矩形カットが、これまでどおり clipboard へフラット文字列（`\n` 連結）を書くのに**加えて**、その kill を「矩形 kill」として **行リスト** と種別を内部ストアに記録する。
- 「直近の kill が矩形 kill か」を判定でき、矩形 kill の各行を復元できる内部ストアを追加する。
- **外部・通常 kill による無効化**: 矩形 kill ストアは、記録時に書いた clipboard 文字列を併せて保持する。判定時に現在の clipboard 内容が記録した文字列と一致しないとき（通常コピー/カットや別アプリのコピーで上書きされたとき）は「矩形 kill ではない」と判定する。これにより外部クリップボード相互作用での誤判定（古い矩形を直近の矩形 kill とみなす）を防ぐ。
- **BREAKING（spec 内部）**: `rectangle-copy` / `rectangle-cut` の「clipboard にはフラット文字列のみで矩形メタ情報は持たせない」という要件を、「フラット文字列に加えて内部の矩形 kill ストアにも記録する」へ変更する。**clipboard に書く文字列表現そのものは不変**（`\n` 連結・末尾改行なし）。
- 非目標（後続 change / 次区間）: 矩形ペーストの挿入動作そのもの（S2）、カーソル3変種（S3）、tab のスペース展開、string-rectangle、複数カーソル併用、挿入プレビュー。通常 `Ctrl-y`（フラット yank）の振る舞いは変えない（C-3）。

## Capabilities

### New Capabilities
- `rectangle-kill-store`: 直近の kill の種別（矩形 / 非矩形）と、矩形 kill の各行・記録時 clipboard 文字列を保持する内部ストア。「直近の kill が矩形 kill か」の判定と各行の復元を提供し、clipboard 内容が記録時と食い違えば矩形 kill 判定を無効化する。バッファ・undo 履歴・clipboard の文字列表現は変えない。

### Modified Capabilities
- `rectangle-copy`: clipboard へのフラット文字列書き込みに加え、同じ行リストを矩形 kill ストアへ記録する（従来「メタ情報を持たせない」を改定）。clipboard の文字列表現と非破壊性（diff empty）は不変。
- `rectangle-cut`: 取り除いた行リストを clipboard 文字列に加えて矩形 kill ストアへ記録する。編集としての振る舞い（1リビジョン・カーソル左上角・幅0 no-op）は不変。

## Impact

- **clipboard / ストア保持先**: 矩形 kill ストアの置き場。`Editor` の `Context`（`clipboard: Arc<dyn Clipboard>` と並ぶ位置）か、`Clipboard` 周辺。`zee/src/clipboard.rs` / `zee/src/editor/buffer.rs`。trait 自体を壊さず内部ストアを足す方向（design で確定）。
- **矩形 copy/cut**: `zee/src/editor/buffer.rs` の `rectangle_copy`（L549–603）/ `rectangle_cut`（L686–732）にストア書き込みを追加（行リストは既に手元にある `parts`）。
- **通常 copy/cut/yank（C-3）**: clipboard 文字列比較で無効化するため、通常 copy/cut/yank 側の**変更は不要**（新しい kill で clipboard 文字列が変われば矩形 kill 判定が自動的に外れる）。
- **安全網**: 矩形 kill 記録 → 判定 → 通常 kill / 外部上書きで無効化、の単体テストを追加（後続 S2 が依存する判定の正しさを先に固める）。
