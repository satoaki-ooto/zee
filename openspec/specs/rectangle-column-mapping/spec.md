## ADDED Requirements

### Requirement: 列は visual column（grapheme 幅準拠）で測る

矩形の「列」は char index ではなく **表示上の視覚列** で測らなければならない（MUST）。CJK は幅2、tab は `tab_width` 分として扱い、`graphemes::width()` / sticky column（`visual_horizontal_offset`）と同じ尺度で列を測る。矩形 (行範囲 × visual column 範囲 `[left, right)`) を、各行について char range へ写像する単一の規則を提供する。

#### Scenario: ASCII 行の列写像

- **WHEN** visual column `[left, right)` を ASCII のみの行に写像する
- **THEN** char range が visual column と1対1に対応し、`[left, right)` がそのまま char range になる

#### Scenario: CJK を含む行の列写像

- **WHEN** 幅2の CJK grapheme を含む行に visual column `[left, right)` を写像する
- **THEN** 各 grapheme の visual column 区間が `[left, right)` と重なるものだけが char range に入り、見た目の列と一致する

#### Scenario: tab を含む行の列写像

- **WHEN** tab（`tab_width` 幅）を含む行に visual column を写像する
- **THEN** tab を `tab_width` 分の幅として列を測り、char range を求める

### Requirement: 短い行（ragged line）は Emacs 準拠で空扱いし、分岐を1箇所に隔離する

矩形の列範囲より短い行は **その行分は「空」として扱う**（空白パディングはしない）ものとする（MUST）。この ragged line のポリシー（空扱い ↔ 空白パディング）は、**後から差し替えられる単一の分岐点として隔離** されていなければならず、コピー・カット・ハイライトの各所に判断が散らばってはならない（MUST NOT）。

#### Scenario: 行末が left より手前の行

- **WHEN** 行末 visual column < `left` の行を写像する
- **THEN** その行の char range は空であり、空白で埋めない

#### Scenario: 行末が範囲の途中までの行

- **WHEN** 行末が `[left, right)` の途中までしか無い行を写像する
- **THEN** `[left, 行末]` までの存在する文字だけが char range に入り、右側を空白で埋めない

#### Scenario: ポリシー差し替えが1箇所で済む

- **WHEN** ragged line ポリシーを空扱いから空白パディングへ切り替える
- **THEN** コピー・カット・ハイライトを横断せず、隔離された1箇所の変更で切り替えられる

### Requirement: 3能力で同一の切り出し規則を共有する

ハイライト・コピー・カットは、列計算と短い行扱いについて **完全に同一の切り出し規則を共有** しなければならない（MUST）。3者で規則が分岐してはならない（MUST NOT）。

#### Scenario: 見た目・コピー・カットの範囲一致

- **WHEN** 同一の矩形について、ハイライト判定・コピー切り出し・カット削除の各範囲を求める
- **THEN** 3者の char range は完全に一致する
