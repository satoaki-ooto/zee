## 1. 安全網（着手前・依存ゼロ）

- [x] 1.1 通常 copy/cut/yank の現挙動を固定するテストを確認・追加（`copy_selection_to_clipboard` / `cut_selection_to_clipboard` / `paste_from_clipboard` の clipboard 入出力、C-3 の基準点）
- [x] 1.2 既存の矩形コピー/カットが clipboard へ書く文字列表現（`\n` 連結・末尾改行なし）を固定するテストを確認・追加（本 change で表現を変えないことの基準点）

## 2. データモデルとストア土台（D1）

- [x] 2.1 矩形 kill のデータモデル `RectangleKill { lines: Vec<String>, clipboard_text: String }` を定義する
- [x] 2.2 内部可変ストア（`Option<RectangleKill>` を `parking_lot::RwLock` で包む）を定義し、`Context`（`zee/src/editor/mod.rs`）に clipboard と並ぶ `Arc<...>` フィールドとして足す。`Context` は `&'static` 共有のため内部可変で持つ（clipboard と同型のパターン）
- [x] 2.3 `Properties`／`Context` 初期化箇所（`editor/mod.rs`）でストアを生成・配線する

## 3. 矩形 copy/cut からの記録（spec: rectangle-copy / rectangle-cut の MODIFIED）

- [x] 3.1 `rectangle_copy`（`buffer.rs:594` 付近）で `set_contents(clipboard_text)` の直後に、`parts`（切り出し行リスト）と `clipboard_text` を矩形 kill として記録する
- [x] 3.2 `rectangle_cut`（`buffer.rs:719` 付近）で同様に、取り除いた行リストと `clipboard_text` を記録する
- [x] 3.3 幅0矩形コピー（clipboard を触らない no-op 経路）では矩形 kill を記録しないことを確認する
- [x] 3.4 矩形削除（clipboard に書かない経路）では矩形 kill を記録しないことを確認する

## 4. 判定・復元 API（D2 / D3、spec: rectangle-kill-store）

- [x] 4.1 「判定付き取得（peek）」を実装する: ストアが `Some` かつ `clipboard.get_contents()` が記録 `clipboard_text` と一致するとき行リストを返し、一致しない/未記録なら「無し」を返す。矩形 kill を消費しない
- [x] 4.2 改行コード差（`\n` ↔ `\r\n`）への対処を入れる: 比較前に同一正規化を通す（system-clipboard feature 有効時に要確認。万一書き換えられても「無し」へ安全縮退）

## 5. テスト（spec の各シナリオを満たす）

- [x] 5.1 矩形コピー → 直後の判定が「矩形 kill である」、復元で切り出し行リスト（短い行の空文字列を含む）が得られる
- [x] 5.2 矩形カット → 同様に記録・判定・復元できる。矩形削除では記録されない
- [x] 5.3 幅0矩形コピー/カットで clipboard もストアも変化しない
- [x] 5.4 矩形コピー後に通常コピー/カットで clipboard を上書き → 判定が「矩形 kill ではない」になる（無効化、通常側コード変更なし）
- [x] 5.5 clipboard 内容が記録文字列と食い違う状態（外部上書き相当）→ 判定が「無し」になる
- [x] 5.6 復元を2回連続取得しても同じ行リストが得られる（非消費＝peek）
- [x] 5.7 通常 copy/cut/yank の clipboard 入出力が 1.1 の基準点と一致（C-3 不変）

## 6. 仕上げ

- [x] 6.1 `cargo test`（zee / zee-edit）と `cargo clippy` を通す。system-clipboard feature 有効ビルドでもビルド確認
- [x] 6.2 矩形コピー → 通常コピー → 判定が外れる、の一連を確認（誤判定しないこと）
- [x] 6.3 後続 S2（基本 yank-rectangle）が「判定付き取得」を入口に使える形になっていることを確認（API の利用面の確認のみ。貼り付けは S2）
