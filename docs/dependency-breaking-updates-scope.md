# 依存更新（独立系）要件定義 — ワーキングメモリ

> 短寿命ファイル。このサイクル（`chore/update-deps-survey`）の焦点を外部化したもの。
> 一区切りしたら観察を `work-journal` に引き継ぎ、本ファイルは次スコープで上書き/破棄する。

## もとの上位要件

`docs/dependency-breaking-updates.md` の推奨対応順序 step1「小〜中の独立系から着手: `colored` / `dirs` / `unicode-width` / `libloading`」。
**何が解けたら終わりか**: この4パッケージを最新メジャーへ追従し、`cargo build --workspace` と `cargo test --workspace` が緑のまま回り、表示幅に依存する挙動に回帰がないこと。

切り出し方針: **性質で2分割**。API/版追従だけで済む独立系3つ（Scope A）と、幅計算値そのものが変わり回帰確認を要する `unicode-width`（Scope B）を分ける。

---

## 実装スコープ（今回やる）

### Scope A: API/版追従の独立系3つ（`colored` / `dirs` / `libloading`）

紐づく上位要件: 上記 step1「独立系から着手」— ワークスペースを最新依存でビルド・テスト通過させる土台づくり。

調査の結論として **いずれも使用 API のシグネチャは不変** で、Cargo.toml のバージョン要求更新が主。コード修正は原則発生しない見込み（発生した場合のみ最小修正）。

- **colored 2 → 3**
  - 使用箇所: `zee/src/main.rs:66` `colored::control::set_override(false)`、`zee-grammar/src/builder.rs` の `Colorize` メソッド群（`.bold()` `.bright_*()` `.dimmed()` 等）。
  - colored 3.x は `Colorize` API・`control::set_override` を維持（追加 API と MSRV 1.70 / winapi→windows が主変更）。→ コード変更見込みなし。
  - 変更後（`zee/Cargo.toml` と `zee-grammar/Cargo.toml` 両方）:
    ```toml
    colored = "3"
    ```
- **dirs 4 → 6（使用中の zee-grammar のみ追従）**
  - 使用箇所: `zee-grammar/src/config.rs:145` `dirs::config_dir()` の1箇所のみ。
  - `config_dir() -> Option<PathBuf>` はシグネチャ不変。→ コード変更見込みなし。
  - 変更後（`zee-grammar/Cargo.toml`）:
    ```toml
    dirs = "6"
    ```
  - 併せて **`zee/Cargo.toml` の `dirs = "4.0.0"` は削除**（版追従しない）。
    - 根拠: `zee/` 内で `dirs` の使用は無く（宣言1行のみ）、`dirs::config_dir()` は commit `fed7bd8` で zee から削除済み。config dir 解決は `zee-grammar` へ移管されており、zee の `dirs` はデッドな依存。追従はデッドコードを最新版で温存するだけで読み手を誤誘導するため削除する。
    - 変更後（`zee/Cargo.toml` から該当行を除去）:
      ```toml
      colored = "3"
      euclid = "0.22.7"
      ```
- **libloading 0.7 → 0.9**
  - 使用箇所: `zee-grammar/src/builder.rs` の `Library::new` / `library.get` / `Symbol<unsafe extern "C" fn() -> Language>`。
  - 高レベル API（`Library::new` / `get` / `Symbol`）は unsafe 境界含め不変。→ コード変更見込みなし。
  - 変更後（`zee-grammar/Cargo.toml`）:
    ```toml
    libloading = "0.9"
    ```
- 検証: 上記反映後に `cargo build --workspace` → `cargo test --workspace`。warning（特に未使用依存）も確認する。

### Scope B: `unicode-width` 0.1 → 0.2（挙動変化を伴う）

紐づく上位要件: 同 step1 ＋ `docs/dependency-breaking-updates.md` の「表示幅に依存する箇所の回帰確認」。他3つと違い **API は不変だが幅計算値が変わる**（例: `\n` を幅1として扱う、ambiguous Modifier_Letters を narrow 扱い、合字対応）ため、回帰確認まで含める。

- 直接依存の使用箇所: `zee-edit/src/graphemes.rs:4,16` の `UnicodeWidthStr::width()`（`graphemes::width()` 経由で矩形列計算 `visual_column_*` が依存）。
- 変更後（`zee-edit/Cargo.toml`）:
  ```toml
  unicode-width = "0.2"
  ```
- 0.2 の挙動変化のうち **このリポジトリへの実害は `\n`=幅1 化のみ**（CJK=2 / tab は不変、ambiguous Modifier_Letters narrow・合字はエディタ常用文字でほぼ無関係）。`width()` 全呼び出し元を調査した結果、`\n` が `width()` に到達するのは **textarea 描画経路のみ**:
  - 安全（`\n` 来ない）: 矩形列計算 `visual_column_*`（改行除去済みの行スライス＋グラフェム単位）、sticky column `movement.rs:67`、`column_offset` `lib.rs:149`（いずれも行内スライス）、`movement.rs:75`（`\n` は `|| grapheme.slice == "\n"` で break し幅を加算しない）。
  - 露出: `textarea.rs:188-218`。`text.line(idx)` は末尾 `\n` を含む行を返し、`\n` グラフェムも `width()` に渡る。0.1 は幅0→`grapheme_width == 0` 分岐でスペース描画、0.2 は幅1→`draw_graphemes('\n')` 分岐へ。行末描画に実差が出るため、必要なら `\n` を明示スキップ/幅0扱いする軽微なコード修正を伴う。

- **合格ライン（このスコープの「完了」の定義）**:
  1. `cargo build --workspace` 成功＋`zee-edit` 既存テスト（約55件）緑化。
  2. **`\n` に絞った回帰テストを追加**: `\n` を含むスライスの `width()`、および textarea の行末描画相当の挙動を最小テストで固定する。CJK/tab は既存テストで足りるため新規追加しない。
  3. **textarea 手動確認**: 複数行バッファの実描画を目視し、行末・折り返し・カーソル位置に崩れがないことを確認する。崩れがあれば `\n` 明示処理の修正を Scope B 内で行う。

---

## 保留（次の実装スコープに回す）

- （現時点で無し。`zee` の未使用 `dirs` は Scope A 内で「削除」と決着済み。）

## 対象外（今回は作らない）

- `zi::unicode_width` 再エクスポート経由の `status_bar.rs` / `splash.rs` / `prompt/buffers.rs`。これらは `zi` クレートの依存であり、`zee-edit/Cargo.toml` のバンプでは動かない。今回のスコープ外（`zi` 更新時に別途扱う）。
- `config/build/tree-sitter-*` の各文法 Cargo.toml。cargo 依存解決対象外（doc 既述）。

## 停止条件メモ（観察用）

- 「各クレートが step1 のどの目的に紐づくか」が一行で言える状態で止めた。これ以上（例: Cargo.toml の1行ごと）に割ると紐づきが希薄になるため、ここを切り出しすぎの境界と判断。
- Scope A を3クレートでまとめたのは、いずれも「API 不変・版追従のみ」という性質が同一で、別々に紐づけても同じ一行になるため。性質が分かれる `unicode-width` だけを割った。

## 残された曖昧さ

- （現時点で無し。合格ラインは Scope B で「既存緑化＋`\n` 回帰テスト＋textarea 手動確認」と決着済み。実装段階で textarea に実差が出た場合の修正方針（`\n` を幅0扱いに固定 vs グラフェム走査から除外）は実装プランで具体化する。）

## 次にどうする

- 実装プランへ渡す。Scope A → Scope B の順で着手（A は機械的、B は回帰確認が本体）。各スコープ完了ごとに `cargo build/test --workspace`。
