#!/usr/bin/env python3
"""かな漢字変換エンジンの peak RSS を比較計測する。

使い方:
  scripts/bench-memory.py [--with-anthy] [--with-mozc-server] [--work DIR]

- jamo: target/release/jamo-cli で変換1回 (辞書ロード込み) を実行し、
  子プロセスの peak RSS (getrusage RUSAGE_CHILDREN) を計測する。
  data/mozc.dict, data/ipadic.dict のうち存在するものが対象。
- --with-anthy: anthy-unicode をソース取得・ビルドし、付属テスト
  (約550件の変換) 実行時の peak RSS を計測する。要 cc/make。
- --with-mozc-server: Arch Linux の fcitx5-mozc パッケージを work 内に
  展開し、mozc_server 起動直後の RSS を計測する。クライアントなしでは
  変換を実行できないため下限値 (mozc は辞書をバイナリ埋め込み +
  mmap/lazy load するため、変換後もバイナリサイズ + 作業領域が上限の目安)。
- 外部エンジンの取得・ビルドは work ディレクトリ内で完結し、
  システムには変更を加えない。取得物は work にキャッシュされ再利用される。

各計測は使い捨ての子 Python プロセスで行う (RUSAGE_CHILDREN は
プロセス生存中の累積 max のため、同一プロセスでの連続計測は汚染される)。
"""

import argparse
import json
import os
import signal
import subprocess
import sys
import time

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
JAMO_CLI = os.path.join(REPO, "target", "release", "jamo-cli")
CONVERT_TEXT = "きょうはいいてんきですね"

ANTHY_VER = "1.0.0.20260213"
ANTHY_URL = (
    "https://github.com/fujiwarat/anthy-unicode/releases/download/"
    f"{ANTHY_VER}/anthy-unicode-{ANTHY_VER}.tar.gz"
)
MOZC_PKG_URL = "https://archlinux.org/packages/extra/x86_64/fcitx5-mozc/download/"


def measure_run(cmd, cwd=None):
    """コマンドを1回実行し、子プロセス peak RSS を MB で返す。"""
    helper = (
        "import subprocess,resource,sys,json\n"
        "r=subprocess.run(sys.argv[1:],capture_output=True)\n"
        "peak=resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss\n"
        "print(json.dumps({'rc':r.returncode,'peak_kb':peak}))\n"
    )
    out = subprocess.run(
        [sys.executable, "-c", helper, *cmd],
        cwd=cwd,
        capture_output=True,
        text=True,
        check=True,
    )
    result = json.loads(out.stdout.strip().splitlines()[-1])
    if result["rc"] != 0:
        raise RuntimeError(f"measured command failed: {cmd}")
    return result["peak_kb"] / 1024


def measure_daemon(cmd, cwd=None, env=None, settle=6.0):
    """デーモンを起動し、settle 秒間ポーリングして観測できた最大 VmHWM を
    MB で返して停止する。

    起動プロセスが fork してデーモン化する場合 (mozc_server 等) は、
    実行ファイルパスで実プロセスを探す。一度も観測できなければ
    RuntimeError (呼び出し側で skip する)。
    """
    proc = subprocess.Popen(
        cmd, cwd=cwd, env=env,
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    best_kb = 0
    pids = set()
    deadline = time.monotonic() + settle
    try:
        while time.monotonic() < deadline:
            time.sleep(0.2)
            for entry in os.listdir("/proc"):
                if not entry.isdigit():
                    continue
                try:
                    with open(f"/proc/{entry}/cmdline", "rb") as f:
                        argv0 = f.read().split(b"\0")[0].decode(errors="replace")
                    if argv0 != cmd[0]:
                        continue
                    pids.add(int(entry))
                    with open(f"/proc/{entry}/status") as f:
                        for line in f:
                            if line.startswith("VmHWM:"):
                                best_kb = max(best_kb, int(line.split()[1]))
                except OSError:
                    continue
    finally:
        for pid in pids:
            try:
                os.kill(pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
        if proc.poll() is None:
            proc.terminate()
        proc.wait(timeout=10)
    if not best_kb:
        raise RuntimeError(f"daemon {cmd[0]} was never observed in /proc")
    return best_kb / 1024


def fetch(url, dest):
    if not os.path.exists(dest):
        subprocess.run(["curl", "-fsSL", "-o", dest, url], check=True)


def bench_jamo(results):
    if not os.path.exists(JAMO_CLI):
        print("skip jamo: build first with `cargo build --release -p jamo-cli`",
              file=sys.stderr)
        return
    for name in ("mozc", "ipadic"):
        dict_path = os.path.join(REPO, "data", f"{name}.dict")
        if not os.path.exists(dict_path):
            print(f"skip jamo+{name}: {dict_path} not found", file=sys.stderr)
            continue
        size_mb = os.path.getsize(dict_path) / 1e6
        peak = measure_run([JAMO_CLI, "convert", "--dict", dict_path, CONVERT_TEXT])
        results.append((f"jamo + {name}.dict", f"{size_mb:.1f} MB (.dict)",
                        peak, "変換1回込み"))


def bench_anthy(work, results):
    src = os.path.join(work, f"anthy-unicode-{ANTHY_VER}")
    test_bin = os.path.join(src, "test", "anthy")
    if not os.path.exists(test_bin):
        tarball = os.path.join(work, "anthy.tar.gz")
        fetch(ANTHY_URL, tarball)
        subprocess.run(["tar", "xzf", tarball, "-C", work], check=True)
        subprocess.run(["./configure", f"--prefix={work}/anthy-prefix"],
                       cwd=src, check=True, capture_output=True)
        subprocess.run(["make", f"-j{os.cpu_count()}"],
                       cwd=src, check=True, capture_output=True)
    dic = os.path.join(src, "mkanthydic", "anthy.dic")
    dic_mb = os.path.getsize(dic) / 1e6 if os.path.exists(dic) else 0
    peak = measure_run(["./anthy", "--quiet"], cwd=os.path.join(src, "test"))
    results.append((f"Anthy (anthy-unicode {ANTHY_VER})",
                    f"{dic_mb:.1f} MB (anthy.dic)" if dic_mb else "-",
                    peak, "付属テスト約550件の変換込み"))


def bench_mozc_server(work, results):
    server = os.path.join(work, "mozc-pkg", "usr", "lib", "mozc", "mozc_server")
    if not os.path.exists(server):
        pkg = os.path.join(work, "fcitx5-mozc.pkg.tar.zst")
        fetch(MOZC_PKG_URL, pkg)
        pkg_dir = os.path.join(work, "mozc-pkg")
        os.makedirs(pkg_dir, exist_ok=True)
        subprocess.run(["tar", "--zstd", "-xf", pkg, "-C", pkg_dir], check=True)
    size_mb = os.path.getsize(server) / 1e6
    home = os.path.join(work, "mozc-home")
    os.makedirs(home, exist_ok=True)
    env = dict(os.environ, HOME=home)
    try:
        peak = measure_daemon([server], env=env)
    except RuntimeError as e:
        print(f"skip mozc_server: {e}", file=sys.stderr)
        return
    results.append(("mozc_server (本家)", f"{size_mb:.1f} MB (バイナリ埋め込み)",
                    peak, "起動直後・変換なし (下限値)"))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--with-anthy", action="store_true")
    parser.add_argument("--with-mozc-server", action="store_true")
    parser.add_argument("--work", default=os.path.join(REPO, "target", "bench-memory"))
    args = parser.parse_args()

    os.makedirs(args.work, exist_ok=True)
    results = []
    bench_jamo(results)
    if args.with_anthy:
        bench_anthy(args.work, results)
    if args.with_mozc_server:
        bench_mozc_server(args.work, results)

    uname = subprocess.run(["uname", "-sm"], capture_output=True, text=True).stdout.strip()
    rev = subprocess.run(["git", "-C", REPO, "rev-parse", "--short", "HEAD"],
                         capture_output=True, text=True).stdout.strip()
    print(f"<!-- {time.strftime('%Y-%m-%d')} {uname} jamo@{rev} -->")
    print("| エンジン | 辞書データ | peak RSS | 計測条件 |")
    print("|---------|-----------|----------|---------|")
    for name, dic, peak, cond in results:
        print(f"| {name} | {dic} | **{peak:.1f} MB** | {cond} |")


if __name__ == "__main__":
    main()
