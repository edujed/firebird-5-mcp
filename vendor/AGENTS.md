# vendor — Code Summary (for AI agents)

> **Purpose:** token-efficient map of `vendor/`. Read this first to locate a
> function before opening the full source. Line numbers are 1-based and refer to
> the file state at the time of writing — re-verify with `grep` if they drift.
>
> **What this is:** a vendored copy of `rsfbclient-rust` 0.29.0 (pure-Rust
> Firebird wire-protocol client, MIT) consumed by the workspace through
> `[patch.crates-io]`, **not** as a workspace member. It is a standalone
> workspace (own `[workspace]` in `Cargo.toml`) so its unit tests run
> independently: `cd vendor/rsfbclient-rust && cargo test --lib`.
>
> **Why it is vendored:** the NUMERIC/DECIMAL precision patch. Upstream coerces
> scaled integers to `f64`, losing exactness (`NUMERIC(18,4)` is the standard
> money type in Firebird schemas). The patch keeps the scaled integer + scale in
> `xsqlda.rs` and renders the exact decimal as text at fetch time in `wire.rs`
> (`scaled_to_decimal_string` / `scaled_to_decimal_string_i128`). All local
> changes are marked with `FIREBIRD-5-MCP PATCH` comments (11 sites: 8 in
> `wire.rs`, 3 in `xsqlda.rs`) — grep for that to find every deviation from
> upstream.
>
> **Shared types** come from `rsfbclient-core` 0.29.0 (crates.io, not vendored):
> `SqlType`, `Column`, `FbError`, `Charset`, `TrOp`, `FreeStmtOp`, `Dialect`,
> `ibase` constants. This crate is the wire-protocol layer on top of those.

## File map

| File | Lines | Role |
|---|---|---|
| `rsfbclient-rust/Cargo.toml` | 91 | Crate manifest — standalone `[workspace]`, feature `fuzz_testing`, deps incl. `rsfbclient-core` 0.29.0 |
| `rsfbclient-rust/Cargo.toml.orig` | 31 | Original upstream manifest (the one above is cargo-normalized) |
| `rsfbclient-rust/src/lib.rs` | 18 | Module declarations; public API: `client::{DbHandle, RustFbClient, RustFbClientAttachmentConfig, StmtHandle, TrHandle}`, `raw::RawValue`, and `pub use` of `blr::*`, `wire::*`, `xsqlda::*` (L18) |
| `rsfbclient-rust/src/client.rs` | 1957 | Connection + client API + batch fetch + zero-copy streaming + SRP handshake |
| `rsfbclient-rust/src/wire.rs` | 1720 | Wire-protocol message builders + parsers; `RowSink` decode; exact-decimal rendering (patch) |
| `rsfbclient-rust/src/consts.rs` | 1954 | `ProtocolVersion`, `WireOp`, `Cnct`, `AuthPluginType`, `gds_to_msg` (huge), `mod blr` |
| `rsfbclient-rust/src/xsqlda.rs` | 472 | `XSqlVar` + `coerce()` (patch), `xsqlda_to_blr`, `parse_xsqlda` |
| `rsfbclient-rust/src/events.rs` | 926 | Remote events: EPB encode/parse, auxiliary `EventChannel` (most of it is tests) |
| `rsfbclient-rust/src/srp.rs` | 352 | SRP (RFC 5054) client — `SRP_GROUP`, `SrpClient`, `srp_private_key` |
| `rsfbclient-rust/src/util.rs` | 204 | `BufMutWireExt` / `BytesWireExt` non-panicking accessors, `ERR_MISSING_BYTES`, `is_incomplete` |
| `rsfbclient-rust/src/blr.rs` | 162 | `params_to_blr` — encodes bind params into BLR + values (blobs via round-trip) |
| `rsfbclient-rust/src/arc4.rs` | 135 | Arc4 stream cipher for Wire Protocol 13 encryption |
| `rsfbclient-rust/src/raw.rs` | 37 | `RawValue` enum — zero-copy row value (text is a `Bytes` handle) |

## `client.rs`

| Item | Lines | Summary |
|---|---|---|
| `fetch_batch_size()` | 30–36 | Rows per `op_fetch` round-trip; env `FB_FETCH_BATCH`, default 200 (upstream used 1) |
| `READ_BUFFER_LEN` | 43 | 64 KiB scratch buffer for one socket read |
| `enum FetchOne<T>` / `Framed` | 50–69 | One parsed `op_fetch_response`: `Row` / `BatchEnd` (batch done, cursor open) / `End` (cursor exhausted) |
| `RustFbClient` | 72–75 | Public client: `Option<FirebirdWireConnection>` + charset |
| `RustFbClientAttachmentConfig` | 79–86 | host/port/db_name/user/pass/role_name |
| `FirebirdWireConnection` | 89–125 | Socket, protocol version, read buffer, `pending` (unconsumed bytes — the protocol has no packet framing), `lazy_count`, charset, auth state, `event_channel` |
| `StmtHandleData` | 128–146 | Statement: handle, xsqlda, blr, param_count, `prefetched` row buffer, `cursor_eof`, `has_blob` |
| `impl RustFbClient` (traits) | 158–366 | `FirebirdClientDbOps` / `FirebirdClientSqlOps` / `FirebirdClientDbEvents` — thin delegation to the connection (trait fns at L162–363) |
| `stream_raw()` | 385–397 | Zero-copy streaming fetch into a reusable `Vec<RawValue>` (blob-free statements only) |
| `FirebirdWireConnection::connect` | 402–555 | TCP connect, `set_nodelay`, SRP handshake (Srp/Srp256, `continue_auth` loop), Arc4 wire encryption |
| `create_database` / `attach_database` | 558–613 | dpb-based attach/create, returns `DbHandle` |
| `detach_database` / `drop_database` | 616–642 | Close event channel first (server drops the aux port with the attachment) |
| `wait_for_event` | 646–667 | Two-phase registration: register with count 0 to sync, then re-register with the current counter |
| `que_events` / `cancel_events` | 685–733 | Register interest, block on `EventChannel::recv_event`; best-effort cancel on error |
| `open_event_channel` / `close_event_channel` | 737–765 | `op_connect_request` → `parse_aux_port` → open aux TCP (reuses main peer address, server-reported address is NAT-wrong) |
| `begin_transaction` | 768–795 | Builds tpb (isolation, data access, lock timeout) |
| `transaction_operation` | 798–810 | Commit / rollback (incl. retaining) |
| `exec_immediate` | 813–830 | One-shot SQL |
| `prepare_statement` | 835–938 | alloc + prepare in one write; reads both responses; loops `info_sql` while xsqlda truncated; `coerce()` + `xsqlda_to_blr`; sets `has_blob` |
| `free_statement` | 941–953 | Lazy response — bumps `lazy_count` |
| `execute` | 956–1000 | Resets cursor state, `params_to_blr`, `op_execute`, then `info_sql` for affected rows |
| `execute2` | 1003–1079 | Like `execute` but returns the rows; blobs resolved only after the response is fully consumed |
| `fetch` | 1085–1102 | One row from the `prefetched` buffer; refills via `fetch_batch` (safety net: >1000 empty batches = error) |
| `fetch_batch` | 1106–1121 | One `op_fetch` for N rows; blob-free → `read_batch_columns`, blobs → `read_batch_deferring_blobs` |
| `read_batch_columns` | 1127–1174 | Decodes rows straight into `Vec<Column>` (no `ParsedColumn` buffer) |
| `read_batch_deferring_blobs` | 1178–1240 | Reads the whole batch first, then resolves blobs (their round-trips must not interleave with unparsed rows) |
| `stream_raw` (conn) | 1246–1281 | Streaming into `Vec<RawValue>`; rejects blob statements and cursors with buffered rows |
| `fetch_batch_raw` | 1285–1336 | One raw batch; `visit` is called per row before the next response is read (text borrows the buffer) |
| `create_blob` / `put_segments` / `open_blob` / `get_segment` / `close_blob` | 1339–1411 | Blob round-trips; `get_segment` returns `(bytes, end)` |
| `read_response` | 1414–1421 | Wrapper over the free `read_response` |
| `read_with` | 1436–1479 | **The** response reader: parse → on `ERR_MISSING_BYTES` read more and retry (rolls back `lazy_count`); surplus bytes stay in `pending` |
| `next_op_code` | 1482–1490 | Skips `op_dummy` keepalives |
| `skip_lazy_responses` | 1493–1507 | Consumes the pending lazy `op_response`s before the wanted op |
| `parse_one_fetch_response{,_columns,_raw}` | 1511–1572 | Frame one fetch response, then decode the row (3 variants) |
| `frame_fetch_response` | 1577–1616 | Peeks status+messages: 100 = end of cursor (consumes both words), messages=0 = batch end, else row |
| `read_response` (free fn) | 1619–1634 | `read_with` + `skip_lazy_responses` + `parse_response` |
| `srp_verifier` / `srp_auth` | 1636–1703 | SRP proof exchange, then `op_crypt` → wraps socket in `Arc4Stream` (`srp_verifier` is `pub(crate)`) |
| `normalize_user` | 1706–1712 | Uppercase unless quoted |
| Handles | 1721–1739 | `DbHandle`, `TrHandle`, `StmtHandle`, `BlobHandle`, `BlobId` (all newtypes over u32/u64) |
| `FbStream` | 1742–1783 | `Plain(TcpStream)` / `Arc4(Arc4Stream<TcpStream>)` with `Read`/`Write` |
| `mod read_tests` | 1786–1895 | Chunked-read tests: reassembly across reads, surplus bytes, end-of-cursor framing |
| `connection_test` | 1899–1957 | Live-connection test (needs a server) |

## `wire.rs`

| Item | Lines | Summary |
|---|---|---|
| `BUFFER_LENGTH` | 19 | 1024 — blob segment / response buffer size |
| `connect` | 22–106 | `op_connect` request: protocol list, SRP `a_pub`, plugin name/list, `ClientCrypt` |
| `cont_auth` / `crypt` | 109–134 | Auth continuation / wire-encryption requests |
| `attach` / `create` | 137–210 | Attach/create requests wrapping `build_dpb` |
| `build_dpb` | 213–329 | dpb items: page size, charset, user, role, dialect, no_db_triggers; password per protocol (V10 plaintext, V11/12 crypt, V13 SRP proof) |
| `DpbData` / `dpb` | 331–360 | dpb item struct + v1/v2 serialization |
| `detach` / `drop_database` | 363–380 | |
| `transaction` / `transaction_operation` | 383–408 | |
| `exec_immediate` | 411–429 | |
| `allocate_statement` / `prepare_statement` | 432–463 | |
| `info_sql` / `free_statement` | 466–487 | |
| `execute` / `execute2` | 490–531 | |
| `fetch` | 534–544 | `op_fetch` with a row count (batch fetch) |
| `connect_request` / `que_events` / `cancel_events` | 551–585 | Event wire ops |
| `EventNotification` / `parse_event_notification` | 589–611 | |
| `parse_aux_port` | 622–637 | Port at offset 2 of the raw sockaddr (network byte order) |
| `create_blob` … `close_blob` | 640–693 | Blob wire ops |
| `Response` / `parse_response` | 697–717 | `op_response`: handle, object_id, data, status vector |
| `parse_fetch_response{,_columns}` | 720–756 | Status word (100 = end) then row |
| `parse_sql_response{,_columns}` | 760–800 | Row without status word (`op_sql_response`) |
| `parse_fetch_response_raw` / `parse_sql_response_raw` | 807–836 | Zero-copy variants into `Vec<RawValue>` |
| `trait RowSink` | 842–873 | One decode, three destinations; `scaled_integer` (L852) / `scaled_int128` (L857) are **patch** methods |
| `ParsedColumnSink` | 876–966 | Collects `ParsedColumn` (blobs deferred); patch sites at L909, L915, L929 |
| `ColumnSink` | 969–1048 | Builds `Column` directly (blob-free only); patch sites at L1002, L1016 |
| `RawSink` | 1052–1116 | Emits `RawValue` (no charset decode, no allocation); patch sites at L1074, L1086 |
| `scaled_to_decimal_string` | 1123–1142 | **PATCH** — exact decimal text for scaled `i64` |
| `scaled_to_decimal_string_i128` | 1146–1165 | **PATCH** — same for `i128` |
| `decode_row` | 1170–1318 | The single wire-format decoder: null bitmap (V13), per-type reads; routes scaled ints to the patch methods (sites at L1239, L1256) |
| `ParsedColumn` | 1321–1377 | `Complete(Column)` / `Blob{...}`; `into_column` does the blob round-trip |
| `parse_status_vector` | 1380–1463 | Error/warning status vector → `FbError::Sql` |
| `ConnectionResponse` / `AuthPlugin` | 1467–1478 | |
| `parse_accept` | 1481–1531 | `op_accept` / `op_accept_data` / `op_cond_accept` |
| `parse_cont_auth` | 1534–1556 | |
| `SrpAuthData` / `parse_srp_auth_data` | 1559–1596 | Salt + hex pub key (raw bytes, not hex-decoded salt) |
| `parse_info_sql_affected_rows` | 1599–1635 | |
| `mod tests` | 1638–1720 | INT128 bit-preservation + **patch** tests for exact decimal rendering (marker at L1675) |

## `xsqlda.rs`

| Item | Lines | Summary |
|---|---|---|
| `XSQLDA_DESCRIBE_VARS` | 13–31 | Info items requested on prepare |
| `XSqlVar` | 35–60 | Column metadata; `alias_name: Arc<str>` (built once, cloned per row) |
| `XSqlVar::coerce` | 64–129 | **PATCH** — keeps scaled `SQL_INT64`/`SQL_INT128` as exact scaled integers instead of coercing to double (patch sites at L82, L93) |
| `xsqlda_to_blr` | 133–180 | XSqlVar slice → BLR (carries the signed scale byte) |
| `PrepareInfo` | 183–187 | stmt_type, param_count, truncated |
| `parse_xsqlda` | 193–258 | Prepare response → statement type + xsqlda (rejects >1024 columns) |
| `parse_select_items` | 261–417 | Item-by-item xsqlda fill; `true` when truncated |
| `err_invalid_xsqlda` | 419–423 | Helper: `FbError` for malformed xsqlda |
| `mod tests` | 426–472 | **PATCH** tests (marker at L431): fixed-point and integer INT128 preserved through `coerce` + BLR |

## `events.rs`

| Item | Lines | Summary |
|---|---|---|
| `EPB_VERSION1` | 33 | EPB format version (1) |
| `MAX_EVENT_NAME_LEN` / `MAX_EVENT_BLOCK_LEN` | 37–41 | u8 / u16 limits |
| `AUX_CONNECT_TIMEOUT` | 56 | 180 s — matches the server's `ConnectionTimeout` default |
| `normalize_event_name` / `encode_event_name` | 63–97 | Trim trailing blanks; encode in the connection charset (limit is on encoded bytes) |
| `event_block` / `parse_event_block` | 105–169 | EPB v1 build/parse (counters little-endian) |
| `event_count` | 172–183 | Counter of one name (missing = error) |
| `connect_aux` / `set_keep_alive` | 189–301 | Aux socket: blocking connect with timeout, keep-alive on (per-OS `set_keep_alive` variants + `keep_alive_option` at L285) |
| `EventChannel` | 305–452 | Aux connection: `open` (L328, reuses main peer address), `recv_event` (L356, skips stale notifications), `recv_notification` (L367, skips `op_dummy`, caps EPB length), `fill` (L427) |
| `mod tests` | 455–926 | ~470 lines of tests (keep-alive, EPB round-trips, charset, aux connect) |

## `srp.rs`

| Item | Lines | Summary |
|---|---|---|
| `SRP_GROUP` | 13–27 | Firebird's 2048-bit safe prime, g=2 (lazy `static`) |
| `SrpClient` | 30–37 | Pre-handshake state |
| `SrpClientVerifier` | 40–44 | Post-handshake: proof + key (SHA1, per Firebird's quirk) |
| `srp_private_key` | 48–60 | RFC 5054 private key |
| `SrpClient::new` / `calc_key` / `process_reply` / `get_a_pub` | 64–160 | Handshake; `process_reply` guards against malicious B |
| `SrpClientVerifier::get_key` / `get_proof` | 166–174 | |
| `powm` | 176–192 | Modular exponentiation |
| `SrpAuthError` | 195–210 | |
| `SrpGroup` | 214–236 | n, g, `powm`, `compute_k` |
| `mod test` | 239–352 | Known-answer tests for SRP (SHA1) and Srp256 (SHA256) |

## `util.rs`

| Item | Lines | Summary |
|---|---|---|
| `BufMutWireExt::put_wire_bytes` | 9–26 | u32 length + data, 4-byte aligned |
| `BytesWireExt` | 29–177 | Non-panicking accessors (`get_u8`…`get_f64`, `get_u16_le`/`get_u32_le`, `get_wire_bytes`, `advance`) — all return `ERR_MISSING_BYTES` on underflow |
| `ERR_MISSING_BYTES` | 183 | "the response is incomplete, read more" sentinel |
| `err_invalid_response` | 185–189 | Helper: `FbError` for malformed responses |
| `is_incomplete` | 191–193 | Distinguishes "read more" from a real error |
| `err_conn_rejected` | 195–204 | |

## `blr.rs`

| Item | Lines | Summary |
|---|---|---|
| `MAX_DATA_LENGTH` | 6 | 32767 — max param data before it goes to a blob |
| `ParamsBlr` | 10–15 | blr + values |
| `params_to_blr` | 18–148 | Encodes `&[SqlType]`: text (blob if >32 KiB), binary (blob), int, int128, float, timestamp, bool, null; V13 null bitmap vs per-column nullind |
| `null_bitmap` | 150–162 | V13: 32 params per u32 |

## `arc4.rs`

| Item | Lines | Summary |
|---|---|---|
| `Arc4` | 9–50 | RC4/Arc4 cipher (KSA + PRGA) |
| `Arc4Stream<S>` | 53–118 | Wraps a stream: separate read/write ciphers, encrypt/decrypt through a scratch buffer |
| `arc4_test` | 120–135 | Known-answer test |

## `raw.rs`

| Item | Lines | Summary |
|---|---|---|
| `RawValue` | 17–28 | `Null` / `Integer(i64)` / `Int128(i128)` / `Floating(f64)` / `Boolean` / `Timestamp` / `Text(Bytes)` — text borrows the wire buffer (O(1) clone) |
| `RawValue::text_bytes` | 34–36 | Build a `Text` from a slice (tests/consumers without a wire buffer) |

## `consts.rs`

| Item | Lines | Summary |
|---|---|---|
| `ProtocolVersion` | 9–14 | V10–V13 |
| `WireOp` | 19–137 | All wire op codes (1–98) |
| `Cnct` | 148–167 | Connect-request user-identification tags |
| `P_REQ_ASYNC` | 171 | Aux-connection type for `ConnectRequest` |
| `AuthPluginType` | 174–205 | `Srp256` / `Srp`; `plugin_list()` must lead with the plugin named in the connect request (Firebird 5 rejects the extra round-trip when WireCrypt = Disabled) |
| `gds_to_msg` | 209–1651 | gds_code → message (the bulk of the file) |
| `mod blr` | 1654–1954 | BLR type/opcode constants |

## Conventions

- **Do not reformat or "clean up" upstream code.** The only intentional deviations from upstream are the `FIREBIRD-5-MCP PATCH` sites (grep for the marker). Everything else should stay byte-identical to rsfbclient-rust 0.29.0 so upstream changes can be re-applied cleanly.
- **The patch is three coordinated pieces:** `xsqlda.rs::coerce` keeps scaled ints as scaled ints; `wire.rs::decode_row` routes non-zero-scale columns to `scaled_integer`/`scaled_int128` sink methods; the sinks render exact decimal text via `scaled_to_decimal_string{,_i128}`. Changing one without the others breaks NUMERIC/DECIMAL exactness.
- **No packet framing.** The protocol is a byte stream; a response is complete exactly when it parses. All reads go through `client.rs::read_with`, which retries on `ERR_MISSING_BYTES` and keeps surplus bytes in `pending`. Never read the socket directly for a response.
- **Lazy responses.** `op_allocate_statement` and `op_free_statement` are acknowledged lazily; `lazy_count` tracks how many pending `op_response`s to skip, and `read_with` rolls it back on a failed parse attempt.
- **Batch fetch.** `fetch` serves rows from `prefetched`; one `op_fetch` requests `FB_FETCH_BATCH` rows (default 200). The terminating `op_fetch_response` (batch end or cursor end) must always be consumed — the batch loops break on `BatchEnd`/`End`, never on `got >= count`.
- **Blobs are two-phase.** A blob column only carries an id on the wire; data needs extra round-trips that must not interleave with unparsed rows, so blob statements buffer the whole batch (`read_batch_deferring_blobs`) before resolving. `stream_raw` and `parse_sql_response_columns` reject blob statements.
- **Events use a second connection.** The server pushes `op_event` on an auxiliary port (never encrypted); the client reuses the main connection's peer address and only takes the port from the server (`parse_aux_port`). A registration is one-shot — waiting again means registering again.
- **Tests:** `cargo test --lib` from `vendor/rsfbclient-rust/` (standalone workspace). `connection_test` (client.rs L1899) needs a live server; the rest are offline.
