## ADDED Requirements

### Requirement: 矩形範囲を行ごとに切り出して clipboard へ連結する

矩形コピー操作をすると、各行 r（r0 から r1 へ昇順）について visual column `[left, right)` に重なる部分文字列を共有の column-mapping 規則で切り出し、**行の出現順に改行 `\n` で連結** した文字列を clipboard に set しなければならない（MUST）。clipboard にはフラットな文字列が入り、「矩形で切った」というメタ情報は持たせない。コピーはバッファ・undo 履歴を一切変更してはならない（MUST NOT、diff は empty）。

#### Scenario: 各行を切り出して連結

- **WHEN** 矩形が行範囲 `r0..=r1`・visual column `[left, right)` で選択され矩形コピーする
- **THEN** 各行の `[left, right)` 部分を切り出し `\n` で連結した文字列が clipboard に set される

#### Scenario: コピーは非破壊

- **WHEN** 矩形コピーをする
- **THEN** バッファは1文字も変更されず、undo 履歴にリビジョンも作られない

#### Scenario: 末尾改行を付けない

- **WHEN** 矩形コピーをする
- **THEN** 行間にのみ `\n` が入り、最終行（r1）の末尾には改行が付かない（Emacs `copy-rectangle-as-kill` 準拠）

### Requirement: 短い行は空扱いで連結する

行末 visual column < `left` の行は切り出しが **空文字列** になり（空白で埋めない）、連結時はその行分として空行が1つ入る（改行は維持）ものとする（MUST、C-5）。行末が `[left, right)` の途中までの行は `[left, 行末]` までの存在する文字だけを切り出し、右側を空白で埋めてはならない（MUST NOT）。選択していない仮想空間を空白として clipboard に混ぜてはならない（MUST NOT）。

#### Scenario: 行末が left より手前

- **WHEN** 行末 < `left` の行を含む矩形をコピーする
- **THEN** その行の切り出しは空文字列になり、連結時は空行1つ分として改行が維持される

#### Scenario: 行末が範囲の途中まで

- **WHEN** 行末が `[left, right)` の途中までの行を含む矩形をコピーする
- **THEN** `[left, 行末]` までの存在する文字だけが切り出され、右側は空白で埋められない

### Requirement: コピー後の状態と幅0矩形

コピー後は矩形選択を解除し通常編集状態に戻らなければならない（MUST、既存 `copy_selection_to_clipboard` の挙動に倣う）。幅0矩形のコピーは **no-op**（clipboard を触らない）とする（MUST）。

#### Scenario: コピー後に選択解除

- **WHEN** 矩形コピーをする
- **THEN** 矩形選択は解除され、通常編集状態に戻る

#### Scenario: 幅0矩形は no-op

- **WHEN** 幅0矩形でコピーする
- **THEN** clipboard は変化しない
