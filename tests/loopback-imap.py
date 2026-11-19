"""Offline IMAP login/switch/watcher smoke test. Run against a non-demo binary.

Only the temporary TLS server on 127.0.0.1 is used. The generated CA is trusted
by the child processes only. No desktop config, service, keyring or Gmail access.
"""
import json
from collections import Counter
import os
from pathlib import Path
import shlex
import shutil
import socketserver
import ssl
import subprocess
import sys
import tempfile
import threading
import time

HOME, WORK = 'Home@example.test', 'Work@example.test'
A, B = 'a' * 32, 'b' * 32


class Imap(socketserver.StreamRequestHandler):
    def handle(self):
        def reply(line):
            self.wfile.write((line + '\r\n').encode())
        email = None
        try:
            reply('* OK synthetic IMAP ready')
            for line in self.rfile:
                args = shlex.split(line.decode())
                tag, cmd = args[:2]
                if cmd == 'LOGIN':
                    if args[2:] not in ([HOME, 'first'], [WORK, 'second']):
                        self.server.rejected.set()
                        reply(tag + ' NO [AUTHENTICATIONFAILED] Invalid credentials'); continue
                    email = args[2]
                elif cmd == 'CAPABILITY':
                    reply('* CAPABILITY IMAP4rev1 IDLE')
                elif cmd in ('SELECT', 'EXAMINE'):
                    for s in ['* 0 EXISTS', '* OK [UIDVALIDITY 7] stable', '* OK [UIDNEXT 43] next']:
                        reply(s)
                elif cmd == 'LOGOUT':
                    reply('* BYE closing'); reply(tag + ' OK logout'); return
                elif cmd == 'IDLE':
                    with self.server.idle_changed:
                        self.server.idling[email] += 1
                        self.server.idle_changed.notify_all()
                    try:
                        reply('+ idling')
                        if not self.rfile.readline():
                            return
                    finally:
                        with self.server.idle_changed:
                            self.server.idling[email] -= 1
                            self.server.idle_changed.notify_all()
                reply(tag + ' OK completed')
        except (BrokenPipeError, ConnectionResetError, ssl.SSLError):
            pass


class TlsServer(socketserver.ThreadingTCPServer):
    daemon_threads = True


def run(root, binary):
    openssl = shutil.which('openssl')
    subprocess.run([openssl, 'req', '-x509', '-nodes', '-newkey', 'rsa:2048', '-days', '1',
                    '-keyout', 'key', '-out', 'cert', '-subj', '/CN=localhost',
                    '-addext', 'subjectAltName=IP:127.0.0.1'], cwd=root, check=True,
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    tls = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    tls.load_cert_chain(root / 'cert', root / 'key')
    with TlsServer(('127.0.0.1', 0), Imap) as server:
        server.rejected = threading.Event()
        server.idling = Counter()
        server.idle_changed = threading.Condition()

        def expect_idle(home_count, work_count):
            with server.idle_changed:
                assert server.idle_changed.wait_for(
                    lambda: (server.idling[HOME], server.idling[WORK]) == (home_count, work_count), 3
                ), (home_count, work_count, server.idling)

        server.socket = tls.wrap_socket(server.socket, server_side=True)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        config = root / 'config/omamail/config.json'
        config.parent.mkdir(parents=True)
        config.write_text(json.dumps(dict(active=A, accounts=[dict(id=k, email=v, imap_host='127.0.0.1', imap_port=server.server_address[1]) for k, v in [(A, HOME), (B, WORK)]])))
        # Only synthetic markers, never persist the supplied password.
        secret = root / 'secret-tool'
        secret.write_text(f'#!{sys.executable}\n' + '''import os, sys
from pathlib import Path
home = Path(os.environ['HOME'])
account = sys.argv[-1]
if sys.argv[1] == 'lookup':
    if account == 'Home@example.test': sys.stdout.write('first')
    elif account == 'Work@example.test': sys.stdout.write('second' if (home/'updated').exists() else 'bad')
    else: sys.exit(1)
elif sys.argv[1] == 'store':
    assert account == 'Work@example.test' and sys.stdin.read() == 'second'
    if (home/'locked').exists(): sys.exit(2)
    (home/'updated').touch()
else: sys.exit(2)
''')
        secret.chmod(0o700)
        systemctl = root / 'systemctl'; systemctl.write_text('#!/bin/sh\nexit 0\n'); systemctl.chmod(0o700)
        env = dict(os.environ, PATH=str(root), HOME=str(root), XDG_CONFIG_HOME=str(root/'config'),
                   XDG_CACHE_HOME=str(root/'cache'), XDG_STATE_HOME=str(root/'state'), SSL_CERT_FILE=str(root/'cert'))
        watch = subprocess.Popen([binary, 'watch'], env=env, stderr=subprocess.DEVNULL)
        try:
            baseline = root/f'state/omamail/accounts/{A}/watch.json'
            for _ in range(150):
                if baseline.exists(): break
                time.sleep(0.05)
            assert json.loads(baseline.read_text())['uidvalidity'] == 7
            assert server.rejected.wait(3), "watcher did not try the second account"
            assert watch.poll() is None and not (root/f'state/omamail/accounts/{B}/watch.json').exists()
        finally:
            watch.terminate(); watch.wait(timeout=3)
        expect_idle(0, 0)
        for ident, email in [(A, HOME), (B, WORK)]:
            file = root/f'state/omamail/accounts/{ident}/list.json'
            file.parent.mkdir(parents=True, exist_ok=True)
            row = dict(uid=42, subject=email, timestamp=0, seen=False, flagged=False,
                       attachment=False, to=[], **{'from':[]})
            file.write_text(json.dumps(dict(v=1,email=email,folders=dict(INBOX=dict(messages=[row], total=1, used=0, uidvalidity=7)))))
        def panel():
            return subprocess.Popen([binary, 'serve'], env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
        def request(cmd, **args):
            p.stdin.write(json.dumps(dict(id=1, cmd=cmd, **args))+'\n'); p.stdin.flush()
            while True:
                result = json.loads(p.stdout.readline())
                if 'id' in result: return result
        with panel() as p:
            start = request('status')['data']; token = start['session']
            assert start['configured'] and start['account'] == A
            expect_idle(1, 0)
            assert request('cached', session=token, folder='INBOX')['data']['messages'][0]['subject'] == HOME
            assert not request('select', session=token, account=B)['ok']
            assert request('status')['data'] == start
            (root/'locked').touch()
            assert not request('login', session=token, email=WORK.upper(), password='second')['ok']
            assert request('status')['data'] == start and json.loads(config.read_text())['active'] == A
            (root/'locked').unlink()
            selected = request('login', session=token, email=WORK.upper(), password='second')['data']
            assert selected['email'] == WORK and selected['account'] == B and len(selected['accounts']) == 2
            expect_idle(0, 1)
            assert request('cached', session=selected['session'], folder='INBOX')['data']['messages'][0]['subject'] == WORK
            for ident, email, counts in [(A, HOME, (1, 0)), (B, WORK, (0, 1)), (A, HOME, (1, 0))]:
                selected = request('select', session=selected['session'], account=ident)['data']
                assert selected['email'] == email
                expect_idle(*counts)
            p.stdin.close()
        with panel() as p:
            resumed = request('status')['data']; assert resumed['account'] == A and resumed['session'] != token
            assert not request('flag', session=token, folder='INBOX', uids=[42], flag='seen')['ok']
            p.stdin.close()
        server.shutdown()


if __name__ == '__main__':
    with tempfile.TemporaryDirectory(prefix='omamail-tls-') as directory:
        run(Path(directory), str(Path(sys.argv[1]).resolve()))
    print('Passed: real TLS login, rejected switch and keyring write, IDLE retirement, reauthentication, restart and independent watcher.')
