# 破壊的変更を伴う依存パッケージ更新一覧

非破壊的更新（semver 互換）は `cargo update` で適用済み。本書は **メジャー / 0.x マイナー跨ぎで API 修正・ビルド検証が必要** な更新対象を1枚にまとめたもの。各パッケージは個別に影響調査 → 対応 → ビルド/テスト検証の順で進める。

- 対象ワークスペース: `zee` / `zee-edit` / `zee-grammar` / `zee-highlight`
- 基準日: 2026-06-13
- Rust: 1.96.0 / edition 2024 / MSRV 1.85

## 一覧

| パッケージ | 現行 | 最新 | Cargo.toml 要求 | 使用クレート | 影響度 | 主な破壊的変更 |
|---|---|---|---|---|---|---|
| **tree-sitter** | 0.20.10 | 0.26.9 | `0.20.8` / `0.20.6` | zee, zee-grammar, zee-highlight | 大 | Language/Parser API 変更。`config/build/` 配下の各 tree-sitter 文法クレートにも波及。文法バージョンの整合が必要 |
| **nom** | 5.1.3 | 8.0.0 | `5.1.2` | zee-highlight | 大 | パーサコンビネータ API 全面刷新（`named!` マクロ廃止、関数スタイルへ移行）。`zee-highlight` のパーサ全書き換え相当 |
| **clap** | 3.2.25 | 4.6.1 | `3.2.14` (derive) | zee | 大 | derive API 再設計。`#[clap(...)]`→`#[arg(...)]`/`#[command(...)]`、`App`→`Command` 等 |
| **ron** | 0.7.1 | 0.12.1 | `0.7.1` | zee | 大 | シリアライズ/デシリアライズ API・拡張仕様変更。設定ファイル(`.ron`)の互換性検証が必要 |
| **thiserror** | 1.0.69 | 2.0.18 | `1.0.31` | zee | 中 | エラー derive のソース/From 周りの仕様変更。属性記法の見直し |
| **git2** | 0.14.4 | 0.21.0 | `0.14.4` | zee | 中 | libgit2 バインディング更新。型シグネチャ・列挙子の変更あり。system libgit2 連携注意 |
| **palette** | 0.5.0 | 0.7.6 | `0.5.0` | zee | 中 | 色型・変換トレイトの再設計。カラースキーム生成箇所の修正が必要 |
| **flexi_logger** | 0.22.6 | 0.31.9 | `0.22.5` | zee | 中 | ロガー構築 API・フォーマッタ仕様の変更 |
| **libloading** | 0.7.4 | 0.9.0 | `0.7.3` | zee-grammar | 小〜中 | `Library`/`Symbol` の unsafe 境界・API 変更。文法 .so 動的ロード箇所 |
| **dirs** | 4.0.0 | 6.0.0 | `4.0.0` | zee, zee-grammar | 小〜中 | プラットフォーム別ディレクトリ解決の挙動・依存(`dirs-sys`)更新 |
| **colored** | 2.2.0 | 3.1.1 | `2.0.0` | zee, zee-grammar | 小 | カラー出力 API のマイナーな破壊的変更 |
| **unicode-width** | 0.1.14 | 0.2.2 | `0.1.9` | zee-edit | 小 | 幅計算テーブル更新・API 微変更。表示幅に依存する箇所の回帰確認 |

## Cargo.toml 別の影響範囲

cargo の依存解決対象は **ワークスペースメンバー 4 クレートのみ**。各 Cargo.toml が受ける破壊的更新は以下の通り。

| Cargo.toml | 影響を受ける破壊的パッケージ（要求 → 最新） |
|---|---|
| `zee/Cargo.toml` | clap `3.2.14`→4.6 / colored `2.0.0`→3.1 / dirs `4.0.0`→6.0 / flexi_logger `0.22.5`→0.31 / git2 `0.14.4`→0.21 / palette `0.5.0`→0.7 / ron `0.7.1`→0.12（deps + build-deps の 2 箇所）/ thiserror `1.0.31`→2.0 / tree-sitter `0.20.8`→0.26 |
| `zee-edit/Cargo.toml` | unicode-width `0.1.9`→0.2 |
| `zee-grammar/Cargo.toml` | colored `2.0.0`→3.1 / dirs `4.0.0`→6.0 / libloading `0.7.3`→0.9 / tree-sitter `0.20.8`→0.26 |
| `zee-highlight/Cargo.toml` | nom `5.1.2`→8.0 / tree-sitter `0.20.6`→0.26 |

### `config/build/tree-sitter-*/Cargo.toml`（22 文法ディレクトリ）について

各ディレクトリに `tree-sitter = "0.19" / "0.20" / "~0.20" / ">= 0.19, < 0.21"` と `cc = "1.0"` の記載があるが、**cargo の依存解決対象外**である。

- ワークスペース members は 4 クレートのみで、これら文法ディレクトリへの `path` 依存・`[patch]`・members 登録は一切ない。
- `zee-grammar/src/builder.rs` は文法を C ソース（`parser.c` / `scanner.c`）として `cc` で直接コンパイルし、`libloading` で `.so` を実行時ロードする方式。各文法の Rust 用 `Cargo.toml`（上流マニフェスト）は zee のビルドで参照されない。
- したがって tree-sitter を 0.26 へ上げても、これらの Cargo.toml 制約がビルドをブロックすることはない。

**真の論点**: Cargo.toml ではなく、コンパイル済みグラマの **C ABI / `LANGUAGE_VERSION` 互換性**。tree-sitter ランタイム(0.26) と、古い ABI 向けに生成された `parser.c` の整合が必要で、互換しない場合は対応する tree-sitter-cli でのグラマ再生成が要る。tree-sitter 更新タスクではこの検証を中心に据える。

## 推奨対応順序（影響度・依存波及を考慮）

1. **小〜中の独立系から着手**: `colored`, `dirs`, `unicode-width`, `libloading`
2. **中規模**: `thiserror`, `flexi_logger`, `palette`, `git2`
3. **設定互換が絡む**: `ron`, `clap`
4. **最大の波及（文法群連動）**: `tree-sitter`, `nom`

各対応後に `cargo build --workspace` と `cargo test --workspace` を実行し回帰を確認する。

## 据え置き理由メモ

- `tree-sitter`: `config/build/tree-sitter-*` の各文法クレートと連動するため、文法側の更新方針を含めた別タスクとして扱う。
- それ以外: 単体で API 修正が必要なため、上記順序で1パッケージずつ段階対応する。
