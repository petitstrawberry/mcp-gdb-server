# MCP GDB Server

[English](README.md)

GDBを操作するためのMCP (Model Context Protocol) サーバー。gdb-multiarchとリモートデバッグターゲットに対応しています。

## 特徴

- **gdb-multiarch対応**: ARM, AArch64, RISC-V, MIPSなど複数アーキテクチャに対応
- **リモートデバッグ**: TCP接続（QEMU、JTAGデバッガ等）とシリアルポート接続に対応
- **GDB/MI プロトコル**: GDB Machine Interfaceを使用した確実な通信
- **豊富なツール**: ブレークポイント、実行制御、メモリ操作、レジスタアクセスなど

## インストール

### ビルド要件

- Rust 1.70以降
- GDB (gdb-multiarch推奨)

### ビルド

```bash
cd mcp-gdb-server
cargo build --release
```

バイナリは `target/release/mcp-gdb-server` に生成されます。

## 使用方法

### Claude Desktopでの設定

`claude_desktop_config.json` に以下を追加：

```json
{
  "mcpServers": {
    "gdb": {
      "command": "/path/to/mcp-gdb-server"
    }
  }
}
```

### 利用可能なツール

#### セッション管理

| ツール | 説明 |
|--------|------|
| `gdb_start` | GDBセッションを開始 |
| `gdb_stop` | GDBセッションを終了 |
| `gdb_status` | 現在のセッション状態を取得 |

#### ファイル操作

| ツール | 説明 |
|--------|------|
| `gdb_load_file` | 実行ファイルを読み込み |

#### リモートデバッグ

| ツール | 説明 |
|--------|------|
| `gdb_target_connect` | リモートターゲットに接続 (TCP/シリアル) |
| `gdb_target_disconnect` | リモートターゲットから切断 |

#### ブレークポイント

| ツール | 説明 |
|--------|------|
| `gdb_break_insert` | ブレークポイントを設定 |
| `gdb_break_delete` | ブレークポイントを削除 |
| `gdb_break_list` | ブレークポイント一覧を表示 |
| `gdb_break_toggle` | ブレークポイントの有効/無効を切り替え |

#### 実行制御

| ツール | 説明 |
|--------|------|
| `gdb_run` | プログラムを開始 |
| `gdb_continue` | 実行を継続 |
| `gdb_next` | ステップオーバー（ソース行） |
| `gdb_step` | ステップイン（ソース行） |
| `gdb_nexti` | ステップオーバー（命令単位） |
| `gdb_stepi` | ステップイン（命令単位） |
| `gdb_finish` | ステップアウト |
| `gdb_interrupt` | 実行を中断 |

#### スタック・スレッド

| ツール | 説明 |
|--------|------|
| `gdb_stack_list` | コールスタックを表示 |
| `gdb_stack_select` | スタックフレームを選択 |
| `gdb_stack_info` | 現在のフレーム情報を取得 |
| `gdb_thread_list` | スレッド一覧を表示 |
| `gdb_thread_select` | スレッドを選択 |

#### メモリ・レジスタ

| ツール | 説明 |
|--------|------|
| `gdb_memory_read` | メモリを読み込み |
| `gdb_memory_write` | メモリに書き込み |
| `gdb_registers_list` | レジスタ一覧を表示 |
| `gdb_register_set` | レジスタ値を設定 |

#### 変数・式評価

| ツール | 説明 |
|--------|------|
| `gdb_evaluate` | 式を評価 |
| `gdb_variable_info` | 変数の詳細情報を取得 |

#### 詳細操作

| ツール | 説明 |
|--------|------|
| `gdb_raw_command` | 生のGDB/MIコマンドを実行 |

## 使用例

### ローカルデバッグ

```
1. gdb_start                           # GDBセッション開始
2. gdb_load_file path="/path/to/binary" # 実行ファイル読み込み
3. gdb_break_insert location="main"    # main関数にブレークポイント
4. gdb_run                             # プログラム開始
5. gdb_next                            # ステップオーバー
6. gdb_evaluate expression="variable"  # 変数値を確認
```

### リモートデバッグ (QEMU)

```
1. gdb_start gdb_path="gdb-multiarch" architecture="aarch64"
2. gdb_target_connect host="localhost" port=1234
3. gdb_break_insert location="0x400000"
4. gdb_continue
```

### リモートデバッグ (シリアルJTAG)

```
1. gdb_start gdb_path="gdb-multiarch" architecture="arm"
2. gdb_target_connect serial_port="/dev/ttyUSB0" baud_rate=115200
3. gdb_break_insert location="main"
4. gdb_continue
```

## アーキテクチャ

```
┌─────────────────────────────────────────────────────────────┐
│                      LLM (Claude等)                          │
└──────────────────────────┬──────────────────────────────────┘
                            │ MCP Protocol (JSON-RPC)
                            ▼
┌─────────────────────────────────────────────────────────────┐
│                   MCP GDB Server                            │
│  ┌─────────────────┐  ┌─────────────────┐                  │
│  │  MCP Handler    │  │  Tool Handlers  │                  │
│  └────────┬────────┘  └────────┬────────┘                  │
│           └─────────────────────┘                          │
│                           │                                 │
│  ┌────────────────────────▼────────────────────────────┐   │
│  │              GDB Client                              │   │
│  │  ┌──────────────┐  ┌──────────────────────────────┐ │   │
│  │  │ MI Parser    │  │ Process Management           │ │   │
│  │  └──────────────┘  └──────────────────────────────┘ │   │
│  └─────────────────────────┬───────────────────────────┘   │
└────────────────────────────┼───────────────────────────────┘
                              │ GDB/MI Protocol
                              ▼
               ┌──────────────────────────────┐
               │   GDB (gdb-multiarch)        │
               │   ┌────────────────────┐     │
               │   │ Remote Target      │     │
               │   │ (QEMU/JTAG/Serial) │     │
               │   └────────────────────┘     │
               └──────────────────────────────┘
```

## ライセンス

MIT License

## 貢献

プルリクエストを歓迎します。大きな変更をする場合は、まずissueを開いて議論してください。
