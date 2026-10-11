#!/usr/bin/env python3
"""Reviewed, backed-up rollout for this supervisor-managed NabiMan installation.

Run prepare, inspect the private plan, rehearse, then activate. Never prints credentials.
The systemd-only helper is packaged but not enabled on this supervisor host.
"""
import argparse, base64, datetime, hashlib, json, os, pathlib, re, shutil
import socket, sqlite3, subprocess, time, uuid
from urllib.request import Request, urlopen
from urllib.error import HTTPError

ROOT = pathlib.Path('/workspace/nabiman')
AUTH = pathlib.Path('/workspace/var/lib/nabiman')
DB = ROOT / 'data/mec.db'
SOURCE = ROOT / 'src'
SUPERVISOR = '/workspace/etc/supervisor/supervisord.conf'
CHROME = '/workspace/nabi/.cache/ms-playwright/chromium-1243/chrome-linux64/chrome'
PUBLIC = 'https://man.bebo.kr'

def read(path): return json.loads(path.read_text())
def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()
def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name('.' + path.name + '.' + uuid.uuid4().hex)
    fd = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(fd, 'w') as stream:
        json.dump(value, stream, ensure_ascii=False, indent=2); stream.write('\n'); stream.flush(); os.fsync(stream.fileno())
    os.replace(temporary, path)
    fd = os.open(path.parent, os.O_RDONLY); os.fsync(fd); os.close(fd)
def supervisor(action):
    result = subprocess.run(['supervisorctl', '-c', SUPERVISOR, action, 'nabiman'], capture_output=True, text=True, timeout=35)
    if result.returncode and not (action == 'stop' and 'NOT_RUNNING' in result.stdout):
        raise RuntimeError('NabiMan supervisor ' + action + ' failed; inspect supervisor status')
def tmux_names(env=None):
    result = subprocess.run(['tmux', 'list-sessions', '-F', '#{session_name}'], env=env, capture_output=True, text=True)
    assert result.returncode == 0, 'Cannot inspect tmux; no empty-state fallback'
    return sorted(result.stdout.splitlines())
def api(base, path, token=None, body=None, method=None):
    headers = {'Content-Type': 'application/json'}
    if token: headers['Authorization'] = 'Bearer ' + token
    request = Request(base + path, data=None if body is None else json.dumps(body).encode(), headers=headers, method=method)
    try: response = urlopen(request, timeout=20)
    except HTTPError as error: response = error
    with response:
        payload = response.read()
        try: value = json.loads(payload)
        except ValueError: value = None
        return response.status, value
def login(base):
    # Consume the existing deployment credential only for its authorized login.
    password = (ROOT / 'secrets/admin_password').read_text().rstrip('\r\n')
    code, value = api(base, '/api/auth/login', body={'username': 'admin', 'password': password})
    assert code == 200 and value.get('success'), 'Existing administrator login failed'
    return value['data']
def snapshot(destination):
    destination.mkdir(mode=0o700)
    shutil.copytree(AUTH, destination / 'auth')
    with sqlite3.connect('file:' + str(DB) + '?mode=ro', uri=True) as source:
        with sqlite3.connect(destination / 'mec.db') as target: source.backup(target)
    assert sqlite3.connect(destination / 'mec.db').execute('PRAGMA quick_check').fetchone()[0] == 'ok'
    shutil.copy2(ROOT / 'bin/nabiman-server', destination / 'nabiman-server')
    shutil.copy2(ROOT / 'bin/start-nabiman.sh', destination / 'start-nabiman.sh')
    write(destination / 'helper-presence.json', {'present': (ROOT / 'bin/nabiman-privileged-helper').exists()})
    for name in ['nabiman-privileged-helper', 'nabiman-recovery', 'management-recovery.py']:
        if (ROOT / 'bin' / name).exists(): shutil.copy2(ROOT / 'bin' / name, destination / name)
    shutil.copytree(ROOT / 'static', destination / 'static')
    write(destination / 'tmux.json', tmux_names())
def session_identity(value):
    return {key: value.get(key) for key in ['owner', 'tmux_name', 'label', 'ssh_cmd', 'owner_id', 'remote_binding']}
def server_identity(value):
    return {key: value.get(key) for key in ['id', 'host', 'port', 'user', 'auth_method', 'host_trust', 'host_trust_version']}
def prepare(deployment):
    subprocess.run(['python3', str(SOURCE/'scripts/release-inventory.py'), 'verify', '--source', str(SOURCE),
        '--binary', str(SOURCE/'backend/target/release/nabiman-server'), '--static', str(SOURCE/'frontend/build')], check=True)
    deployment.mkdir(mode=0o700)
    snapshot(deployment / 'preflight.before')
    candidate = deployment / 'candidate'; candidate.mkdir(mode=0o700)
    for name in ['nabiman-server', 'nabiman-privileged-helper', 'nabiman-recovery']:
        shutil.copy2(SOURCE / 'backend/target/release' / name, candidate / name)
        (candidate / name).chmod(0o755)
    shutil.copy2(SOURCE / 'scripts/management-recovery.py', candidate / 'management-recovery.py')
    shutil.copytree(SOURCE / 'frontend/build', candidate / 'static')
    # Keep old content-addressed chunks for pages already loaded during rollout.
    for source in (ROOT / 'static/static').rglob('*'):
        if source.is_file():
            target = candidate / 'static/static' / source.relative_to(ROOT / 'static/static')
            if not target.exists(): target.parent.mkdir(parents=True, exist_ok=True); shutil.copy2(source, target)
    shutil.copytree(SOURCE / 'packaging', candidate / 'packaging')
    subprocess.run(['tar', '--exclude=./.git', '--exclude=./backend/target', '--exclude=./backend/static',
                    '--exclude=./frontend/node_modules', '--exclude=./frontend/build', '--exclude=./screenshots',
                    '-czf', str(deployment / 'source.tar.gz'), '-C', str(SOURCE), '.'], check=True)
    before = deployment / 'preflight.before/auth'
    users = read(before / 'users.json'); sessions = read(before / 'terminal_sessions.json')
    servers = read(before / 'remote_servers.json')
    preserved = all(value.get('owner_id') for value in sessions.values()) and all(value.get('host_trust') and value.get('host_trust_version') for value in servers)
    old_users = users if preserved else read(ROOT / 'deployments/scope-settings-20261005/data.before/users.json')
    assert len(users) == 1 and users[0]['username'] == 'admin' and users[0]['role'] == 'admin' and users[0]['status'] == 'active'
    owner = {key: users[0][key] for key in ['id', 'username', 'created_at']}
    assert owner in [{key: user[key] for key in owner} for user in old_users], 'Historical owner identity changed'
    approvals = {}
    for server in servers:
        if preserved:
            approvals[server['id']] = {'identity': server_identity(server)}
            continue
        assert not server.get('host_trust'), 'Existing explicit host trust needs individual review'
        name = server['host'] if server['port'] == 22 else f"[{server['host']}]:{server['port']}"
        found = subprocess.run(['ssh-keygen', '-F', name, '-f', '/root/.ssh/known_hosts'], check=True, capture_output=True, text=True)
        keys = [line.split()[1:3] for line in found.stdout.splitlines() if line and not line.startswith(('#', '@'))]
        assert keys, 'No existing trusted host key'
        keys.sort(key=lambda key: {'ssh-ed25519': 0, 'ecdsa-sha2-nistp256': 1, 'ssh-rsa': 2}.get(key[0], 9))
        kind, encoded = keys[0]
        fingerprint = 'SHA256:' + base64.b64encode(hashlib.sha256(base64.b64decode(encoded, validate=True)).digest()).decode().rstrip('=')
        approvals[server['id']] = {'identity': server_identity(server), 'version': str(uuid.uuid4()), 'trust': {
            'public_key': kind + ' ' + encoded, 'fingerprint': fingerprint,
            'verified_by': 'deployment:' + deployment.name,
            'verified_at': datetime.datetime.now(datetime.timezone.utc).isoformat(),
            'reason': 'Reviewed migration from existing root known_hosts; live key equality checked before deployment. No new TOFU trust.'}}
    targets = {}
    for sid, session in sessions.items():
        if preserved:
            assert session.get('owner_id') == owner['id'], 'This rollout verifier requires all terminals owned by the reviewed admin'
            assert session['tmux_name'] in tmux_names(), 'Registered terminal no longer exists'
            targets[sid] = {'identity': session_identity(session)}
            continue
        assert session['owner'] == owner['username'] and not session.get('owner_id')
        assert session['tmux_name'] in tmux_names(), 'Registered terminal no longer exists'
        target = None
        if session['label'] != 'local':
            matches = [server for server in servers if session['label'] == f"ssh:{server['user']}@{server['host']}:{server['port']}"]
            assert len(matches) == 1, 'Ambiguous legacy remote target'
            server = matches[0]; command = session.get('ssh_cmd') or ''
            assert re.search(r'(?<![\w@.-])' + re.escape(server['user'] + '@' + server['host']) + r'(?:[ ;]|$)', command)
            assert re.search(r'-p\s+' + str(server['port']) + r'\b', command)
            target = server['id']
        else: assert not session.get('ssh_cmd'), 'Local label hides a remote command'
        targets[sid] = {'identity': session_identity(session), 'server_id': target}
    plan = {'mode': 'preserve' if preserved else 'legacy-migration', 'owner': owner, 'sessions': targets, 'servers': approvals, 'known_hosts_sha256': sha(pathlib.Path('/root/.ssh/known_hosts')),
            'source_sha256': sha(deployment / 'source.tar.gz'), 'binary_sha256': sha(candidate / 'nabiman-server'), 'helper_sha256': sha(candidate / 'nabiman-privileged-helper'), 'recovery_sha256': sha(candidate / 'nabiman-recovery'), 'recovery_script_sha256': sha(candidate / 'management-recovery.py'),
            'static': {str(path.relative_to(candidate / 'static')): sha(path) for path in (candidate / 'static').rglob('*') if path.is_file()},
            'limitations': ['Existing supervisor host has no systemd service inventory; helper stays inactive', 'Expired legacy share links are preserved and remain unusable', 'Existing root API runtime remains a commercial hardening follow-up']}
    write(deployment / 'plan.json', plan)
    print(json.dumps({'prepared': str(deployment), 'plan_sha256': sha(deployment / 'plan.json'), 'sessions': len(targets), 'servers': len(approvals)}))
def migrate(directory, plan):
    users = read(directory / 'users.json')
    assert any(all(user.get(key) == value for key, value in plan['owner'].items()) and user['role'] == 'admin' and user['status'] == 'active' for user in users)
    sessions = read(directory / 'terminal_sessions.json'); servers = read(directory / 'remote_servers.json')
    assert set(sessions) == set(plan['sessions']), 'Terminal inventory changed; re-review before migration'
    assert {server['id'] for server in servers} == set(plan['servers']), 'Remote inventory changed'
    for server in servers:
        approval = plan['servers'][server['id']]
        assert server_identity(server) == approval['identity'], 'Remote target or trust changed'
        if plan.get('mode') == 'preserve': continue
        server['host_trust'] = approval['trust']; server['host_trust_version'] = approval['version']
    by_id = {server['id']: server for server in servers}
    for sid, session in sessions.items():
        expected = plan['sessions'][sid]
        assert session_identity(session) == expected['identity'], 'Terminal identity changed'
        if plan.get('mode') == 'preserve': continue
        session['owner_id'] = plan['owner']['id']
        if expected['server_id']:
            server = by_id[expected['server_id']]
            session['remote_binding'] = {'server_id': server['id'], **{key: server[key] for key in ['host', 'port', 'user', 'auth_method']},
                'approval_version': server['host_trust_version'], 'public_key': server['host_trust']['public_key'], 'fingerprint': server['host_trust']['fingerprint']}
    if plan.get('mode') == 'preserve': return
    write(directory / 'remote_servers.json', servers); write(directory / 'terminal_sessions.json', sessions)
def check(base, plan):
    session = login(base); token = session['token']
    scope = '/api/scope-settings/server/' + next(iter(plan['servers']))
    for endpoint in ['/api/users/me', '/api/users', '/api/terminal/sessions', '/api/remote-servers', scope]:
        code, value = api(base, endpoint, token)
        assert code == 200 and value and value.get('success'), 'Read-only API verification failed: ' + endpoint
        if endpoint == '/api/terminal/sessions':
            assert {row['id'] for row in value['data']} == set(plan['sessions']), 'Terminal ownership migration lost sessions'
            assert all(row['state'] == 'alive' for row in value['data']), 'Migrated session blocked or unavailable'
        if endpoint == '/api/remote-servers':
            assert len(value['data']) == len(plan['servers']) and all(row.get('host_trust') for row in value['data'])
    code, identity = api(base, '/api/diagnostics/runtime', token)
    assert code == 200 and identity['data']['executable_sha256'] == plan['binary_sha256'], 'Running executable differs from reviewed candidate'
    assert api(base, '/api/diagnostics/runtime')[0] == 401
    assert api(base, '/api/users')[0] == 401
    api(base, '/api/auth/logout', token, {})
    return identity['data']
def audit_ready(base):
    session = login(base)
    try:
        code, value = api(base, '/api/audit/health', session['token'])
        assert code == 200 and value.get('success') and value['data']['writable'], 'Local audit writer unavailable; deployment refused'
        code, value = api(base, '/api/audit/forwarding', session['token'])
        assert code == 200 and value.get('success'), 'Audit forwarding storage unavailable; deployment refused'
    finally: api(base, '/api/auth/logout', session['token'], {})
def rehearse(deployment, plan):
    stage = deployment / ('rehearsal-' + uuid.uuid4().hex[:8]); stage.mkdir(mode=0o700)
    shutil.copytree(deployment / 'preflight.before/auth', stage / 'auth')
    shutil.copy2(deployment / 'preflight.before/mec.db', stage / 'mec.db')
    migrate(stage / 'auth', plan)
    (stage / 'tmux').mkdir(mode=0o700)
    (stage / 'tmp').mkdir(mode=0o700)
    with socket.socket() as sock: sock.bind(('127.0.0.1', 0)); port = sock.getsockname()[1]
    env = {key: value for key, value in os.environ.items() if not key.startswith('NABIMAN_') and key not in ['TMUX', 'SSH_AUTH_SOCK', 'KUBECONFIG']}
    env.update(NABIMAN_PORT=str(port), NABIMAN_DATA_DIR=str(stage / 'auth'), NABIMAN_STATIC=str(deployment / 'candidate/static'),
        NABIMAN_MEC_DB=str(stage / 'mec.db'), NABIMAN_MEC_MODE='auto', NABIMAN_MEC_KUBECONFIG=str(stage / 'no-kubeconfig'),
        TMUX_TMPDIR=str(stage / 'tmux'), TMPDIR=str(stage / 'tmp'), NABIMAN_AUDIT_FORWARDING_DISABLED='1', NABIMAN_API_TOKENS_DISABLED='1')
    assert read(stage / 'auth/backup_schedules.json') == [] and read(stage / 'auth/alert_rules.json') == []
    names = [value['identity']['tmux_name'] for value in plan['sessions'].values()]
    process = None
    try:
        for name in names: subprocess.run(['tmux', 'new-session', '-d', '-s', name, 'sleep 3600'], env=env, check=True, capture_output=True)
        with (stage / 'server.log').open('w') as log:
            process = subprocess.Popen([str(deployment / 'candidate/nabiman-server')], env=env, cwd=stage, stdout=log, stderr=log)
        base = f'http://127.0.0.1:{port}'
        for _ in range(100):
            assert process.poll() is None, 'Rehearsal server exited; inspect private server.log'
            try:
                with urlopen(base + '/', timeout=2) as response:
                    if response.status == 200: break
            except OSError: time.sleep(.1)
        else: raise AssertionError('Rehearsal startup timeout')
        check(base, plan)
        audit_ready(base)
        assert tmux_names(env) == sorted(names)
        write(deployment / 'rehearsal.json', {'passed': True, 'plan_sha256': sha(deployment / 'plan.json'), 'binary_sha256': sha(deployment / 'candidate/nabiman-server'), 'migrated_sessions': len(names)})
        print('PASS: real release binary on copied production auth/DB, reviewed migration, isolated tmux, login and protected read-only APIs')
    finally:
        if process and process.poll() is None:
            process.terminate()
            try: process.wait(timeout=12)
            except subprocess.TimeoutExpired: process.kill(); process.wait(timeout=5)
        for name in names: subprocess.run(['tmux', 'kill-session', '-t', '=' + name], env=env, capture_output=True)
def verify_artifacts(deployment, plan):
    candidate = deployment / 'candidate'
    assert sha(deployment / 'source.tar.gz') == plan['source_sha256']
    assert sha(candidate / 'nabiman-server') == plan['binary_sha256']
    assert sha(candidate / 'nabiman-privileged-helper') == plan['helper_sha256']
    if 'recovery_sha256' in plan:
        assert sha(candidate / 'nabiman-recovery') == plan['recovery_sha256']
        assert sha(candidate / 'management-recovery.py') == plan['recovery_script_sha256']
    assert all(sha(candidate / 'static' / name) == digest for name, digest in plan['static'].items())
def rollback(deployment):
    supervisor('stop')
    before = deployment / 'cutover.before'
    failed = deployment / ('failed-' + uuid.uuid4().hex); failed.mkdir(mode=0o700)
    os.rename(AUTH, failed / 'auth'); shutil.copytree(before / 'auth', AUTH)
    for suffix in ['', '-wal', '-shm']:
        path = pathlib.Path(str(DB) + suffix)
        if path.exists(): os.rename(path, failed / ('mec.db' + suffix))
    shutil.copy2(before / 'mec.db', DB)
    replacement = ROOT / 'bin/.nabiman-rollback'; shutil.copy2(before / 'nabiman-server', replacement); os.replace(replacement, ROOT / 'bin/nabiman-server')
    for name in ['nabiman-privileged-helper', 'nabiman-recovery', 'management-recovery.py']:
        if name == 'nabiman-privileged-helper' and not (before / 'helper-presence.json').exists() and not (before / name).exists(): continue  # Legacy snapshot never captured the helper; do not infer absence.
        if (before / name).exists():
            stage = ROOT / 'bin' / ('.rollback-' + name); shutil.copy2(before / name, stage); os.replace(stage, ROOT / 'bin' / name)
        else: (ROOT / 'bin' / name).unlink(missing_ok=True)
    if (ROOT / 'static').is_symlink(): (ROOT / 'static').unlink()
    else: os.rename(ROOT / 'static', failed / 'static')
    shutil.copytree(before / 'static', ROOT / 'static')
    supervisor('start'); write(deployment / 'rollback.json', {'restored': True, 'failed_state_preserved': str(failed), 'helper_restored': (before / 'helper-presence.json').exists() or (before / 'nabiman-privileged-helper').exists()})
def activate(deployment, plan, approved_hash):
    assert approved_hash == sha(deployment / 'plan.json'), 'Explicit reviewed plan digest required'
    assert read(deployment / 'rehearsal.json')['plan_sha256'] == approved_hash
    verify_artifacts(deployment, plan)
    assert sha(pathlib.Path('/root/.ssh/known_hosts')) == plan['known_hosts_sha256'], 'Known host trust changed'
    supervisor('stop')
    changed = False
    try:
        snapshot(deployment / 'cutover.before')
        prepared = deployment / 'cutover.auth'; shutil.copytree(AUTH, prepared); migrate(prepared, plan)
        # After this boundary any failure restores binary, static and all mutable stores.
        changed = True
        if plan.get('mode') != 'preserve':
            for name in ['remote_servers.json', 'terminal_sessions.json']: write(AUTH / name, read(prepared / name))
        replacement = ROOT / 'bin/.nabiman-candidate'; shutil.copy2(deployment / 'candidate/nabiman-server', replacement); os.replace(replacement, ROOT / 'bin/nabiman-server')
        for name in ['nabiman-privileged-helper', 'nabiman-recovery', 'management-recovery.py']:
            stage = ROOT / 'bin' / ('.candidate-' + name); shutil.copy2(deployment / 'candidate' / name, stage); stage.chmod(0o755); os.replace(stage, ROOT / 'bin' / name)
        os.rename(ROOT / 'static', deployment / 'static.previous')
        os.symlink(deployment / 'candidate/static', ROOT / 'static')
        supervisor('start')
        identity = check('http://127.0.0.1:18080', plan)
        assert check(PUBLIC, plan) == identity, 'Public proxy reaches a different running instance'
        audit_ready(PUBLIC)
        assert set(read(deployment / 'cutover.before/tmux.json')).issubset(tmux_names()), 'An existing tmux session disappeared'
        assert sha(ROOT / 'bin/nabiman-server') == plan['binary_sha256']
        assert sha(ROOT / 'bin/nabiman-privileged-helper') == plan['helper_sha256']
        assert sha(ROOT / 'bin/nabiman-recovery') == plan['recovery_sha256']
        assert sha(ROOT / 'bin/management-recovery.py') == plan['recovery_script_sha256']
        write(deployment / 'activated.json', {'deployed': True, 'at': datetime.datetime.now(datetime.timezone.utc).isoformat(), 'binary_sha256': plan['binary_sha256'],
            'runtime': identity, 'mode': plan.get('mode'), 'public_url': PUBLIC, 'sessions_preserved': len(plan['sessions']), 'host_keys_preserved' if plan.get('mode') == 'preserve' else 'host_keys_migrated': len(plan['servers']), 'rollback_directory': str(deployment / 'cutover.before'), 'limitations': plan['limitations']})
        print('PASS: production supervisor restarted; local and public HTTPS authenticated checks passed; existing tmux sessions preserved')
    except BaseException:
        if changed: rollback(deployment)
        else: supervisor('start')
        raise

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['prepare', 'rehearse', 'activate', 'rollback'])
    parser.add_argument('--deployment', type=pathlib.Path, required=True)
    parser.add_argument('--approved-plan-sha256')
    args = parser.parse_args()
    assert os.geteuid() == 0, 'Use the existing authorized deployment operator'
    os.umask(0o077)
    assert args.deployment.parent == ROOT / 'deployments'
    if args.action == 'prepare': prepare(args.deployment)
    elif args.action == 'rollback': rollback(args.deployment)
    else:
        plan = read(args.deployment / 'plan.json')
        if args.action == 'rehearse': rehearse(args.deployment, plan)
        else: activate(args.deployment, plan, args.approved_plan_sha256)
