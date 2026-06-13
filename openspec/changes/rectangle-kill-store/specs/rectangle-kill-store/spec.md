## ADDED Requirements

### Requirement: 矩形 kill の記録

矩形コピー / 矩形カットは、clipboard へフラット文字列を書き込むのに加えて、その kill を **矩形 kill** として内部ストアに記録しなければならない（MUST）。記録内容は、(a) 各行の切り出し結果の **行リスト**（短い行は空文字列のまま＝C-5 の空扱いを保持）と、(b) 記録時に clipboard へ書いた **`\n` 連結文字列** とする。幅0矩形のコピーは clipboard を触らない no-op であり、矩形 kill を記録してはならない（MUST NOT）。記録は clipboard の文字列表現・バッファ・undo 履歴を一切変えてはならない（MUST NOT change）。

#### Scenario: 矩形コピーで矩形 kill が記録される

- **WHEN** 矩形が行範囲 `r0..=r1`・visual column `[left, right)` で選択され矩形コピーする
- **THEN** 各行の切り出し行リストと、clipboard へ書いた `\n` 連結文字列が矩形 kill として記録される

#### Scenario: 矩形カットでも矩形 kill が記録される

- **WHEN** 矩形カットする
- **THEN** 取り除いた各行の行リストと clipboard へ書いた `\n` 連結文字列が矩形 kill として記録される

#### Scenario: 幅0矩形は記録しない

- **WHEN** 幅0矩形でコピー/カットする
- **THEN** clipboard も矩形 kill ストアも変化しない

### Requirement: 矩形 kill の有効性は clipboard 内容との一致で判定する

「直近の kill が矩形 kill か」の判定は、矩形 kill が記録されており、**かつ現在の clipboard 内容が記録時の `\n` 連結文字列と一致する**場合にのみ「矩形 kill である」と判定しなければならない（MUST）。現在の clipboard 内容が記録文字列と一致しないとき（通常コピー/カットや外部アプリのコピーで上書きされたとき）は「矩形 kill ではない」と判定しなければならない（MUST）。この無効化のために通常コピー/カット/yank 側のコードを変更してはならない（MUST NOT、C-3）。

#### Scenario: 記録直後は矩形 kill と判定される

- **WHEN** 矩形コピー/カットの直後に判定する（clipboard は記録時のまま）
- **THEN** 「矩形 kill である」と判定される

#### Scenario: 通常コピーで矩形 kill 判定が外れる

- **WHEN** 矩形コピーの後に通常コピー/カットで clipboard が別の文字列に上書きされ、その後に判定する
- **THEN** 「矩形 kill ではない」と判定される

#### Scenario: 外部コピーで矩形 kill 判定が外れる

- **WHEN** 矩形コピーの後に外部アプリのコピー等で clipboard 内容が記録文字列と食い違い、その後に判定する
- **THEN** 「矩形 kill ではない」と判定され、外部由来の内容を矩形と誤解しない

### Requirement: 各行の復元は非消費（peek）

矩形 kill が有効と判定されるとき、記録した行リストを復元として取得できなければならない（MUST）。判定および復元の取得は矩形 kill を **消費してはならない**（MUST NOT consume）—— clipboard が記録文字列と一致し続ける限り、同じ矩形 kill を繰り返し取得できる。矩形 kill が無効/未記録のときは「無し」を返さなければならない（MUST）。

#### Scenario: 有効な矩形 kill の行を復元する

- **WHEN** 有効な矩形 kill に対して復元を取得する
- **THEN** 記録した行リスト（短い行の空文字列を含む）が得られる

#### Scenario: 繰り返し取得できる

- **WHEN** 有効な矩形 kill から復元を2回連続で取得する（間に新たな kill が無い）
- **THEN** 2回とも同じ行リストが得られる（1回目で消費されない）

#### Scenario: 無効時は無しを返す

- **WHEN** 矩形 kill が未記録、または clipboard 上書きで無効な状態で復元を取得する
- **THEN** 「無し」が返る
