## MODIFIED Requirements

### Requirement: 矩形範囲を行ごとに切り出して clipboard へ連結する

矩形コピー操作をすると、各行 r（r0 から r1 へ昇順）について visual column `[left, right)` に重なる部分文字列を共有の column-mapping 規則で切り出し、**行の出現順に改行 `\n` で連結** した文字列を clipboard に set しなければならない（MUST）。clipboard に書く文字列表現はフラットな文字列であり、その表現自体は変えない（末尾改行なし）。これに**加えて**、切り出した行リストと clipboard へ書いた連結文字列を **矩形 kill ストアに記録** しなければならない（MUST、`rectangle-kill-store` 参照）。コピーはバッファ・undo 履歴を一切変更してはならない（MUST NOT、diff は empty）。

#### Scenario: 各行を切り出して連結

- **WHEN** 矩形が行範囲 `r0..=r1`・visual column `[left, right)` で選択され矩形コピーする
- **THEN** 各行の `[left, right)` 部分を切り出し `\n` で連結した文字列が clipboard に set される

#### Scenario: 矩形 kill として記録される

- **WHEN** 矩形コピーする
- **THEN** clipboard への set に加えて、切り出した行リストと連結文字列が矩形 kill ストアに記録される

#### Scenario: コピーは非破壊

- **WHEN** 矩形コピーをする
- **THEN** バッファは1文字も変更されず、undo 履歴にリビジョンも作られない

#### Scenario: 末尾改行を付けない

- **WHEN** 矩形コピーをする
- **THEN** 行間にのみ `\n` が入り、最終行（r1）の末尾には改行が付かない（Emacs `copy-rectangle-as-kill` 準拠）
