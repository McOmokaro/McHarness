# aria2 Ingestor Dossier

**Lineage:** aria2 (aria2/aria2) — GPL-2.0-or-later, C++17, `v1.37.0`
**Status:** `extracting` → `forged`
**Forge target:** `mch-download` (Rust) — a `DownloadSession` abstraction owned by `mch-core`

---

## What I read, and how

I cloned the repo (`--depth 1`, ~937 source files under `src/`) into `forge/aria2` and read the C++ sources, not the README. aria2 is a single-threaded, event-driven reactor that multiplexes every protocol (HTTP/HTTPS/FTP/SFTP/BitTorrent/Metalink) onto **one** download model: a file is a `BitfieldMan` of fixed-size pieces, and every transport is reduced to "fetch piece N". That unification is the thing worth stealing — not the protocol parsers.

Four threads are load-bearing, and each maps to a section below:

- **Segmentation** — `src/Segment.h`, `src/PiecedSegment.*`, `src/SegmentMan.*`, `src/BitfieldMan.*`, `src/HttpRequest.cc`
- **Checkpoint** — `src/DefaultBtProgressInfoFile.*`, `src/BitfieldMan.*`, `src/AutoSaveCommand.*`, `src/SessionSerializer.*`
- **RPC** — `src/RpcMethodImpl.*`, `src/RpcMethodFactory.cc`, `src/HttpServer*.cc`, `src/WebSocketSessionMan.cc`
- **Scheduler** — `src/DownloadEngine.*`, `src/RequestGroupMan.*`, `src/RequestGroup.*`, `src/DefaultBtRequestFactory.cc`

Everything below is a claim I can point at a file for. Where I extrapolate, I say so.

---

## Architecture

### The engine is a reactor, and every action is a `Command`

`DownloadEngine::run` (`src/DownloadEngine.cc:158`) is the whole OS:

```cpp
while (!commands_.empty() || !routineCommands_.empty()) {
  if (!commands_.empty()) waitData();          // eventPoll_->poll(tv)
  executeCommand(commands_,        Command::STATUS_ACTIVE);
  executeCommand(routineCommands_, Command::STATUS_ALL);
  afterEachIteration();
}
```

- `Command` (`src/Command.h:44`) is a coroutine-by-hand: `virtual bool execute()` returns `true` to die, `false` to reschedule; state is a priority (`STATUS_INACTIVE/ACTIVE/REALTIME/ONESHOT_REALTIME`) plus four IO-event latches (`read/write/error/hup`).
- `EventPoll` (`src/EventPoll.h:51`) abstracts epoll/kqueue/port/poll/select; `EpollEventPoll` is the default (`src/DownloadEngineFactory.cc:102-141`). The engine registers a `(socket, Command*)` pair for read/write readiness; the scheduler never blocks on IO.
- Two queues: `commands_` (active work) and `routineCommands_` (always-run housekeeping — `FillRequestGroupCommand`, `AutoSaveCommand`, `SaveSessionCommand`, `HaveEraseCommand`, socket-pool eviction), wired in `src/DownloadEngineFactory.cc:163-183`.
- One download = one `RequestGroup`; all groups live in `RequestGroupMan`. A download advances through a **chain of commands** (`CreateRequestCommand → …RequestCommand → HttpResponseCommand → HttpDownloadCommand`), each command enqueueing its successor. `RequestGroup::createInitialCommand` / `createNextCommand` (`src/RequestGroup.h:232-242`) build these.

### The object graph (memorize this shape)

```
DownloadEngine
├── EventPoll                         (reactor)
├── RequestGroupMan                   (global scheduler / queue)
│   └── RequestGroup (1 per download)
│       ├── DownloadContext           (piece length, total length, FileEntries, piece hashes)
│       ├── PieceStorage              (BitfieldMan: which pieces are done/used)
│       ├── SegmentMan                (in-flight segment ownership per connection)
│       ├── BtProgressInfoFile        (.aria2 control file; Null* when resume disabled)
│       └── BtRuntime / PeerStorage   (BT only: swarm + peer set)
└── FileAllocationMan, CheckIntegrityMan, DNSCache, socket pool
```

`DownloadContext` (`src/DownloadContext.h:58`) is per-download immutable-ish metadata: `pieceLength_`, `fileEntries_` (one file, or many in a BT torrent), `pieceHashes_`, `basePath_` (the thing the `.aria2` name is derived from). `RequestGroup` (`src/RequestGroup.h:78`) is the mutable runtime: it holds `numConcurrentCommand_`, the `SegmentMan`, halt/pause flags, and `saveControlFile_`.

### One model, every protocol

The key design decision: **HTTP and BitTorrent share `PieceStorage`/`BitfieldMan`**. An HTTP download is a "torrent" of one file with `pieceLength` = `--piece-length` (default 1M, `src/OptionHandlerFactory.cc:905`); BitTorrent sets `pieceLength` from the torrent. `SegmentMan` is the piece-to-connection allocator for HTTP/FTP; `Peer`+`BtRequestFactory` play the same role for BT.

---

## Crown Jewels

### 1. Segmented / multi-connection download model

**The abstraction:** a `Segment` is an interface (`src/Segment.h:49`) with two concrete forms — `PiecedSegment` (known-length; `piece_`, `pieceLength_`, `writtenLength_`, `src/PiecedSegment.h:44`) and `GrowSegment` (unknown-length, chunked; `src/GrowSegment.h`). A segment is a *view over a piece*: `getPosition() = index * pieceLength`, `getPositionToWrite() = position + writtenLength`. The piece grid is one `BitfieldMan` with `blockLength = pieceLength` and `numBlocks = ceil(totalLength/pieceLength)` (`src/BitfieldMan.cc:62-70`).

**Splitting** is *not* a function named `split`; it falls out of a **stream piece selector** plus a size gate. `SegmentMan::getSegment(cuid, minSplitSize)` (`src/SegmentMan.cc:203`) calls `pieceStorage_->getMissingPiece(minSplitSize, ignoreBitfield, len, cuid)`, which delegates to a `StreamPieceSelector` (`src/DefaultPieceStorage.cc:100`):

- Default `DefaultStreamPieceSelector` → `BitfieldMan::getSparseMissingUnusedIndex` (`src/BitfieldMan.cc:244`): pick the largest contiguous *free* run, and only return its start if it begins at piece 0 / after a completed piece, **or** the run is at least `minSplitSize` bytes (`:286-288`). That single check is the "don't open a connection for a 10 KB tail" rule.
- Alternatives: `InorderStreamPieceSelector` (`getInorderMissingUnusedIndex`, sequential) and `GeomStreamPieceSelector` (strided), chosen via `--stream-piece-selector` in `src/DefaultPieceStorage.cc:103-114`.

**The knobs** (all in `src/OptionHandlerFactory.cc`, defaults verified):

| Option | Default | Effect / source |
|---|---|---|
| `--split` (`-s`) | 5 | `numConcurrentCommand_`; commands capped to `min(numPieces, split)` (`src/RequestGroup.cc:138`) |
| `--max-connection-per-server` (`-x`) | 1 (1..16) | per-`FileEntry` cap; extra URIs held in `requestPool_` (`src/FileEntry.cc:146-151`) |
| `--min-split-size` | 20M | the min-run gate above (`src/AbstractCommand.cc:888`) |
| `--piece-length` | 1M | grid granularity for HTTP (`OptionHandlerFactory.cc:905`) |
| `--lowest-speed-limit` | 0 | abort a laggard connection (`src/DownloadCommand.cc:302`) |
| `--max-file-not-found` | 0 | 0 ⇒ abort on first 404 (`src/HttpSkipResponseCommand.cc:216`) |

**HTTP ranged request** (`src/HttpRequest.cc:87-215`): `Range: bytes=start-end` where `start = fileEntry->gtoloff(segment->getPositionToWrite())`. In non-pipelined mode the end offset is *extended to the next used segment* — `end = min(fileLength, pieceLength * getNextUsedIndex(index))` (`src/HttpRequestCommand.cc:174`, `src/DefaultPieceStorage.cc:834`) — so one connection streams a contiguous run up to a neighbour's boundary instead of stopping at every piece edge.

**Ownership & stealing** (`src/SegmentMan.h:76`): `usedSegmentEntries_` is a deque of `{cuid, segment}`. On cancel, `cancelSegmentInternal` flushes the write cache, clears the piece, and **memoizes `writtenLength`** in `segmentWrittenLengthMemo_` so a later re-checkout resumes mid-piece (`src/SegmentMan.cc:285-309`). `getCleanSegmentIfOwnerIsIdle` can reclaim a zero-written segment whose owner is idle (`src/SegmentMan.cc:254`); `updateFastestPeerStat` prefers the fastest host for re-splits (`src/SegmentMan.cc:439`).

**Concurrent writes into one file** (`src/AbstractDiskWriter.cc:257-291`): the file is opened once; each connection `seek(offset)`s to its own absolute position and runs an EINTR-safe `write` loop — segments are independent by construction. After a segment completes, `PiecedSegment::updateWrittenLength` advances `writtenLength` and flips the piece sub-blocks; for BT, `DownloadCommand.cc:244-276` computes the piece digest (through the write cache) and validates the piece hash before `completePiece`.

### 2. Checkpoint / resume — the `.aria2` control file

**Location & format** (`src/DefaultBtProgressInfoFile.cc:111`, verified by reading the source): the file is **binary, big-endian**, name = `DownloadContext::getBasePath() + ".aria2"` (`:70-85`). Layout:

```
2B   version          0x00 0x01                       (v0 was ASCII; v1 binary — :118-121)
4B   extension        0x00000001 if BitTorrent else 0 (:125-130)
4B   infoHashLength   20 for BT else 0                (:136-146)
20B  infoHash         BT only                         (INFO_HASH_LENGTH, src/BtConstants.h:43)
4B   pieceLength                                      (:149)
8B   totalLength                                      (:152)
8B   uploadLength     BT only else 0                  (:155-162)
4B   bitfieldLength                                   (:164)
nB   bitfield         RAW, 1 bit/piece, MSB-first     (:167; src/BitfieldMan.cc:566)
4B   numInFlightPiece                                 (:171)
     per in-flight piece: {4B index, 4B length, 4B bfLen, bf bytes}  (:176-187)
```

There is **no RLE/compression** on the bitfield in 1.37.0, and **no ASCII `A2VER` tag** — the header is the two bytes `00 01`. `load()` hex-encodes those bytes and accepts only `0000`/`0001`, else throws "Unsupported ctrl file version" (`:243-254`).

**Resume reconstruction** (`src/DefaultBtProgressInfoFile.cc:331-401`): read raw bitfield → `pieceStorage_->setBitfield(saved, len)` → `BitfieldMan::setBitfield` copies it and `updateCache()` recomputes `cachedCompletedLength_`/`cachedNumMissingBlock_` (`src/BitfieldMan.cc:663`). If saved `pieceLength != current`, a `util::convertBitfield` remaps blocks (accepted only with `--allow-piece-length-change`, `src/util.cc:1956`). Partially-downloaded in-flight pieces are re-added via `pieceStorage_->addInFlightPiece`.

**The write is crash-safe and idempotent** (`src/DefaultBtProgressInfoFile.cc:199-223`): `save()` SHA1s the stream and **skips the write if unchanged** (`lastDigest_`), then writes `<name>__temp` and `File::renameTo`s it — atomic replace, no torn file.

**Autosave & save-on-exit**: `AutoSaveCommand` (`src/AutoSaveCommand.cc:57`, a `TimeBasedCommand`) fires every `--auto-save-interval` (default 60s, only installed when >0, `src/DownloadEngineFactory.cc:172`) and calls `RequestGroupMan::save()` (`src/RequestGroupMan.cc:595`), which saves each group's control file **unless** it finished and needs no verification — then it *removes* it (`removeControlFile`). `DownloadEngine::onEndOfRun` also calls `rgman->save()` on shutdown (`src/DownloadEngine.cc:241`).

**Garbage collection / validation** (`load()` throws `DL_ABORT_EX` on any inconsistency, so a corrupt control file aborts rather than silently mangling): infoHash mismatch (`:268`), `pieceLength == 0` (`:294`), `totalLength` mismatch (`:303`), bitfield length mismatch (`:324`), piece index out of range (`:349`). `--check-integrity` re-hashes on-disk data before trusting the bitfield (`src/RequestGroup.cc:221-269`).

**Session file ≠ control file** (`src/SessionSerializer.cc`): `--save-session` writes a *URI list + options* (leading-space `pref=value` lines, `gid=<hex>`, Tab-separated URIs, optional gzip when the name ends `.gz`; atomic temp+rename, `:72-95`). It carries **no bitfield** — progress lives only in each download's `.aria2`. The session format is exactly the `--input-file` format aria2 reads back.

### 3. RPC interface surface

**Three wires, one method registry.** `src/HttpServer.cc:373-395` routes by path/method: `POST /jsonrpc` → JSON-RPC, `GET /jsonrpc` → JSON-RPC-over-GET (JSONP), `POST /rpc` → XML-RPC, and `ws(s)://…/jsonrpc` → JSON-RPC-over-WebSocket. Dispatch lives in `src/HttpServerBodyCommand.cc` (JSON `:254-311`, XML `:223-252`); `rpc::processJsonRpcRequest` is at `src/rpc_helper.cc:80`. `src/RpcMethodFactory.cc:53-94` is the authoritative registry — **33 aria2 methods + 3 system methods**:

```
aria2.addUri          aria2.getOption          aria2.getGlobalOption
aria2.addTorrent*     aria2.changeUri          aria2.changeGlobalOption
aria2.addMetalink*    aria2.changeOption       aria2.purgeDownloadResult
aria2.getPeers*       aria2.getUris            aria2.removeDownloadResult
aria2.remove          aria2.getFiles           aria2.getVersion
aria2.pause           aria2.getServers         aria2.getSessionInfo
aria2.forcePause      aria2.tellActive         aria2.shutdown
aria2.pauseAll        aria2.tellWaiting        aria2.forceShutdown
aria2.forcePauseAll   aria2.tellStopped        aria2.getGlobalStat
aria2.unpause         aria2.tellStatus         aria2.saveSession
aria2.unpauseAll      aria2.changePosition     system.multicall
aria2.forceRemove                              system.listMethods
                                               system.listNotifications
```
(`*` = compile-gated on `ENABLE_BITTORRENT`/`ENABLE_METALINK`.) Each handler is a class in `src/RpcMethodImpl.h` exposing `static const char* getMethodName()` (line refs: `addUri` `:136`, `tellStatus` `:295`, `saveSession` `:516`).

**Envelope & errors** (`src/RpcRequest.h:48`, `src/RpcResponse.cc:154`): request = `{jsonrpc:"2.0", id, method, params[]}` (named params *not* supported, `src/rpc_helper.cc:99`); response = `{id, jsonrpc:"2.0", (result | error)}`. Error codes verified in source: `-32700` parse (`HttpServerBodyCommand.cc:279`), `-32600` invalid request (`:306`), `-32602` invalid params (`rpc_helper.cc:101`), and code `1` = generic method-error; XML-RPC faults use `faultCode=1`+`faultString` (`src/RpcMethod.cc:65-102`). (`-32601`/`-32603` are **not** in aria2 despite the JSON-RPC spec.)

**Auth** — two layers:
- Token (`--rpc-secret`): `RpcMethod::authorize` (`src/RpcMethod.cc:70`) strips a leading literal `"token:"` from `params[0]`, then `DownloadEngine::validateToken` (`src/DownloadEngine.cc:615`) does a **constant-time HMAC compare**. `system.listMethods`/`listNotifications` intentionally bypass the token (`src/RpcMethodImpl.cc:1491`); `system.multicall` checks per nested call.
- HTTP Basic (deprecated `--rpc-user`/`--rpc-passwd`): `src/HttpServer.cc:321-366`, on failure returns `401` with `WWW-Authenticate: Basic realm=aria2`.

**Notifications** (WebSocket only, `src/WebSocketSessionMan.cc:86-125`): `aria2.onDownloadStart`, `onDownloadPause`, `onDownloadStop`, `onDownloadComplete`, `onDownloadError`, `onBtDownloadComplete`. Fired as `{jsonrpc:"2.0", method, params:[{gid}]}` with **no `id`** (a true JSON-RPC notification), driven by `Notifier::onEvent` → `RequestGroupMan::notifyDownloadEvent` (`src/RequestGroupMan.cc:582`).

**Options (`src/prefs.cc`, `src/OptionHandlerFactory.cc`):** `--enable-rpc`, `--rpc-listen-port` (6800), `--rpc-listen-all`, `--rpc-allow-origin-all` (CORS), `--rpc-max-request-size` (2M), `--rpc-secret`, `--rpc-secure` (TLS → `https`/`wss`), `--rpc-certificate`/`--rpc-private-key`.

### 4. Request scheduler / piece manager

**Global queue** — `RequestGroupMan` (`src/RequestGroupMan.h:69`): `requestGroups_` (active) + `reservedGroups_` (queue) + `downloadResults_` (finished, bounded by `maxDownloadResult_`). The scheduler step is `fillRequestGroupFromReserver` (`src/RequestGroupMan.cc:515`), itself a `Command` run every tick:

```cpp
if (maxConcurrentDownloads <= numActive_) return;    // --max-concurrent-downloads (default 5, -j)
int num = maxConcurrentDownloads - numActive_;
while (count < num && !reservedGroups_.empty()) {
  groupToAdd = reservedGroups_.pop_front();
  if (paused || !isDependencyResolved()) { pending.push_back(groupToAdd); continue; }
  configureRequestGroup(groupToAdd); ++numActive_;
  e->addCommand(createInitialCommand(groupToAdd, e));   // spawn the download's first Command
}
```

`--max-concurrent-downloads` defaults to 5 (`src/OptionHandlerFactory.cc:431`). `optimizeConcurrentDownloads()` (`src/RequestGroupMan.cc:1063`) can *dynamically* raise the cap from measured speed. Queue checks are event-driven: `requestQueueCheck()` sets a flag consumed by `FillRequestGroupCommand` (`src/FillRequestGroupCommand.cc:57`), so adding/finishing a download reacts immediately instead of polling. `isSameFileBeingDownloaded` (`:862`) prevents two groups racing the same path.

**Download results** — a finished/failed download becomes a `DownloadResult` (`src/DownloadResult.h:56`: `gid`, lengths, `bitfield`, `infoHash`, `dir`, `pieceLength`, `result`) via `addDownloadResult`, and surfaces through `tellStopped`/`getGlobalStat`. The outcome enum is `error_code::Value` (`src/error_code.h:44`): `0 FINISHED` … `9 NOT_ENOUGH_DISK_SPACE`, `10 PIECE_LENGTH_CHANGED`, `13 FILE_ALREADY_EXISTS`, `32 CHECKSUM_ERROR`. These ints are the RPC `status`/`errorCode` surface — a stable contract worth copying verbatim.

**BitTorrent piece/request pipeline** — the BT analogue of segments is the **block** (16 KB, `BLOCK_LENGTH`, `src/Piece.h`; `MAX_BLOCK_LENGTH` 64 KB, `src/BtConstants.h:49`). `DefaultBtRequestFactory::createRequestMessages(max, endGame)` (`src/DefaultBtRequestFactory.cc:166`) walks its target pieces and emits up to `max` `BtRequestMessage`s, each `begin = blockIndex * piece->getBlockLength()`; endgame mode (`:197`) floods *all* still-missing blocks (shuffled) — classic rarest-first → endgame hedge. Outstanding requests are tracked by `BtMessageDispatcher::isOutstandingRequest` (`:226`), with `DEFAULT_MAX_OUTSTANDING_REQUEST = 6` (`BtConstants.h:51`). Piece selection for BT is pluggable via `PieceSelector` (`src/PieceSelector.h:48`, `select(index, bitfield, nbits)`), e.g. `RarestPieceSelector` (`src/RarestPieceSelector.cc`) reading `PieceStatMan` counts.

**Peer policy** — `Peer` (`src/Peer.h:56`) carries choke/interest state (`amChoking`, `peerChoking`, `amInterested`, `optionalUnchoke`, `snubbing`), bitfield, and peer-allowed fast set. Choking is timer-driven (`ActivePeerConnectionCommand`, `PeerChokeCommand` → `BtLeecherStateChoke`/`BtSeederStateChoke`), `--bt-max-peers` default 55 (`OptionHandlerFactory.cc:1563`), `--bt-request-timeout` default 60s (`:1634`).

---

## Rust Re-implementation Targets

Target: a `DownloadSession` trait in `mch-download`, generic over a transport, with a `PieceGrid` shared by HTTP and BT.

| Concept | aria2 | `mch-download` |
|---|---|---|
| Event reactor | `DownloadEngine::run` (`DownloadEngine.cc:158`) | tokio runtime; `Command` → `Future`/`async fn step()` |
| IO readiness | `EventPoll` (`EventPoll.h:51`) | tokio `readable()/writable()` or mio; keep the poll-method abstraction |
| Global scheduler | `RequestGroupMan::fillRequestGroupFromReserver` (`RequestGroupMan.cc:515`) | `Scheduler::pump()` with `--max-concurrent` semaphore + event-driven `queue_check` |
| One download | `RequestGroup` (`RequestGroup.h:78`) | `DownloadSession` (owns `PieceGrid` + `SegmentAllocator` + `Checkpoint`) |
| Piece grid | `BitfieldMan` (`BitfieldMan.cc:62`) | `PieceGrid { blocks, block_len, bits: BitVec }` (MSB-first — for wire compat) |
| Segment | `Segment`/`PiecedSegment` (`Segment.h:49`) | `Range { start, len, written }` |
| Allocator | `SegmentMan` + `StreamPieceSelector` (`SegmentMan.h:76`) | `SegmentAllocator` with `min_split_size` gate + sparse/inorder/geom strategies |
| HTTP Range | `HttpRequest.cc:87` | `Range: bytes=` builder; carry the "extend to next used piece" rule |
| Checkpoint | `DefaultBtProgressInfoFile` (`DefaultBtProgressInfoFile.cc:111`) | `ControlFile` — same big-endian byte layout, temp+rename, digest-skip |
| Autosave | `AutoSaveCommand` (`AutoSaveCommand.cc:57`) | `tokio::time::interval` task; save-on-exit via `Drop`/shutdown hook |
| Session file | `SessionSerializer` (`SessionSerializer.cc`) | `session::write_uri_list` (gzip-aware), reused as the `--input-file` reader |
| Scheduler API | `RequestGroupMan` + `DownloadResult` (`DownloadResult.h:56`) | `ControlHandle` returning `DownloadResult` structs |
| RPC | `RpcMethodFactory` + `RpcMethodImpl` | `mch-rpc`: JSON-RPC 2.0 method table (36 names) + WS notifications |
| Auth | `RpcMethod::authorize` / `validateToken` (`RpcMethod.cc:70`) | `token:` strip + constant-time HMAC (`subtle::ConstantTimeEq`) |
| BT requests | `DefaultBtRequestFactory` (`DefaultBtRequestFactory.cc:166`) | `BlockPipeline` (16 KB blocks, endgame flood, outstanding-request set) |
| Error codes | `error_code::Value` (`error_code.h:44`) | `enum DownloadError` — reuse the same integer values as the wire contract |

---

## What I take, and what I leave

**Take:**
1. **One piece grid for all transports.** HTTP-as-a-one-file-torrent is why aria2's resume, RPC, and stats are protocol-agnostic. Build `PieceGrid` first; make every transport fill it.
2. **The `.aria2` byte layout verbatim** — big-endian, 2-byte version header, raw piece bitfield. Wire-compatible resume is a real feature; the format is simple enough to copy exactly.
3. **Atomic + idempotent checkpointing** — temp-file + rename, and SHA1-skip-unchanged so autosave is cheap.
4. **The `min_split_size` gate** in the stream selector — it is the whole reason aria2 doesn't open 5 connections for a 30 KB tail.
5. **Endgame request flood + outstanding-request set** (BT) and **extend-Range-to-next-used-piece** (HTTP) — two small rules that eliminate tail latency.
6. **The `error_code::Value` integer table** as the public status contract, and the **36-method RPC surface** as the control API shape.
7. **Event-driven queue check** (`requestQueueCheck` flag) instead of a polling scheduler loop.
8. **Constant-time token compare** and the `system.*` methods' deliberate auth bypass.

**Leave:**
1. The **hand-rolled `Command` coroutine**. Rust async/await replaces it; keep the *reactor* idea, drop the state machine.
2. **Select/poll/kqueue/port fallbacks** — target tokio/mio only; keep `EventPoll` as a trait seam, not five implementations.
3. **Metalink**, DHT, and the FTP/SFTP negotiation minutiae for v1 — land HTTP(S) + BitTorrent behind the same `DownloadSession`, then widen.
4. The **deprecated HTTP-Basic RPC auth** — token-only.
5. **Session-hash dedup on every save** (`lastSessionHash_`) is nice-to-have; the control-file digest-skip already covers the hot path.
6. The **`optimizeConcurrentDownloads` speed heuristic** (`RequestGroupMan.cc:1063`) — ship the static cap first; the adaptive cap is a later tuning pass.

The value is the **unification**, not the protocol code: one `PieceGrid`, one `Checkpoint`, one `Scheduler`, one RPC surface, and every transport is just "who fills block N". That is what `mch-download` is for.
