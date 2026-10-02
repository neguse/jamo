#!/usr/bin/env python3
"""jamo と本家 mozc_server を同じコーパスで比較する (精度・レイテンシ・メモリ)。

使い方:
  cargo build --release -p jamo-cli
  scripts/bench-engines.py [--mozc-server PATH] [--user NAME] [--repeat N] [--work DIR]

- jamo: `jamo-cli bench` (プロセス内変換) で Top-1 / Top-5 / レイテンシ / VmHWM を取る。
- mozc: mozc_server を起動し、本家クライアントと同じ IPC (abstract unix socket に
  commands.Input を生で送り、half-close して commands.Output を受ける) で
  かなを AS_IS 入力 → Space で変換 → preedit の文節と all_candidate_words を読む。
  各文の変換は REVERT で捨てる (学習させない)。メモリは /proc/<pid>/status の VmHWM。
  protobuf は必要なフィールドだけ手で符号化・復号するので依存ライブラリは不要。
- mozc_server は root では起動できない (abort する)。root で実行するときは
  --user で一般ユーザーを指定すると、そのユーザーで自分自身を再実行する
  (IPC は同じ uid からしか受け付けない)。
- --mozc-server を省くと work ディレクトリに Arch Linux の fcitx5-mozc パッケージを
  展開して使う (bench-memory.py と同じ)。要 `tar --zstd` または `zstd`。

Top-5 は accuracy_test.rs と同じ定義: 期待文を各文節の上位 5 候補から 1 つずつ
選んで組み立てられるか。mozc の候補は変換直後の all_candidate_words (注目文節) を
Right キーで文節を移しながら読む。
"""

import argparse
import json
import os
import shutil
import socket
import subprocess
import sys
import time

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MOZC_PKG_URL = "https://archlinux.org/packages/extra/x86_64/fcitx5-mozc/download/"

# ---- 最小 protobuf 符号化 (commands.proto の必要フィールドのみ) ----
# Input: type=1 (enum), id=2 (uint64), key=3 (KeyEvent), command=4 (SessionCommand)
# KeyEvent: special_key=3 (enum), key_string=5 (string), input_style=6 (enum)
# SessionCommand: type=1 (enum)
# Output: id=1, preedit=5 (Preedit), error_code=11, all_candidate_words=14 (CandidateList)
# Preedit: cursor=1, Segment group=2 { value=4 (string), key=6 (string) }
# CandidateList: focused_index=1, candidates=2 (CandidateWord { value=4 })
INPUT_CREATE_SESSION, INPUT_DELETE_SESSION, INPUT_SEND_KEY, INPUT_SEND_COMMAND = 1, 2, 3, 5
KEY_SPACE, KEY_RIGHT = 4, 7
INPUT_STYLE_AS_IS = 1
COMMAND_REVERT = 1


def varint(v):
    out = bytearray()
    while True:
        b = v & 0x7F
        v >>= 7
        if v:
            out.append(b | 0x80)
        else:
            out.append(b)
            return bytes(out)


def field_varint(num, v):
    return varint(num << 3) + varint(v)


def field_bytes(num, data):
    return varint((num << 3) | 2) + varint(len(data)) + data


def read_varint(buf, pos):
    shift = 0
    v = 0
    while True:
        b = buf[pos]
        pos += 1
        v |= (b & 0x7F) << shift
        if not b & 0x80:
            return v, pos
        shift += 7


def decode(buf):
    """フィールド番号 → [値] の辞書。長さ付きは bytes、group は入れ子の辞書。"""
    fields = {}
    pos = 0
    while pos < len(buf):
        tag, pos = read_varint(buf, pos)
        num, wt = tag >> 3, tag & 7
        if wt == 0:
            v, pos = read_varint(buf, pos)
        elif wt == 1:
            v, pos = buf[pos:pos + 8], pos + 8
        elif wt == 2:
            n, pos = read_varint(buf, pos)
            v, pos = buf[pos:pos + n], pos + n
        elif wt == 5:
            v, pos = buf[pos:pos + 4], pos + 4
        elif wt == 3:  # start group: 対応する end group まで読み飛ばして再帰復号
            depth = 1
            start = pos
            content_end = pos
            while depth:
                content_end = pos
                t, pos = read_varint(buf, pos)
                w = t & 7
                if w == 3:
                    depth += 1
                elif w == 4:
                    depth -= 1
                elif w == 0:
                    _, pos = read_varint(buf, pos)
                elif w == 1:
                    pos += 8
                elif w == 2:
                    n, pos = read_varint(buf, pos)
                    pos += n
                elif w == 5:
                    pos += 4
            v = decode(buf[start:content_end])
        else:
            raise ValueError(f"unsupported wire type {wt}")
        fields.setdefault(num, []).append(v)
    return fields


def first(fields, num, default=None):
    return fields.get(num, [default])[0]


class MozcClient:
    def __init__(self, config_dir):
        self.config_dir = config_dir
        self.addr = None
        self.pid = None

    def connect_info(self):
        data = open(os.path.join(self.config_dir, ".session.ipc"), "rb").read()
        f = decode(data)
        key = first(f, 1).decode()
        self.pid = first(f, 2)
        self.addr = ("\0tmp/.mozc." + key + ".session").encode()

    def call(self, payload):
        s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        s.connect(self.addr)
        s.sendall(payload)
        s.shutdown(socket.SHUT_WR)
        buf = bytearray()
        while True:
            d = s.recv(65536)
            if not d:
                break
            buf += d
        s.close()
        return decode(bytes(buf))

    def create_session(self):
        out = self.call(field_varint(1, INPUT_CREATE_SESSION))
        return first(out, 1)

    def delete_session(self, sid):
        self.call(field_varint(1, INPUT_DELETE_SESSION) + field_varint(2, sid))

    def send_key(self, sid, key_string=None, special_key=None):
        key = b""
        if special_key is not None:
            key += field_varint(3, special_key)
        if key_string is not None:
            key += field_bytes(5, key_string.encode()) + field_varint(6, INPUT_STYLE_AS_IS)
        return self.call(field_varint(1, INPUT_SEND_KEY) + field_varint(2, sid) + field_bytes(3, key))

    def revert(self, sid):
        cmd = field_varint(1, COMMAND_REVERT)
        return self.call(field_varint(1, INPUT_SEND_COMMAND) + field_varint(2, sid) + field_bytes(4, cmd))

    def vm_hwm_kb(self):
        with open(f"/proc/{self.pid}/status") as f:
            for line in f:
                if line.startswith("VmHWM:"):
                    return int(line.split()[1])
        return None


def preedit_segments(out):
    pre = first(out, 5)
    if pre is None:
        return []
    return [first(seg, 4, b"").decode() for seg in decode(pre).get(2, [])]


def all_candidates(out, top_n):
    lst = first(out, 14)
    if lst is None:
        return []
    return [first(decode(c), 4, b"").decode() for c in decode(lst).get(2, [])][:top_n]


def reachable(clause_candidates, expected):
    if not clause_candidates:
        return expected == ""
    head, rest = clause_candidates[0], clause_candidates[1:]
    return any(expected.startswith(c) and reachable(rest, expected[len(c):]) for c in head)


def load_corpus(path):
    cases = []
    with open(path, encoding="utf-8") as f:
        for line in f:
            line = line.rstrip("\n")
            if not line or line.startswith("#"):
                continue
            cat, inp, exp = line.split("\t")
            cases.append((cat, inp, exp))
    return cases


def bench_jamo(jamo_cli, dict_path, corpus, repeat):
    r = subprocess.run([jamo_cli, "bench", "--dict", dict_path, "--repeat", str(repeat), corpus],
                       capture_output=True, text=True, check=True)
    rows = {}
    summary = None
    for line in r.stdout.splitlines():
        if line.startswith("#summary "):
            summary = json.loads(line[len("#summary "):])
        else:
            inp, exp, actual, hit5, us = line.split("\t")
            rows[inp] = (actual, hit5 == "1", float(us))
    return summary, rows


def ensure_mozc_server(work):
    server = os.path.join(work, "mozc-pkg", "usr", "lib", "mozc", "mozc_server")
    if os.path.exists(server):
        return server
    pkg = os.path.join(work, "fcitx5-mozc.pkg.tar.zst")
    if not os.path.exists(pkg):
        subprocess.run(["curl", "-fsSL", "-o", pkg, MOZC_PKG_URL], check=True)
    pkg_dir = os.path.join(work, "mozc-pkg")
    os.makedirs(pkg_dir, exist_ok=True)
    if subprocess.run(["tar", "--zstd", "-xf", pkg, "-C", pkg_dir], capture_output=True).returncode != 0:
        tar_path = pkg[:-4]
        subprocess.run(["zstd", "-d", "-f", "-o", tar_path, pkg], check=True)
        subprocess.run(["tar", "-xf", tar_path, "-C", pkg_dir], check=True)
    return server


def bench_mozc(server, work, corpus_cases, repeat):
    home = os.path.join(work, "mozc-home")
    config_dir = os.path.join(home, ".config", "mozc")
    os.makedirs(config_dir, exist_ok=True)
    for name in (".session.ipc", ".server.lock"):
        try:
            os.remove(os.path.join(config_dir, name))
        except FileNotFoundError:
            pass
    env = dict(os.environ, HOME=home)
    t0 = time.monotonic()
    proc = subprocess.Popen([server], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    client = MozcClient(config_dir)
    deadline = time.monotonic() + 15
    while True:
        try:
            client.connect_info()
            sid = client.create_session()
            break
        except (FileNotFoundError, ConnectionRefusedError, IndexError, KeyError):
            if proc.poll() is not None:
                raise RuntimeError(f"mozc_server exited with {proc.returncode} (root では起動できない。--user を指定)")
            if time.monotonic() > deadline:
                proc.kill()
                raise RuntimeError("mozc_server did not come up")
            time.sleep(0.1)
    load_us = (time.monotonic() - t0) * 1e6
    idle_kb = client.vm_hwm_kb()
    size_bytes = os.path.getsize(server)

    rows = {}
    top1 = top5 = 0
    latencies = []
    try:
        for _cat, inp, exp in corpus_cases:
            # ウォームアップ 1 回 + repeat 回の変換 (かな入力 + Space) を計測
            def convert():
                client.send_key(sid, key_string=inp)
                return client.send_key(sid, special_key=KEY_SPACE)
            out = convert()
            client.revert(sid)
            start = time.perf_counter()
            for _ in range(repeat):
                out = convert()
                client.revert(sid)
            us = (time.perf_counter() - start) * 1e6 / repeat
            latencies.append(us)
            # 候補取得 (計測外): 変換直後は先頭文節が注目、Right で次の文節へ
            out = convert()
            segments = preedit_segments(out)
            cands = []
            for i in range(len(segments)):
                if i > 0:
                    out = client.send_key(sid, special_key=KEY_RIGHT)
                top = all_candidates(out, 5)
                if segments[i] not in top:
                    top = [segments[i]] + top[:4]
                cands.append(top)
            client.revert(sid)
            actual = "".join(segments)
            hit1 = actual == exp
            hit5 = reachable(cands, exp)
            top1 += hit1
            top5 += hit5
            rows[inp] = (actual, hit5, us)
        hwm_kb = client.vm_hwm_kb()
    finally:
        try:
            client.delete_session(sid)
        except OSError:
            pass
        proc.terminate()
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()
    lat = sorted(latencies)
    summary = {
        "total": len(corpus_cases), "top1": top1, "top5": top5,
        "mean_us": sum(latencies) / len(latencies), "p50_us": lat[len(lat) // 2],
        "p95_us": lat[min(len(lat) - 1, round((len(lat) - 1) * 0.95))],
        "load_us": load_us, "dict_bytes": size_bytes, "idle_kb": idle_kb, "vm_hwm_kb": hwm_kb,
    }
    return summary, rows


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--corpus", default=os.path.join(REPO, "jamo-core", "tests", "corpus.tsv"))
    parser.add_argument("--jamo-cli", default=os.path.join(REPO, "target", "release", "jamo-cli"))
    parser.add_argument("--dict", default=os.path.join(REPO, "data", "mozc.dict"))
    parser.add_argument("--mozc-server", default=None)
    parser.add_argument("--user", default=None, help="root のとき mozc を動かす一般ユーザー")
    parser.add_argument("--repeat", type=int, default=20)
    parser.add_argument("--work", default=os.path.join(REPO, "target", "bench-engines"))
    parser.add_argument("--skip-mozc", action="store_true")
    parser.add_argument("--drop-caches", action="store_true",
                        help="計測前にページキャッシュを捨てる (root)。mmap 辞書を 4KB ページで"
                             "フォルトさせ、large folio による RSS の水増しを避ける")
    parser.add_argument("--json", default=None, help="結果 JSON の出力先")
    parser.add_argument("--note", default="", help=argparse.SUPPRESS)  # 再実行時に計測条件を引き継ぐ
    args = parser.parse_args()

    if args.drop_caches:
        if os.geteuid() != 0:
            sys.exit("--drop-caches needs root")
        # ページキャッシュに large folio (2MB) で載っているファイルを mmap すると 1 バイト
        # 触れただけで 2MB が RSS に数えられる。捨ててから計測すると実際に触れた 4KB
        # ページだけが載る (cold start の条件。両エンジンに同じ条件で適用される)
        os.sync()
        with open("/proc/sys/vm/drop_caches", "w") as f:
            f.write("3\n")

    if os.geteuid() == 0 and args.user and not args.skip_mozc:
        # mozc_server は root を拒否し、IPC は同じ uid のみ受け付けるので、全体を再実行する
        home = subprocess.run(["sh", "-c", f"echo ~{args.user}"], capture_output=True, text=True).stdout.strip()
        work = os.path.join(home, "bench-engines")
        os.makedirs(args.work, exist_ok=True)
        if args.mozc_server is None:
            args.mozc_server = ensure_mozc_server(args.work)
        subprocess.run(["chmod", "-R", "a+rX", os.path.dirname(os.path.dirname(os.path.dirname(args.mozc_server)))], check=True)
        cmd = ["runuser", "-u", args.user, "--", "env", f"HOME={home}", sys.executable, os.path.abspath(__file__),
               "--corpus", args.corpus, "--jamo-cli", args.jamo_cli, "--dict", args.dict,
               "--mozc-server", args.mozc_server, "--repeat", str(args.repeat), "--work", work,
               "--json", os.path.join(work, "result.json"),
               "--note", " (ページキャッシュを捨てた cold start)" if args.drop_caches else ""]
        rc = subprocess.run(cmd).returncode
        if rc == 0 and args.json:
            shutil.copyfile(os.path.join(work, "result.json"), args.json)
        sys.exit(rc)

    os.makedirs(args.work, exist_ok=True)
    cases = load_corpus(args.corpus)
    results = {}
    print(f"corpus: {len(cases)} sentences, repeat {args.repeat}", file=sys.stderr)
    results["jamo"] = bench_jamo(args.jamo_cli, args.dict, args.corpus, args.repeat)
    if not args.skip_mozc:
        server = args.mozc_server or ensure_mozc_server(args.work)
        results["mozc"] = bench_mozc(server, args.work, cases, args.repeat)

    uname = subprocess.run(["uname", "-sm"], capture_output=True, text=True).stdout.strip()
    rev = subprocess.run(["git", "-C", REPO, "rev-parse", "--short", "HEAD"], capture_output=True, text=True).stdout.strip()
    print(f"<!-- {time.strftime('%Y-%m-%d')} {uname} jamo@{rev} corpus {len(cases)} 文, repeat {args.repeat} -->")
    print("| 指標 | jamo (mozc 辞書) | mozc_server (本家) |")
    print("|------|------------------|--------------------|")

    def cell(name, fmt):
        return " | ".join(fmt(results[e][0]) if e in results else "-" for e in ("jamo", "mozc"))

    n = len(cases)
    def rate(key):
        return lambda s: "%d/%d (%.1f%%)" % (s[key], n, s[key] / n * 100)

    print("| Top-1 | %s |" % cell("top1", rate("top1")))
    print("| Top-5 (候補選択で到達可能) | %s |" % cell("top5", rate("top5")))
    print("| 変換レイテンシ 平均 | %s |" % cell("mean", lambda s: "%.0f us" % s["mean_us"]))
    print("| 変換レイテンシ p50 / p95 | %s |" % cell("p", lambda s: "%.0f / %.0f us" % (s["p50_us"], s["p95_us"])))
    print("| 起動 (辞書ロード) | %s |" % cell("load", lambda s: "%.1f ms" % (s["load_us"] / 1000)))
    print("| 辞書 / バイナリ | %s |" % cell("size", lambda s: "%.1f MB" % (s["dict_bytes"] / 1e6)))
    print("| peak RSS (VmHWM) | %s |" % cell("hwm", lambda s: "%.1f MB" % ((s["vm_hwm_kb"] or 0) / 1024)))
    if "mozc" in results:
        print("| 起動直後 RSS (mozc) | - | %.1f MB |" % (results["mozc"][0]["idle_kb"] / 1024))
    print()
    print("計測条件: jamo はプロセス内変換 (`jamo-cli bench`)、mozc は IPC 往復込み "
          "(かな入力 + Space の 2 呼び出し)。メモリは各プロセスの VmHWM"
          + (args.note or (" (ページキャッシュを捨てた cold start)" if args.drop_caches else "")) + "。")

    if "mozc" in results:
        print()
        print("| 入力 | 期待 | jamo | mozc |")
        print("|------|------|------|------|")
        for _cat, inp, exp in cases:
            j = results["jamo"][1].get(inp, ("-", False, 0))
            m = results["mozc"][1].get(inp, ("-", False, 0))
            if j[0] == exp and m[0] == exp:
                continue
            def mark(a):
                return a if a == exp else f"**{a}**"

            print(f"| {inp} | {exp} | {mark(j[0])} | {mark(m[0])} |")

    if args.json:
        with open(args.json, "w") as f:
            json.dump({k: {"summary": v[0], "rows": v[1]} for k, v in results.items()}, f, ensure_ascii=False, indent=1)


if __name__ == "__main__":
    main()
