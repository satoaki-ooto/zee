## ADDED Requirements

### Requirement: 矩形範囲を各行から取り除き左右を繋ぐ

矩形カット/削除操作をすると、各行 r で visual column `[left, right)` に当たる文字を共有の column-mapping 規則で削除し、`[0, left)` と `[right, 行末]` を連結して矩形の穴を閉じなければならない（MUST）。矩形に含まれない文字（`[0, left)`・`[right, 行末]`・選択行範囲外の行）を削除・改変してはならない（MUST NOT、C-2）。削除される列範囲はハイライト/コピーで見えていた範囲と一致しなければならない（MUST、C-4）。

#### Scenario: 各行の矩形穴を閉じる

- **WHEN** 矩形が行範囲 `r0..=r1`・visual column `[left, right)` で選択され矩形カット/削除する
- **THEN** 各行で `[left, right)` の文字が削除され `[0, left)` と `[right, 行末]` が連結される

#### Scenario: 範囲外を巻き込まない

- **WHEN** 矩形カット/削除する
- **THEN** `[0, left)`・`[right, 行末]`・選択行範囲外の行は一切変更されない

### Requirement: カットと削除の差は clipboard 書き込み有無のみ

カットは取り除いた内容を copy と **同一規則** で `\n` 連結して clipboard に set してから削除し、削除は clipboard に書かずに削除しなければならない（MUST）。編集としての振る舞いは両者同一でなければならない（MUST）。

#### Scenario: カットは clipboard に書く

- **WHEN** 矩形カットする
- **THEN** 各行の `[left, right)` を `\n` 連結した文字列が clipboard に set され、その後にバッファから削除される

#### Scenario: 削除は clipboard に書かない

- **WHEN** 矩形削除する
- **THEN** clipboard は変化せず、バッファの削除は矩形カットと同一になる

#### Scenario: 中途半端状態を残さない

- **WHEN** 矩形カットする
- **THEN** 操作は全行成功か全体取り消しのいずれかで、clipboard 書き込み済みなのに削除が一部失敗してバッファと食い違う状態にはならない

### Requirement: 1リビジョンの undo 原子性

複数行の矩形削除は **1リビジョン** として履歴に積まれ、undo を1回すると全行の矩形削除が同時に巻き戻りバッファが矩形操作の直前と完全一致し、redo を1回すると全行同時に再適用されなければならない（MUST、C-1）。undo 1回で一部の行だけが戻る／全体を戻すのに undo を複数回要する状態になってはならない（MUST NOT）。

#### Scenario: 1回の undo で全行が戻る

- **WHEN** 矩形カット/削除を1回行った直後に undo を1回する
- **THEN** 全行の矩形削除が同時に巻き戻り、バッファは矩形操作の直前と完全一致する

#### Scenario: 1回の redo で全行が再適用される

- **WHEN** 上記 undo の直後に redo を1回する
- **THEN** 全行の矩形削除が同時に再適用される

### Requirement: カット後のカーソルと短い行扱い

カット/削除後は矩形選択を解除し、カーソルを矩形の **左上角**（`r0`, visual column `left`）に置かなければならない（MUST、Emacs `kill-rectangle` 準拠）。行末 < `left` の行は何も削除せず、行末が `[left, right)` の途中までの行は `[left, 行末]` までの存在する文字だけ削除する（C-5）。短い行を空白でパディングしてから削除してはならない（MUST NOT）。通常の連続削除（`delete_selection`）の undo 粒度・カーソル挙動は変わってはならない（MUST NOT、C-3）。

#### Scenario: カーソルが左上角に来る

- **WHEN** 矩形カット/削除する
- **THEN** 矩形選択は解除され、カーソルは (`r0`, visual column `left`) に置かれる

#### Scenario: 行末が left より手前の行

- **WHEN** 行末 < `left` の行を含む矩形をカット/削除する
- **THEN** その行は何も削除されず、空白で詰めて揃えたりしない

### Requirement: 幅0矩形は no-op

幅0矩形（列範囲が空）のカット/削除は **no-op** で、バッファ・clipboard・undo 履歴のいずれも変化してはならない（MUST NOT change）。padding しない。

#### Scenario: 幅0矩形のカット/削除

- **WHEN** 幅0矩形でカット/削除する
- **THEN** バッファ・clipboard・undo 履歴のいずれも変化しない
