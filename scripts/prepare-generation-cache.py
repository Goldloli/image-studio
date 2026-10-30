#!/usr/bin/env python3
"""Explicit fixed-source acceptance download; never activates a model installation.

Two files at a time, eight bounded official-HTTPS Range transfers per file. Chunk
receipts are written only after fsync; completed files require full size/SHA-256.
--background detaches this fixed helper and writes a durable PID/log receipt.
"""
import argparse
import concurrent.futures
from datetime import datetime, timezone
import fcntl
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import re
import stat
import threading
import time
import urllib.request
from urllib.parse import urlparse

ROOT = Path(__file__).resolve().parent.parent
DATA = ROOT / 'target/first-generation-validation'
CACHE = DATA / 'cache/downloads'
RECEIPT = DATA / 'model-download-receipt.json'
LOG = DATA / 'model-download-ranges.log'
CHUNK = 32 * 1024 * 1024
ALLOWED = {'huggingface.co', 'us.aws.cdn.hf.co', 'cdn-lfs.huggingface.co', 'cdn-lfs.hf.co',
           'cdn-lfs-us-1.hf.co', 'cas-bridge.xethub.hf.co',
           'cas-server.xethub.hf.co', 'transfer.xethub.hf.co'}


def atomic_json(path, value):
    temporary = path.with_suffix('.json.tmp')
    descriptor = os.open(temporary, os.O_CREAT | os.O_WRONLY | os.O_TRUNC | os.O_NOFOLLOW | os.O_NONBLOCK, 0o600)
    if not stat.S_ISREG(os.fstat(descriptor).st_mode):
        os.close(descriptor)
        raise ValueError('receipt target is not a regular file')
    with os.fdopen(descriptor, 'w') as stream:
        json.dump(value, stream, indent=2)
        stream.flush()
        os.fsync(stream.fileno())
    temporary.replace(path)


def verified(path, item):
    if not path.is_file() or path.is_symlink() or path.stat().st_size != item['bytes']:
        return False
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest() == item['sha256']


class OfficialRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        parsed = urlparse(newurl)
        if parsed.scheme != 'https' or parsed.hostname not in ALLOWED or parsed.username or parsed.password:
            raise ValueError('redirect outside official HTTPS sources')
        return super().redirect_request(req, fp, code, msg, headers, newurl)


def open_range(item, start, end, etag=None):
    headers = {'Range': f'bytes={start}-{end}', 'User-Agent': 'ImageStudio/0.1-fixed-cache-preparation'}
    if etag:
        headers['If-Range'] = etag
    opener = urllib.request.build_opener(OfficialRedirect())
    response = opener.open(urllib.request.Request(item['url'], headers=headers), timeout=30)
    expected = f"bytes {start}-{end}/{item['bytes']}"
    if response.status != 206 or response.headers.get('Content-Range') != expected:
        response.close()
        raise ValueError('server did not return the exact requested byte range')
    received = response.headers.get('ETag', '')
    if not received or received.startswith('W/') or (etag and received != etag):
        response.close()
        raise ValueError('strong remote file identity changed or is missing')
    return response, received


def download(item):
    target = CACHE / item['sha256']
    if verified(target, item):
        return
    begin = time.monotonic()
    print('Downloading', item['path'], item['bytes'], flush=True)
    for attempt in range(12):
        try:
            with open_range(item, 0, 0)[0] as response:
                etag = response.headers['ETag']
                if len(response.read(2)) != 1:
                    raise ValueError('initial range size mismatch')
            break
        except Exception as error:
            print('Retry identity', item['path'], attempt + 1, type(error).__name__, flush=True)
            if attempt == 11:
                raise
            time.sleep(min(10, 0.5 * 2 ** attempt))
    part = CACHE / (item['sha256'] + '.ranges.part')
    metadata = CACHE / (item['sha256'] + '.ranges.json')
    identity = dict(sha256=item['sha256'], total=item['bytes'], etag=etag, chunk_bytes=CHUNK)
    try:
        saved = json.loads(metadata.read_text())
        if any(saved.get(k) != v for k, v in identity.items()) or part.stat().st_size != item['bytes']:
            saved = {}
    except (OSError, ValueError):
        saved = {}
    completed = set(saved.get('completed', []))
    chunks = (item['bytes'] + CHUNK - 1) // CHUNK
    if any(type(index) is not int or not 0 <= index < chunks for index in completed):
        completed = set()
    descriptor = os.open(part, os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW | os.O_NONBLOCK, 0o600)
    if not stat.S_ISREG(os.fstat(descriptor).st_mode):
        os.close(descriptor)
        raise ValueError('download target is not a regular file')
    os.ftruncate(descriptor, item['bytes'])
    progress_lock = threading.Lock()

    def fetch(index):
        start = index * CHUNK
        end = min(item['bytes'], start + CHUNK) - 1
        for attempt in range(12):
            try:
                response, _ = open_range(item, start, end, etag)
                with response:
                    offset = start
                    while True:
                        data = response.read(min(256 * 1024, end + 1 - offset) or 1)
                        if not data:
                            break
                        if offset + len(data) > end + 1:
                            raise ValueError('range response exceeded its boundary')
                        consumed = 0
                        while consumed < len(data):
                            consumed += os.pwrite(descriptor, data[consumed:], offset + consumed)
                        offset += len(data)
                    if offset != end + 1:
                        raise ValueError('range response ended early')
                with progress_lock:
                    os.fsync(descriptor)
                    completed.add(index)
                    atomic_json(metadata, {**identity, 'completed': sorted(completed)})
                    if len(completed) % 16 == 0 or len(completed) == chunks:
                        print('Received ranges', item['path'], f'{len(completed)}/{chunks}', flush=True)
                return
            except Exception as error:
                print('Retry range', item['path'], index, attempt + 1, type(error).__name__, flush=True)
                if attempt == 11:
                    raise RuntimeError('repeated official range transfer failure: ' + item['path']) from error
                time.sleep(min(10, 0.5 * 2 ** attempt))
    try:
        pending = [index for index in range(chunks) if index not in completed]
        with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
            list(pool.map(fetch, pending))
    finally:
        os.close(descriptor)
    if not verified(part, item):
        metadata.unlink(missing_ok=True)
        raise RuntimeError('full-file SHA-256 mismatch; incomplete chunks have not been activated')
    part.replace(target)
    metadata.unlink(missing_ok=True)
    print('Verified', item['path'], 'seconds', round(time.monotonic() - begin, 2), flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--background', action='store_true')
    args = parser.parse_args()
    CACHE.mkdir(parents=True, exist_ok=True)
    (DATA / 'data').mkdir(parents=True, exist_ok=True)
    manifest_path = ROOT / 'resources/local-generation/model-manifest.json'
    manifest = json.loads(manifest_path.read_text())
    assert manifest['repository'] == 'Tongyi-MAI/Z-Image-Turbo'
    assert manifest['revision'] == 'f332072aa78be7aecdf3ee76d5c247082da564a6'
    for item in manifest['files']:
        assert re.fullmatch('[0-9a-f]{64}', item['sha256']) and item['bytes'] > 0
        assert item['url'] == f"https://huggingface.co/{manifest['repository']}/resolve/{manifest['revision']}/{item['path']}"
    lock = (DATA / 'data/installation.lock').open('a')
    try:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        raise SystemExit('The fixed validation data root is already being prepared or installed.')
    if args.background:
        fcntl.flock(lock, fcntl.LOCK_UN)
        with LOG.open('a') as log:
            child = subprocess.Popen([sys.executable, str(Path(__file__).resolve())], cwd=ROOT,
                                     stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT,
                                     start_new_session=True, close_fds=True)
        receipt = dict(pid=child.pid, script=str(Path(__file__).resolve()), data_root=str(DATA),
                       log=str(LOG), manifest_sha256=hashlib.sha256(manifest_path.read_bytes()).hexdigest(),
                       state='running', started_utc=datetime.now(timezone.utc).isoformat(),
                       model_bytes=manifest['component_bytes'], full_model_verified=False,
                       progress_receipts_glob=str(CACHE / '*.ranges.json'), file_parallelism=2,
                       ranges_per_file=8, range_bytes=CHUNK,
                       script_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest())
        atomic_json(RECEIPT, receipt)
        print(json.dumps(receipt, indent=2))
        return
    receipt = json.loads(RECEIPT.read_text()) if RECEIPT.exists() else {}
    receipt.update(pid=os.getpid(), state='running', full_model_verified=False)
    try:
        files = sorted(manifest['files'], key=lambda item: (item['bytes'] > 200_000_000, item['bytes']))
        with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
            list(pool.map(download, files))
        receipt.update(state='complete', full_model_verified=True, completed_utc=datetime.now(timezone.utc).isoformat())
        print('All pinned model files verified. No installation has been activated.', flush=True)
    except Exception as error:
        receipt.update(state='failed', error_type=type(error).__name__, full_model_verified=False)
        raise
    finally:
        atomic_json(RECEIPT, receipt)


if __name__ == '__main__':
    main()
