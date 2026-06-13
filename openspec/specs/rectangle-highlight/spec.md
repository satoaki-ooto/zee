## ADDED Requirements

### Requirement: 矩形選択範囲を画面にハイライトする

矩形選択が active な間、選択行範囲の各行について、矩形の visual column 範囲 `[left, right)` に重なる grapheme が selection 背景色でハイライトされなければならない（MUST）。矩形モードを抜ける／キャンセルするとハイライトは消え通常表示に戻る。通常の連続選択のハイライトは従来どおりで、矩形と排他とする（C-3）。

#### Scenario: 選択行の重なる grapheme が塗られる

- **WHEN** 矩形が visual column `[left, right)`・行 r を含み、行 r を描画する
- **THEN** visual column 区間が `[left, right)` と重なる grapheme だけが selection 背景色でハイライトされる

#### Scenario: モード離脱でハイライトが消える

- **WHEN** 矩形モードを抜ける／キャンセルする
- **THEN** 矩形ハイライトは消え、通常表示に戻る

#### Scenario: 通常選択のハイライトが不変

- **WHEN** 通常の連続選択を行う
- **THEN** 既存 `cursor.selection().contains()` 経路の見え方は変わらない

### Requirement: 見た目の列範囲と切り出し範囲が一致する

ハイライトされる列範囲は、同じ行をコピー/カットしたときに切り出される範囲と **完全に一致** しなければならない（MUST、C-4）。ハイライト列範囲と実際の copy/cut 列範囲がずれてはならない（MUST NOT）。

#### Scenario: ハイライトと切り出しの一致

- **WHEN** 行 r の各 grapheme を描画し、同じ行を copy/cut する
- **THEN** ハイライトされた範囲と切り出される範囲が完全に一致する（共有の column-mapping 規則を用いる）

### Requirement: 短い行と仮想空間を塗らない

短い行（行末が `left` より手前）の行では、その行ではハイライトされる文字が無く、行末より右の仮想空間を塗ってはならない（MUST NOT、C-5 空扱い）。行末が `[left, right)` の途中までの行は、行頭側の存在する文字だけをハイライトする。ハイライトは表示のみで、バッファ・カーソル状態を変更してはならない（MUST NOT）。

#### Scenario: 行末が left より手前

- **WHEN** 行末 visual column < `left` の行を描画する
- **THEN** その行ではハイライトされる文字が無く、行末より右の仮想空間も塗らない

#### Scenario: 行末が範囲の途中まで

- **WHEN** 行末が `[left, right)` の途中までの行を描画する
- **THEN** 存在する文字だけがハイライトされ、右側の仮想空間は塗らない

### Requirement: 幅0矩形のハイライト

幅0矩形でも選択状態は成立するため、列位置に **細い縦線（カーソル様マーカー）** で示さなければならない（MUST）。

#### Scenario: 幅0矩形を縦線で示す

- **WHEN** 幅0の矩形が選択されている各行を描画する
- **THEN** その列位置に細い縦線（カーソル様マーカー）が表示される
