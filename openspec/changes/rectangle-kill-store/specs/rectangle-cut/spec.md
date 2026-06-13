## MODIFIED Requirements

### Requirement: カットと削除の差は clipboard 書き込み有無のみ

カットは取り除いた内容を copy と **同一規則** で `\n` 連結して clipboard に set し、**かつ同じ行リストと連結文字列を矩形 kill ストアに記録** してから削除しなければならない（MUST、`rectangle-kill-store` 参照）。削除は clipboard に書かず、矩形 kill ストアにも記録せずに削除しなければならない（MUST NOT）。編集としての振る舞いは両者同一でなければならない（MUST）。clipboard に書く文字列表現（`\n` 連結・末尾改行なし）は変えない。

#### Scenario: カットは clipboard に書く

- **WHEN** 矩形カットする
- **THEN** 各行の `[left, right)` を `\n` 連結した文字列が clipboard に set され、その後にバッファから削除される

#### Scenario: カットは矩形 kill として記録される

- **WHEN** 矩形カットする
- **THEN** clipboard への set に加えて、取り除いた行リストと連結文字列が矩形 kill ストアに記録される

#### Scenario: 削除は clipboard に書かない

- **WHEN** 矩形削除する
- **THEN** clipboard も矩形 kill ストアも変化せず、バッファの削除は矩形カットと同一になる

#### Scenario: 中途半端状態を残さない

- **WHEN** 矩形カットする
- **THEN** 操作は全行成功か全体取り消しのいずれかで、clipboard 書き込み済みなのに削除が一部失敗してバッファと食い違う状態にはならない
